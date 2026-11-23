//! A provider written once against the `moonframe-provider` SDK, run
//! compiled into the app. The crate that builds to a wasm component from a
//! `kind = "wasm"` folder links in as an ordinary dependency instead, and
//! the app registers it for a `kind = "native"` folder of the same name:
//!
//! ```ignore
//! Moonframe::new(...).native("system", Portable::<system::System>::new)
//! ```
//!
//! It is called as the wasm host calls it: `start` with the `[settings]`
//! table, `restore`, a first `tick` published at once, then ticks and
//! messages, each answer published. Its `log`, `system_load` and
//! `host_call` reach the app on the provider's thread, gated by the same
//! `capabilities`. Compiled in it has the user's access and no sandbox, as
//! any native provider: link only code you would run yourself.

use {
    crate::{Handle, Native, json},
    moonframe_core::Data,
    moonframe_provider::{Provider, native::Host},
    std::{
        collections::BTreeMap,
        marker::PhantomData,
        path::PathBuf,
        rc::Rc,
        sync::{Mutex, PoisonError},
    },
};

/// What a provider saved as it stopped, by folder, for the next one this
/// launch: the handoff a wasm build makes to its next build.
static SAVED: Mutex<BTreeMap<PathBuf, String>> = Mutex::new(BTreeMap::new());

/// A [`Provider`] run as a [`Native`] one.
pub struct Portable<P: Provider + Send> {
    running: Option<(P, Handle)>,
    /// `P` is made on the provider's thread; only it needs to be `Send`.
    kind: PhantomData<fn() -> P>,
}

impl<P: Provider + Send> Portable<P> {
    /// One yet to start; what [`crate::Natives`] makes each time.
    pub fn new() -> Self {
        Self {
            running: None,
            kind: PhantomData,
        }
    }
}

impl<P: Provider + Send> Default for Portable<P> {
    fn default() -> Self {
        Self::new()
    }
}

/// Answers the provider's host calls through its [`Handle`].
struct Calls(Handle);

impl Host for Calls {
    fn log(&self, message: &str) {
        tracing::info!(provider = %self.0.name(), "{}", moonframe_core::printable(message));
    }

    fn system_load(&self) -> Result<String, String> {
        self.0.system_load().map(|load| load.to_string())
    }

    fn host_call(&self, capability: &str, method: &str, args: &str) -> Result<String, String> {
        self.0.host_call(capability, method, args)
    }
}

/// Runs `call` with the provider's host calls answered by `host`.
fn entered<R>(host: &Handle, call: impl FnOnce() -> R) -> R {
    moonframe_provider::native::enter(Rc::new(Calls(host.clone())), call)
}

fn publish(host: &Handle, answer: Option<serde_json::Value>) {
    if let Some(value) = answer {
        host.publish_json(&value);
    }
}

impl<P: Provider + Send> Native for Portable<P> {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let settings = json::to_json(host.settings());
        let mut provider = entered(host, || P::start(settings))?;
        let saved = SAVED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(host.dir());
        if let Some(state) = saved
            && let Err(problem) = entered(host, || provider.restore(&state))
        {
            tracing::warn!(provider = %host.name(), "restore: {problem}; starting fresh");
        }
        let first = entered(host, || provider.tick());
        publish(host, first);
        self.running = Some((provider, host.clone()));
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        if let Some((provider, _)) = &mut self.running {
            let answer = entered(host, || provider.tick());
            publish(host, answer);
        }
    }

    fn receive(&mut self, host: &Handle, _from: &str, message: Data) {
        if let Some((provider, _)) = &mut self.running {
            let message = json::to_json(&message);
            let answer = entered(host, || provider.receive(message));
            publish(host, answer);
        }
    }
}

impl<P: Provider + Send> Drop for Portable<P> {
    /// Its stop: what it saves goes to the next one in this folder.
    fn drop(&mut self) {
        let Some((mut provider, host)) = self.running.take() else {
            return;
        };
        // A panic saving loses the state, not the thread that drops it.
        let saved = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            entered(&host, || provider.save())
        }));
        if let Ok(Some(state)) = saved {
            SAVED
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(host.dir().to_owned(), state);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
