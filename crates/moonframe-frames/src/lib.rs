//! Frames shared with another process: a ring of slots in POSIX shared
//! memory, which `ui.video` shows. An engine writes frames at its own pace
//! and never waits; a viewer takes the newest and never holds it up, so a
//! viewer that falls behind skips frames instead of delaying anything.
//!
//! Everything is little-endian. Offsets are in bytes.
//!
//! ```text
//! header (64 bytes)
//!    0  magic        b"MFRING1\0"
//!    8  slots        u32
//!   16  slot_size    u64   a slot's bytes, its 64-byte header included
//!   24  latest       u64   the newest finished frame's number; 0 for none
//! slot i at 64 + i * slot_size; frame n goes in slot (n - 1) % slots
//!    0  number       u64   0 while it is written, then the frame's number
//!    8  width        u32
//!   12  height       u32
//!   16  format       u32   see [`Format`]
//!   20  stride       u32   bytes from one row to the next
//!   24  time         u64   capture time, nanoseconds since the Unix epoch
//!   32  pattern      u32   a Bayer frame's [`Pattern`]
//!   64  pixels
//! ```
//!
//! To write frame `n`: set the slot's `number` to 0, write the fields and
//! pixels, set `number` to `n`, then `latest` to `n`. A reader checks
//! `number` before and after reading, and drops a frame the writer lapped.
//! `examples/framebench/engine/ring.py` is a writer in Python.

mod format;
#[cfg(unix)]
mod ring;
mod tone;

pub use {
    format::{Format, Pattern},
    tone::Tone,
};
#[cfg(unix)]
pub use ring::{FrameInfo, Reader, Writer};

/// A source's longest name: macOS allows 31 bytes with the leading `/`.
pub const MAX_NAME: usize = 30;

/// Whether `name` can name a ring: letters, digits, `.`, `_` and `-`.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::valid_name;

    #[test]
    fn names_stay_within_what_shared_memory_allows() {
        assert!(valid_name("nova-preview_1.a"));
        for name in ["", ".hidden", "a/b", "../x", &"x".repeat(31)] {
            assert!(!valid_name(name), "{name}");
        }
    }
}
