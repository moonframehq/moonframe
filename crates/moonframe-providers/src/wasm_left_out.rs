//! Wasm providers in a build without the `wasm` feature: each publishes
//! nothing, so `app.<name>` stays nil as mods already allow, and fails only
//! when a mod messages it, as that asks for something.

use {crate::Context, std::sync::mpsc::RecvTimeoutError};

pub(crate) fn run(context: &Context) {
    tracing::debug!(provider = %context.name, "wasm providers are left out of this build");
    loop {
        if context.stopped() {
            return;
        }
        match context.messages.recv_timeout(context.manifest.interval) {
            Ok(message) => context.fail(format!(
                "{} asked `{}`, a wasm provider, which this build leaves out; enable the \
                 `wasm` feature",
                message.from, context.name
            )),
            Err(RecvTimeoutError::Timeout) => {},
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}
