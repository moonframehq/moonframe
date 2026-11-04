//! What process and socket providers share: lines of JSON read from a
//! program and published as they arrive, lines written to it from an
//! [`Outbox`], and the loop between them.

use {
    crate::{Context, Event, Link, Publisher, json, outbox::Outbox},
    moonframe_core::{Data, Error},
    std::{
        io::{BufRead, BufReader, Read, Write},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{Receiver, RecvTimeoutError, TryRecvError},
        },
        time::{Duration, Instant},
    },
};

/// How often the provider's thread looks up while no message comes: to
/// see that it should stop, or that the program went. Output never waits
/// for this; it is published as it is read.
const POLL: Duration = Duration::from_millis(100);
/// The most often a provider reports its queue.
const REPORT: Duration = Duration::from_millis(200);

/// Why the program's output ended.
pub(crate) enum Ended {
    Closed,
    Problem(String),
}

/// Why [`pump`] returned.
pub(crate) enum Pumped {
    Stopped,
    Ended(Ended),
}

/// Publishes each line of `output` as it arrives, until it ends or
/// `stop` is set. Lines are no longer than [`json::MAX_JSON`], so a
/// runaway program cannot fill memory with one.
pub(crate) fn publish_lines(
    output: impl Read,
    publisher: &Publisher,
    stop: &Arc<AtomicBool>,
) -> Ended {
    let mut reader = BufReader::new(output);
    let limit = json::MAX_JSON as u64 + 1;
    loop {
        let mut line = Vec::new();
        match reader.by_ref().take(limit).read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return Ended::Closed,
            Ok(_) if line.len() as u64 >= limit && line.last() != Some(&b'\n') => {
                return Ended::Problem(format!(
                    "printed a line longer than {} bytes",
                    json::MAX_JSON
                ));
            },
            Ok(_) => {},
        }
        // A provider being replaced says nothing more.
        if stop.load(Ordering::Relaxed) {
            return Ended::Closed;
        }
        let text = String::from_utf8_lossy(&line);
        if text.trim().is_empty() {
            continue;
        }
        match parse(&publisher.name, &text) {
            Ok(data) => publisher.publish(data),
            Err(problem) => return Ended::Problem(problem.to_string()),
        }
    }
}

/// Reads `text` as JSON, timing it for the perf log.
fn parse(name: &str, text: &str) -> Result<Data, Error> {
    let started = Instant::now();
    let parsed = json::data(text);
    let elapsed = started.elapsed();
    moonframe_core::perf!(
        elapsed,
        "provider {name}: {} bytes read in {elapsed:.1?}",
        text.len()
    );
    parsed
}

/// Writes each line of `outbox` to `input`, until it closes or the
/// program stops reading.
pub(crate) fn write_lines(mut input: impl Write, outbox: &Outbox) {
    while let Some(line) = outbox.pop() {
        if writeln!(input, "{line}").is_err() {
            return;
        }
    }
}

/// Queues the provider's messages for the program until it is told to
/// stop, the output ends (`ended` hears why), or `exited` says the program
/// went: what it started may hold its output open long after.
pub(crate) fn pump(
    context: &Context,
    outbox: &Outbox,
    ended: &Receiver<Ended>,
    reporter: &mut Reporter,
    mut exited: impl FnMut() -> bool,
) -> Pumped {
    let mut warned = 0;
    loop {
        match context.messages.recv_timeout(POLL) {
            Ok(message) => outbox.push(&message),
            Err(RecvTimeoutError::Timeout) => {},
            Err(RecvTimeoutError::Disconnected) => return Pumped::Stopped,
        }
        if context.stopped() {
            return Pumped::Stopped;
        }
        match ended.try_recv() {
            Ok(why) => return Pumped::Ended(why),
            Err(TryRecvError::Empty) => {},
            Err(TryRecvError::Disconnected) => return Pumped::Ended(Ended::Closed),
        }
        if exited() {
            // Its last lines go out before it is said to have gone.
            let why = ended.recv_timeout(POLL).unwrap_or(Ended::Closed);
            return Pumped::Ended(why);
        }
        let (waiting, dropped) = outbox.load();
        if dropped > warned {
            if warned == 0 {
                tracing::warn!(
                    provider = %context.name,
                    "not reading its messages; dropping them until it does"
                );
            }
            warned = dropped;
        }
        reporter.queue(waiting, dropped);
    }
}

/// Tells the app how a provider's link stands, as `app.links.<name>`.
pub(crate) struct Reporter {
    name: String,
    events: crate::Events,
    told: Link,
    link: Link,
    /// Dropped in earlier connections.
    carried: u64,
    at: Option<Instant>,
}

impl Reporter {
    pub(crate) fn new(context: &Context) -> Self {
        Self {
            name: context.name.clone(),
            events: Arc::clone(&context.events),
            told: Link::default(),
            link: Link::default(),
            carried: 0,
            at: None,
        }
    }

    /// The queue now, told at most every [`REPORT`].
    fn queue(&mut self, waiting: usize, dropped: u64) {
        self.link.waiting = waiting;
        self.link.dropped = self.carried + dropped;
        if self.at.is_none_or(|at| at.elapsed() >= REPORT) {
            self.tell();
        }
    }

    /// Whether the provider's program is there, told at once.
    pub(crate) fn connected(&mut self, connected: bool) {
        self.link.connected = Some(connected);
        self.tell();
    }

    /// Adds `dropped` to what is kept across connections.
    pub(crate) fn carry(&mut self, dropped: u64) {
        self.carried += dropped;
        self.link.dropped = self.carried;
        self.link.waiting = 0;
    }

    fn tell(&mut self) {
        if self.link != self.told {
            self.told = self.link;
            self.at = Some(Instant::now());
            (self.events)(Event::Link {
                provider: self.name.clone(),
                link: self.link,
            });
        }
    }
}

/// Waits `delay` with the program gone, counting what arrives as dropped;
/// whether to go on.
pub(crate) fn wait(context: &Context, delay: Duration, reporter: &mut Reporter) -> bool {
    let until = Instant::now() + delay;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return !context.stopped();
        }
        match context.messages.recv_timeout(left) {
            Ok(_) => {
                reporter.carry(1);
                reporter.tell();
            },
            Err(RecvTimeoutError::Timeout) => {},
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}
