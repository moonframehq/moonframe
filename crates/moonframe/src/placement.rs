//! Where the main window was and how large, kept per app (and profile) in
//! the app's state folder, so the next launch opens it there.
//!
//! The file is read before the event loop starts and written off the UI
//! thread, at most twice a second while the window moves, and once more as
//! the app quits. A frame that no longer fits a screen (the screen it was on
//! is gone, or it was moved off) is dropped for the shell's size, centered.
//! Runs without the user's folder (`--no-user`: checks, screenshots) and
//! shells that say `window = { remember = false }` keep nothing.

use {
    gpui_kit::{
        App, AppContext as _, Bounds, Context, DisplayId, Pixels, Subscription, Window,
        WindowBounds, point, px, size,
    },
    serde::{Deserialize, Serialize},
    std::{
        cell::{Cell, RefCell},
        path::{Path, PathBuf},
        rc::Rc,
        time::Duration,
    },
};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;

/// How long a burst of moves and resizes settles before it is saved.
const SETTLE: Duration = Duration::from_millis(500);
/// How much of a saved frame must lie on its screen to be reopened there:
/// enough of the top to grab and drag back.
const GRAB: (f32, f32) = (120., 40.);

/// The main window as it last was.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct Saved {
    /// Windowed, maximized, or fullscreen; the frame is the one it goes
    /// back to when it is neither.
    state: State,
    frame: Frame,
    /// The screen it was on: the system's stable id for it, where there is
    /// one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum State {
    Windowed,
    Maximized,
    Fullscreen,
}

/// A rectangle in logical pixels, as GPUI places windows.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct Frame {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Frame {
    fn of(bounds: Bounds<Pixels>) -> Self {
        Self {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        }
    }

    fn bounds(self) -> Bounds<Pixels> {
        Bounds::new(
            point(px(self.x), px(self.y)),
            size(px(self.width), px(self.height)),
        )
    }

    fn finite(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|value| value.is_finite())
    }
}

/// Where the main window's place is kept: `window.json` beside the state
/// database, `window-<profile>.json` for a profile.
pub(crate) fn file(profile: Option<&str>) -> PathBuf {
    crate::paths::state(profile).with_file_name(match profile {
        Some(profile) => format!("window-{profile}.json"),
        None => "window.json".to_owned(),
    })
}

/// What `path` keeps, or `None` when there is nothing (yet) or it is
/// unreadable, which opens the window at its default place.
pub(crate) fn load(path: &Path) -> Option<Saved> {
    let text = std::fs::read_to_string(path).ok()?;
    parse(&text)
        .inspect_err(|problem| {
            tracing::warn!(
                "{}: {problem}; opening at the default place",
                path.display()
            );
        })
        .ok()
}

fn parse(text: &str) -> serde_json::Result<Saved> {
    serde_json::from_str(text)
}

fn serialize(saved: &Saved) -> serde_json::Result<String> {
    serde_json::to_string_pretty(saved)
}

/// `frame` kept within `screen`, both in the same coordinates: shrunk to
/// fit, then moved fully onto it. `None` when too little of it lies on the
/// screen to reach, or it is no frame at all.
pub(crate) fn fit(frame: Frame, screen: Frame) -> Option<Frame> {
    if !frame.finite() || !screen.finite() || frame.width <= 0. || frame.height <= 0. {
        return None;
    }
    let overlap = |start: f32, length: f32, screen_start: f32, screen_length: f32| {
        (start + length).min(screen_start + screen_length) - start.max(screen_start)
    };
    // The title bar must be on the screen: the top of the frame.
    if overlap(frame.x, frame.width, screen.x, screen.width) < GRAB.0
        || overlap(frame.y, GRAB.1, screen.y, screen.height) < GRAB.1
    {
        return None;
    }
    let width = frame.width.min(screen.width);
    let height = frame.height.min(screen.height);
    Some(Frame {
        x: frame.x.clamp(screen.x, screen.x + screen.width - width),
        y: frame.y.clamp(screen.y, screen.y + screen.height - height),
        width,
        height,
    })
}

/// The bounds and screen to reopen `saved` at, if its screen is still
/// there and the frame still fits on it.
pub(crate) fn reopen(saved: &Saved, cx: &App) -> Option<(WindowBounds, DisplayId)> {
    let display = match &saved.display {
        Some(id) => cx.displays().into_iter().find(|display| {
            display
                .uuid()
                .is_ok_and(|uuid| uuid.to_string() == id.as_str())
        })?,
        None => cx.primary_display()?,
    };
    let frame = fit(saved.frame, screen_frame(display.visible_bounds()))?;
    let bounds = frame.bounds();
    let bounds = match saved.state {
        State::Windowed => WindowBounds::Windowed(bounds),
        State::Maximized => WindowBounds::Maximized(bounds),
        State::Fullscreen => WindowBounds::Fullscreen(bounds),
    };
    Some((bounds, display.id()))
}

/// The screen as window bounds see it. macOS gives a window's bounds from
/// the top left of its own screen; the others in the desktop's coordinates.
fn screen_frame(visible: Bounds<Pixels>) -> Frame {
    let mut frame = Frame::of(visible);
    if cfg!(target_os = "macos") {
        frame.x = 0.;
        frame.y = 0.;
    }
    frame
}

/// What `window` is now, to save.
fn snapshot(window: &Window, cx: &App) -> Saved {
    let (state, bounds) = match window.window_bounds() {
        WindowBounds::Windowed(bounds) if window.is_maximized() => (State::Maximized, bounds),
        WindowBounds::Windowed(bounds) => (State::Windowed, bounds),
        WindowBounds::Maximized(bounds) => (State::Maximized, bounds),
        WindowBounds::Fullscreen(bounds) => (State::Fullscreen, bounds),
    };
    Saved {
        state,
        frame: Frame::of(bounds),
        display: window
            .display(cx)
            .and_then(|display| display.uuid().ok())
            .map(|uuid| uuid.to_string()),
    }
}

/// Saves `window`'s place to `path` as it moves, resizes, or goes
/// fullscreen, and once more as the app quits; the caller keeps the
/// subscriptions as long as the window is open.
pub(crate) fn remember<T: 'static>(
    path: PathBuf,
    window: &mut Window,
    cx: &mut Context<T>,
) -> [Subscription; 2] {
    // The newest place not yet written, and whether a write is waiting
    // for the burst to settle.
    let latest: Rc<RefCell<Option<Saved>>> = Rc::default();
    let waiting = Rc::new(Cell::new(false));
    let path = Rc::new(path);
    let (on_quit, quit_path) = (latest.clone(), path.clone());
    let moved = cx.observe_window_bounds(window, move |_, window, cx| {
        let now = snapshot(window, cx);
        if latest.borrow().as_ref() == Some(&now) {
            return;
        }
        *latest.borrow_mut() = Some(now);
        if waiting.replace(true) {
            return;
        }
        let (latest, waiting, path) = (latest.clone(), waiting.clone(), path.clone());
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(SETTLE).await;
            waiting.set(false);
            let Some(saved) = latest.borrow_mut().take() else {
                return;
            };
            let path = PathBuf::clone(&path);
            cx.background_spawn(async move { write(&path, &saved) })
                .await;
        })
        .detach();
    });
    let quitting = cx.on_app_quit(move |_, cx| {
        let saved = on_quit.borrow_mut().take();
        let path = PathBuf::clone(&quit_path);
        cx.background_spawn(async move {
            if let Some(saved) = saved {
                write(&path, &saved);
            }
        })
    });
    [moved, quitting]
}

/// Writes `saved` to `path` through a temporary file, so a crash mid-write
/// leaves the last place whole.
fn write(path: &Path, saved: &Saved) {
    let written = serialize(saved)
        .map_err(std::io::Error::other)
        .and_then(|text| {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let temporary = path.with_extension("json.tmp");
            std::fs::write(&temporary, text)?;
            std::fs::rename(&temporary, path)
        });
    if let Err(problem) = written {
        tracing::warn!(
            "cannot keep the window's place in {}: {problem}",
            path.display()
        );
    }
}
