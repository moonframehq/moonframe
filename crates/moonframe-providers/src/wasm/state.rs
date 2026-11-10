//! A wasm provider's state across a new build, for a component of the
//! `stateful-provider` world. WIT has no optional exports, so `save` and
//! `restore` are looked up by name: a plain `provider` component has neither,
//! and starts over as it always did.
//!
//! The old build saves as it stops, and the new one restores after `start`,
//! so `start` sees the settings as they are now. The state stays in
//! [`Kept`](crate::kept::Kept) until the new build's first tick returns, so
//! a compile error or a panic that early costs nothing: the build that fixes
//! it gets the state. A provider that traps later, while running, has nothing
//! to save, and its next build starts fresh.

use {
    super::{State, refuel, text},
    crate::Context,
    std::{
        sync::atomic::Ordering,
        thread,
        time::{Duration, Instant},
    },
    wasmtime::{
        Store,
        component::{Instance, TypedFunc},
    },
};

/// How long a new build waits for the old one to save: past a call that
/// spends all its fuel, which ends it however it was written.
const LEAVE_MAX: Duration = Duration::from_secs(10);
const LEAVE_POLL: Duration = Duration::from_millis(5);

/// The two exports of a `stateful-provider`.
pub(super) struct Handoff {
    save: TypedFunc<(), (Option<String>,)>,
    restore: TypedFunc<(String,), (Result<(), String>,)>,
}

impl Handoff {
    /// The exports, if the component has both. Ones of the wrong type are
    /// the provider's problem, never a quiet loss of its state.
    pub(super) fn find(
        instance: &Instance,
        store: &mut Store<State>,
    ) -> Result<Option<Self>, String> {
        let (Some(save), Some(restore)) = (
            instance.get_func(&mut *store, "save"),
            instance.get_func(&mut *store, "restore"),
        ) else {
            return Ok(None);
        };
        let wrong = |error| format!("`save` or `restore` is not as in provider.wit: {error:#}");
        Ok(Some(Self {
            save: save.typed(&*store).map_err(wrong)?,
            restore: restore.typed(&*store).map_err(wrong)?,
        }))
    }

    /// Gives the new build what the last one saved. One that refuses it
    /// runs fresh; one that traps fails, and the state stays for the next.
    pub(super) fn restore(
        &self,
        context: &Context,
        store: &mut Store<State>,
    ) -> Result<(), String> {
        let Some(state) = context.kept.get(&context.state_key).or_else(|| {
            context.state_file.as_ref().and_then(|file| {
                let size = std::fs::metadata(file).ok()?.len();
                (size <= 1024 * 1024)
                    .then(|| std::fs::read_to_string(file).ok())
                    .flatten()
            })
        }) else {
            return Ok(());
        };
        refuel(store)?;
        let (answer,) = self
            .restore
            .call(&mut *store, (state,))
            .map_err(|error| format!("restore: {}", text(error)))?;
        match answer {
            Ok(()) => tracing::info!(provider = %context.name, "state restored"),
            Err(problem) => tracing::warn!(
                provider = %context.name,
                "starting fresh: restore refused the last build's state: {}",
                moonframe_core::printable(&problem)
            ),
        }
        Ok(())
    }

    /// Keeps this build's state for the next. A failure leaves what is
    /// kept, as there is no better state to hand on.
    pub(super) fn save(&self, context: &Context, store: &mut Store<State>) {
        let saved = refuel(store).and_then(|()| self.save.call(&mut *store, ()).map_err(text));
        match saved {
            Ok((Some(state),)) if state.len() <= 1024 * 1024 => {
                if let Some(file) = &context.state_file {
                    let saved = (|| -> std::io::Result<()> {
                        if let Some(parent) = file.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        let temp = file.with_extension(format!("{}.tmp", std::process::id()));
                        std::fs::write(&temp, &state)?;
                        std::fs::rename(temp, file)
                    })();
                    if let Err(error) = saved {
                        tracing::warn!(provider = %context.name, "cannot save state: {error}");
                    }
                }
                context.kept.keep(&context.state_key, state);
            },
            Ok((Some(_),)) => tracing::warn!(provider = %context.name, "saved state exceeds 1 MiB"),
            Ok((None,)) => {},
            Err(problem) => {
                tracing::warn!(provider = %context.name, "state not saved: {problem}");
            },
        }
    }
}

/// Waits for every older build of this folder, including ones hidden by a
/// failed replacement. Cancellation must leave the saved state untouched.
pub(super) fn wait_for_old_build(context: &Context) -> bool {
    let until = Instant::now() + LEAVE_MAX + context.capabilities.limits.timeout;
    while context
        .wasm_build
        .as_ref()
        .is_some_and(|build| build.waiting())
        || context
            .replaces
            .as_ref()
            .is_some_and(|left| !left.load(Ordering::Relaxed))
    {
        if context.stopped() {
            return false;
        }
        if Instant::now() >= until {
            tracing::warn!(provider = %context.name, "the last build has not stopped; its state may be lost");
            break;
        }
        thread::sleep(LEAVE_POLL);
    }
    !context.stopped()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
