//! The `shell` native provider: shell commands a mod runs for the user (a
//! chat's `!git status`), in the user's login shell, in the workspace,
//! with their output shared as it comes, in colour.
//!
//! Messages (only from mods in provider.toml's `[settings] runners`; a run
//! obeys only the mod that started it):
//! - `{ run = "<command>", id?, cwd? }` starts one in `cwd` (an existing
//!   folder; `~/` is home), else the app's repository. Answered with
//!   `{ reply = id, action = "run", ok, run | error }`, and when it ends with
//!   `{ reply = id, action = "finished", ok, run, status, code? }`.
//! - `{ stop = "<run>" }` ends it and what it started; `{ forget = "<run>" }`
//!   drops a run that ended.
//! - Anything else publishes again.
//!
//! `app.shell` is `{ available, cwd, runs }`, newest first. A run is
//! `{ id, owner, command, cwd, status, code?, error?, started, finished?,
//! elapsed_ms, output, text, lines, hidden }`: `status` is `"running"`,
//! `"finished"` (exit 0), `"failed"` or `"stopped"`; `output` its last
//! lines with style marks (see `ansi`), `text` the same without them, at
//! most 15 KiB from the end; `hidden` the lines left out before them.

mod ansi;
mod run;

use {
    moonframe_core::Data,
    moonframe_providers::{Handle, Mods, Native, owner, to_json},
    run::Run,
    serde_json::{Value, json},
    std::{
        path::{Path, PathBuf},
        time::{Duration, Instant},
    },
};

/// Commands running at once, across every mod.
const MAX_RUNNING: usize = 8;
/// Runs kept, those that ended dropped oldest first.
const MAX_RUNS: usize = 40;
/// Runs whose whole kept output is shared; older ones share their end.
const FULL_RUNS: usize = 8;
/// Lines shared of a run's output: the newest runs, then older ones.
const FULL_LINES: usize = 2000;
const SHORT_LINES: usize = 40;
/// The longest command a mod may send.
const MAX_COMMAND: usize = 16 * 1024;
/// How often a running command's clock moves in `app.shell`.
const CLOCK: Duration = Duration::from_millis(500);

/// Makes the provider, for the app's registry.
pub fn native() -> Box<dyn Native> {
    Box::new(Shell::default())
}

#[derive(Default)]
pub struct Shell {
    runners: Mods,
    /// Where a command runs without a `cwd`.
    cwd: PathBuf,
    /// The user's shell.
    shell: PathBuf,
    runs: Vec<Run>,
    /// Runs whose end was already told to their mod.
    told: Vec<String>,
    serial: u64,
    dirty: bool,
    clock: Option<Instant>,
}

impl Native for Shell {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let settings = to_json(host.settings());
        self.runners = Mods::from_settings(host.settings(), "runners");
        let home = std::env::var_os("HOME").map(PathBuf::from);
        self.cwd = match settings.get("cwd").and_then(Value::as_str) {
            Some(cwd) => expand(cwd, home.as_deref()),
            None => repository(host.dir()).unwrap_or_else(|| home.clone().unwrap_or_default()),
        };
        self.shell = std::env::var_os("SHELL")
            .filter(|shell| !shell.is_empty())
            .map_or_else(|| PathBuf::from("/bin/sh"), PathBuf::from);
        self.dirty = true;
        self.publish(host);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        let mut changed = false;
        let mut ended = Vec::new();
        for run in &self.runs {
            let mut shared = run::lock(&run.shared);
            changed |= std::mem::take(&mut shared.changed);
            if shared.status != run::Status::Running && !self.told.contains(&run.id) {
                ended.push(run.id.clone());
            }
        }
        let running = self.runs.iter().any(Run::running);
        if running && self.clock.is_none_or(|clock| clock.elapsed() >= CLOCK) {
            self.clock = Some(Instant::now());
            changed = true;
        }
        self.dirty |= changed;
        for id in ended {
            self.tell_end(host, &id);
            self.told.push(id);
        }
        self.publish(host);
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        let message = to_json(&message);
        let text = |key: &str| message.get(key).and_then(Value::as_str);
        if let Some(command) = text("run") {
            let reply = message.get("id").cloned().unwrap_or(json!("run"));
            let answer = match self.run(from, command, text("cwd")) {
                Ok(run) => json!({ "reply": reply, "action": "run", "ok": true, "run": run }),
                Err(error) => {
                    json!({ "reply": reply, "action": "run", "ok": false, "error": error })
                },
            };
            send(host, from, &answer);
        } else if let Some(id) = text("stop") {
            if let Some(run) = self.own(from, id) {
                run.stop();
            }
        } else if let Some(id) = text("forget") {
            let owner = owner(from).to_owned();
            self.runs
                .retain(|run| !(run.id == id && run.owner == owner && !run.running()));
        }
        self.dirty = true;
        self.publish(host);
    }
}

impl Drop for Shell {
    /// The app quits or the provider restarts: no command outlives it.
    fn drop(&mut self) {
        for run in &self.runs {
            run.stop();
        }
    }
}

impl Shell {
    fn run(&mut self, from: &str, command: &str, cwd: Option<&str>) -> Result<String, String> {
        if !self.runners.allows(from) {
            return Err(format!(
                "`{from}` may not run commands: only the mods in providers/shell's `runners` may"
            ));
        }
        let command = command.trim();
        if command.is_empty() {
            return Err("nothing to run".to_owned());
        }
        if command.len() > MAX_COMMAND {
            return Err(format!("a command is at most {MAX_COMMAND} bytes"));
        }
        if self.runs.iter().filter(|run| run.running()).count() >= MAX_RUNNING {
            return Err(format!(
                "{MAX_RUNNING} commands are running; stop one first"
            ));
        }
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let cwd = match cwd {
            Some(cwd) => {
                let cwd = expand(cwd, home.as_deref());
                if !cwd.is_absolute() || !cwd.is_dir() {
                    return Err(format!("{} is not a folder", cwd.display()));
                }
                cwd
            },
            None => self.cwd.clone(),
        };
        self.serial += 1;
        let id = format!("{}-{}", run::now_ms(), self.serial);
        let run = Run::start(
            id.clone(),
            owner(from).to_owned(),
            command.to_owned(),
            &cwd,
            &self.shell,
        );
        self.runs.insert(0, run);
        self.trim();
        Ok(id)
    }

    /// Drops the oldest runs that ended, past [`MAX_RUNS`].
    fn trim(&mut self) {
        while self.runs.len() > MAX_RUNS {
            let Some(at) = self.runs.iter().rposition(|run| !run.running()) else {
                break;
            };
            let gone = self.runs.remove(at);
            self.told.retain(|id| id != &gone.id);
        }
    }

    fn own(&self, from: &str, id: &str) -> Option<&Run> {
        let owner = owner(from);
        self.runs
            .iter()
            .find(|run| run.id == id && run.owner == owner)
    }

    fn tell_end(&self, host: &Handle, id: &str) {
        let Some(run) = self.runs.iter().find(|run| run.id == id) else {
            return;
        };
        let summary = run.json(0);
        let status = summary.get("status").cloned().unwrap_or(Value::Null);
        let answer = json!({
            "reply": id,
            "action": "finished",
            "ok": status == "finished",
            "run": id,
            "status": status,
            "code": summary.get("code").cloned().unwrap_or(Value::Null),
        });
        send(host, &run.owner, &answer);
    }

    fn publish(&mut self, host: &Handle) {
        if !std::mem::take(&mut self.dirty) {
            return;
        }
        let runs: Vec<Value> = self
            .runs
            .iter()
            .enumerate()
            .map(|(at, run)| {
                run.json(if at < FULL_RUNS {
                    FULL_LINES
                } else {
                    SHORT_LINES
                })
            })
            .collect();
        host.publish_json(&json!({
            "available": true,
            "cwd": self.cwd.display().to_string(),
            "runs": runs,
        }));
    }
}

fn send(host: &Handle, to: &str, answer: &Value) {
    if let Ok(data) = moonframe_providers::from_json(answer) {
        host.send(to, data);
    }
}

fn expand(path: &str, home: Option<&Path>) -> PathBuf {
    match (path.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ if path == "~" => home.map(Path::to_path_buf).unwrap_or_default(),
        _ => PathBuf::from(path),
    }
}

/// The git repository holding the app the provider folder `dir` is in.
fn repository(dir: &Path) -> Option<PathBuf> {
    let start = dir.join("..").join("..");
    let start = std::path::absolute(&start).unwrap_or(start);
    let start = start.canonicalize().unwrap_or(start);
    start
        .ancestors()
        .find(|folder| folder.join(".git").exists())
        .map(Path::to_path_buf)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
