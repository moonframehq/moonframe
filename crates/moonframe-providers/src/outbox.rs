//! Messages waiting for a program that is slow to read them, in a process
//! or socket provider. `urgent` and `latest` in provider.toml name the
//! messages it treats apart, by a message's name: the string itself, or a
//! key of its table (`"stop"`, `{ exposure = 12 }`).
//!
//! - An urgent message goes ahead of all but other urgent ones, and is
//!   never dropped: it makes room by dropping the newest ordinary message.
//! - A latest message replaces the one of that name still waiting, where
//!   it waits: a slider's drag leaves one value in the queue, not hundreds.
//! - Past [`CAPACITY`] an ordinary message is dropped and counted, as a
//!   program that isn't reading must not hold anything up.

use {
    crate::{Manifest, Message, json},
    moonframe_core::Data,
    std::{
        collections::VecDeque,
        sync::{Condvar, Mutex, PoisonError},
    },
};

/// Ordinary messages that wait before new ones are dropped.
pub(crate) const CAPACITY: usize = 256;

struct Waiting {
    line: String,
    /// The name it replaces by, if it is a latest message.
    latest: Option<String>,
    urgent: bool,
}

#[derive(Default)]
struct Queue {
    waiting: VecDeque<Waiting>,
    dropped: u64,
    closed: bool,
}

pub(crate) struct Outbox {
    urgent: Vec<String>,
    latest: Vec<String>,
    queue: Mutex<Queue>,
    ready: Condvar,
}

impl Outbox {
    pub(crate) fn new(manifest: &Manifest) -> Self {
        Self {
            urgent: manifest.urgent.clone(),
            latest: manifest.latest.clone(),
            queue: Mutex::default(),
            ready: Condvar::new(),
        }
    }

    fn queue(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Queues `message` by the rules above.
    pub(crate) fn push(&self, message: &Message) {
        let urgent = named(&message.data, &self.urgent).is_some();
        let latest = named(&message.data, &self.latest).map(str::to_owned);
        let line = line(message);
        let mut queue = self.queue();
        if let Some(name) = &latest
            && let Some(waiting) = queue
                .waiting
                .iter_mut()
                .find(|waiting| waiting.latest.as_ref() == Some(name))
        {
            waiting.line = line;
            return;
        }
        if urgent {
            // The same stop twice says nothing new, so a program that
            // isn't reading can't grow the queue with them.
            if queue
                .waiting
                .iter()
                .any(|waiting| waiting.urgent && waiting.line == line)
            {
                return;
            }
            if queue.waiting.len() >= CAPACITY
                && queue.waiting.back().is_some_and(|last| !last.urgent)
            {
                queue.waiting.pop_back();
                queue.dropped += 1;
            }
            let place = queue
                .waiting
                .iter()
                .position(|waiting| !waiting.urgent)
                .unwrap_or(queue.waiting.len());
            queue.waiting.insert(place, Waiting {
                line,
                latest,
                urgent,
            });
        } else if queue.waiting.len() >= CAPACITY {
            queue.dropped += 1;
            return;
        } else {
            queue.waiting.push_back(Waiting {
                line,
                latest,
                urgent,
            });
        }
        self.ready.notify_one();
    }

    /// The next line to write, waiting for one; `None` once closed.
    pub(crate) fn pop(&self) -> Option<String> {
        let mut queue = self.queue();
        loop {
            if queue.closed {
                return None;
            }
            if let Some(waiting) = queue.waiting.pop_front() {
                return Some(waiting.line);
            }
            queue = self
                .ready
                .wait(queue)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Ends [`Outbox::pop`]; what still waits counts as dropped.
    pub(crate) fn close(&self) {
        let mut queue = self.queue();
        queue.closed = true;
        queue.dropped += queue.waiting.len() as u64;
        queue.waiting.clear();
        self.ready.notify_all();
    }

    /// How many messages wait, and how many were dropped so far.
    pub(crate) fn load(&self) -> (usize, u64) {
        let queue = self.queue();
        (queue.waiting.len(), queue.dropped)
    }
}

/// The name in `names` that `data` goes by, if any.
fn named<'a>(data: &'a Data, names: &[String]) -> Option<&'a str> {
    match data {
        Data::String(text) => names.contains(text).then_some(text.as_str()),
        Data::Map(map) => map
            .keys()
            .find(|key| names.contains(key))
            .map(String::as_str),
        _ => None,
    }
}

/// The line a message is written as: compact JSON, `from` first, so a
/// script can match a whole line exactly (the `herdr` provider does).
pub(crate) fn line(message: &Message) -> String {
    serde_json::json!({
        "from": message.from,
        "message": json::json(&message.data),
    })
    .to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, std::collections::BTreeMap};

    fn outbox(urgent: &[&str], latest: &[&str]) -> Outbox {
        let names = |names: &[&str]| names.iter().map(|name| (*name).to_owned()).collect();
        Outbox {
            urgent: names(urgent),
            latest: names(latest),
            queue: Mutex::default(),
            ready: Condvar::new(),
        }
    }

    fn message(data: Data) -> Message {
        Message {
            from: "ui".into(),
            data,
            origin: None,
        }
    }

    fn value(key: &str, value: i64) -> Message {
        message(Data::Map(BTreeMap::from([(
            key.to_owned(),
            Data::Integer(value),
        )])))
    }

    fn drain(outbox: &Outbox) -> Vec<String> {
        let mut lines = Vec::new();
        while outbox.load().0 > 0 {
            lines.push(outbox.pop().unwrap());
        }
        lines
    }

    #[test]
    fn a_message_is_one_compact_line_with_its_sender_first() {
        let message = Message {
            from: "herdr".into(),
            data: Data::from("install"),
            origin: None,
        };
        assert_eq!(line(&message), r#"{"from":"herdr","message":"install"}"#);
    }

    #[test]
    fn an_urgent_message_goes_ahead_of_ordinary_ones() {
        let outbox = outbox(&["stop", "pause"], &[]);
        outbox.push(&message(Data::from("one")));
        outbox.push(&message(Data::from("stop")));
        outbox.push(&value("pause", 1));
        let lines = drain(&outbox);
        assert!(lines[0].contains("stop"));
        assert!(lines[1].contains("pause"));
        assert!(lines[2].contains("one"));
    }

    #[test]
    fn a_full_queue_drops_and_counts_ordinary_messages_but_never_urgent_ones() {
        let outbox = outbox(&["stop"], &[]);
        for index in 0..CAPACITY + 3 {
            outbox.push(&value("n", index as i64));
        }
        assert_eq!(outbox.load(), (CAPACITY, 3));
        outbox.push(&message(Data::from("stop")));
        assert_eq!(outbox.load(), (CAPACITY, 4));
        assert!(outbox.pop().unwrap().contains("stop"));
    }

    #[test]
    fn the_same_urgent_message_waits_once() {
        let outbox = outbox(&["stop"], &[]);
        for _ in 0..1000 {
            outbox.push(&message(Data::from("stop")));
        }
        assert_eq!(outbox.load(), (1, 0));
    }

    #[test]
    fn a_latest_message_replaces_the_one_waiting_where_it_waits() {
        let outbox = outbox(&[], &["exposure"]);
        outbox.push(&value("exposure", 1));
        outbox.push(&value("gain", 2));
        outbox.push(&value("exposure", 3));
        let lines = drain(&outbox);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains(r#""exposure":3"#));
        assert!(lines[1].contains("gain"));
    }

    #[test]
    fn closing_ends_the_writer_and_counts_what_waited() {
        let outbox = outbox(&[], &[]);
        outbox.push(&message(Data::from("one")));
        outbox.close();
        assert_eq!(outbox.pop(), None);
        assert_eq!(outbox.load(), (0, 1));
    }
}
