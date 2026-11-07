//! How close each module's Luau state comes to its limits, for the perf log
//! (`moonframe::perf`), so a limit is changed on what mods really use.
//!
//! Read as each call ends, and as a module's source and `init` end: garbage not yet collected counts, and memory a
//! call took and gave back within itself does not. Luau gives no hook for
//! the moment in between, so a peak is what was seen, not the exact most.

use std::cell::Cell;

const MIB: f64 = 1024. * 1024.;

/// Growth worth another line, as a fraction of the last one logged. Without
/// it each small rise would be a line.
const STEP: f64 = 1.25;

/// Under this nothing is logged: every module starts about here.
const FLOOR: usize = 1024 * 1024;

/// The share of a limit that earns a warning, once per limit.
const NEAR: f64 = 0.75;

/// What one reading has to say.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Report {
    Quiet,
    Grew,
    /// It came near a limit for the first time.
    Near,
}

/// One limit's highest reading.
#[derive(Debug)]
struct Peak {
    limit: u64,
    floor: u64,
    seen: Cell<u64>,
    logged: Cell<u64>,
    warned: Cell<bool>,
}

impl Peak {
    fn new(limit: u64, floor: u64) -> Self {
        Self {
            limit,
            floor,
            seen: Cell::new(0),
            logged: Cell::new(0),
            warned: Cell::new(false),
        }
    }

    fn read(&self, value: u64) -> Report {
        self.seen.set(self.seen.get().max(value));
        if !self.warned.get() && value as f64 >= self.limit as f64 * NEAR {
            self.warned.set(true);
            self.logged.set(value);
            return Report::Near;
        }
        if value >= self.floor && value as f64 >= self.logged.get() as f64 * STEP {
            self.logged.set(value);
            return Report::Grew;
        }
        Report::Quiet
    }
}

/// The highest memory and instruction checks a module's state was seen at.
#[derive(Debug)]
pub(crate) struct Peaks {
    name: String,
    memory: Peak,
    checks: Peak,
}

impl Peaks {
    pub(crate) fn new(name: &str, memory: usize, checks: u64) -> Self {
        Self {
            name: name.to_owned(),
            memory: Peak::new(memory as u64, FLOOR as u64),
            // A tenth of the budget, or every call would be a line.
            checks: Peak::new(checks, checks / 10),
        }
    }

    /// Takes the readings as a call ends, and logs what they show.
    pub(crate) fn read(&self, memory: usize, checks: u64) {
        let name = &self.name;
        let limit = self.memory.limit as f64 / MIB;
        let used = memory as f64 / MIB;
        match self.memory.read(memory as u64) {
            Report::Near => tracing::warn!(
                target: "moonframe::perf",
                "{name}: {used:.1} MiB of Luau memory, near its limit of {limit:.0} MiB"
            ),
            Report::Grew => tracing::debug!(
                target: "moonframe::perf",
                "{name}: Luau memory reached {used:.1} MiB of {limit:.0} MiB"
            ),
            Report::Quiet => {},
        }
        let budget = self.checks.limit;
        match self.checks.read(checks) {
            Report::Near => tracing::warn!(
                target: "moonframe::perf",
                "{name}: a call used {checks} of its {budget} instruction checks"
            ),
            Report::Grew => tracing::debug!(
                target: "moonframe::perf",
                "{name}: a call reached {checks} of {budget} instruction checks"
            ),
            Report::Quiet => {},
        }
    }

    /// The highest memory and instruction checks seen so far.
    #[cfg(test)]
    pub(crate) fn seen(&self) -> (u64, u64) {
        (self.memory.seen.get(), self.checks.seen.get())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "peaks/tests.rs"]
mod tests;
