//! The Luau side of moonframe. Luau modules describe the whole window: an
//! app folder holds `main.luau`, the shell that lays out the window, and
//! `mods/<name>/main.luau`, extensions the shell places by name or by region.
//! Each module returns `mod { init, update, view }`; `view` returns a tree of
//! plain tables that [`Node`] checks, so the app draws data and never calls
//! Lua while painting.
//!
//! ```lua
//! return mod {
//!   region = "left",
//!   init = function(previous) return previous or { count = 0 } end,
//!   update = function(state, msg)
//!     if msg == "add" then state.count += 1 end
//!   end,
//!   view = function(state, app)
//!     return ui.panel { title = "Counter", grow = true,
//!       ui.text(`Clicked {state.count} times`),
//!       ui.button { "Add", msg = "add" },
//!     }
//!   end,
//! }
//! ```
//!
//! Nothing here touches GPUI, files outside the app folder, or the network.

#[macro_use]
mod tokens;

mod component;
mod data;
mod error;
mod keys;
mod menus;
mod module;
mod node;
mod persist;
mod read;
mod roots;
mod style;
mod vm;
mod window;
mod workspace;

pub use {
    component::{Component, Demo, catalogue},
    data::Data,
    error::Error,
    keys::{Chord, Keys, Pressed},
    menus::{About, AppMenu, MenuCommand, MenuEntry},
    module::Meta,
    node::*,
    persist::Persist,
    read::MAX_DOCUMENT,
    roots::{AppLayer, Roots, mod_name},
    style::{
        Case, Color, Corners, Curve, Font, Look, MonoFont, Paint, Part, Resolved, Rules, Shade,
        Shadow, ShadowLayer, ShadowPreset, Sides, States, Style, Stylesheet, Token, Transition,
        TransitionKind,
    },
    vm::printable,
    window::{Chrome, Platform, Titlebar, WindowState},
    workspace::{DEMO, PROVIDER, SHELL, Slot, Workspace},
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
