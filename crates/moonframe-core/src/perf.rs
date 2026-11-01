//! The perf log (`moonframe::perf`): timings worth seeing only when slow.

use std::time::Duration;

/// Work that took this long is logged at `info`; anything quicker only at
/// `debug`, so a fast app keeps a quiet log.
pub const SLOW: Duration = Duration::from_millis(8);

/// Work that took this long is logged at `warn`: a whole frame at 60 Hz.
pub const VERY_SLOW: Duration = Duration::from_millis(16);

/// Logs to `moonframe::perf` at a level chosen by how long `$elapsed` took:
/// `debug` under [`SLOW`], `info` from it, `warn` from [`VERY_SLOW`].
#[macro_export]
macro_rules! perf {
    ($elapsed:expr, $($arg:tt)+) => {{
        let elapsed: ::std::time::Duration = $elapsed;
        if elapsed >= $crate::perf::VERY_SLOW {
            ::tracing::warn!(target: "moonframe::perf", $($arg)+);
        } else if elapsed >= $crate::perf::SLOW {
            ::tracing::info!(target: "moonframe::perf", $($arg)+);
        } else {
            ::tracing::debug!(target: "moonframe::perf", $($arg)+);
        }
    }};
}
