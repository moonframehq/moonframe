//! Guest SDK for Moonframe Wasm providers. Compile to `wasm32-wasip2`.
//! Implement `bindings::Guest`, then export with
//! `bindings::export!(MyProvider with_types_in moonframe_provider::bindings)`.
//! Enable `stateful` to implement `save`/`restore` too.

pub mod bindings {
    #[cfg(not(feature = "stateful"))]
    wit_bindgen::generate!({ path: "../../wit/provider.wit", world: "provider", pub_export_macro: true });
    #[cfg(feature = "stateful")]
    wit_bindgen::generate!({ path: "../../wit/provider.wit", world: "stateful-provider", pub_export_macro: true });
}

pub use bindings::moonframe::provider::host::{log, system_load};

/// A serde JSON call to a capability listed in this provider's manifest.
pub fn host_call<T: serde::Serialize + ?Sized, U: serde::de::DeserializeOwned>(
    capability: &str,
    method: &str,
    args: &T,
) -> Result<U, String> {
    let args = serde_json::to_string(args).map_err(|e| e.to_string())?;
    let result = bindings::moonframe::provider::host::host_call(capability, method, &args)?;
    serde_json::from_str(&result).map_err(|e| e.to_string())
}
