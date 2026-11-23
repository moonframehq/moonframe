//! The one trait a provider implements, for either host.

use serde_json::Value;

/// A provider: made by [`Provider::start`], then ticked every `interval`
/// seconds of provider.toml and handed the mods' messages. What `tick` and
/// `receive` return is published as `app.<name>`; `None` keeps what was
/// published last.
///
/// Both hosts call it from one thread at a time, in the same order: `start`,
/// `restore` if a previous build saved something, a first `tick`, then
/// ticks and messages as they come.
pub trait Provider: Sized + 'static {
    /// Makes the provider from the `[settings]` table of provider.toml.
    fn start(settings: Value) -> Result<Self, String>;

    /// Called every `interval` seconds.
    fn tick(&mut self) -> Option<Value> {
        None
    }

    /// A message a mod sent with `to = "provider:<name>"`.
    fn receive(&mut self, message: Value) -> Option<Value> {
        let _ = message;
        None
    }

    /// Called as this provider stops, to hand its state to the next one:
    /// up to 1 MiB of text, usually JSON. A wasm build calls it only with
    /// the `stateful` feature; compiled in, it is kept for this launch.
    fn save(&mut self) -> Option<String> {
        None
    }

    /// Called after `start`, and before the first `tick`, with what the
    /// last one saved. An error starts it fresh, and is logged.
    fn restore(&mut self, state: &str) -> Result<(), String> {
        let _ = state;
        Ok(())
    }
}
