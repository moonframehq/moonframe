//! Embeds every `locales/en-US/*.ftl` file as `text::FILES`, so a file
//! synced from herdr-gpui needs no list edited beside it.

use std::{fmt::Write, fs, path::Path};

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let dir = Path::new(&manifest).join("locales/en-US");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names: Vec<String> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(".ftl"))
        .collect();
    names.sort_unstable();
    let mut out = String::from("&[\n");
    for name in &names {
        let feature = name.trim_end_matches(".ftl");
        let path = dir.join(name);
        let _ = writeln!(
            out,
            "    herdr_i18n::File {{ language: \"en-US\", feature: {feature:?}, source: include_str!({:?}) }},",
            path.display().to_string(),
        );
    }
    out.push(']');
    let target = Path::new(&std::env::var("OUT_DIR").unwrap_or_default()).join("files.rs");
    if let Err(error) = fs::write(&target, out) {
        panic!("cannot write {}: {error}", target.display());
    }
}
