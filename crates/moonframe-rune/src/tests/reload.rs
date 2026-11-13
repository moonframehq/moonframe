//! What live reload does with a change, as GPUI's watcher sorts it.

use {
    crate::watch::{Changed, changed},
    std::path::{Path, PathBuf},
};

#[test]
fn a_change_in_a_providers_folder_restarts_providers_and_any_other_reloads_mods() {
    let providers = [PathBuf::from("/app/providers")];
    let at = |path: &str| changed(Path::new(path), &providers);
    // A provider an agent adds: its script and manifest.
    assert_eq!(at("/app/providers/news/news.sh"), Some(Changed::Provider));
    assert_eq!(
        at("/app/providers/news/provider.toml"),
        Some(Changed::Provider)
    );
    // Its builds would loop back into a rebuild.
    assert_eq!(at("/app/providers/news/target/out.wasm"), None);
    assert_eq!(at("/app/providers/news/Cargo.lock"), None);
    // A mod's files, and its words.
    assert_eq!(at("/app/mods/news/main.luau"), Some(Changed::Module));
    assert_eq!(at("/app/style.luau"), Some(Changed::Module));
    assert_eq!(at("/app/locales/en.ftl"), Some(Changed::Module));
    // Neither: other files, and git's own.
    assert_eq!(at("/app/notes.txt"), None);
    assert_eq!(at("/app/.git/index.luau"), None);
}
