//! The optional, independent pattern library: no providers or user-app state.
use moonframe::{Moonframe, rust_embed::RustEmbed};

#[derive(RustEmbed)]
#[crate_path = "moonframe::rust_embed"]
#[folder = "../../patterns/app"]
struct Patterns;

fn main() -> std::process::ExitCode {
    let app = Moonframe::new("moonframe-patterns")
        .title("Moonframe Patterns")
        .app_id("dev.moonframe.patterns")
        .version(env!("CARGO_PKG_VERSION"))
        .project_dir(env!("CARGO_MANIFEST_DIR"));
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/../../patterns/app");
    if std::path::Path::new(source).is_dir() {
        app.app_dir(source).run()
    } else {
        app.embedded_app::<Patterns>().run()
    }
}
