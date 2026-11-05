//! The theme of the Omarchy desktop, when the app runs on one: it stands in
//! for every theme the user has not picked, and follows `omarchy theme set`
//! while the app runs.
//!
//! Omarchy keeps the theme in use in a folder it replaces whole on each
//! change. Its `colors.toml` is read the way `omarchy-theme-color` reads it,
//! so the window gets the palette Omarchy's own terminals do.

use {
    super::{Scheme, XTERM, hex, luminance, mix},
    notify::{RecommendedWatcher, RecursiveMode, Watcher as _},
    std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
        sync::{PoisonError, RwLock},
    },
};

/// The theme's name, in the picker and in `settings.toml`. It stays the
/// same as Omarchy's theme changes, so a window keeps following it.
pub(crate) const NAME: &str = "Omarchy";

/// Where Omarchy keeps its current theme, under the home folder: since it
/// moved to local state, then before.
const FOLDERS: [&str; 2] = [".local/state/omarchy/current", ".config/omarchy/current"];
const THEME: &str = "theme";
const COLORS: &str = "colors.toml";
/// Written after the theme is swapped in, so it marks the swap as finished.
const THEME_NAME: &str = "theme.name";
/// Beside `colors.toml`, in a theme older than its `mode` key.
const LIGHT_MARKER: &str = "light.mode";

/// Omarchy's theme as last read. Each one is leaked, since a `Scheme` lives
/// as long as the app: a few hundred bytes per theme change.
static CURRENT: RwLock<Option<&'static Scheme>> = RwLock::new(None);

/// Omarchy's theme, once [`load`] has found one.
pub(crate) fn current() -> Option<&'static Scheme> {
    *CURRENT.read().unwrap_or_else(PoisonError::into_inner)
}

/// The parent of Omarchy's theme, even while the theme itself is absent
/// during a swap. Prefer local state without falling back mid-swap.
fn folder(home: &Path) -> Option<PathBuf> {
    FOLDERS
        .iter()
        .map(|folder| home.join(folder))
        .find(|folder| folder.is_dir())
}

/// Reads Omarchy's theme, and returns whether it changed. Keeps the one it
/// has when none can be read, as while Omarchy swaps the folder.
pub(crate) fn load() -> bool {
    let Some(folder) = std::env::home_dir().and_then(|home| folder(&home)) else {
        return false;
    };
    if !folder.join(THEME).join(COLORS).is_file() {
        return false;
    }
    let scheme = match read(&folder.join(THEME)) {
        Ok(scheme) => scheme,
        Err(problem) => {
            tracing::warn!("omarchy theme in {}: {problem}", folder.display());
            return false;
        },
    };
    let mut current = CURRENT.write().unwrap_or_else(PoisonError::into_inner);
    if current.is_some_and(|current| *current == scheme) {
        return false;
    }
    *current = Some(Box::leak(Box::new(scheme)));
    true
}

/// Calls `changed` as Omarchy swaps its theme, if the app runs on Omarchy.
/// The theme's folder is replaced, so it is the one above that is watched.
pub(crate) fn watch(
    changed: impl Fn() + Send + 'static,
) -> notify::Result<Option<RecommendedWatcher>> {
    let Some(home) = std::env::home_dir() else {
        return Ok(None);
    };
    watch_home(&home, changed)
}

fn watch_home(
    home: &Path,
    changed: impl Fn() + Send + 'static,
) -> notify::Result<Option<RecommendedWatcher>> {
    let Some(folder) = folder(home) else {
        return Ok(None);
    };
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let swapped = |path: &PathBuf| {
            path.file_name()
                .is_some_and(|name| name == THEME || name == THEME_NAME)
        };
        if event.is_ok_and(|event| !event.kind.is_access() && event.paths.iter().any(swapped)) {
            changed();
        }
    })?;
    watcher.watch(&folder, RecursiveMode::NonRecursive)?;
    Ok(Some(watcher))
}

/// The scheme in an Omarchy theme's folder.
fn read(theme: &Path) -> Result<Scheme, String> {
    let text = fs::read_to_string(theme.join(COLORS)).map_err(|error| error.to_string())?;
    parse(&text, theme.join(LIGHT_MARKER).is_file())
}

/// A `colors.toml`'s keys and their values, read line by line as Omarchy
/// does: its themes may leave a value unquoted, which TOML would refuse.
fn entries(text: &str) -> HashMap<String, &str> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .filter_map(|(key, value)| {
            let key: String = key
                .chars()
                .filter(|letter| !matches!(letter, '"' | '\'' | ' '))
                .collect();
            if key.is_empty() || key.starts_with('#') {
                return None;
            }
            let quoted = value
                .split(['"', '\''])
                .nth(1)
                .filter(|_| value.contains(['"', '\'']));
            Some((key, quoted.unwrap_or(value).trim()))
        })
        .collect()
}

/// The scheme a `colors.toml` describes, `light_marker` being whether a
/// `light.mode` sits beside it. A color the file leaves out falls back as
/// `omarchy-theme-color` has it: newer names first, then the legacy ones.
fn parse(text: &str, light_marker: bool) -> Result<Scheme, String> {
    let entries = entries(text);
    let color = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| entries.get(*key).copied().and_then(hex))
    };
    let background =
        color(&["background", "bg", "color0"]).ok_or("no background color".to_owned())?;
    let foreground =
        color(&["foreground", "fg", "color7"]).ok_or("no foreground color".to_owned())?;
    let bright_foreground =
        color(&["bright_foreground", "bright_fg", "color15"]).unwrap_or(foreground);
    let red = color(&["red", "color1"]).unwrap_or(XTERM[1]);
    let green = color(&["green", "color2"]).unwrap_or(XTERM[2]);
    let yellow = color(&["yellow", "color3"]).unwrap_or(XTERM[3]);
    let blue = color(&["blue", "color4"]).unwrap_or(XTERM[4]);
    let magenta = color(&["magenta", "color5", "purple"]).unwrap_or(XTERM[5]);
    let cyan = color(&["cyan", "color6"]).unwrap_or(XTERM[6]);
    let bright = |keys: &[&str], base| color(keys).unwrap_or_else(|| mix(base, 0xffffff, 20));
    let dark = match entries.get("mode").or_else(|| entries.get("theme_type")) {
        Some(&"light") => false,
        Some(&"dark") => true,
        _ if light_marker => false,
        _ => luminance(background) < 0.5,
    };
    Ok(Scheme {
        name: NAME,
        background,
        foreground,
        cursor: bright_foreground,
        selection: color(&["selection", "selection_background"]),
        accent: color(&["accent"]).unwrap_or(blue),
        dark,
        ansi: [
            background,
            red,
            green,
            yellow,
            blue,
            magenta,
            cyan,
            foreground,
            color(&["muted", "color8", "dark_foreground", "dark_fg"]).unwrap_or(foreground),
            bright(&["bright_red", "color9"], red),
            bright(&["bright_green", "color10"], green),
            bright(&["bright_yellow", "color11"], yellow),
            bright(&["bright_blue", "color12"], blue),
            bright(&["bright_magenta", "color13", "bright_purple"], magenta),
            bright(&["bright_cyan", "color14"], cyan),
            bright_foreground,
        ],
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Omarchy's Tokyo Night, as it ships.
    const TOKYO_NIGHT: &str = r##"mode = "dark"

accent = "#7aa2f7"
selection = "#292e42"
muted = "#414868"

background = "#1a1b26"
foreground = "#a9b1d6"
bright_foreground = "#c0caf5"

red = "#f7768e"
yellow = "#e0af68"
green = "#9ece6a"
cyan = "#449dab"
blue = "#7aa2f7"
magenta = "#ad8ee6"

bright_red = "#ff7a93"
bright_yellow = "#ff9e64"
bright_green = "#b9f27c"
bright_cyan = "#0db9d7"
bright_blue = "#7da6ff"
bright_magenta = "#bb9af7"
"##;

    #[test]
    fn a_theme_gets_the_palette_omarchy_gives_its_terminals() {
        let scheme = parse(TOKYO_NIGHT, false).unwrap();
        assert_eq!(scheme.name, NAME);
        assert_eq!(scheme.background, 0x1a1b26);
        assert_eq!(scheme.foreground, 0xa9b1d6);
        assert_eq!(scheme.cursor, 0xc0caf5);
        assert_eq!(scheme.selection, Some(0x292e42));
        assert!(scheme.dark());
        assert_eq!(scheme.ansi, [
            0x1a1b26, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xad8ee6, 0x449dab, 0xa9b1d6,
            0x414868, 0xff7a93, 0xb9f27c, 0xff9e64, 0x7da6ff, 0xbb9af7, 0x0db9d7, 0xc0caf5,
        ]);
    }

    #[test]
    fn the_accent_is_the_themes_own_and_else_its_blue() {
        let text = "background = \"#ffffff\"\nforeground = \"#000000\"\nblue = \"#0000ff\"\n";
        assert_eq!(parse(text, false).unwrap().accent, 0x0000ff);
        let accented = format!("{text}accent = \"#56949f\"\n");
        let scheme = parse(&accented, false).unwrap();
        assert_eq!(scheme.accent, 0x56949f);
        assert_eq!(scheme.config().colors.primary.as_deref(), Some("#56949f"));
    }

    #[test]
    fn a_theme_of_ansi_names_alone_still_reads() {
        let mut text = String::new();
        for index in 0..16 {
            text.push_str(&format!("color{index} = \"#0000{index:02x}\"\n"));
        }
        let scheme = parse(&text, false).unwrap();
        assert_eq!(scheme.background, 0x000000);
        assert_eq!(scheme.foreground, 0x000007);
        assert_eq!(scheme.ansi[1], 0x000001);
        assert_eq!(scheme.ansi[8], 0x000008);
        assert_eq!(scheme.ansi[13], 0x00000d);
        assert_eq!(scheme.ansi[15], 0x00000f);
    }

    #[test]
    fn short_names_and_loose_syntax_read_as_omarchy_reads_them() {
        let text = "# a comment = \"#ff0000\"\n[section]\nbg = #101010\n\
                    'fg' = \"#fafafa\" # white\npurple = '#aa00aa'\n";
        let scheme = parse(text, false).unwrap();
        assert_eq!(scheme.background, 0x101010);
        assert_eq!(scheme.foreground, 0xfafafa);
        assert_eq!(scheme.ansi[5], 0xaa00aa);
        // An unset bright color is its base lightened.
        assert_eq!(scheme.ansi[13], mix(0xaa00aa, 0xffffff, 20));
    }

    #[test]
    fn the_mode_comes_from_the_key_then_the_marker_then_the_background() {
        let colors =
            |background: &str| format!("background = \"{background}\"\nfg = \"#808080\"\n");
        let dark_background = colors("#101010");
        let light_background = colors("#fafafa");
        assert!(parse(&dark_background, false).unwrap().dark());
        assert!(!parse(&light_background, false).unwrap().dark());
        assert!(!parse(&dark_background, true).unwrap().dark());
        // The key says so even of a background that looks otherwise.
        let forced = format!("mode = \"dark\"\n{light_background}");
        assert!(parse(&forced, true).unwrap().dark());
        let forced = format!("theme_type = \"light\"\n{dark_background}");
        assert!(!parse(&forced, false).unwrap().dark());
        // What is neither word says nothing.
        let unknown = format!("mode = \"auto\"\n{light_background}");
        assert!(!parse(&unknown, false).unwrap().dark());
    }

    #[test]
    fn a_theme_without_its_base_colors_is_refused() {
        assert!(parse("", false).unwrap_err().contains("background"));
        assert!(
            parse("background = \"#000000\"", false)
                .unwrap_err()
                .contains("foreground")
        );
    }

    #[test]
    fn the_theme_is_found_where_omarchy_keeps_it() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(folder(home.path()), None);

        let older = home.path().join(FOLDERS[1]);
        fs::create_dir_all(older.join(THEME)).unwrap();
        fs::write(older.join(THEME).join(COLORS), TOKYO_NIGHT).unwrap();
        assert_eq!(folder(home.path()), Some(older));

        // Keep the new parent even while its theme is being swapped out.
        let newer = home.path().join(FOLDERS[0]);
        fs::create_dir_all(&newer).unwrap();
        assert_eq!(folder(home.path()).as_ref(), Some(&newer));
        fs::create_dir_all(newer.join(THEME)).unwrap();
        fs::write(newer.join(THEME).join(COLORS), TOKYO_NIGHT).unwrap();
        assert_eq!(folder(home.path()), Some(newer.clone()));

        fs::write(newer.join(THEME).join(LIGHT_MARKER), "").unwrap();
        assert!(read(&newer.join(THEME)).unwrap().dark(), "mode wins");
    }

    #[test]
    fn watching_starts_between_removing_and_replacing_the_theme() {
        let home = tempfile::tempdir().unwrap();
        let current = home.path().join(FOLDERS[0]);
        let next = current.join("next-theme");
        fs::create_dir_all(&next).unwrap();
        fs::write(next.join(COLORS), TOKYO_NIGHT).unwrap();
        assert!(!current.join(THEME).exists());

        let (sender, changes) = std::sync::mpsc::channel();
        let _watcher = watch_home(home.path(), move || {
            let _ = sender.send(());
        })
        .unwrap()
        .expect("the parent must be watched even without theme/colors.toml");

        fs::rename(next, current.join(THEME)).unwrap();
        fs::write(current.join(THEME_NAME), "tokyo-night").unwrap();
        changes
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("finishing the swap must notify the app");
        assert_eq!(read(&current.join(THEME)).unwrap().background, 0x1a1b26);
    }
}
