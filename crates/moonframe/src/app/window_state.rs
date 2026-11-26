//! The window as views read it, `app.window`.

use super::*;

/// The window as views see it. The title bar is hidden only where the
/// system went along: a Linux compositor may keep its own decorations.
pub(crate) fn window_state(titlebar: Titlebar, window: &Window, cx: &App) -> WindowState {
    let platform = Platform::current();
    let titlebar = match (titlebar, platform, window.window_decorations()) {
        (Titlebar::Hidden, Platform::Linux, Decorations::Server) => Titlebar::System,
        (titlebar, ..) => titlebar,
    };
    WindowState {
        platform,
        titlebar,
        maximized: window.is_maximized(),
        fullscreen: window.is_fullscreen(),
        active: window.is_window_active(),
        display: window
            .display(cx)
            .map(|display| display_number(display.id())),
        size: Some(points(window.viewport_size())),
    }
}

/// A window's size in whole points.
fn points(size: Size<Pixels>) -> (u32, u32) {
    let whole = |length: Pixels| f32::from(length).round().max(0.0) as u32;
    (whole(size.width), whole(size.height))
}
