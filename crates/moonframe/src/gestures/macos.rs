//! The trackpad on macOS. A local event monitor sees each event before any
//! view does, which lets gestures through GPUI's closed input enum without
//! patching it. It only looks: every event carries on to GPUI as it would
//! have (a swipe also goes on to be its mouse's back or forward button),
//! and what it finds goes down a channel to a task, since a monitor runs in
//! the middle of AppKit's dispatch, where GPUI may already be at work.
//!
//! The monitor needs `unsafe`: its handler is called with a raw event, and
//! must hand it back for AppKit to keep going.
#![allow(unsafe_code)]

use {
    super::{
        Gesture, SwipeDirection, begin_frame, deliver,
        swipe::{Scrolls, Stroke},
    },
    block2::RcBlock,
    futures::{StreamExt as _, channel::mpsc},
    gpui_kit::*,
    objc2::{rc::Retained, runtime::AnyObject},
    objc2_app_kit::{
        NSEvent, NSEventMask, NSEventModifierFlags, NSEventPhase, NSEventType, NSView,
    },
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    std::{
        cell::{OnceCell, RefCell},
        ptr::NonNull,
    },
};

thread_local! {
    /// The monitor, installed with the first area and kept for the app's life.
    static MONITOR: OnceCell<Option<Retained<AnyObject>>> = const { OnceCell::new() };
    /// The scroll under way, which may turn out a swipe.
    static SCROLLS: RefCell<Scrolls> = RefCell::new(Scrolls::default());
}

/// A gesture on its way to the window it happened in.
struct Happened {
    gesture: Gesture,
    /// AppKit's number for the window.
    window: isize,
    /// From the window's bottom-left corner.
    at: (f64, f64),
    modifiers: Modifiers,
}

/// Starts hearing gestures. Later calls do nothing.
pub(super) fn install(cx: &mut App) {
    if MONITOR.with(|monitor| monitor.get().is_some()) {
        return;
    }
    let (sender, mut receiver) = mpsc::unbounded();
    let handler = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit passes a valid event that outlives this call.
        let native = unsafe { event.as_ref() };
        if let Some(happened) = read(native) {
            let _ = sender.unbounded_send(happened);
        }
        event.as_ptr()
    });
    // SAFETY: the handler always returns the event AppKit gave it, which
    // AppKit keeps alive.
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(
            NSEventMask::Rotate
                | NSEventMask::SmartMagnify
                | NSEventMask::Swipe
                | NSEventMask::ScrollWheel,
            &handler,
        )
    };
    MONITOR.with(|slot| {
        let _ = slot.set(monitor);
    });
    cx.on_window_closed(|_, closed| begin_frame(closed))
        .detach();
    cx.spawn(async move |cx| {
        while let Some(happened) = receiver.next().await {
            cx.update(|cx| route(&happened, cx));
        }
    })
    .detach();
}

/// The gesture in `event`, if it holds one.
fn read(event: &NSEvent) -> Option<Happened> {
    let gesture = match event.r#type() {
        // AppKit counts counterclockwise.
        NSEventType::Rotate => Gesture::Rotate {
            degrees: -event.rotation(),
            phase: phase(event.phase())?,
        },
        NSEventType::SmartMagnify => Gesture::SmartZoom,
        NSEventType::Swipe => Gesture::Swipe(swiped(event.deltaX(), event.deltaY())?),
        NSEventType::ScrollWheel => Gesture::Swipe(scrolled(event)?),
        _ => return None,
    };
    let at = event.locationInWindow();
    Some(Happened {
        gesture,
        window: event.windowNumber(),
        at: (at.x, at.y),
        modifiers: modifiers(event.modifierFlags()),
    })
}

/// What a touch phase reads as, if it is one a gesture has.
fn phase(phase: NSEventPhase) -> Option<TouchPhase> {
    match phase {
        NSEventPhase::Began => Some(TouchPhase::Started),
        NSEventPhase::Changed | NSEventPhase::Stationary => Some(TouchPhase::Moved),
        NSEventPhase::Ended => Some(TouchPhase::Ended),
        NSEventPhase::Cancelled => Some(TouchPhase::Cancelled),
        _ => None,
    }
}

/// A swipe event's way, as GPUI reads it for its back and forward buttons.
fn swiped(dx: f64, dy: f64) -> Option<SwipeDirection> {
    match (dx, dy) {
        (dx, _) if dx > 0.0 => Some(SwipeDirection::Back),
        (dx, _) if dx < 0.0 => Some(SwipeDirection::Forward),
        (_, dy) if dy > 0.0 => Some(SwipeDirection::Up),
        (_, dy) if dy < 0.0 => Some(SwipeDirection::Down),
        _ => None,
    }
}

/// The swipe a trackpad's scroll finishes, if it does. Momentum after the
/// fingers lift has no phase of its own, so it never counts.
fn scrolled(event: &NSEvent) -> Option<SwipeDirection> {
    if !event.hasPreciseScrollingDeltas() {
        return None;
    }
    let stroke = match event.phase() {
        NSEventPhase::Began => Stroke::Began,
        NSEventPhase::Changed => Stroke::Changed,
        NSEventPhase::Ended => Stroke::Ended,
        NSEventPhase::Cancelled => Stroke::Cancelled,
        _ => return None,
    };
    // Natural scrolling moves the content with the fingers; turned off,
    // the same fingers report the other way.
    let way = if event.isDirectionInvertedFromDevice() {
        1.0
    } else {
        -1.0
    };
    let (dx, dy) = (event.scrollingDeltaX() * way, event.scrollingDeltaY() * way);
    SCROLLS.with_borrow_mut(|scrolls| scrolls.feed(stroke, dx, dy))
}

fn modifiers(flags: NSEventModifierFlags) -> Modifiers {
    Modifiers {
        control: flags.contains(NSEventModifierFlags::Control),
        alt: flags.contains(NSEventModifierFlags::Option),
        shift: flags.contains(NSEventModifierFlags::Shift),
        platform: flags.contains(NSEventModifierFlags::Command),
        function: flags.contains(NSEventModifierFlags::Function),
    }
}

/// Hands `happened` to the window it happened in.
fn route(happened: &Happened, cx: &mut App) {
    for handle in cx.windows() {
        let handled = handle.update(cx, |_, window, cx| {
            if number(window) != Some(happened.window) {
                return false;
            }
            let (x, y) = happened.at;
            let position = point(px(x as f32), window.viewport_size().height - px(y as f32));
            deliver(happened.gesture, position, happened.modifiers, window, cx);
            true
        });
        if handled.unwrap_or(false) {
            return;
        }
    }
}

/// AppKit's number for `window`.
fn number(window: &Window) -> Option<isize> {
    // Window has an inherent `window_handle` of its own.
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    // SAFETY: the handle holds the window's live view, read on the main
    // thread, where this runs.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    view.window().map(|window| window.windowNumber())
}
