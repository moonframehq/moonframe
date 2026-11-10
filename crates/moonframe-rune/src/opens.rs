//! How often a mod may open web pages in the user's browser (`app:open`), as
//! GPUI limits it: a burst at most, so a mod cannot flood the browser.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// At most this many pages opened in [`WINDOW`].
const BURST: usize = 3;
const WINDOW: Duration = Duration::from_secs(10);

/// The times pages were opened lately.
#[derive(Default)]
pub(crate) struct Opens(VecDeque<Instant>);

impl Opens {
    /// Whether a page may open at `now`; counts it when it may.
    pub(crate) fn may_open(&mut self, now: Instant) -> bool {
        while self
            .0
            .front()
            .is_some_and(|at| now.duration_since(*at) >= WINDOW)
        {
            self.0.pop_front();
        }
        if self.0.len() >= BURST {
            return false;
        }
        self.0.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burst_opens_and_the_next_waits() {
        let mut opens = Opens::default();
        let now = Instant::now();
        assert!((0..BURST).all(|_| opens.may_open(now)));
        assert!(!opens.may_open(now));
        assert!(opens.may_open(now + WINDOW));
    }
}
