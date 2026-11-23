//! One provider crate, run both ways side by side.
//!
//! `app/providers/focus` is a Rust crate written against the
//! `moonframe-provider` SDK. Its folder says `kind = "wasm"`, so the app
//! builds it to a WebAssembly component and runs it sandboxed, rebuilding
//! it as it changes. The same crate is a dependency of this binary, and the
//! line below registers it as the native provider that
//! `app/providers/focus-native` (`kind = "native"`) runs. Both publish the
//! same data; `build` says which is which.
//!
//! An app that ships the native build only needs this line and a folder
//! with `kind = "native"`; one that ships the wasm build only needs the
//! folder. The provider's code doesn't change.
//!
//! `cargo run -p moonframe-portable-example` opens it (the wasm build needs
//! `rustup target add wasm32-wasip2`); `-- --check` loads it headlessly.

use {
    moonframe::{Moonframe, Portable},
    std::process::ExitCode,
};

fn main() -> ExitCode {
    Moonframe::new("portable-example")
        .title("Portable Provider")
        .app_id("dev.moonframe.portable-example")
        .version(env!("CARGO_PKG_VERSION"))
        .app_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../app"))
        .native("focus-native", Portable::<focus::Focus>::new)
        .run()
}
