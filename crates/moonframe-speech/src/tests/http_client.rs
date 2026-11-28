//! ureq's default `gzip` decodes every gzip answer, and drops its
//! Content-Encoding, for every crate in a build that turns it on: neither
//! the workspace's ureq nor this crate's may.

use super::*;

fn manifest(path: &str) -> toml::Value {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
    toml::from_str(&text).unwrap()
}

#[test]
fn ureq_comes_without_its_default_gzip() {
    let workspace = manifest("../../Cargo.toml");
    let ureq = &workspace["workspace"]["dependencies"]["ureq"];
    assert_eq!(
        ureq.get("default-features").and_then(toml::Value::as_bool),
        Some(false)
    );
    let features: Vec<&str> = ureq["features"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(toml::Value::as_str)
        .collect();
    assert!(features.contains(&"rustls"), "{features:?}");
    assert!(!features.contains(&"gzip"), "{features:?}");
    let own = &manifest("Cargo.toml")["dependencies"]["ureq"];
    assert!(
        own.get("features").is_none() && own.get("default-features").is_none(),
        "{own}"
    );
}
