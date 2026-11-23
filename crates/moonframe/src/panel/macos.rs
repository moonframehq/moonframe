//! Panel mode with AppKit; see the parent module for what it does.
//!
//! What trying it on a real app found:
//!
//! - GPUI makes an app `Regular` while it opens a windowed app, so the
//!   `Accessory` policy is set from `App::set_activation_policy`, which
//!   applies at once.
//! - A click on a status item reaches the app as a mouse down in the item's
//!   own window, active app or not. The button's action does not always
//!   fire, so a local monitor takes the click, and the button never sees it.
//! - `NSApp.activate()` is asynchronous on macOS 26: the app is active about
//!   400 ms later. With `activate()` alone another app stayed in front, so
//!   the panel asks with `activateIgnoringOtherApps` too.
//! - A click elsewhere makes the panel resign key. That is what hides it.
//! - An app with no Dock icon and no item has no way back to its window, so
//!   while the owner shows no item the app goes back to `Regular`. At launch
//!   the item may come late (a mod in a package loaded after start), so the
//!   app waits [`GRACE`] before it falls back.
//! - A sheet on the panel, or an open or save panel of the app, takes key
//!   status: the panel stays while one is up.

#![allow(unsafe_code, reason = "AppKit through objc2; see the SAFETY notes")]

use {
    block2::RcBlock,
    gpui_kit::{ActivationPolicy, App, Window},
    objc2::{
        ClassType, MainThreadMarker,
        rc::Retained,
        runtime::{NSObjectProtocol, ProtocolObject},
    },
    objc2_app_kit::{
        NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSSavePanel, NSScreen, NSView,
        NSWindow, NSWindowDidResignKeyNotification, NSWindowWillCloseNotification,
    },
    objc2_foundation::{NSNotification, NSNotificationCenter, NSPoint, NSRect, NSSize},
    raw_window_handle::RawWindowHandle,
    std::{
        cell::{Cell, RefCell},
        ptr::{self, NonNull},
        sync::OnceLock,
        time::Duration,
    },
};

/// The panel hangs this far under the item, in points.
const GAP: f64 = 4.0;
/// The panel keeps this far from the screen's edges.
const EDGE: f64 = 8.0;
/// How long the owner's item may take to come at launch, from the first
/// check, before the app falls back to a normal window.
const GRACE: Duration = Duration::from_secs(3);
/// The key code of W on an ANSI keyboard (`kVK_ANSI_W`).
const KEY_W: u16 = 13;

/// The mod whose item the panel hangs under. Set once, by [`start`].
static OWNER: OnceLock<String> = OnceLock::new();

thread_local! {
    /// The main window, once [`attach`] has it.
    static PANEL: RefCell<Option<Retained<NSWindow>>> = const { RefCell::new(None) };
    /// What watches the panel lose focus.
    static WATCH: RefCell<Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>> =
        const { RefCell::new(Vec::new()) };
    /// A hide is under way: AppKit tells the panel it resigned key inside it.
    static HIDING: Cell<bool> = const { Cell::new(false) };
    /// The owner shows no item: the app has its Dock icon, and the window is
    /// a normal one.
    static STRANDED: Cell<bool> = const { Cell::new(false) };
    /// The panel's height before [`show`] cut it to fit a short screen, and
    /// the height it was cut to: a panel no longer that tall was resized since.
    static TALL: Cell<Option<(f64, f64)>> = const { Cell::new(None) };
    /// The grace for the owner's item to come at launch is over, or waiting.
    static GRACE_OVER: Cell<bool> = const { Cell::new(false) };
    static GRACE_WAITING: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn on() -> bool {
    OWNER.get().is_some() && !STRANDED.get()
}

pub(super) fn is(window: &Window) -> bool {
    on() && same(native(window).as_deref(), panel().as_deref())
}

pub(super) fn start(owner: &str, cx: &mut App) {
    if OWNER.set(owner.to_owned()).is_err() {
        return;
    }
    cx.set_activation_policy(ActivationPolicy::Accessory);
    // A click on the item: the app's local monitor sees it first.
    let clicks = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit hands the monitor a live event.
        let event_ref = unsafe { event.as_ref() };
        if on_item(event_ref) {
            toggle();
            return ptr::null_mut();
        }
        event.as_ptr()
    });
    // SAFETY: the block returns the event it got, or null.
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::LeftMouseDown, &clicks)
    };
    // The monitors live as long as the app.
    std::mem::forget(monitor);

    // Cmd-W: an app with no Dock icon has no menu bar, so the menu's Close
    // Window never sees the key. The panel hides on it.
    let keys = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit hands the monitor a live event.
        let event_ref = unsafe { event.as_ref() };
        if closes_panel(event_ref) {
            hide();
            return ptr::null_mut();
        }
        event.as_ptr()
    });
    // SAFETY: the block returns the event it got, or null.
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &keys)
    };
    std::mem::forget(monitor);
}

pub(super) fn attach(window: &Window, cx: &mut App) {
    let Some(panel) = native(window) else {
        // Nothing could show the window: a normal one, with its Dock icon.
        tracing::warn!("panel mode: the main window has no AppKit window; it stays a normal one");
        STRANDED.set(true);
        cx.set_activation_policy(ActivationPolicy::Regular);
        window.defer(cx, |window, cx| {
            if !crate::background() {
                cx.activate(true);
            }
            window.activate_window();
        });
        return;
    };
    // A click elsewhere: the panel loses key status. A sheet on it, or an
    // open or save panel, takes it too, and the panel stays for those.
    let lost = RcBlock::new(|_: NonNull<NSNotification>| {
        if !dialog_up() {
            hide();
        }
    });
    // Closed while it is a normal window: nothing is left to show.
    let closed = RcBlock::new(|_: NonNull<NSNotification>| forget());
    let center = NSNotificationCenter::defaultCenter();
    // SAFETY: AppKit's notification names, constants for the app's life.
    let names = unsafe {
        [
            NSWindowDidResignKeyNotification,
            NSWindowWillCloseNotification,
        ]
    };
    for (name, block) in names.into_iter().zip([&lost, &closed]) {
        // SAFETY: main thread; the blocks keep no borrow across the call.
        let watch = unsafe {
            center.addObserverForName_object_queue_usingBlock(Some(name), Some(&panel), None, block)
        };
        WATCH.with_borrow_mut(|all| all.push(watch));
    }
    panel.orderOut(None);
    PANEL.with_borrow_mut(|slot| *slot = Some(panel));
}

/// The panel's window closed: drops it and what watches it.
fn forget() {
    PANEL.with_borrow_mut(|slot| *slot = None);
    let watches = WATCH.with_borrow_mut(std::mem::take);
    let center = NSNotificationCenter::defaultCenter();
    for watch in watches {
        // SAFETY: each was added to the default center by `attach`.
        unsafe { center.removeObserver(watch.as_ref()) };
    }
}

/// Whether a sheet is on the panel, or an open or save panel of the app is up.
fn dialog_up() -> bool {
    let (Some(panel), Some(main)) = (panel(), MainThreadMarker::new()) else {
        return false;
    };
    panel.attachedSheet().is_some()
        || NSApplication::sharedApplication(main)
            .windows()
            .iter()
            .any(|window| window.isVisible() && window.isKindOfClass(NSSavePanel::class()))
}

pub(super) fn show() {
    show_with(true);
}

/// Shows the panel under its item; with `activate`, brings the app forward
/// and gives the panel the keys, else only orders it in.
fn show_with(activate: bool) {
    let (Some(panel), Some(main)) = (panel(), MainThreadMarker::new()) else {
        return;
    };
    place(&panel, main);
    if !activate {
        panel.orderFront(None);
        return;
    }
    let app = NSApplication::sharedApplication(main);
    app.activate();
    #[allow(
        deprecated,
        reason = "the cooperative `activate` did not bring the app forward"
    )]
    app.activateIgnoringOtherApps(true);
    panel.makeKeyAndOrderFront(None);
}

/// Puts the panel under its item, or in the middle of the screen without
/// one, as tall as it was before a short screen cut it, or as the user left it.
fn place(panel: &NSWindow, main: MainThreadMarker) {
    let mut size = panel.frame().size;
    size.height = restored(TALL.take(), size.height);
    let item = item_frame();
    let frame = match item {
        Some((icon, area)) => frame_under(icon, size, area),
        None => {
            let room = panel
                .screen()
                .or_else(|| NSScreen::mainScreen(main))
                .map_or(size.height, |screen| {
                    screen.visibleFrame().size.height - 2.0 * EDGE
                });
            let height = size.height.min(room.max(0.0));
            NSRect::new(panel.frame().origin, NSSize::new(size.width, height))
        },
    };
    if frame.size.height < size.height {
        TALL.set(Some((size.height, frame.size.height)));
    }
    panel.setFrame_display(frame, true);
    if item.is_none() {
        panel.center();
    }
}

/// The height to show the panel at: the one `tall` kept, unless the panel is
/// no longer the height it was cut to (`current`), as the user resized it.
fn restored(tall: Option<(f64, f64)>, current: f64) -> f64 {
    match tall {
        Some((tall, cut)) if (cut - current).abs() < 0.5 => tall,
        _ => current,
    }
}

pub(super) fn hide() {
    // A normal window, while the item is away, does not hide.
    let (true, Some(panel)) = (on(), panel()) else {
        return;
    };
    // Inside a hide AppKit tells the panel it resigned key: only the call
    // that started the hide ends it.
    if HIDING.get() || !panel.isVisible() {
        return;
    }
    HIDING.set(true);
    panel.orderOut(None);
    HIDING.set(false);
}

pub(super) fn check(cx: &mut App) {
    let Some(owner) = OWNER.get() else {
        return;
    };
    // Before the window is the panel, nothing is hidden yet.
    if panel().is_none() {
        return;
    }
    let shown = crate::status_item::owners()
        .iter()
        .any(|item| item == owner);
    let Some(change) = change(STRANDED.get(), shown) else {
        return;
    };
    // At launch the item may come late: wait for it a while.
    if change == Change::Strand && !GRACE_OVER.get() {
        if !GRACE_WAITING.replace(true) {
            cx.spawn(async |cx| {
                cx.background_executor().timer(GRACE).await;
                cx.update(|cx| {
                    GRACE_OVER.set(true);
                    check(cx);
                });
            })
            .detach();
        }
        return;
    }
    STRANDED.set(change == Change::Strand);
    match change {
        Change::Strand => {
            tracing::warn!(
                "panel mode: the mod `{owner}` shows no menu bar item, so the window is a \
                 normal one until it does"
            );
            cx.set_activation_policy(ActivationPolicy::Regular);
            // A background launch keeps the user's app in front.
            show_with(!crate::background());
        },
        Change::Recover => {
            cx.set_activation_policy(ActivationPolicy::Accessory);
            // The normal window, if up, moves under the item.
            if let (Some(panel), Some(main)) = (panel(), MainThreadMarker::new())
                && panel.isVisible()
            {
                place(&panel, main);
            }
        },
    }
}

/// How panel mode stands next to the owner's item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    /// The item went away, or never came: a normal window, and a Dock icon.
    Strand,
    /// The item is back: a panel again.
    Recover,
}

/// What changes, when the app is `stranded` or not and the item is `shown`
/// or not.
fn change(stranded: bool, shown: bool) -> Option<Change> {
    match (stranded, shown) {
        (false, false) => Some(Change::Strand),
        (true, true) => Some(Change::Recover),
        _ => None,
    }
}

/// A click on the item: shows the panel, or hides it.
fn toggle() {
    let Some(panel) = panel() else {
        return;
    };
    if panel.isVisible() {
        hide();
    } else {
        show();
    }
}

/// The panel, a clone: AppKit calls back into this module from its own
/// calls, so no borrow of the slot lives across one.
fn panel() -> Option<Retained<NSWindow>> {
    PANEL.with_borrow(Clone::clone)
}

/// The AppKit window of a GPUI window.
fn native(window: &Window) -> Option<Retained<NSWindow>> {
    // Window has an inherent `window_handle` of its own.
    let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    // SAFETY: the handle holds the window's live view, read on the main
    // thread, where this runs.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    view.window()
}

/// Whether `event` is a plain click on the item of the panel's mod. A
/// Ctrl-click goes on: it opens the item's menu.
fn on_item(event: &NSEvent) -> bool {
    let (Some(owner), Some(main)) = (OWNER.get(), MainThreadMarker::new()) else {
        return false;
    };
    if event
        .modifierFlags()
        .contains(NSEventModifierFlags::Control)
    {
        return false;
    }
    same(
        crate::status_item::window(owner).as_deref(),
        event.window(main).as_deref(),
    )
}

/// Whether `a` and `b` are the same window; `false` when either is missing.
fn same(a: Option<&NSWindow>, b: Option<&NSWindow>) -> bool {
    a.zip(b).is_some_and(|(a, b)| ptr::eq(a, b))
}

/// Whether `event` is Cmd-W typed in the panel. Another window of the app
/// keeps its own Cmd-W.
fn closes_panel(event: &NSEvent) -> bool {
    // A normal window, while the item is away, has the menu's Close Window.
    let (true, Some(main)) = (on(), MainThreadMarker::new()) else {
        return false;
    };
    // Caps Lock, the keypad and Fn do not count.
    let held = NSEventModifierFlags::Command
        | NSEventModifierFlags::Shift
        | NSEventModifierFlags::Option
        | NSEventModifierFlags::Control;
    let keys = event
        .charactersIgnoringModifiers()
        .map(|keys| keys.to_string())
        .unwrap_or_default();
    event.modifierFlags() & held == NSEventModifierFlags::Command
        && is_w(&keys, event.keyCode())
        && same(panel().as_deref(), event.window(main).as_deref())
}

/// Whether `keys`, typed on the key `code`, is a W: any case, and on a
/// layout that types no Latin letter, the key where an ANSI W is.
fn is_w(keys: &str, code: u16) -> bool {
    if keys.is_ascii() {
        keys.eq_ignore_ascii_case("w")
    } else {
        code == KEY_W
    }
}

/// The item's frame, and the part of its screen free of the menu bar and Dock.
fn item_frame() -> Option<(NSRect, NSRect)> {
    let main = MainThreadMarker::new()?;
    let item = crate::status_item::window(OWNER.get()?)?;
    let area = item
        .screen()
        .or_else(|| NSScreen::mainScreen(main))?
        .visibleFrame();
    Some((item.frame(), area))
}

/// The panel's frame: under `icon`, centered on it, inside `area` however
/// narrow it is, and no taller than the part of `area` under the item.
/// AppKit's origin is the screen's bottom left.
fn frame_under(icon: NSRect, size: NSSize, area: NSRect) -> NSRect {
    let top_left = top_left_under(icon, size, area);
    let room = (top_left.y - area.origin.y - EDGE).max(0.0);
    let height = size.height.min(room);
    NSRect::new(
        NSPoint::new(top_left.x, top_left.y - height),
        NSSize::new(size.width, height),
    )
}

/// Where the panel's top left goes: under `icon`, centered on it, and inside
/// `area` however narrow it is.
fn top_left_under(icon: NSRect, size: NSSize, area: NSRect) -> NSPoint {
    let left = area.origin.x + EDGE;
    let right = area.origin.x + area.size.width - size.width - EDGE;
    let centered = icon.origin.x + icon.size.width / 2.0 - size.width / 2.0;
    NSPoint::new(centered.clamp(left, right.max(left)), icon.origin.y - GAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
    }

    #[test]
    fn the_panel_hangs_centered_under_the_item() {
        let screen = rect(0.0, 0.0, 2560.0, 1410.0);
        let icon = rect(2177.0, 1410.0, 40.0, 30.0);
        let at = top_left_under(icon, NSSize::new(640.0, 560.0), screen);
        // Centered on the icon, a few points under it.
        assert_eq!(at.x, 2177.0 + 20.0 - 320.0);
        assert_eq!(at.y, 1410.0 - GAP);
    }

    #[test]
    fn the_panel_stays_inside_the_screen() {
        let screen = rect(100.0, 0.0, 1000.0, 800.0);
        let size = NSSize::new(640.0, 560.0);
        let near_right = top_left_under(rect(1080.0, 800.0, 20.0, 30.0), size, screen);
        assert_eq!(near_right.x, 100.0 + 1000.0 - 640.0 - EDGE);
        let near_left = top_left_under(rect(100.0, 800.0, 20.0, 30.0), size, screen);
        assert_eq!(near_left.x, 100.0 + EDGE);
    }

    #[test]
    fn a_panel_wider_than_the_screen_keeps_its_left_edge() {
        let screen = rect(0.0, 0.0, 500.0, 800.0);
        let at = top_left_under(
            rect(250.0, 800.0, 20.0, 30.0),
            NSSize::new(640.0, 560.0),
            screen,
        );
        assert_eq!(at.x, EDGE);
    }

    #[test]
    fn a_panel_that_fits_keeps_its_height() {
        let screen = rect(0.0, 0.0, 2560.0, 1410.0);
        let icon = rect(2177.0, 1410.0, 40.0, 30.0);
        let frame = frame_under(icon, NSSize::new(640.0, 560.0), screen);
        assert_eq!(frame.size, NSSize::new(640.0, 560.0));
        // Its top hangs under the item.
        assert_eq!(frame.origin.y + frame.size.height, 1410.0 - GAP);
    }

    #[test]
    fn a_panel_taller_than_the_screen_is_cut_to_fit() {
        // A short screen whose Dock takes the bottom 60 points.
        let screen = rect(0.0, 60.0, 1440.0, 815.0);
        let icon = rect(1200.0, 875.0, 40.0, 24.0);
        let frame = frame_under(icon, NSSize::new(1280.0, 820.0), screen);
        assert_eq!(frame.origin.y + frame.size.height, 875.0 - GAP);
        assert_eq!(frame.origin.y, 60.0 + EDGE);
        assert_eq!(frame.size.width, 1280.0);
    }

    #[test]
    fn the_panel_follows_the_item_and_the_app_falls_back_without_it() {
        assert_eq!(change(false, true), None);
        assert_eq!(change(false, false), Some(Change::Strand));
        assert_eq!(change(true, false), None);
        assert_eq!(change(true, true), Some(Change::Recover));
    }

    #[test]
    fn a_cut_panel_gets_its_height_back_unless_resized() {
        assert_eq!(restored(Some((820.0, 790.0)), 790.0), 820.0);
        // The user dragged it to 500 since.
        assert_eq!(restored(Some((820.0, 790.0)), 500.0), 500.0);
        assert_eq!(restored(None, 600.0), 600.0);
    }

    #[test]
    fn cmd_w_is_any_case_and_any_layout() {
        assert!(is_w("w", KEY_W));
        // Caps Lock.
        assert!(is_w("W", KEY_W));
        // A Russian layout types `ц` there.
        assert!(is_w("ц", KEY_W));
        assert!(!is_w("q", 12));
        assert!(!is_w("й", 12));
    }

    #[test]
    fn a_missing_window_is_never_the_panel() {
        assert!(!same(None, None));
    }

    #[test]
    fn without_a_panel_every_call_does_nothing() {
        // Not started and not attached: no panel, no item, no crash.
        assert!(!on());
        show();
        hide();
        toggle();
        assert!(panel().is_none());
        assert!(item_frame().is_none());
    }
}
