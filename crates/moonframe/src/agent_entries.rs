//! User-selected miniapp sources that are part of the window's editable app.

use {
    moonframe_core::entries::Entries,
    moonframe_providers::Editable,
    std::path::{Path, PathBuf},
};

pub(crate) const ROLE_PREFIX: &str = "entry:";

/// Selected entries inside the user's folder, resolved so a link cannot
/// extend the agent's scope beyond it.
pub(crate) fn selected(entries: &Entries, user: Option<&Path>) -> Vec<(String, PathBuf)> {
    let Some(user) = user.and_then(|path| path.canonicalize().ok()) else {
        return Vec::new();
    };
    entries
        .snapshot()
        .into_values()
        .filter_map(|entry| {
            let root = entry.root.canonicalize().ok()?;
            let path = entry.path.canonicalize().ok()?;
            (path.is_dir()
                && root.starts_with(&user)
                && path.starts_with(&root)
                && path.starts_with(&user)
                && path != user)
                .then_some((entry.name, path))
        })
        .collect()
}

/// Adds selected miniapps as separate source roots, not app precedence layers.
pub(crate) fn append(editable: &mut Editable, entries: &[(String, PathBuf)]) {
    editable.layers.extend(
        entries
            .iter()
            .map(|(name, path)| (format!("{ROLE_PREFIX}{name}"), path.clone())),
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, moonframe_core::entries::RootPolicy};

    #[test]
    fn only_selected_sources_under_the_user_folder_are_included() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("config");
        let miniapps = user.join("miniapps");
        let clock = miniapps.join("clock");
        let unselected = miniapps.join("unselected");
        let outside_root = user.join("other");
        let elsewhere = dir.path().join("checkout");
        for folder in [&clock, &unselected, &outside_root, &elsewhere] {
            std::fs::create_dir_all(folder).unwrap();
        }

        let entries = Entries::default();
        entries
            .add_root("user", &miniapps, RootPolicy::default())
            .unwrap();
        entries.add_entry("user", "clock", &clock).unwrap();
        entries
            .add_entry("user", "outside-root", &outside_root)
            .unwrap();
        entries
            .add_root("checkout", &elsewhere, RootPolicy::default())
            .unwrap();
        entries.add_entry("checkout", "other", &elsewhere).unwrap();

        let selected = selected(&entries, Some(&user));
        assert_eq!(selected, [(
            "user:clock".to_owned(),
            clock.canonicalize().unwrap()
        )]);

        let mut editable = Editable::default();
        append(&mut editable, &selected);
        assert_eq!(editable.layers[0].0, "entry:user:clock");
    }

    #[cfg(unix)]
    #[test]
    fn a_user_entry_linked_outside_its_root_is_not_included() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("config");
        let root = user.join("miniapps");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("linked")).unwrap();

        let entries = Entries::default();
        entries
            .add_root("user", &root, RootPolicy::default())
            .unwrap();
        entries
            .add_entry("user", "linked", root.join("linked"))
            .unwrap();

        assert!(selected(&entries, Some(&user)).is_empty());
    }
}
