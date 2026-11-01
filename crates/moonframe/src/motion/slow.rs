//! View > Slow Animations: every animation in the app at a tenth of its
//! speed, to watch how it moves. Frames read the time from [`now`] rather
//! than [`Instant::now`]. While the switch is on, `now` advances a tenth as
//! fast from the moment it was turned on; turning it off returns to the
//! real time, so whatever is mid-way finishes at once.
//!
//! The switch is process-wide, like the menu bar that holds it.

use std::{sync::Mutex, time::Instant};

/// How many times slower animations run while the switch is on.
const FACTOR: u32 = 10;

/// When slow motion was turned on, while it is.
static SINCE: Mutex<Option<Instant>> = Mutex::new(None);

fn since() -> Option<Instant> {
    SINCE.lock().map_or(None, |since| *since)
}

/// Turns slow motion on or off.
pub(crate) fn set_slow(slow: bool) {
    if let Ok(mut since) = SINCE.lock() {
        *since = slow.then(Instant::now);
    }
}

/// Whether animations run slowly.
pub(crate) fn is_slow() -> bool {
    since().is_some()
}

/// The time animations read: the real time, or while slow motion is on,
/// time that has passed a tenth as fast since it was turned on.
pub(crate) fn now() -> Instant {
    at(Instant::now(), since())
}

fn at(real: Instant, since: Option<Instant>) -> Instant {
    match since {
        Some(since) => since + real.saturating_duration_since(since) / FACTOR,
        None => real,
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::time::Duration};

    #[test]
    fn slow_time_passes_a_tenth_as_fast_from_when_it_was_turned_on() {
        let on = Instant::now();
        let later = on + Duration::from_secs(10);
        assert_eq!(at(later, None), later);
        assert_eq!(at(later, Some(on)), on + Duration::from_secs(1));
        assert_eq!(at(on, Some(on)), on, "no jump when it turns on");
        // Time read before the switch stays where it was.
        let earlier = on.checked_sub(Duration::from_secs(1)).unwrap_or(on);
        assert_eq!(at(earlier, Some(on)), on);
    }
}
