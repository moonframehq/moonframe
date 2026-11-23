//! A focus timer, written once for both kinds of provider.
//!
//! Nothing here knows which host runs it:
//!
//! - **wasm**: the `focus` folder (`kind = "wasm"`) builds this crate to a
//!   component; `export!` below makes its exports.
//! - **native**: the app depends on this crate and registers
//!   `Portable::<Focus>::new` for the `focus-native` folder
//!   (`kind = "native"`); there `export!` expands to nothing.
//!
//! The rules that keep it portable:
//!
//! - Talk to the app only through the SDK: return what to publish, and use
//!   `moonframe_provider::{log, host_call, system_load}`. They reach
//!   whichever host runs the provider.
//! - Keep state in `self`, not in globals: compiled in, a restart makes a
//!   new `Focus` in the same process.
//! - Don't read files, the network or the environment: a wasm provider is
//!   sandboxed from them. Ask the host for what you need with a capability
//!   (`capabilities` in provider.toml) instead.
//! - Count ticks rather than reading the clock, as this one does: the tests
//!   can then drive time.
//! - Anything that must differ goes behind `#[cfg(target_arch = "wasm32")]`;
//!   this crate needs none.
//!
//! The tests in `tests.rs` run natively (`cargo test`), with no wasm.

use {
    moonframe_provider::Provider,
    serde::{Deserialize, Serialize},
    serde_json::{Value, json},
};

const DEFAULT_MINUTES: u64 = 25;
const MAX_MINUTES: u64 = 240;

/// The timer: `left` seconds of a `length`-second session, counting down
/// while `running`; `sessions` finished so far.
#[derive(Debug, Serialize, Deserialize)]
pub struct Focus {
    length: u64,
    left: u64,
    running: bool,
    sessions: u64,
    /// Whether views have it yet; not saved, so a new build shows itself.
    #[serde(skip)]
    shown: bool,
}

impl Focus {
    fn new(minutes: u64) -> Self {
        Self {
            length: minutes * 60,
            left: minutes * 60,
            running: false,
            sessions: 0,
            shown: false,
        }
    }

    /// What views read as `app.focus` (or `app["focus-native"]`).
    fn view(&self) -> Value {
        json!({
            "left": self.left,
            "length": self.length,
            "clock": format!("{:02}:{:02}", self.left / 60, self.left % 60),
            "running": self.running,
            "sessions": self.sessions,
            "build": if cfg!(target_arch = "wasm32") { "wasm" } else { "native" },
        })
    }
}

/// `minutes` from provider.toml's `[settings]` or a message, checked.
fn minutes(value: Option<&Value>) -> Result<u64, String> {
    match value {
        None => Ok(DEFAULT_MINUTES),
        Some(value) => value
            .as_u64()
            .filter(|minutes| (1..=MAX_MINUTES).contains(minutes))
            .ok_or_else(|| format!("`minutes` is a whole number from 1 to {MAX_MINUTES}")),
    }
}

impl Provider for Focus {
    /// The `[settings]` table; an error here is the provider's status, and
    /// it never runs.
    fn start(settings: Value) -> Result<Self, String> {
        Ok(Self::new(minutes(settings.get("minutes"))?))
    }

    /// Once a second (`interval = 1.0`), and once right after `start`.
    /// `None` publishes nothing new, so a paused timer costs the views
    /// nothing once they have it.
    fn tick(&mut self) -> Option<Value> {
        if !self.running {
            return (!std::mem::replace(&mut self.shown, true)).then(|| self.view());
        }
        self.left = self.left.saturating_sub(1);
        if self.left == 0 {
            self.sessions += 1;
            self.running = false;
            self.left = self.length;
            moonframe_provider::log(&format!("session {} done", self.sessions));
        }
        Some(self.view())
    }

    /// From a mod: `{ to = "provider:focus", msg = "start" }`, and `"pause"`,
    /// `"reset"`, or `{ minutes = 5 }` for a new length. The answer is
    /// published at once.
    fn receive(&mut self, message: Value) -> Option<Value> {
        match &message {
            Value::String(command) if command == "start" => self.running = true,
            Value::String(command) if command == "pause" => self.running = false,
            Value::String(command) if command == "reset" => {
                self.running = false;
                self.left = self.length;
            },
            Value::Object(fields) if fields.contains_key("minutes") => {
                match minutes(fields.get("minutes")) {
                    Ok(minutes) => {
                        *self = Self {
                            sessions: self.sessions,
                            ..Self::new(minutes)
                        }
                    },
                    Err(problem) => moonframe_provider::log(&problem),
                }
            },
            // The first view asks with `"status"`; anything else is only
            // answered too.
            _ => {},
        }
        Some(self.view())
    }

    /// As this build stops: a wasm build hands it to the next one, and
    /// compiled in it carries over a restart within this launch.
    fn save(&mut self) -> Option<String> {
        serde_json::to_string(self).ok()
    }

    /// After `start`, with what the last build saved; an error starts the
    /// timer fresh and is logged.
    fn restore(&mut self, state: &str) -> Result<(), String> {
        *self = serde_json::from_str(state).map_err(|e| e.to_string())?;
        Ok(())
    }
}

// The component's exports when built for wasm32; nothing natively.
moonframe_provider::export!(Focus);

#[cfg(test)]
mod tests;
