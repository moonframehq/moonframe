//! What the app remembers between runs: `settings.toml` in the user's
//! folder. Only the keys the app writes are touched; others stay.

use std::{fs, io::ErrorKind, path::Path};

const THEME: &str = "theme";

/// The saved theme's name, if any.
pub(crate) fn theme(path: &Path) -> Option<String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!("cannot read {}: {error}", path.display());
            return None;
        },
    };
    match text.parse::<toml::Table>() {
        Ok(table) => table.get(THEME)?.as_str().map(str::to_owned),
        Err(error) => {
            tracing::warn!("{}: {error}", path.display());
            None
        },
    }
}

/// Saves `name` as the theme.
pub(crate) fn save_theme(path: &Path, name: &str) -> Result<(), String> {
    let mut table = match fs::read_to_string(path) {
        Ok(text) => text
            .parse::<toml::Table>()
            .map_err(|error| format!("{}: {error}", path.display()))?,
        Err(error) if error.kind() == ErrorKind::NotFound => toml::Table::new(),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    table.insert(THEME.to_owned(), name.into());
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
    fn the_theme_is_saved_beside_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.toml");
        assert_eq!(theme(&path), None);
        save_theme(&path, "Gruvbox Dark").unwrap();
        assert_eq!(theme(&path).as_deref(), Some("Gruvbox Dark"));

        fs::write(&path, "other = 1\ntheme = \"Nord\"\n").unwrap();
        save_theme(&path, "Dracula").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("other = 1"), "{text}");
        assert_eq!(theme(&path).as_deref(), Some("Dracula"));

        fs::write(&path, "theme = [").unwrap();
        assert_eq!(theme(&path), None);
        assert!(save_theme(&path, "Nord").is_err());
    }
}
