//! Color themes: the stock look, or one of the bundled terminal color
//! schemes. Each scheme is a background, a foreground, and 16 terminal colors;
//! the whole window's colors are derived from them, and the terminals take
//! the 16 colors as their palette.
//!
//! The Ghostty theme files in `assets/themes/` are unmodified copies from
//! iTerm2-Color-Schemes; see `assets/themes/NOTICE.md`.
//!
//! On the Omarchy desktop, its theme is one more: see [`omarchy`].

pub(crate) mod omarchy;

use {
    gpui_kit::{
        Hsla, SharedString,
        component::{ThemeConfig, ThemeConfigColors, ThemeMode, highlighter::HighlightTheme},
        rgb,
    },
    moonframe_core::Data,
    std::{collections::BTreeMap, sync::LazyLock},
};

/// The stock look: gpui-component's light and dark themes, with the
/// stylesheet's `colors` over them.
pub(crate) const DEFAULT: &str = "Default";

/// A terminal color scheme. Colors are packed `0xRRGGBB`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Scheme {
    pub(crate) name: &'static str,
    pub(crate) background: u32,
    pub(crate) foreground: u32,
    pub(crate) cursor: u32,
    pub(crate) selection: Option<u32>,
    accent: u32,
    dark: bool,
    pub(crate) ansi: [u32; 16],
}

/// Ghostty theme files compiled in, under their upstream file names.
macro_rules! bundled {
    ($($name:literal),* $(,)?) => {
        const BUNDLED: &[(&str, &str)] = &[$((
            $name,
            include_str!(concat!("../../../assets/themes/", $name)),
        )),*];
    };
}

bundled![
    "Atom One Dark",
    "Atom One Light",
    "Ayu Mirage",
    "Catppuccin Frappe",
    "Catppuccin Macchiato",
    "Everforest Dark Med",
    "GitHub Dark Default",
    "GitHub Light Default",
    "Gruvbox Dark",
    "Gruvbox Light",
    "Gruvbox Material Dark",
    "Kanagawa Wave",
    "Material",
    "Monokai Classic",
    "Monokai Pro",
    "Monokai Pro Light",
    "Night Owl",
    "Nightfox",
    "One Half Dark",
    "One Half Light",
    "Rose Pine",
    "Rose Pine Dawn",
    "Rose Pine Moon",
    "Snazzy",
    "TokyoNight",
    "TokyoNight Moon",
    "TokyoNight Storm",
    "Tomorrow Night",
];

/// Hand-written palettes: name, background, foreground, colors.
const HANDWRITTEN: &[(&str, u32, u32, [u32; 16])] = &[
    ("Slate", 0x101419, 0xd8dee9, [
        0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xc0c0c0, 0x808080,
        0xff0000, 0x00ff00, 0xffff00, 0x0000ff, 0xff00ff, 0x00ffff, 0xffffff,
    ]),
    ("Nord", 0x2e3440, 0xd8dee9, [
        0x3b4252, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x88c0d0, 0xe5e9f0, 0x4c566a,
        0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x8fbcbb, 0xeceff4,
    ]),
    ("Dracula", 0x282a36, 0xf8f8f2, [
        0x21222c, 0xff5555, 0x50fa7b, 0xf1fa8c, 0xbd93f9, 0xff79c6, 0x8be9fd, 0xf8f8f2, 0x6272a4,
        0xff6e6e, 0x69ff94, 0xffffa5, 0xd6acff, 0xff92df, 0xa4ffff, 0xffffff,
    ]),
    ("Catppuccin Mocha", 0x1e1e2e, 0xcdd6f4, [
        0x45475a, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xbac2de, 0x585b70,
        0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xa6adc8,
    ]),
    // A Halloween night (examples/lantern): near-black purple, its blue
    // slot pumpkin orange so the window's accent is too.
    ("Hallows", 0x0f0818, 0xebe4f2, [
        0x1a1124, 0xff5f6d, 0x3ddc84, 0xf5b545, 0xff8a3d, 0xd16bff, 0x5ec8e5, 0xcfc6da, 0x4a3d5c,
        0xff7a86, 0x6af0a2, 0xffd27a, 0xffa465, 0xe39bff, 0x8fdcf2, 0xf4eefa,
    ]),
    ("Catppuccin Latte", 0xeff1f5, 0x4c4f69, [
        0x5c5f77, 0xd20f39, 0x40a02b, 0xdf8e1d, 0x1e66f5, 0xea76cb, 0x179299, 0xacb0be, 0x6c6f85,
        0xd20f39, 0x40a02b, 0xdf8e1d, 0x1e66f5, 0xea76cb, 0x179299, 0xbcc0cc,
    ]),
];

/// Every scheme, by name without regard to case.
static SCHEMES: LazyLock<Vec<Scheme>> = LazyLock::new(|| {
    let mut schemes: Vec<Scheme> = HANDWRITTEN
        .iter()
        .map(|&(name, background, foreground, ansi)| Scheme {
            name,
            background,
            foreground,
            cursor: foreground,
            selection: None,
            accent: ansi[4],
            dark: luminance(background) < 0.5,
            ansi,
        })
        .chain(BUNDLED.iter().filter_map(|(name, text)| {
            Scheme::parse(name, text)
                .inspect_err(|error| tracing::warn!("theme {name}: {error}"))
                .ok()
        }))
        .collect();
    schemes.sort_by_key(|scheme| scheme.name.to_lowercase());
    schemes
});

/// Every theme's name: the stock one first, then Omarchy's on Omarchy, then
/// the schemes.
pub(crate) fn names() -> impl Iterator<Item = &'static str> {
    std::iter::once(DEFAULT)
        .chain(omarchy::current().map(|scheme| scheme.name))
        .chain(SCHEMES.iter().map(|scheme| scheme.name))
}

/// The themes made for dark windows, or for light ones: the stock look
/// (which has both) first, then the schemes of that kind.
pub(crate) fn names_for(dark: bool) -> impl Iterator<Item = &'static str> {
    names().filter(move |name| find(name).is_none_or(|scheme| scheme.dark() == dark))
}

/// The theme a window shows when it is `dark`: the one `previewed` in the
/// Themes window (`Some(None)` for the stock look), unless that is a theme for
/// the other appearance; else the one `picked` for this. `None` is the
/// stock look.
pub(crate) fn shown(
    previewed: Option<Option<&'static Scheme>>,
    picked: Option<&'static Scheme>,
    dark: bool,
) -> Option<&'static Scheme> {
    let fits = |scheme: Option<&Scheme>| scheme.is_none_or(|scheme| scheme.dark() == dark);
    previewed
        .filter(|scheme| fits(*scheme))
        .unwrap_or(picked)
        .filter(|scheme| scheme.dark() == dark)
}

/// Whether `name` is a theme for dark windows (or for light ones, when
/// `dark` is false): the stock look is for both, and so is Omarchy's, which
/// is one or the other as Omarchy changes it.
pub(crate) fn fits(name: &str, dark: bool) -> Result<Option<&'static Scheme>, String> {
    if name == DEFAULT {
        return Ok(None);
    }
    let scheme = find(name).ok_or_else(|| format!("no theme called `{name}`"))?;
    if scheme.name != omarchy::NAME && scheme.dark() != dark {
        let (kind, other) = if dark {
            ("dark", "light")
        } else {
            ("light", "dark")
        };
        return Err(format!(
            "`{name}` is a {other} theme; the {kind} theme must be a {kind} one"
        ));
    }
    Ok(Some(scheme))
}

/// What a theme picker shows of a theme: its text on its background, its
/// accent, and six of its colors (red, green, yellow, blue, magenta, cyan).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Preview {
    pub(crate) background: u32,
    pub(crate) foreground: u32,
    pub(crate) accent: u32,
    pub(crate) colors: [u32; 6],
}

/// The stock look's colors in light windows and in dark ones, before the
/// stylesheet's: gpui-component's neutral themes.
pub(crate) const STOCK_LIGHT: Preview = Preview {
    background: 0xffffff,
    foreground: 0x0a0a0a,
    accent: 0x171717,
    colors: [0xef4444, 0x22c55e, 0xeab308, 0x3b82f6, 0xa855f7, 0x06b6d4],
};
pub(crate) const STOCK_DARK: Preview = Preview {
    background: 0x0a0a0a,
    foreground: 0xfafafa,
    accent: 0xfafafa,
    colors: [0xf87171, 0x4ade80, 0xfacc15, 0x60a5fa, 0xc084fc, 0x22d3ee],
};

impl Preview {
    fn data(&self, name: &str, dark: bool) -> Data {
        let hex = |color: u32| Data::from(format!("#{color:06x}").as_str());
        Data::Map(BTreeMap::from([
            ("name".to_owned(), Data::from(name)),
            ("dark".to_owned(), Data::Bool(dark)),
            ("background".to_owned(), hex(self.background)),
            ("foreground".to_owned(), hex(self.foreground)),
            ("accent".to_owned(), hex(self.accent)),
            (
                "colors".to_owned(),
                Data::List(self.colors.iter().map(|&color| hex(color)).collect()),
            ),
        ]))
    }
}

/// Every theme as views read it in `app.appearance.themes`: `{ name,
/// dark, background, foreground, accent, colors }`, the stock look twice
/// (in `light` and in `dark`, its colors as the stylesheet makes them),
/// then Omarchy's on Omarchy, then each scheme, by name.
pub(crate) fn listing(light: Preview, dark: Preview) -> Data {
    Data::List(
        [light.data(DEFAULT, false), dark.data(DEFAULT, true)]
            .into_iter()
            .chain(
                omarchy::current()
                    .into_iter()
                    .chain(SCHEMES.iter())
                    .map(|scheme| scheme.preview().data(scheme.name, scheme.dark())),
            )
            .collect(),
    )
}

/// The scheme called `name`; `None` for the stock look or a name no theme
/// has.
pub(crate) fn find(name: &str) -> Option<&'static Scheme> {
    omarchy::current()
        .into_iter()
        .chain(SCHEMES.iter())
        .find(|scheme| scheme.name == name)
}

impl Scheme {
    /// A Ghostty theme file's colors. Other settings are ignored, and an
    /// unset palette entry keeps xterm's.
    fn parse(name: &'static str, text: &str) -> Result<Self, String> {
        let mut scheme = Self {
            name,
            background: 0x000000,
            foreground: 0xffffff,
            cursor: 0xffffff,
            selection: None,
            accent: XTERM[4],
            dark: true,
            ansi: XTERM,
        };
        let mut cursor = None;
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fail = |problem: &str| format!("line {}: {problem}", number + 1);
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| fail("expected key = value"))?;
            let value = value.trim();
            match key.trim() {
                "background" => scheme.background = hex(value).ok_or_else(|| fail("bad color"))?,
                "foreground" => scheme.foreground = hex(value).ok_or_else(|| fail("bad color"))?,
                "cursor-color" => cursor = Some(hex(value).ok_or_else(|| fail("bad color"))?),
                "selection-background" => {
                    scheme.selection = Some(hex(value).ok_or_else(|| fail("bad color"))?);
                },
                "palette" => {
                    let (index, color) = value
                        .split_once('=')
                        .ok_or_else(|| fail("expected palette = N=#rrggbb"))?;
                    let index: usize = index.trim().parse().map_err(|_| fail("bad index"))?;
                    // Entries past the 16 named colors are the computed cube.
                    if let Some(slot) = scheme.ansi.get_mut(index) {
                        *slot = hex(color.trim()).ok_or_else(|| fail("bad color"))?;
                    }
                },
                _ => {},
            }
        }
        scheme.cursor = cursor.unwrap_or(scheme.foreground);
        scheme.accent = scheme.ansi[4];
        scheme.dark = luminance(scheme.background) < 0.5;
        Ok(scheme)
    }

    pub(crate) fn dark(&self) -> bool {
        self.dark
    }

    fn preview(&self) -> Preview {
        let mut colors = [0; 6];
        colors.copy_from_slice(&self.ansi[1..7]);
        Preview {
            background: self.background,
            foreground: self.foreground,
            accent: self.accent,
            colors,
        }
    }

    /// The 16 terminal colors.
    #[cfg(any(feature = "terminal", test))]
    pub(crate) fn palette(&self) -> [(u8, u8, u8); 16] {
        self.ansi.map(|color| {
            let [_, red, green, blue] = color.to_be_bytes();
            (red, green, blue)
        })
    }

    /// The window's colors: chrome blended from the background toward the
    /// foreground, the accent (the blue, unless the theme has its own), and the terminal colors for
    /// success, warning, and danger. Panels (`card`, gpui's popover) are the
    /// theme's own background, so a terminal in one shows the scheme as
    /// drawn; the sidebar behind them is set apart: darker in a dark theme,
    /// tinted toward the text in a light one.
    pub(crate) fn config(&self) -> ThemeConfig {
        let blend = |percent| color(mix(self.background, self.foreground, percent));
        let accent = self.accent;
        let sidebar = if self.dark() {
            color(mix(self.background, 0x000000, 25))
        } else {
            blend(4)
        };
        let card = color(self.background);
        let on_accent = if contrast(accent, self.background) >= contrast(accent, self.foreground) {
            self.background
        } else {
            self.foreground
        };
        let mut colors = ThemeConfigColors::default();
        colors.background = Some(color(self.background));
        colors.foreground = Some(color(self.foreground));
        colors.border = Some(blend(14));
        colors.input = Some(blend(18));
        colors.ring = Some(color(accent));
        colors.caret = Some(color(self.cursor));
        colors.selection = Some(color(
            self.selection
                .unwrap_or_else(|| mix(self.background, accent, 30)),
        ));
        colors.muted = Some(blend(8));
        colors.muted_foreground = Some(blend(55));
        colors.accent = Some(blend(10));
        colors.accent_foreground = Some(color(self.foreground));
        colors.secondary = Some(blend(10));
        colors.secondary_hover = Some(blend(14));
        colors.secondary_active = Some(blend(18));
        colors.secondary_foreground = Some(color(self.foreground));
        colors.primary = Some(color(accent));
        colors.primary_hover = Some(color(mix(accent, self.foreground, 12)));
        colors.primary_active = Some(color(mix(accent, self.background, 12)));
        colors.primary_foreground = Some(color(on_accent));
        colors.list_hover = Some(blend(8));
        colors.list_active = Some(color(mix(self.background, accent, 22)));
        colors.list_active_border = Some(color(accent));
        colors.sidebar = Some(sidebar.clone());
        colors.sidebar_foreground = Some(color(self.foreground));
        colors.sidebar_border = Some(blend(14));
        colors.title_bar = Some(sidebar);
        colors.title_bar_border = Some(blend(14));
        colors.popover = Some(card);
        colors.popover_foreground = Some(color(self.foreground));
        colors.scrollbar_thumb = Some(blend(25));
        colors.scrollbar_thumb_hover = Some(blend(35));
        colors.success = Some(color(self.ansi[2]));
        colors.warning = Some(color(self.ansi[3]));
        colors.danger = Some(color(self.ansi[1]));
        colors.info = Some(color(self.ansi[6]));
        ThemeConfig {
            name: self.name.into(),
            mode: if self.dark() {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            colors,
            highlight: Some(self.highlight()),
            ..ThemeConfig::default()
        }
    }

    /// The code editor's colors: gpui-kit's own syntax colors for a light
    /// or a dark theme, on this scheme's background. Without them the
    /// editor keeps the highlight theme set before, the stock light one,
    /// and draws white in a dark window.
    fn highlight(&self) -> gpui_kit::component::highlighter::HighlightThemeStyle {
        let stock = if self.dark() {
            HighlightTheme::default_dark()
        } else {
            HighlightTheme::default_light()
        };
        let mut style = stock.style.clone();
        let hsla = |value: u32| -> Hsla { rgb(value).into() };
        style.editor_background = Some(hsla(self.background));
        style.editor_foreground = Some(hsla(self.foreground));
        style.editor_active_line = Some(hsla(mix(self.background, self.foreground, 6)));
        style.editor_line_number = Some(hsla(mix(self.background, self.foreground, 45)));
        style.editor_active_line_number = Some(hsla(self.foreground));
        style.editor_gutter_background = Some(hsla(self.background));
        style
    }
}

/// xterm's 16 colors, for entries a file leaves unset.
const XTERM: [u32; 16] = [
    0x000000, 0xcd3131, 0x0dbc79, 0xe5e510, 0x2472c8, 0xbc3fbc, 0x11a8cd, 0xe5e5e5, 0x666666,
    0xf14c4c, 0x23d18b, 0xf5f543, 0x3b8eea, 0xd670d6, 0x29b8db, 0xffffff,
];

/// `#rrggbb` (the `#` optional).
fn hex(value: &str) -> Option<u32> {
    let digits = value.strip_prefix('#').unwrap_or(value);
    (digits.len() == 6 && digits.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(digits, 16).ok())
        .flatten()
}

fn color(value: u32) -> SharedString {
    format!("#{value:06x}").into()
}

/// `percent` of `over` blended onto `base`, per channel.
pub(crate) fn mix(base: u32, over: u32, percent: u32) -> u32 {
    let channel = |shift: u32| {
        let base = (base >> shift) & 255;
        let over = (over >> shift) & 255;
        (base * (100 - percent) + over * percent) / 100
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

/// Relative luminance, from 0 (black) to 1 (white).
fn luminance(color: u32) -> f32 {
    let channel = |shift: u32| ((color >> shift) & 255) as f32 / 255.;
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

fn contrast(one: u32, other: u32) -> f32 {
    (luminance(one) - luminance(other)).abs()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_theme_parses_with_its_own_colors() {
        for (name, text) in BUNDLED {
            let scheme = Scheme::parse(name, text).unwrap();
            assert_ne!(scheme.ansi, XTERM, "{name} kept xterm's colors");
        }
        assert_eq!(
            SCHEMES.len(),
            BUNDLED.len() + HANDWRITTEN.len(),
            "a theme failed to load"
        );
    }

    #[test]
    fn names_list_the_stock_look_first_then_schemes_by_name() {
        let names: Vec<&str> = names().collect();
        assert_eq!(names[0], DEFAULT);
        assert_eq!(names.len(), 1 + SCHEMES.len());
        let rest: Vec<String> = names[1..].iter().map(|name| name.to_lowercase()).collect();
        assert!(rest.is_sorted());
        assert!(find(DEFAULT).is_none());
    }

    #[test]
    fn light_and_dark_each_list_their_own_themes() {
        let dark: Vec<&str> = names_for(true).collect();
        let light: Vec<&str> = names_for(false).collect();
        assert_eq!(dark[0], DEFAULT);
        assert_eq!(light[0], DEFAULT);
        assert!(dark.contains(&"Gruvbox Dark") && !dark.contains(&"Gruvbox Light"));
        assert!(light.contains(&"Gruvbox Light") && !light.contains(&"Gruvbox Dark"));
        assert_eq!(dark.len() + light.len(), names().count() + 1);

        assert_eq!(fits(DEFAULT, true), Ok(None));
        assert_eq!(fits("Nord", true).unwrap().unwrap().name, "Nord");
        assert!(fits("Nord", false).unwrap_err().contains("is a dark theme"));
        assert!(fits("Nope", false).unwrap_err().contains("no theme called"));
    }

    #[test]
    fn a_preview_for_the_other_appearance_gives_way_to_the_picked_theme() {
        let nord = find("Nord");
        let dawn = find("Rose Pine Dawn");
        // Previewing in the appearance it is for.
        assert_eq!(shown(Some(dawn), None, false), dawn);
        // The stock look previewed fits either.
        assert_eq!(shown(Some(None), nord, true), None);
        // The window turned dark while a light theme was previewed.
        assert_eq!(shown(Some(dawn), nord, true), nord);
        // No preview: the picked one, only where it fits.
        assert_eq!(shown(None, nord, true), nord);
        assert_eq!(shown(None, nord, false), None);
    }

    #[test]
    fn the_listing_has_the_stock_look_for_each_and_every_scheme() {
        let Data::List(themes) = listing(STOCK_LIGHT, STOCK_DARK) else {
            panic!("not a list");
        };
        assert_eq!(themes.len(), SCHEMES.len() + 2);
        let field = |theme: &Data, key: &str| match theme {
            Data::Map(fields) => fields.get(key).cloned().unwrap_or_default(),
            _ => Data::Nil,
        };
        assert_eq!(field(&themes[0], "name"), Data::from(DEFAULT));
        assert_eq!(field(&themes[0], "dark"), Data::Bool(false));
        assert_eq!(field(&themes[1], "dark"), Data::Bool(true));
        assert_eq!(field(&themes[1], "background"), Data::from("#0a0a0a"));
        let gruvbox = themes
            .iter()
            .find(|theme| field(theme, "name") == Data::from("Gruvbox Dark"))
            .unwrap();
        assert_eq!(field(gruvbox, "dark"), Data::Bool(true));
        assert_eq!(field(gruvbox, "background"), Data::from("#282828"));
        assert_eq!(field(gruvbox, "accent"), Data::from("#458588"));
        let Data::List(colors) = field(gruvbox, "colors") else {
            panic!("no colors");
        };
        assert_eq!(colors.len(), 6);
        assert_eq!(colors[0], Data::from("#cc241d"));
    }

    #[test]
    fn a_ghostty_file_reads_its_colors() {
        let scheme = Scheme::parse(
            "Test",
            "# comment\npalette = 1=#ff0000\nbackground = #fafafa\nforeground = 101010\n\
             selection-background = #cccccc\nfont-family = ignored",
        )
        .unwrap();
        assert_eq!(scheme.background, 0xfafafa);
        assert_eq!(scheme.foreground, 0x101010);
        assert_eq!(scheme.cursor, 0x101010);
        assert_eq!(scheme.selection, Some(0xcccccc));
        assert_eq!(scheme.ansi[1], 0xff0000);
        assert_eq!(scheme.ansi[2], XTERM[2]);
        assert!(!scheme.dark());
        assert!(Scheme::parse("Bad", "background = #12").is_err());
    }

    #[test]
    fn the_window_takes_the_scheme_colors() {
        let gruvbox = find("Gruvbox Dark").unwrap();
        assert!(gruvbox.dark());
        let config = gruvbox.config();
        assert_eq!(config.mode, ThemeMode::Dark);
        assert_eq!(config.colors.background.as_deref(), Some("#282828"));
        // Panels, and the terminals in them, are the scheme's background.
        assert_eq!(config.colors.popover.as_deref(), Some("#282828"));
        assert_eq!(config.colors.primary.as_deref(), Some("#458588"));
        assert_eq!(config.colors.danger.as_deref(), Some("#cc241d"));
        assert_eq!(gruvbox.palette()[1], (0xcc, 0x24, 0x1d));
        assert_eq!(
            find("Gruvbox Light").unwrap().config().mode,
            ThemeMode::Light
        );
    }
}
