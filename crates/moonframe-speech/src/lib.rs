//! The `speech` native provider: dictation. A mod asks it to listen, the
//! user talks, and the words come back to that mod. The microphone is
//! read here; turning sound into text is an engine's job, chosen in
//! provider.toml: Apple's on-device recognizer, a local program, or a
//! remote service (see `engine`).
//!
//! Messages (only from mods in `[settings] recorders`; a take obeys only
//! the mod that started it):
//! - `{ start = true, id?, engine? }` starts a take, with the engine named
//!   or the first ready in `prefer`. Answered with
//!   `{ reply = id, action = "start", ok, engine | error }`.
//! - `{ stop = true }` ends it and transcribes it. Answered with
//!   `{ reply = id, action = "transcript", ok, text | error, engine }`.
//! - `{ release = true }` is the button coming up: it ends a take held for
//!   a moment, as `stop` does, and leaves a tap's take listening until the
//!   next `stop`. So one button is both hold to talk and tap to toggle, and
//!   a view, which has no clock, need not tell them apart.
//! - `{ cancel = true }` throws it away, at any point. Answered as a
//!   transcript with `cancelled = true`.
//! - Anything else, from anyone, checks the engines again.
//!
//! `app.speech` is `{ available, reason?, engine?, engines, state, owner?,
//! level, seconds }`: `state` is `"idle"`, `"recording"` or
//! `"transcribing"`, `level` how loud it is from 0 to 1, and `engines`
//! `{ name, title, kind, remote, ready, reason? }`. What was said is never
//! published: only the mod that asked gets it.

// keep-sorted start
mod audio;
mod engine;
mod mic;
// keep-sorted end

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;

pub use {
    audio::Audio,
    mic::{Microphone, System, Take},
};
use {
    engine::{Cancel, Engine, Engines, Status},
    moonframe_core::Data,
    moonframe_providers::{Handle, Mods, Native, from_json, owner, to_json},
    serde_json::{Value, json},
    std::{
        io::Write,
        sync::{Arc, Mutex, MutexGuard, PoisonError},
        thread,
        time::{Duration, Instant},
    },
};

/// The longest take when `max_seconds` isn't set.
const MAX_SECONDS: u32 = 120;
/// The most `max_seconds` may ask for: a take is held in memory.
const LIMIT_SECONDS: u32 = 600;
/// A press held this long is hold to talk: letting go ends the take.
const HOLD: Duration = Duration::from_millis(350);
/// Shorter than this is a slip of the finger, not speech.
const MIN_SECONDS: f32 = 0.2;

/// Makes the provider, for the app's registry.
pub fn native() -> Box<dyn Native> {
    Box::new(Speech::new(Box::new(System)))
}

/// Who asked for what's under way.
struct Asker {
    from: String,
    id: Value,
    engine: Engine,
}

enum Phase {
    Idle,
    Recording {
        asker: Asker,
        take: Box<dyn Take>,
        started: Instant,
    },
    Transcribing {
        asker: Asker,
        cancel: Cancel,
        outcome: Shared<Option<Result<String, String>>>,
    },
}

type Shared<T> = Arc<Mutex<T>>;

fn lock<T>(shared: &Mutex<T>) -> MutexGuard<'_, T> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct Speech {
    microphone: Box<dyn Microphone>,
    recorders: Mods,
    engines: Engines,
    max_seconds: u32,
    /// What the last check found; `None` while it runs for the first time.
    statuses: Shared<Option<Vec<Status>>>,
    phase: Phase,
    last: Option<Value>,
}

impl Speech {
    /// A provider recording from `microphone`.
    pub fn new(microphone: Box<dyn Microphone>) -> Self {
        Self {
            microphone,
            recorders: Mods::default(),
            engines: Engines::default(),
            max_seconds: MAX_SECONDS,
            statuses: Arc::default(),
            phase: Phase::Idle,
            last: None,
        }
    }

    /// Looks at the engines off this thread: one may ask the login shell.
    fn check_engines(&self) {
        let engines = self.engines.all.clone();
        let statuses = Arc::clone(&self.statuses);
        thread::spawn(move || {
            let found = engines.iter().map(Engine::check).collect();
            *lock(&statuses) = Some(found);
        });
    }

    fn statuses(&self) -> Vec<Status> {
        lock(&self.statuses).clone().unwrap_or_default()
    }

    fn begin(&mut self, from: &str, wanted: Option<&str>) -> Result<Asker, String> {
        if !self.recorders.allows(from) {
            return Err(format!(
                "`{}` isn't in the speech provider's recorders",
                owner(from)
            ));
        }
        if !matches!(self.phase, Phase::Idle) {
            return Err("Already listening".into());
        }
        let statuses = self.statuses();
        let engine = match wanted {
            Some(name) => {
                let engine = self
                    .engines
                    .named(name)
                    .ok_or_else(|| format!("No speech engine named `{name}`"))?;
                if let Some(reason) = statuses
                    .iter()
                    .find(|status| status.name == name)
                    .and_then(|status| status.reason.clone())
                {
                    return Err(reason);
                }
                engine
            },
            None => self
                .engines
                .default_engine(&statuses)
                .ok_or_else(|| nothing_ready(&statuses))?,
        };
        Ok(Asker {
            from: from.to_owned(),
            id: Value::Null,
            engine: engine.clone(),
        })
    }

    /// Ends the take and hands it to its engine.
    fn transcribe(&mut self, host: &Handle) {
        let Phase::Recording { asker, take, .. } = std::mem::replace(&mut self.phase, Phase::Idle)
        else {
            return;
        };
        let audio = take.finish();
        if audio.seconds() < MIN_SECONDS {
            answer(
                host,
                &asker,
                Err("That was too short to hear anything".into()),
            );
            return;
        }
        if !audio.heard() {
            answer(
                host,
                &asker,
                Err(
                    "No sound reached the microphone: check that the app may use it in the \
                     system's privacy settings"
                        .into(),
                ),
            );
            return;
        }
        let cancel = Cancel::default();
        let outcome: Shared<_> = Arc::default();
        let engine = asker.engine.clone();
        let (stopping, done) = (cancel.clone(), Arc::clone(&outcome));
        thread::spawn(move || {
            let result = run_engine(&engine, &audio, &stopping);
            *lock(&done) = Some(result);
        });
        self.phase = Phase::Transcribing {
            asker,
            cancel,
            outcome,
        };
    }

    fn cancel(&mut self, host: &Handle) {
        let asker = match std::mem::replace(&mut self.phase, Phase::Idle) {
            Phase::Idle => return,
            Phase::Recording { asker, .. } => asker,
            Phase::Transcribing { asker, cancel, .. } => {
                cancel.set();
                asker
            },
        };
        send(
            host,
            &asker.from,
            &json!({
                "reply": asker.id, "action": "transcript", "ok": false, "cancelled": true,
                "engine": asker.engine.name,
            }),
        );
    }

    /// Whether the mod `from` is the one the take under way belongs to.
    fn owns(&self, from: &str) -> bool {
        match &self.phase {
            Phase::Idle => false,
            Phase::Recording { asker, .. } | Phase::Transcribing { asker, .. } => {
                owner(&asker.from) == owner(from)
            },
        }
    }

    fn publish(&mut self, host: &Handle) {
        let statuses = lock(&self.statuses).clone();
        let list = statuses.as_deref().unwrap_or_default();
        let engines: Vec<Value> = self
            .engines
            .all
            .iter()
            .map(|engine| engine.publish(list.iter().find(|status| status.name == engine.name)))
            .collect();
        let default = self.engines.default_engine(list);
        let (state, asker, level, seconds) = match &self.phase {
            Phase::Idle => ("idle", None, 0.0, 0),
            Phase::Recording {
                asker,
                take,
                started,
            } => (
                "recording",
                Some(asker),
                (take.level() * 50.0).round() / 50.0,
                started.elapsed().as_secs(),
            ),
            Phase::Transcribing { asker, .. } => ("transcribing", Some(asker), 0.0, 0),
        };
        let mut value = json!({
            "available": default.is_some(),
            "engines": engines,
            "state": state,
            "level": level,
            "seconds": seconds,
        });
        if let Some(default) = default {
            value["engine"] = json!(default.name);
        } else if statuses.is_some() {
            value["reason"] = json!(nothing_ready(list));
        } else {
            value["reason"] = json!("Looking for speech engines");
        }
        if let Some(asker) = asker {
            value["owner"] = json!(owner(&asker.from));
        }
        if self.last.as_ref() != Some(&value) {
            host.publish_json(&value);
            self.last = Some(value);
        }
    }
}

impl Native for Speech {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let settings = to_json(host.settings());
        self.recorders = Mods::from_settings(host.settings(), "recorders");
        self.engines = Engines::read(host.settings());
        for problem in &self.engines.problems {
            tracing::warn!(target: "moonframe::speech", "{problem}");
        }
        self.max_seconds =
            settings
                .get("max_seconds")
                .and_then(Value::as_u64)
                .map_or(MAX_SECONDS, |seconds| {
                    u32::try_from(seconds)
                        .unwrap_or(LIMIT_SECONDS)
                        .clamp(1, LIMIT_SECONDS)
                });
        self.check_engines();
        self.publish(host);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        match &mut self.phase {
            Phase::Idle => {},
            Phase::Recording { take, started, .. } => {
                if let Some(problem) = take.problem() {
                    if let Phase::Recording { asker, .. } =
                        std::mem::replace(&mut self.phase, Phase::Idle)
                    {
                        answer(host, &asker, Err(problem));
                    }
                } else if started.elapsed().as_secs() >= u64::from(self.max_seconds) {
                    self.transcribe(host);
                }
            },
            Phase::Transcribing { outcome, .. } => {
                let result = lock(outcome).take();
                if let Some(result) = result
                    && let Phase::Transcribing { asker, .. } =
                        std::mem::replace(&mut self.phase, Phase::Idle)
                {
                    answer(host, &asker, result);
                }
            },
        }
        self.publish(host);
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        let message = to_json(&message);
        let id = |fallback: &str| message.get("id").cloned().unwrap_or(json!(fallback));
        if message.get("start").is_some() {
            let wanted = message.get("engine").and_then(Value::as_str);
            let reply = id("start");
            let opened = self.begin(from, wanted).and_then(|mut asker| {
                asker.id = reply.clone();
                // Counted from before the microphone opens: a release that
                // waited on it was still a press that long.
                let started = Instant::now();
                let take = self.microphone.open(self.max_seconds)?;
                Ok((asker, take, started))
            });
            match opened {
                Ok((asker, take, started)) => {
                    let name = asker.engine.name.clone();
                    self.phase = Phase::Recording {
                        asker,
                        take,
                        started,
                    };
                    send(
                        host,
                        from,
                        &json!({ "reply": reply, "action": "start", "ok": true, "engine": name }),
                    );
                },
                Err(error) => send(
                    host,
                    from,
                    &json!({ "reply": reply, "action": "start", "ok": false, "error": error }),
                ),
            }
        } else if message.get("stop").is_some() {
            if self.owns(from) {
                self.transcribe(host);
            }
        } else if message.get("release").is_some() {
            let held = matches!(&self.phase, Phase::Recording { started, .. } if started.elapsed() >= HOLD);
            if held && self.owns(from) {
                self.transcribe(host);
            }
        } else if message.get("cancel").is_some() {
            if self.owns(from) {
                self.cancel(host);
            }
        } else {
            self.check_engines();
        }
        self.publish(host);
    }
}

impl Drop for Speech {
    /// The app quits or the provider restarts: no engine runs on for nobody.
    fn drop(&mut self) {
        if let Phase::Transcribing { cancel, .. } = &self.phase {
            cancel.set();
        }
    }
}

/// The take as a private temporary file, gone once the engine is done.
fn run_engine(engine: &Engine, audio: &Audio, cancel: &Cancel) -> Result<String, String> {
    let mut file = tempfile::Builder::new()
        .prefix("moonframe-speech-")
        .suffix(".wav")
        .tempfile()
        .map_err(|error| format!("Could not keep the take: {error}"))?;
    file.write_all(&audio.wav())
        .and_then(|()| file.flush())
        .map_err(|error| format!("Could not keep the take: {error}"))?;
    engine.transcribe(file.path(), cancel)
}

fn nothing_ready(statuses: &[Status]) -> String {
    let why = statuses.iter().find_map(|status| status.reason.as_deref());
    match why {
        Some(why) => format!("No speech engine is ready. {why}"),
        None => "No speech engine is set up".to_owned(),
    }
}

fn answer(host: &Handle, asker: &Asker, result: Result<String, String>) {
    let mut value = json!({
        "reply": asker.id, "action": "transcript", "ok": result.is_ok(),
        "engine": asker.engine.name,
    });
    match result {
        Ok(text) => value["text"] = json!(text),
        Err(error) => value["error"] = json!(error),
    }
    send(host, &asker.from, &value);
}

fn send(host: &Handle, to: &str, value: &Value) {
    if let Ok(data) = from_json(value) {
        host.send(to, data);
    }
}
