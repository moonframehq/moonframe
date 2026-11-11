//! How long each part of a run took, in milliseconds from its start, so a
//! slow run says where its time went: the agent starting, its handshake and
//! session, its first word, its tools (on each tool), the end of its turn,
//! and the check of the app after it.

use {
    serde_json::{Value, json},
    std::time::Instant,
};

#[derive(Debug)]
pub(crate) struct Timing {
    /// When the run began, in this process.
    start: Instant,
    /// The agent's process started.
    pub(crate) spawned: Option<u64>,
    /// It answered `initialize`.
    pub(crate) initialized: Option<u64>,
    /// Its session opened, and the prompt went.
    pub(crate) session: Option<u64>,
    /// Its first words or tool call.
    pub(crate) first_token: Option<u64>,
    /// Its turn ended.
    pub(crate) ended: Option<u64>,
    /// How long the check of the app after it took, and before it.
    pub(crate) check: Option<u64>,
    pub(crate) baseline: Option<u64>,
    /// It reused an agent already running, so it started nothing.
    pub(crate) warm: bool,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            spawned: None,
            initialized: None,
            session: None,
            first_token: None,
            ended: None,
            check: None,
            baseline: None,
            warm: false,
        }
    }
}

impl Timing {
    /// A run that took a warm agent: its agent started and, if `ready`,
    /// shook hands and opened its session before it.
    pub(crate) fn warm(ready: bool) -> Self {
        Self {
            warm: true,
            spawned: Some(0),
            initialized: ready.then_some(0),
            session: ready.then_some(0),
            ..Self::default()
        }
    }

    /// Milliseconds since the run began.
    pub(crate) fn now(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({
            "spawned": self.spawned,
            "initialized": self.initialized,
            "session": self.session,
            "first_token": self.first_token,
            "ended": self.ended,
            "check": self.check,
            "baseline": self.baseline,
            "warm": self.warm,
        })
    }

    /// The timing saved with a run.
    pub(crate) fn restore(value: Option<&Value>) -> Self {
        let field = |key: &str| value.and_then(|value| value.get(key)?.as_u64());
        Self {
            spawned: field("spawned"),
            initialized: field("initialized"),
            session: field("session"),
            first_token: field("first_token"),
            ended: field("ended"),
            check: field("check"),
            baseline: field("baseline"),
            warm: value
                .and_then(|value| value.get("warm")?.as_bool())
                .unwrap_or(false),
            ..Self::default()
        }
    }
}
