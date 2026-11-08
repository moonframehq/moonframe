//! The window around the layout: what the shell asks of it (`window = { ... }`
//! in `main.luau`), and what every view sees of it as `app.window`.
//!
//! ```lua
//! return mod {
//!   window = { titlebar = "hidden", traffic_lights = { x = 12, y = 14 } },
//!   view = function(state, app)
//!     return ui.column { grow = true,
//!       ui.row { drag = true, height = 38, ... }, -- the title bar
//!       ...
//!     }
//!   end,
//! }
//! ```

use {
    crate::{
        Data, Error,
        read::{Cx, Fields, Read},
    },
    std::collections::BTreeMap,
};

tokens! {
    /// Who draws the title bar.
    Titlebar {
        /// The system's, with its title and window buttons.
        System = "system",
        /// None: the layout reaches the top edge and draws its own. macOS
        /// keeps its traffic lights over it; Linux and Windows draw no
        /// window buttons, so the layout does (`minimize`, `maximize` and
        /// `close-window` commands).
        Hidden = "hidden",
    }
}

#[allow(
    clippy::derivable_impls,
    reason = "`tokens!` writes the derives, without Default"
)]
impl Default for Titlebar {
    fn default() -> Self {
        Self::System
    }
}

tokens! {
    /// The system the app runs on, for views that draw window buttons.
    Platform {
        Macos = "macos",
        Linux = "linux",
        Windows = "windows",
    }
}

impl Platform {
    /// This build's; other Unix desktops count as Linux, as GPUI treats them.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// What the shell asks of the window. The host opens the window with it,
/// so a change takes effect when the app next starts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Chrome {
    pub titlebar: Titlebar,
    /// Where macOS puts its traffic lights under a hidden title bar: the
    /// first button's offset from the window's top left, in pixels.
    pub traffic_lights: Option<(f32, f32)>,
    /// The size the window opens at, `(width, height)`, before the user
    /// resizes it; the host's default when unset.
    pub size: Option<(f32, f32)>,
}

/// The sizes a shell may ask the window to open at.
const WINDOW_WIDTH: std::ops::RangeInclusive<f32> = 640.0..=4000.0;
const WINDOW_HEIGHT: std::ops::RangeInclusive<f32> = 400.0..=3000.0;

/// How far traffic lights can move from the corner.
const TRAFFIC_LIGHTS: std::ops::RangeInclusive<f32> = 0.0..=200.0;

impl Read for Chrome {
    fn read(value: &mlua::Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&["titlebar", "traffic_lights", "size"]])?;
        let size = match fields.raw("size") {
            None => None,
            Some(value) => {
                let cx = cx.key("size");
                let size = Fields::read(value, &cx, &[&["width", "height"]])?;
                Some((
                    size.number("width", WINDOW_WIDTH)?.unwrap_or(1280.0),
                    size.number("height", WINDOW_HEIGHT)?.unwrap_or(820.0),
                ))
            },
        };
        Ok(Self {
            titlebar: fields.get("titlebar")?.unwrap_or_default(),
            traffic_lights: traffic_lights(&fields, cx)?,
            size,
        })
    }
}

/// Where a `window` table puts the traffic lights.
fn traffic_lights(fields: &Fields, cx: &Cx) -> Result<Option<(f32, f32)>, Error> {
    let Some(value) = fields.raw("traffic_lights") else {
        return Ok(None);
    };
    let cx = cx.key("traffic_lights");
    let point = Fields::read(value, &cx, &[&["x", "y"]])?;
    Ok(Some((
        point.number("x", TRAFFIC_LIGHTS)?.unwrap_or(0.0),
        point.number("y", TRAFFIC_LIGHTS)?.unwrap_or(0.0),
    )))
}

/// The window a mod opens in, of its own (`window = { size = { width,
/// height }, min_size = { width, height }, display = "external",
/// fullscreen = true, titlebar = "hidden", traffic_lights = { x, y } }` on a
/// mod other than the shell): the size it opens at, the smallest the user can
/// make it, where, and whether its layout goes under the title bar, as
/// [`Chrome`] says for the shell. The host keeps it within the screen and
/// falls back to its own size for what is unset.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModWindowSize {
    pub size: Option<(f32, f32)>,
    pub min_size: Option<(f32, f32)>,
    pub place: WindowPlace,
    pub titlebar: Titlebar,
    /// Where macOS puts its traffic lights under a hidden title bar: the
    /// first button's offset from the window's top left, in pixels.
    pub traffic_lights: Option<(f32, f32)>,
}

/// Where a mod's window opens: on which screen, and whether it fills it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowPlace {
    pub display: Option<Display>,
    pub fullscreen: bool,
}

/// A screen a window can ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    /// The screen with the menu bar.
    Primary,
    /// The first screen that is not the primary one; the primary one when
    /// there is no other, so a preview still opens.
    External,
    /// An `id` from `app.displays`.
    Id(i64),
}

impl Display {
    /// Reads `"primary"`, `"external"` or an `id` from `app.displays`.
    pub(crate) fn from_data(value: &Data) -> Option<Self> {
        match value {
            Data::String(name) if name == "primary" => Some(Self::Primary),
            Data::String(name) if name == "external" => Some(Self::External),
            Data::Integer(id) => Some(Self::Id(*id)),
            _ => None,
        }
    }
}

/// The sizes a mod's window may ask for, opening and at its smallest.
const MOD_WINDOW_WIDTH: std::ops::RangeInclusive<f32> = 200.0..=4000.0;
const MOD_WINDOW_HEIGHT: std::ops::RangeInclusive<f32> = 160.0..=3000.0;

impl Read for ModWindowSize {
    fn read(value: &mlua::Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&[
            "size",
            "min_size",
            "display",
            "fullscreen",
            "titlebar",
            "traffic_lights",
        ]])?;
        let pair = |key: &str| -> Result<Option<(f32, f32)>, Error> {
            let Some(value) = fields.raw(key) else {
                return Ok(None);
            };
            let cx = cx.key(key);
            let size = Fields::read(value, &cx, &[&["width", "height"]])?;
            match (
                size.number("width", MOD_WINDOW_WIDTH)?,
                size.number("height", MOD_WINDOW_HEIGHT)?,
            ) {
                (Some(width), Some(height)) => Ok(Some((width, height))),
                _ => Err(cx.fail("takes both `width` and `height`")),
            }
        };
        let (size, min_size) = (pair("size")?, pair("min_size")?);
        if let (Some(size), Some(min)) = (size, min_size)
            && (min.0 > size.0 || min.1 > size.1)
        {
            return Err(cx
                .key("min_size")
                .fail("is larger than `size`, the size the window opens at"));
        }
        let display = match fields.raw("display") {
            None => None,
            Some(mlua::Value::String(name)) => Some(name.to_string_lossy().to_string())
                .and_then(|name| Display::from_data(&Data::String(name))),
            Some(mlua::Value::Integer(id)) => Display::from_data(&Data::Integer(*id)),
            Some(_) => None,
        };
        if fields.raw("display").is_some() && display.is_none() {
            return Err(cx
                .key("display")
                .fail("is \"primary\", \"external\" or an id from `app.displays`"));
        }
        let fullscreen = match fields.raw("fullscreen") {
            None => false,
            Some(mlua::Value::Boolean(fullscreen)) => *fullscreen,
            Some(_) => return Err(cx.key("fullscreen").fail("is true or false")),
        };
        Ok(Self {
            size,
            min_size,
            place: WindowPlace {
                display,
                fullscreen,
            },
            titlebar: fields.get("titlebar")?.unwrap_or_default(),
            traffic_lights: traffic_lights(&fields, cx)?,
        })
    }
}

/// The window as views see it: `app.window`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowState {
    pub platform: Platform,
    /// The title bar the window has: `hidden` only where the system agreed
    /// to leave it out (a Linux compositor may insist on its own).
    pub titlebar: Titlebar,
    pub maximized: bool,
    pub fullscreen: bool,
    /// Whether it is the frontmost window, which title bars often show.
    pub active: bool,
    /// The screen it is on: an `id` from `app.displays`.
    pub display: Option<i64>,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            platform: Platform::current(),
            titlebar: Titlebar::System,
            maximized: false,
            fullscreen: false,
            active: true,
            display: None,
        }
    }
}

impl WindowState {
    pub(crate) fn data(&self) -> Data {
        let mut fields = BTreeMap::from([
            ("platform".to_owned(), Data::from(self.platform.name())),
            ("titlebar".to_owned(), Data::from(self.titlebar.name())),
            ("maximized".to_owned(), Data::Bool(self.maximized)),
            ("fullscreen".to_owned(), Data::Bool(self.fullscreen)),
            ("active".to_owned(), Data::Bool(self.active)),
        ]);
        if let Some(display) = self.display {
            fields.insert("display".to_owned(), Data::Integer(display));
        }
        Data::Map(fields)
    }
}
