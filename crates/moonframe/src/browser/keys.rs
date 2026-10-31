//! Keys for a page that holds the keyboard, on macOS. AppKit first offers
//! every key down to the window's views as a possible shortcut, and GPUI's
//! view takes that offer ahead of the page inside it: it would run the
//! app's bindings, hand the key to a focused terminal (which intercepts
//! every key), and send arrows through its own text input. So a local event
//! monitor, which runs before any of that, hands each key straight to the
//! page while a page is the window's first responder.
//! Two kinds of key go elsewhere, as in Safari:
//!
//! - Cut, copy, paste, select all, undo and redo reach a page as the edit
//!   actions, not as keys: WebKit fires the page's clipboard events only
//!   for those, and the app has no Edit menu to send them.
//! - The keys macOS keeps for the app and its windows (quit, hide,
//!   minimize, cycling windows) stay with the app; a page would swallow
//!   them.
//!
//! The monitor and the edit actions need `unsafe`: the handler must return
//! a valid event or null, and an action must be a selector the page
//! implements, neither of which `objc2-app-kit` can check.
#![allow(unsafe_code)]

use {
    block2::RcBlock,
    objc2::{
        ClassType as _, MainThreadMarker,
        rc::Retained,
        runtime::{AnyObject, NSObjectProtocol as _},
        sel,
    },
    objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventType},
    objc2_web_kit::WKWebView,
    std::{
        cell::OnceCell,
        ptr::{NonNull, null_mut},
    },
};

thread_local! {
    /// The monitor, installed with the first page and kept for the app's life.
    static MONITOR: OnceCell<Option<Retained<AnyObject>>> = const { OnceCell::new() };
}

/// Starts routing keys to pages. Later calls do nothing.
pub(super) fn install() {
    MONITOR.with(|monitor| {
        monitor.get_or_init(|| {
            let handler = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
                // SAFETY: AppKit passes a valid event that outlives this call.
                let native = unsafe { event.as_ref() };
                if deliver(native) {
                    null_mut()
                } else {
                    event.as_ptr()
                }
            });
            // SAFETY: the handler returns either the event AppKit gave it,
            // which AppKit keeps alive, or null, which drops the event.
            unsafe {
                NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                    NSEventMask::KeyDown | NSEventMask::KeyUp,
                    &handler,
                )
            }
        });
    });
}

/// Where a key goes while a page holds the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    /// To the page, as a key.
    Page,
    /// To the page, as an edit action.
    Edit(Edit),
    /// On to the app, as if no page were there.
    App,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edit {
    Cut,
    Copy,
    Paste,
    SelectAll,
    Undo,
    Redo,
}

/// The modifiers a key was pressed with, as far as routing cares.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Held {
    command: bool,
    shift: bool,
    option: bool,
    control: bool,
}

/// Routes a key down by its modifiers and the character it types without
/// them, lowercased. Key ups always follow to the page.
fn route(held: Held, key: &str) -> Route {
    let command = held.command && !held.option && !held.control;
    if command && !held.shift {
        match key {
            "x" => return Route::Edit(Edit::Cut),
            "c" => return Route::Edit(Edit::Copy),
            "v" => return Route::Edit(Edit::Paste),
            "a" => return Route::Edit(Edit::SelectAll),
            "z" => return Route::Edit(Edit::Undo),
            // Quit, hide, and minimize.
            "q" | "h" | "m" => return Route::App,
            _ => {},
        }
    }
    if command && held.shift && key == "z" {
        return Route::Edit(Edit::Redo);
    }
    // Hide the other apps, and cycle the app's windows either way.
    let app_key = held.command
        && !held.control
        && match key {
            "h" => held.option && !held.shift,
            "`" | "~" => !held.option,
            _ => false,
        };
    if app_key {
        Route::App
    } else {
        Route::Page
    }
}

/// Hands `event` to the page holding its window's keyboard, if a page does.
/// Returns whether it did.
fn deliver(event: &NSEvent) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let Some(page) = event
        .window(mtm)
        .and_then(|window| window.firstResponder())
        .filter(|responder| responder.isKindOfClass(WKWebView::class()))
    else {
        return false;
    };
    match event.r#type() {
        NSEventType::KeyDown => {
            let flags = event.modifierFlags();
            let held = Held {
                command: flags.contains(NSEventModifierFlags::Command),
                shift: flags.contains(NSEventModifierFlags::Shift),
                option: flags.contains(NSEventModifierFlags::Option),
                control: flags.contains(NSEventModifierFlags::Control),
            };
            let key = event
                .charactersIgnoringModifiers()
                .map(|key| key.to_string().to_lowercase())
                .unwrap_or_default();
            match route(held, &key) {
                Route::Page => page.keyDown(event),
                Route::App => return false,
                Route::Edit(edit) => {
                    let action = match edit {
                        Edit::Cut => sel!(cut:),
                        Edit::Copy => sel!(copy:),
                        Edit::Paste => sel!(paste:),
                        Edit::SelectAll => sel!(selectAll:),
                        Edit::Undo => sel!(undo:),
                        Edit::Redo => sel!(redo:),
                    };
                    // SAFETY: WebKit's view implements the standard edit
                    // actions, which take an optional sender; one it lacks
                    // is passed up the responder chain, or dropped.
                    unsafe { page.tryToPerform_with(action, None) };
                },
            }
        },
        NSEventType::KeyUp => page.keyUp(event),
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMAND: Held = Held {
        command: true,
        shift: false,
        option: false,
        control: false,
    };

    #[test]
    fn a_page_takes_its_keys_and_leaves_the_apps() {
        assert_eq!(route(Held::default(), "a"), Route::Page);
        assert_eq!(route(Held::default(), "\u{1b}"), Route::Page);
        assert_eq!(route(COMMAND, "c"), Route::Edit(Edit::Copy));
        assert_eq!(route(COMMAND, "v"), Route::Edit(Edit::Paste));
        assert_eq!(
            route(
                Held {
                    shift: true,
                    ..COMMAND
                },
                "z"
            ),
            Route::Edit(Edit::Redo)
        );
        assert_eq!(route(COMMAND, "q"), Route::App);
        assert_eq!(route(COMMAND, "`"), Route::App);
        // The app's own shortcuts are the page's while it has the keyboard,
        // as in a browser: ⌘R, ⌘L, ⌘F.
        assert_eq!(route(COMMAND, "r"), Route::Page);
        assert_eq!(
            route(
                Held {
                    control: true,
                    ..Held::default()
                },
                "c"
            ),
            Route::Page
        );
    }
}
