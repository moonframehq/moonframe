//! Compile-time capabilities, shared before the first Luau view runs.

use moonframe_core::{Persist, Roots, Workspace};

/// Keep the names identical to Cargo's features. These describe what was
/// compiled in, not whether a provider is configured or a backend installed.
fn compiled() -> std::collections::BTreeMap<String, bool> {
    macro_rules! features {
        ($($name:literal),* $(,)?) => {
            [$(($name.to_owned(), cfg!(feature = $name))),*].into()
        };
    }
    features![
        // keep-sorted start
        "acp",
        "alacritty",
        "awake",
        "browser",
        "capture",
        "chat",
        "crash",
        "displays",
        "dock",
        "docs",
        "forge",
        "ghostty",
        "git",
        "guard",
        "herdr-core",
        "map",
        "map-benchmark",
        "markdown",
        "math",
        "prose",
        "restart",
        "sealed",
        "shell",
        "socket",
        "studio",
        "syntax",
        "terminal",
        "tokio",
        "updates",
        "usage",
        "video",
        "wasm",
        // keep-sorted end
    ]
}

pub(crate) fn open(
    roots: impl Into<Roots>,
    dark: bool,
    persist: Option<Box<dyn Persist>>,
) -> Workspace {
    Workspace::open_with_capabilities(roots, dark, persist, compiled())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    #[test]
    fn every_cargo_capability_is_exposed() {
        let manifest: toml::Value =
            toml::from_str(include_str!("../Cargo.toml")).expect("Cargo manifest");
        let features = manifest["features"].as_table().expect("features");
        let expected: std::collections::BTreeSet<_> = features
            .keys()
            .filter(|name| name.as_str() != "default")
            .cloned()
            .collect();
        assert_eq!(
            super::compiled()
                .into_keys()
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
    }

    #[test]
    fn capabilities_are_available_on_first_render_and_after_reload() {
        let dir = tempfile::tempdir().expect("temporary app");
        std::fs::write(
            dir.path().join("main.luau"),
            format!(
                r#"return mod {{ view = function(_, app)
                assert(app.capabilities.browser == {})
                assert(app.capabilities.terminal == {})
                assert(app.capabilities.alacritty == {})
                return ui.text "capabilities available"
            end }}"#,
                cfg!(feature = "browser"),
                cfg!(feature = "terminal"),
                cfg!(feature = "alacritty"),
            ),
        )
        .expect("shell");
        let mut workspace = super::open(dir.path(), false, None);
        assert!(
            workspace.problems().is_empty(),
            "{:?}",
            workspace.problems()
        );
        workspace.reload();
        assert!(
            workspace.problems().is_empty(),
            "{:?}",
            workspace.problems()
        );
    }
}
