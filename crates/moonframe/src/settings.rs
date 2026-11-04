//! What the app remembers between runs: `settings.toml` in the user's
//! folder, or the profile's. Only the keys the app writes are touched;
//! others stay.

use std::{fs, io::ErrorKind, path::Path};

/// The theme picked before light and dark kept one each: read once, to
/// carry it over.
pub(crate) const THEME: &str = "theme";
/// The theme light windows show, and the one dark windows show.
pub(crate) const LIGHT_THEME: &str = "light_theme";
pub(crate) const DARK_THEME: &str = "dark_theme";
/// `system`, `light` or `dark`.
pub(crate) const APPEARANCE: &str = "appearance";
/// How Inspect measures: `off`, `redlines`, `rulers`, `box` or `between`.
pub(crate) const INSPECT_MEASURE: &str = "inspect_measure";

/// The text saved under `key`, if any.
pub(crate) fn get(path: &Path, key: &str) -> Option<String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!("cannot read {}: {error}", path.display());
            return None;
        },
    };
    match text.parse::<toml::Table>() {
        Ok(table) => table.get(key)?.as_str().map(str::to_owned),
        Err(error) => {
            tracing::warn!("{}: {error}", path.display());
            None
        },
    }
}

/// Saves each `(key, value)`, keeping the other keys.
pub(crate) fn save(path: &Path, entries: &[(&str, &str)]) -> Result<(), String> {
    let mut table = match fs::read_to_string(path) {
        Ok(text) => text
            .parse::<toml::Table>()
            .map_err(|error| format!("{}: {error}", path.display()))?,
        Err(error) if error.kind() == ErrorKind::NotFound => toml::Table::new(),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    for (key, value) in entries {
        table.insert((*key).to_owned(), (*value).into());
    }
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    }
    fs::write(path, table.to_string()).map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_saved_beside_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.toml");
        assert_eq!(get(&path, THEME), None);
        save(&path, &[(THEME, "Gruvbox Dark")]).unwrap();
        assert_eq!(get(&path, THEME).as_deref(), Some("Gruvbox Dark"));

        fs::write(&path, "other = 1\ntheme = \"Nord\"\n").unwrap();
        save(&path, &[(APPEARANCE, "dark"), (THEME, "Dracula")]).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("other = 1"), "{text}");
        assert_eq!(get(&path, THEME).as_deref(), Some("Dracula"));
        assert_eq!(get(&path, APPEARANCE).as_deref(), Some("dark"));

        fs::write(&path, "theme = [").unwrap();
        assert_eq!(get(&path, THEME), None);
        assert!(save(&path, &[(THEME, "Nord")]).is_err());
    }
}
