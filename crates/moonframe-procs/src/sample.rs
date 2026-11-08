//! CPU readings from the time the CPUs spent so far, sampled every two
//! seconds where no `iostat` prints them (Linux and Windows): each reading
//! splits the time between two samples, as `iostat` splits its interval.

use {
    crate::{Work, table::Reading},
    std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc::Sender,
        },
        thread,
        time::{Duration, Instant},
    },
};

/// How often a reading comes, as `iostat -w2`.
#[cfg_attr(
    test,
    allow(dead_code, reason = "the app's cadence; tests use a short one")
)]
pub(crate) const EVERY: Duration = Duration::from_secs(2);
/// How often a waiting sampler looks whether it was stopped.
const WAKE: Duration = Duration::from_millis(100);

/// The time all CPUs spent so far, in the system's own ticks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Times {
    pub(crate) user: u64,
    pub(crate) system: u64,
    pub(crate) idle: u64,
    /// All of it: more than the three when some is in neither (as steal).
    pub(crate) total: u64,
}

/// One look at the CPU: its times so far and the `[1m, 5m, 15m]` load.
pub(crate) type Sample = (Times, [f64; 3]);

/// Takes a sample, `None` when it cannot.
pub(crate) type Sampler = Box<dyn FnMut() -> Option<Sample> + Send>;

/// `part` of `whole` in whole percents, rounded.
fn percent(part: u64, whole: u64) -> i64 {
    (part as f64 * 100.0 / whole as f64).round() as i64
}

/// The reading between two samples; `None` when no time passed.
pub(crate) fn reading(before: Times, (after, load): Sample) -> Option<Reading> {
    let total = after.total.saturating_sub(before.total);
    if total == 0 {
        return None;
    }
    let share = |now: u64, then: u64| percent(now.saturating_sub(then).min(total), total);
    Some(Reading {
        user: share(after.user, before.user),
        system: share(after.system, before.system),
        idle: share(after.idle, before.idle),
        load,
    })
}

/// Sends a reading every `every`, from samples after `first`, until
/// `stopped`.
pub(crate) fn every(
    mut sampler: Sampler,
    first: Times,
    every: Duration,
    readings: &Sender<Work>,
    stopped: &AtomicBool,
) {
    let mut before = first;
    let mut due = Instant::now();
    loop {
        due += every;
        loop {
            if stopped.load(Ordering::SeqCst) {
                return;
            }
            let left = due.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            thread::sleep(left.min(WAKE));
        }
        let Some(sample) = sampler() else {
            continue;
        };
        let times = sample.0;
        if let Some(reading) = reading(before, sample)
            && readings.send(Work::Reading(reading)).is_err()
        {
            return;
        }
        before = times;
    }
}
