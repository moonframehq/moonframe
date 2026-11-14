//! Hot keys with Carbon's `RegisterEventHotKey`: system-wide, and with no
//! permission to ask for, unlike watching every key. The handler, on the
//! application's event target, hears each press on the main thread.

#![allow(unsafe_code, reason = "Carbon's C API; each call says why it is sound")]

use {
    moonframe_core::Chord,
    std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        ffi::c_void,
        ptr,
    },
};

type OSStatus = i32;
type EventTargetRef = *mut c_void;
type EventRef = *mut c_void;
type EventHandlerCallRef = *mut c_void;
type EventHandlerRef = *mut c_void;
type EventHotKeyRef = *mut c_void;
type EventHandlerUPP = unsafe extern "C" fn(EventHandlerCallRef, EventRef, *mut c_void) -> OSStatus;

#[repr(C)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct EventHotKeyID {
    signature: u32,
    id: u32,
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> EventTargetRef;
    fn InstallEventHandler(
        target: EventTargetRef,
        handler: EventHandlerUPP,
        count: u32,
        types: *const EventTypeSpec,
        user_data: *mut c_void,
        out: *mut EventHandlerRef,
    ) -> OSStatus;
    fn RegisterEventHotKey(
        key_code: u32,
        modifiers: u32,
        id: EventHotKeyID,
        target: EventTargetRef,
        options: u32,
        out: *mut EventHotKeyRef,
    ) -> OSStatus;
    fn UnregisterEventHotKey(hot_key: EventHotKeyRef) -> OSStatus;
    fn GetEventParameter(
        event: EventRef,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        size: usize,
        actual_size: *mut usize,
        data: *mut c_void,
    ) -> OSStatus;
}

const fn four(code: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*code)
}

const EVENT_CLASS_KEYBOARD: u32 = four(b"keyb");
const EVENT_HOT_KEY_PRESSED: u32 = 5;
const PARAM_DIRECT_OBJECT: u32 = four(b"----");
const TYPE_HOT_KEY_ID: u32 = four(b"hkid");
/// This app's hot keys, among every app's.
const SIGNATURE: u32 = four(b"MnFr");
/// Another app registered the same keys for itself alone.
const HOT_KEY_EXISTS: OSStatus = -9878;
/// Passes an event on to the next handler.
const EVENT_NOT_HANDLED: OSStatus = -9874;
/// `kEventHotKeyExclusive`: no other app that asks the same may share
/// them, and registering fails when one holds them so. Apps that share
/// theirs are not seen.
const EXCLUSIVE: u32 = 1;

const CMD_KEY: u32 = 1 << 8;
const SHIFT_KEY: u32 = 1 << 9;
const OPTION_KEY: u32 = 1 << 11;
const CONTROL_KEY: u32 = 1 << 12;

thread_local! {
    static INSTALLED: Cell<bool> = const { Cell::new(false) };
    /// The keys of each registration, by its id.
    static KEYS: RefCell<HashMap<u32, String>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<u32> = const { Cell::new(1) };
}

/// A hot key the system holds for the app, until dropped.
pub(super) struct Registered {
    hot_key: EventHotKeyRef,
    id: u32,
}

impl Drop for Registered {
    fn drop(&mut self) {
        // SAFETY: a registration RegisterEventHotKey returned, given back
        // once, on the main thread where it was made.
        unsafe { UnregisterEventHotKey(self.hot_key) };
        KEYS.with_borrow_mut(|keys| keys.remove(&self.id));
    }
}

/// Registers `chord` as `keys`: `None` when another app holds it for
/// itself alone.
pub(super) fn register(keys: &str, chord: &Chord) -> Result<Option<Registered>, String> {
    if objc2::MainThreadMarker::new().is_none() {
        return Err("hot keys are registered from the main thread only".to_owned());
    }
    let code = key_code(&chord.key)
        .ok_or_else(|| format!("`{}` is not a key on a Mac keyboard", chord.key))?;
    install()?;
    let id = NEXT_ID.get();
    NEXT_ID.set(id.wrapping_add(1).max(1));
    let mut hot_key = ptr::null_mut();
    // SAFETY: plain values, the app's own event target, and a pointer to
    // a local the call fills in.
    let status = unsafe {
        RegisterEventHotKey(
            code,
            modifiers(chord),
            EventHotKeyID {
                signature: SIGNATURE,
                id,
            },
            GetApplicationEventTarget(),
            EXCLUSIVE,
            &raw mut hot_key,
        )
    };
    match status {
        0 => {
            KEYS.with_borrow_mut(|held| held.insert(id, keys.to_owned()));
            Ok(Some(Registered { hot_key, id }))
        },
        HOT_KEY_EXISTS => Ok(None),
        status => Err(format!("Carbon refused it ({status})")),
    }
}

/// Installs the handler that hears presses, once.
fn install() -> Result<(), String> {
    if INSTALLED.get() {
        return Ok(());
    }
    let pressed = EventTypeSpec {
        event_class: EVENT_CLASS_KEYBOARD,
        event_kind: EVENT_HOT_KEY_PRESSED,
    };
    // SAFETY: the app's event target, a handler that lives as long as the
    // program, one event type read during the call, and no user data.
    let status = unsafe {
        InstallEventHandler(
            GetApplicationEventTarget(),
            handle,
            1,
            &raw const pressed,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(format!("Carbon would not listen for hot keys ({status})"));
    }
    INSTALLED.set(true);
    Ok(())
}

/// A hot key was pressed: finds which, and queues its press. Another's
/// (a different signature) goes on to the next handler.
unsafe extern "C" fn handle(_: EventHandlerCallRef, event: EventRef, _: *mut c_void) -> OSStatus {
    let mut id = EventHotKeyID::default();
    // SAFETY: the event Carbon passed, read into a value of the size given.
    let status = unsafe {
        GetEventParameter(
            event,
            PARAM_DIRECT_OBJECT,
            TYPE_HOT_KEY_ID,
            ptr::null_mut(),
            size_of::<EventHotKeyID>(),
            ptr::null_mut(),
            (&raw mut id).cast(),
        )
    };
    if status != 0 || id.signature != SIGNATURE {
        return EVENT_NOT_HANDLED;
    }
    if let Some(keys) = KEYS.with_borrow(|keys| keys.get(&id.id).cloned()) {
        super::pressed(&keys);
    }
    0
}

/// Carbon's modifiers for `chord`; `secondary` is ⌘ here.
fn modifiers(chord: &Chord) -> u32 {
    [
        (chord.cmd || chord.secondary, CMD_KEY),
        (chord.shift, SHIFT_KEY),
        (chord.alt, OPTION_KEY),
        (chord.ctrl, CONTROL_KEY),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .fold(0, |all, (_, flag)| all | flag)
}

/// The virtual key code of `key` on an ANSI keyboard, as Carbon names
/// keys: where the key sits, not what it types.
fn key_code(key: &str) -> Option<u32> {
    const LETTERS: [u32; 26] = [
        0x00, 0x0B, 0x08, 0x02, 0x0E, 0x03, 0x05, 0x04, 0x22, 0x26, 0x28, 0x25, 0x2E, 0x2D, 0x1F,
        0x23, 0x0C, 0x0F, 0x01, 0x11, 0x20, 0x09, 0x0D, 0x07, 0x10, 0x06,
    ];
    const DIGITS: [u32; 10] = [0x1D, 0x12, 0x13, 0x14, 0x15, 0x17, 0x16, 0x1A, 0x1C, 0x19];
    const FUNCTION: [u32; 20] = [
        0x7A, 0x78, 0x63, 0x76, 0x60, 0x61, 0x62, 0x64, 0x65, 0x6D, 0x67, 0x6F, 0x69, 0x6B, 0x71,
        0x6A, 0x40, 0x4F, 0x50, 0x5A,
    ];
    let mut chars = key.chars();
    if let (Some(only), None) = (chars.next(), chars.next()) {
        return match only {
            'a'..='z' => Some(LETTERS[only as usize - 'a' as usize]),
            '0'..='9' => Some(DIGITS[only as usize - '0' as usize]),
            '=' => Some(0x18),
            '-' => Some(0x1B),
            ']' => Some(0x1E),
            '[' => Some(0x21),
            '\'' => Some(0x27),
            ';' => Some(0x29),
            '\\' => Some(0x2A),
            ',' => Some(0x2B),
            '/' => Some(0x2C),
            '.' => Some(0x2F),
            '`' => Some(0x32),
            _ => None,
        };
    }
    if let Some(number) = key.strip_prefix('f').and_then(|n| n.parse::<usize>().ok()) {
        return number
            .checked_sub(1)
            .and_then(|at| FUNCTION.get(at))
            .copied();
    }
    Some(match key {
        "enter" => 0x24,
        "tab" => 0x30,
        "space" => 0x31,
        "backspace" => 0x33,
        "escape" => 0x35,
        "insert" => 0x72,
        "home" => 0x73,
        "pageup" => 0x74,
        "delete" => 0x75,
        "end" => 0x77,
        "pagedown" => 0x79,
        "left" => 0x7B,
        "right" => 0x7C,
        "down" => 0x7D,
        "up" => 0x7E,
        _ => return None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_found_where_they_sit() {
        assert_eq!(key_code("a"), Some(0x00));
        assert_eq!(key_code("z"), Some(0x06));
        assert_eq!(key_code("0"), Some(0x1D));
        assert_eq!(key_code("space"), Some(0x31));
        assert_eq!(key_code("f1"), Some(0x7A));
        assert_eq!(key_code("f20"), Some(0x5A));
        // What only shift types, and keys a Mac lacks, have none.
        assert_eq!(key_code("?"), None);
        assert_eq!(key_code("f21"), None);
        let chord = Chord::parse("secondary-shift-k").unwrap_or_else(|problem| panic!("{problem}"));
        assert_eq!(modifiers(&chord), CMD_KEY | SHIFT_KEY);
    }
}
