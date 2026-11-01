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
        let traffic_lights = match fields.raw("traffic_lights") {
            None => None,
            Some(value) => {
                let cx = cx.key("traffic_lights");
                let point = Fields::read(value, &cx, &[&["x", "y"]])?;
                Some((
                    point.number("x", TRAFFIC_LIGHTS)?.unwrap_or(0.0),
                    point.number("y", TRAFFIC_LIGHTS)?.unwrap_or(0.0),
                ))
            },
        };
        Ok(Self {
            titlebar: fields.get("titlebar")?.unwrap_or_default(),
            traffic_lights,
            size,
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
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            platform: Platform::current(),
            titlebar: Titlebar::System,
            maximized: false,
            fullscreen: false,
            active: true,
        }
    }
}

impl WindowState {
    pub(crate) fn data(&self) -> Data {
        Data::Map(BTreeMap::from([
            ("platform".to_owned(), Data::from(self.platform.name())),
            ("titlebar".to_owned(), Data::from(self.titlebar.name())),
            ("maximized".to_owned(), Data::Bool(self.maximized)),
            ("fullscreen".to_owned(), Data::Bool(self.fullscreen)),
            ("active".to_owned(), Data::Bool(self.active)),
        ]))
    }
}
