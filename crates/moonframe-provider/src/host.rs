//! What a provider asks of the host, the same calls on either host.

use serde_json::Value;

#[cfg(target_arch = "wasm32")]
use crate::bindings::moonframe::provider::host as raw;

/// Writes a line to the terminal that runs the app.
pub fn log(message: &str) {
    #[cfg(target_arch = "wasm32")]
    raw::log(message);
    #[cfg(not(target_arch = "wasm32"))]
    crate::native::log(message);
}

/// This machine's load: `{ "cpu", "memory", "memory_used", "memory_total",
/// "received", "transmitted" }`, fractions from 0 to 1 except bytes and
/// bytes per second. Needs `capabilities = ["system"]` in provider.toml.
pub fn system_load() -> Result<Value, String> {
    #[cfg(target_arch = "wasm32")]
    let text = raw::system_load()?;
    #[cfg(not(target_arch = "wasm32"))]
    let text = crate::native::system_load()?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

/// A serde JSON call to a capability listed in this provider's manifest.
pub fn host_call<T: serde::Serialize + ?Sized, U: serde::de::DeserializeOwned>(
    capability: &str,
    method: &str,
    args: &T,
) -> Result<U, String> {
    let args = serde_json::to_string(args).map_err(|e| e.to_string())?;
    #[cfg(target_arch = "wasm32")]
    let result = raw::host_call(capability, method, &args)?;
    #[cfg(not(target_arch = "wasm32"))]
    let result = crate::native::host_call(capability, method, &args)?;
    serde_json::from_str(&result).map_err(|e| e.to_string())
}
