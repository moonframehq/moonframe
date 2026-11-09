//! Native providers: Rust compiled into the app, for what needs the user's
//! own access and real libraries (git, forges, herdr-core) rather than a
//! script. A folder with `kind = "native"` runs the one the app registered
//! under the folder's name, so the app's folders still decide what runs and
//! a build without it says so.
//!
//! A native provider gets a [`Handle`] it may clone into its own threads: a
//! push that waits on a hardware key runs there while the provider keeps
//! answering. Besides publishing, it can send a message to a mod, which is
//! how actions answer the mod that asked (`{ reply = id, ok = ... }`).

use {
    crate::{Context, Event, json},
    moonframe_core::{Data, Error},
    std::{
        cell::Cell,
        collections::BTreeMap,
        panic::{AssertUnwindSafe, catch_unwind},
        path::{Path, PathBuf},
        sync::{Arc, atomic::Ordering, mpsc::RecvTimeoutError},
        thread,
        time::{Duration, Instant},
    },
};

thread_local! {
    /// The window whose message the provider on this thread is handling,
    /// which what it sends meanwhile answers.
    static ANSWERING: Cell<Option<u64>> = const { Cell::new(None) };
}

/// A provider written in Rust and built into the app.
///
/// Every call runs on the provider's own thread; a panic stops the provider
/// and reports it, never the app. Dropping it is its stop: it has a couple
/// of seconds for that, as the app quits or it is replaced, for programs it
/// started to leave.
pub trait Native: Send {
    /// Called once, before anything else.
    fn start(&mut self, host: &Handle) -> Result<(), String>;

    /// Called every `interval` seconds of provider.toml.
    fn tick(&mut self, host: &Handle);

    /// A message from the mod `from`.
    fn receive(&mut self, host: &Handle, from: &str, message: Data);
}

/// Makes a native provider; the app registers one per name, and it is
/// called each time the provider starts.
pub type Factory = Arc<dyn Fn() -> Box<dyn Native> + Send + Sync>;

/// The native providers this build has, by name.
pub type Natives = BTreeMap<String, Factory>;

/// What a native provider can do with the app, from any of its threads.
#[derive(Clone)]
pub struct Handle {
    name: String,
    dir: PathBuf,
    settings: Data,
    events: crate::Events,
    publisher: crate::Publisher,
}

impl Handle {
    /// A handle for the provider `name` in `dir`, whose events go to
    /// `events`: what the host makes, and what a native provider's own
    /// tests make to drive it.
    pub fn new(name: &str, dir: &Path, settings: Data, events: crate::Events) -> Self {
        Self {
            name: name.to_owned(),
            dir: dir.to_owned(),
            settings,
            publisher: crate::Publisher::new(
                name.to_owned(),
                Arc::clone(&events),
                crate::Publish::All,
            ),
            events,
        }
    }

    /// Its name: `app.<name>` in views, `provider:<name>` to mods.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its folder, where provider.toml is.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The `[settings]` table of provider.toml.
    pub fn settings(&self) -> &Data {
        &self.settings
    }

    /// Shares `data` as `app.<name>`.
    pub fn publish(&self, data: Data) {
        self.publisher.publish(data);
    }

    /// Shares JSON as `app.<name>`, bounded like a process provider's; one
    /// past the bounds stops nothing but is reported.
    pub fn publish_json(&self, value: &serde_json::Value) {
        match json::from_json(value) {
            Ok(data) => self.publish(data),
            Err(problem) => self.fail(problem.to_string()),
        }
    }

    /// Sends `msg` to the mod `to`, whose `update` gets it as any message.
    pub fn send(&self, to: &str, msg: Data) {
        (self.events)(Event::Sent {
            provider: self.name.clone(),
            to: to.to_owned(),
            msg,
            origin: ANSWERING.get(),
        });
    }

    /// Says what is wrong; it keeps running.
    pub fn fail(&self, problem: impl Into<String>) {
        (self.events)(Event::Failed {
            provider: self.name.clone(),
            problem: Error::new(problem),
        });
    }
}

pub(crate) fn run(context: &Context, factory: Option<Factory>) {
    let Some(factory) = factory else {
        context.fail(format!(
            "this build has no native `{}` provider; enable its feature",
            context.name
        ));
        return;
    };
    let host = Handle {
        name: context.name.clone(),
        dir: context.dir.clone(),
        settings: json::from_json(&context.manifest.settings).unwrap_or(Data::Nil),
        events: Arc::clone(&context.events),
        publisher: crate::Publisher::of(context),
    };
    // A panic in the provider is its own failure; the app goes on.
    let guarded = |call: &mut dyn FnMut() -> Result<(), String>| {
        catch_unwind(AssertUnwindSafe(call)).unwrap_or_else(|panic| {
            let what = panic
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "a panic".to_owned());
            Err(format!("stopped: {what}"))
        })
    };
    // Bounded: a slow one is given up on, as it would be at shutdown.
    if let Some(left) = &context.replaces {
        let started = Instant::now();
        while !left.load(Ordering::Relaxed) && started.elapsed() < crate::NATIVE_GRACE {
            thread::sleep(Duration::from_millis(5));
        }
    }
    let mut provider = factory();
    if let Err(problem) = guarded(&mut || provider.start(&host)) {
        context.fail(format!("start: {problem}"));
        return;
    }
    tracing::info!(provider = %context.name, "running");
    loop {
        if context.stopped() {
            return;
        }
        let result = match context.messages.recv_timeout(context.manifest.interval) {
            Ok(message) => {
                // `guarded` takes an `FnMut`; the message is moved in once.
                let mut data = Some(message.data);
                ANSWERING.set(message.origin);
                let result = guarded(&mut || {
                    if let Some(data) = data.take() {
                        provider.receive(&host, &message.from, data);
                    }
                    Ok(())
                });
                ANSWERING.set(None);
                result
            },
            Err(RecvTimeoutError::Timeout) => guarded(&mut || {
                provider.tick(&host);
                Ok(())
            }),
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if let Err(problem) = result {
            context.fail(problem);
            return;
        }
    }
}
