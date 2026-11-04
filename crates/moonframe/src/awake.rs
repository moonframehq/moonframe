//! Keeping the machine awake for mods (`app:awake`), as herdr-gpui's
//! caffeine cup does: `keepawake` holds IOKit power assertions on macOS, the
//! thread execution state on Windows, and D-Bus inhibitors on Linux. Each
//! ends with the process, so a crash leaves nothing held.
//!
//! One thread owns the hold: Windows ties it to the thread that made it, and
//! Linux makes blocking calls that must stay off the UI thread. Each mod
//! holds for itself, and the machine stays awake while any does. Holds keep
//! the system from sleeping when idle; closing a laptop's lid still does.

use {
    moonframe_core::{Awake as Request, Data},
    std::{
        collections::BTreeMap,
        sync::{
            Arc,
            mpsc::{self, Receiver, Sender},
        },
    },
};

/// What the system is asked to keep awake.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Want {
    display: bool,
    reason: String,
}

struct Order {
    want: Option<Want>,
    reply: Sender<Result<(), String>>,
}

/// Makes a hold, which lasts until dropped. Tests stand in for the system.
type Start = Arc<dyn Fn(&Want) -> Result<Box<dyn Send>, String> + Send + Sync>;

/// The answer to a change, once the thread has made it.
pub(crate) type Reply = Receiver<Result<(), String>>;

/// Which mods hold, and how that stands with the system.
pub(crate) struct Awake {
    /// Each mod holding, and whether it wants the display on and why.
    holds: BTreeMap<String, (bool, Option<String>)>,
    /// What the thread was last told.
    asked: Option<Want>,
    /// What the system last agreed to.
    held: Option<Want>,
    problem: Option<String>,
    /// Dropping it ends the thread, which lets go there.
    worker: Option<Sender<Order>>,
    start: Start,
}

impl Default for Awake {
    fn default() -> Self {
        Self::with(Arc::new(hold))
    }
}

impl Awake {
    fn with(start: Start) -> Self {
        Self {
            holds: BTreeMap::new(),
            asked: None,
            held: None,
            problem: None,
            worker: None,
            start,
        }
    }

    /// Takes `request` from `owner`; what to wait on, if the system has to
    /// be asked. A failed start shows in [`Self::data`].
    pub(crate) fn change(&mut self, owner: &str, request: Request) -> Option<Reply> {
        match request {
            Request::Hold { display, reason } => {
                self.holds.insert(owner.to_owned(), (display, reason));
            },
            Request::Release => {
                self.holds.remove(owner);
            },
        }
        let want = self.wanted();
        if want == self.asked {
            return None;
        }
        let worker = match self.worker.take() {
            Some(worker) => worker,
            None => match self.spawn() {
                Ok(worker) => worker,
                Err(problem) => {
                    self.problem = Some(problem);
                    return None;
                },
            },
        };
        let (reply, answer) = mpsc::channel();
        let order = Order {
            want: want.clone(),
            reply,
        };
        if worker.send(order).is_err() {
            self.problem = Some("the keep-awake thread stopped".to_owned());
            self.asked = None;
            return None;
        }
        self.worker = Some(worker);
        self.asked = want;
        Some(answer)
    }

    /// Takes the system's answer to the last change.
    pub(crate) fn settled(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.held.clone_from(&self.asked);
                self.problem = None;
            },
            Err(problem) => {
                tracing::warn!("could not keep the machine awake: {problem}");
                // Asking again tries again.
                self.asked = None;
                self.held = None;
                self.problem = Some(problem);
            },
        }
    }

    /// As views read it, in `app.awake`.
    pub(crate) fn data(&self) -> Data {
        let mut map = BTreeMap::from([
            ("held".to_owned(), Data::Bool(self.held.is_some())),
            (
                "display".to_owned(),
                Data::Bool(self.held.as_ref().is_some_and(|held| held.display)),
            ),
        ]);
        if let Some(problem) = &self.problem {
            map.insert("problem".to_owned(), Data::from(problem.as_str()));
        }
        Data::Map(map)
    }

    fn wanted(&self) -> Option<Want> {
        if self.holds.is_empty() {
            return None;
        }
        let reasons: Vec<&str> = self
            .holds
            .values()
            .filter_map(|(_, reason)| reason.as_deref())
            .collect();
        Some(Want {
            display: self.holds.values().any(|(display, _)| *display),
            reason: if reasons.is_empty() {
                format!("{} keeps the machine awake", crate::title())
            } else {
                reasons.join(", ")
            },
        })
    }

    fn spawn(&self) -> Result<Sender<Order>, String> {
        let (worker, orders) = mpsc::channel();
        let start = self.start.clone();
        std::thread::Builder::new()
            .name("moonframe-awake".into())
            .spawn(move || run(&orders, &start))
            .map_err(|error| format!("cannot start the keep-awake thread: {error}"))?;
        Ok(worker)
    }
}

/// Serves orders until the app drops its sender, then lets go here.
fn run(orders: &Receiver<Order>, start: &Start) {
    let mut held = None;
    for Order { want, reply } in orders {
        let result = match want {
            None => {
                held = None;
                Ok(())
            },
            // The new hold is made before the old one goes.
            Some(want) => start(&want).map(|hold| held = Some(hold)),
        };
        let _ = reply.send(result);
    }
    drop(held);
}

/// Idle, not `sleep`: that one needs AC power, and the lid closing should
/// still sleep.
#[cfg(feature = "awake")]
fn hold(want: &Want) -> Result<Box<dyn Send>, String> {
    let identity = crate::identity();
    keepawake::Builder::default()
        .display(want.display)
        .idle(true)
        .reason(want.reason.as_str())
        .app_name(identity.title.as_str())
        .app_reverse_domain(identity.id.as_str())
        .create()
        .map(|held| Box::new(held) as Box<dyn Send>)
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "awake"))]
fn hold(_: &Want) -> Result<Box<dyn Send>, String> {
    Err("this app was built without the `awake` feature".to_owned())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {
        super::*,
        std::sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    /// Counts holds alive, and remembers what was asked.
    #[derive(Default)]
    struct Fake {
        alive: AtomicUsize,
        asked: Mutex<Vec<Want>>,
    }

    struct Held(Arc<Fake>);

    impl Drop for Held {
        fn drop(&mut self) {
            self.0.alive.fetch_sub(1, Ordering::SeqCst);
        }
    }

    fn awake(fake: &Arc<Fake>) -> Awake {
        let fake = fake.clone();
        Awake::with(Arc::new(move |want| {
            fake.alive.fetch_add(1, Ordering::SeqCst);
            fake.asked.lock().unwrap().push(want.clone());
            Ok(Box::new(Held(fake.clone())) as Box<dyn Send>)
        }))
    }

    fn hold(display: bool) -> Request {
        Request::Hold {
            display,
            reason: Some("Scanning".to_owned()),
        }
    }

    fn settle(awake: &mut Awake, reply: Option<Reply>) {
        awake.settled(reply.unwrap().recv().unwrap());
    }

    fn held(awake: &Awake) -> bool {
        let Data::Map(map) = awake.data() else {
            panic!("not a map");
        };
        map["held"] == Data::Bool(true)
    }

    #[test]
    fn the_machine_stays_awake_while_any_mod_holds() {
        let fake = Arc::new(Fake::default());
        let mut awake = awake(&fake);
        let reply = awake.change("scan", hold(false));
        settle(&mut awake, reply);
        assert!(held(&awake));

        let reply = awake.change("render", hold(true));
        settle(&mut awake, reply);
        assert!(
            fake.asked.lock().unwrap().last().unwrap().display,
            "one mod wanting the display on is enough"
        );
        assert_eq!(fake.alive.load(Ordering::SeqCst), 1, "holds piled up");

        let reply = awake.change("render", Request::Release);
        settle(&mut awake, reply);
        assert!(held(&awake), "scan still holds");
        assert!(!fake.asked.lock().unwrap().last().unwrap().display);

        let reply = awake.change("scan", Request::Release);
        settle(&mut awake, reply);
        assert!(!held(&awake));
        assert_eq!(fake.alive.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn asking_for_the_same_again_asks_the_system_nothing() {
        let fake = Arc::new(Fake::default());
        let mut awake = awake(&fake);
        let reply = awake.change("scan", hold(true));
        settle(&mut awake, reply);
        assert!(awake.change("scan", hold(true)).is_none());
        assert!(awake.change("other", Request::Release).is_none());
    }

    #[test]
    fn a_refused_hold_says_why_and_can_be_tried_again() {
        let mut awake = Awake::with(Arc::new(|_| Err("no".to_owned())));
        let reply = awake.change("scan", hold(true));
        settle(&mut awake, reply);
        assert!(!held(&awake));
        let Data::Map(map) = awake.data() else {
            panic!("not a map");
        };
        assert_eq!(map["problem"], Data::from("no"));
        assert!(awake.change("scan", hold(true)).is_some());
    }

    #[test]
    fn dropping_it_lets_go() {
        let fake = Arc::new(Fake::default());
        let mut held = awake(&fake);
        let reply = held.change("scan", hold(true));
        settle(&mut held, reply);
        drop(held);
        let started = std::time::Instant::now();
        while fake.alive.load(Ordering::SeqCst) > 0 {
            assert!(started.elapsed().as_secs() < 5, "still held");
            std::thread::yield_now();
        }
    }
}
