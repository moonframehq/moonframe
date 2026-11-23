//! Guest SDK for Moonframe providers written in Rust, which build either
//! way from the same source:
//!
//! - **wasm**: compiled to `wasm32-wasip2` and run sandboxed, from a folder
//!   with `kind = "wasm"`, rebuilt as it changes;
//! - **native**: linked into the app as an ordinary crate and registered
//!   with `moonframe_providers::Portable`, from a folder with
//!   `kind = "native"`.
//!
//! Implement [`Provider`], then call [`export!`] once at the crate root; it
//! makes the component's exports on wasm32 and nothing elsewhere. The
//! [`log`], [`system_load`] and [`host_call`] functions reach whichever
//! host runs the provider, with the same `capabilities` from provider.toml.
//!
//! ```ignore
//! #[derive(Default)]
//! struct Counter(u64);
//!
//! impl moonframe_provider::Provider for Counter {
//!     fn start(_settings: serde_json::Value) -> Result<Self, String> {
//!         Ok(Self::default())
//!     }
//!     fn tick(&mut self) -> Option<serde_json::Value> {
//!         self.0 += 1;
//!         Some(self.0.into())
//!     }
//! }
//!
//! moonframe_provider::export!(Counter);
//! ```
//!
//! Enable `stateful` to have [`Provider::save`] and [`Provider::restore`]
//! called as a new wasm build replaces the old one.

#[cfg(target_arch = "wasm32")]
pub mod bindings {
    #[cfg(not(feature = "stateful"))]
    wit_bindgen::generate!({ path: "../../wit/provider.wit", world: "provider", pub_export_macro: true });
    #[cfg(feature = "stateful")]
    wit_bindgen::generate!({ path: "../../wit/provider.wit", world: "stateful-provider", pub_export_macro: true });
}

mod host;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
mod provider;
#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub mod wasm;

pub use {
    host::{host_call, log, system_load},
    provider::Provider,
};

/// Makes `$provider`, a [`Provider`], what this crate's component exports.
/// Compiled natively it expands to nothing: the app registers the type.
#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! export {
    ($provider:ty) => {};
}
