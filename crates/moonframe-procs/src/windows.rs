//! What Windows says: the CPU times from `GetSystemTimes`, and memory as
//! `sysinfo` reads it, shaped here so a test can give the numbers.

use {
    crate::sample::Times,
    serde_json::{Value, json},
};

/// The times from `GetSystemTimes`, whose kernel time includes the idle.
pub(crate) fn times_of(idle: u64, kernel: u64, user: u64) -> Times {
    Times {
        user,
        system: kernel.saturating_sub(idle),
        idle,
        total: kernel.saturating_add(user),
    }
}

/// A byte count as JSON's whole number.
fn whole(bytes: u64) -> i64 {
    i64::try_from(bytes).unwrap_or(i64::MAX)
}

/// `memory` from what Windows reports: physical memory in use and all of
/// it, and the page file in use as swap. Windows gives no active, wired or
/// compressed count and no pressure level: those are 0 and "normal", which
/// the monitor shows as no warning.
pub(crate) fn memory(total: u64, used: u64, swap: u64) -> Value {
    json!({
        "used": whole(used.min(total)),
        "total": whole(total),
        "active": 0,
        "wired": 0,
        "compressed": 0,
        "swap": whole(swap),
        "pressure": "normal",
    })
}

/// The CPU times so far, `None` when Windows does not give them.
#[cfg(windows)]
#[allow(unsafe_code, reason = "one Win32 call; see SAFETY")]
pub(crate) fn times() -> Option<Times> {
    /// Win32's `FILETIME`: 100-nanosecond ticks in two halves.
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    impl FileTime {
        fn ticks(&self) -> u64 {
            (u64::from(self.high) << 32) | u64::from(self.low)
        }
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
    }

    let mut idle = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    // SAFETY: three valid, writable `FILETIME`s, which outlive the call.
    let done = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) };
    (done != 0).then(|| times_of(idle.ticks(), kernel.ticks(), user.ticks()))
}
