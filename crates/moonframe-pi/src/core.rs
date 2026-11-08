//! How the pi provider runs. `Native::receive` must never wait, so it only
//! posts: a message goes to a loop on a thread of its own, which does what is
//! quick under the engine's lock and starts a worker for what waits on pi or
//! git. Each pi has its own threads, and streams into the engine as it
//! speaks; a publisher thread turns changes into at most one `app.pi` a
//! frame. Nothing holds the lock while it waits for anything.
//!
//! Stopping a run cannot queue behind any of that: `abort` was `urgent` as a
//! process provider, which native manifests refuse, so [`Core::abort`] goes
//! straight from `receive` to pi, past the loop and the lock, through what
//! [`Urgent`] keeps.

use {
    crate::{Config, commands, engine::Engine, rpc::Rpc, runtime::Runtime, snapshot, value::lock},
    moonframe_providers::Handle,
    serde_json::{Value, json},
    std::{
        collections::HashMap,
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{
            Arc, Condvar, Mutex, MutexGuard, PoisonError, RwLock, Weak,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver, RecvTimeoutError, Sender},
        },
        thread,
        time::{Duration, Instant},
    },
};

/// About eleven a second: each publish redraws the conversation.
const FRAME: Duration = Duration::from_millis(90);
/// How often the loop looks at the clock when nothing arrives.
const WAKE: Duration = Duration::from_millis(100);
const TICK: Duration = Duration::from_secs(1);
/// How long leaving waits for pis to go: closing stdin, then the signal.
const LEAVING: Duration = Duration::from_millis(1700);
/// How long a thread's queue of work waits for more before it goes.
const QUEUE_IDLE: Duration = Duration::from_secs(5);

type Job = Box<dyn FnOnce(&Arc<Core>) -> Result<(), String> + Send>;

/// Wakes the publisher.
#[derive(Default)]
pub(crate) struct Notify {
    dirty: Mutex<bool>,
    woken: Condvar,
}

impl Notify {
    pub(crate) fn changed(&self) {
        *lock(&self.dirty) = true;
        self.woken.notify_one();
    }

    /// Waits until something changed or `stop` says to leave.
    fn wait(&self, stop: &AtomicBool) {
        let mut dirty = lock(&self.dirty);
        while !*dirty && !stop.load(Ordering::SeqCst) {
            dirty = self
                .woken
                .wait(dirty)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn clear(&self) {
        *lock(&self.dirty) = false;
    }
}

/// What stopping a run needs, under a lock nothing holds for long.
#[derive(Default)]
pub(crate) struct Urgent {
    pub(crate) selected: Option<String>,
    pub(crate) live: HashMap<String, Arc<Rpc>>,
}

/// The provider, shared by its threads.
pub(crate) struct Core {
    pub(crate) config: Arc<Config>,
    engine: Mutex<Engine>,
    pub(crate) urgent: Mutex<Urgent>,
    pub(crate) sessions: crate::sessions::Sessions,
    notify: Arc<Notify>,
    runtime: RwLock<Arc<Runtime>>,
    stopping: AtomicBool,
    /// The pis started for one job, such as a fork: in no thread, but
    /// stopped with the rest as the provider goes.
    helpers: Mutex<Vec<Weak<Rpc>>>,
    /// Work that must reach a thread's pi in the order it was asked for,
    /// such as prompts, a queue per thread.
    queues: Mutex<HashMap<String, Sender<Job>>>,
    /// When sessions were last looked at.
    pub(crate) scanned: Mutex<Option<Instant>>,
}

impl Core {
    pub(crate) fn new(config: Config) -> Result<Arc<Self>, String> {
        let config = Arc::new(config);
        let notify = Arc::new(Notify::default());
        let engine = Engine::new(Arc::clone(&config), Arc::clone(&notify))?;
        let selected = engine.data().selected.clone();
        let runtime = Arc::new(Runtime::new(&config));
        Ok(Arc::new(Self {
            config,
            engine: Mutex::new(engine),
            urgent: Mutex::new(Urgent {
                selected,
                live: HashMap::new(),
            }),
            sessions: crate::sessions::Sessions::default(),
            notify,
            runtime: RwLock::new(runtime),
            stopping: AtomicBool::new(false),
            helpers: Mutex::default(),
            queues: Mutex::default(),
            scanned: Mutex::new(None),
        }))
    }

    /// The engine. Never hold this across anything that waits.
    pub(crate) fn lock(&self) -> MutexGuard<'_, Engine> {
        lock(&self.engine)
    }

    pub(crate) fn runtime(&self) -> Arc<Runtime> {
        let runtime = self.runtime.read().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(&runtime)
    }

    pub(crate) fn set_runtime(&self, new: Runtime) {
        *self.runtime.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(new);
    }

    pub(crate) fn changed(&self) {
        self.notify.changed();
    }

    pub(crate) fn stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }

    /// Set as the provider goes: a program run to its end watches it.
    pub(crate) fn stop_flag(&self) -> &AtomicBool {
        &self.stopping
    }

    /// Reports a failure as a toast in `app.pi.events`.
    pub(crate) fn fail(&self, problem: &str, thread: Option<&str>) {
        tracing::warn!("pilot: {problem}");
        let mut engine = self.lock();
        engine.emit("toast", thread, "That didn't work", problem, "error");
    }

    /// Runs `work` on a thread of its own; what it returns as a failure
    /// becomes a toast, and a panic stays in the thread.
    pub(crate) fn work(
        self: &Arc<Self>,
        work: impl FnOnce(&Arc<Self>) -> Result<(), String> + Send + 'static,
    ) {
        let core = Arc::clone(self);
        thread::spawn(move || {
            let result = guarded(|| work(&core));
            if let Err(problem) = result {
                core.fail(&problem, None);
            }
            core.changed();
        });
    }

    /// Keeps `rpc` to be stopped with the rest, or stops it now if the
    /// provider is already going.
    pub(crate) fn keep_helper(&self, rpc: &Arc<Rpc>) -> Result<(), String> {
        let mut helpers = lock(&self.helpers);
        helpers.retain(|kept| kept.upgrade().is_some_and(|kept| !kept.exited()));
        helpers.push(Arc::downgrade(rpc));
        drop(helpers);
        if self.stopping() {
            rpc.stop();
            return Err("Pilot is closing.".to_owned());
        }
        Ok(())
    }

    /// [`Core::work`], after whatever was queued for `key` before it: two
    /// prompts to one thread reach its pi in the order they were sent.
    pub(crate) fn work_in_order(
        self: &Arc<Self>,
        key: &str,
        work: impl FnOnce(&Arc<Self>) -> Result<(), String> + Send + 'static,
    ) {
        let mut queues = lock(&self.queues);
        let mut job: Job = Box::new(work);
        if let Some(queue) = queues.get(key) {
            match queue.send(job) {
                Ok(()) => return,
                Err(mpsc::SendError(back)) => job = back,
            }
        }
        let (queue, jobs) = mpsc::channel();
        let _ = queue.send(job);
        queues.insert(key.to_owned(), queue);
        drop(queues);
        let (core, key) = (Arc::clone(self), key.to_owned());
        thread::spawn(move || core.serve_queue(&key, &jobs));
    }

    fn serve_queue(self: &Arc<Self>, key: &str, jobs: &Receiver<Job>) {
        let run = |job: Job| {
            if let Err(problem) = guarded(|| job(self)) {
                self.fail(&problem, None);
            }
            self.changed();
        };
        loop {
            match jobs.recv_timeout(QUEUE_IDLE) {
                Ok(job) => run(job),
                Err(RecvTimeoutError::Timeout) => {
                    // Under the lock work is queued under: nothing slips in
                    // between the last look and the queue going.
                    let mut queues = lock(&self.queues);
                    if let Ok(job) = jobs.try_recv() {
                        drop(queues);
                        run(job);
                    } else {
                        queues.remove(key);
                        return;
                    }
                },
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    /// Stops the run of `thread`, or of the selected one, now.
    pub(crate) fn abort(self: &Arc<Self>, thread: Option<String>) {
        let rpc = {
            let urgent = lock(&self.urgent);
            let id = thread.or_else(|| urgent.selected.clone());
            id.and_then(|id| urgent.live.get(&id).cloned())
        };
        if let Some(rpc) = rpc {
            self.work(move |_| rpc.send("abort", json!({})).map(|_| ()));
        }
    }

    /// Stops everything, and waits for each pi to go.
    pub(crate) fn shutdown(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.notify.changed();
        let rpcs: Vec<Arc<Rpc>> = {
            let mut engine = self.lock();
            engine.store.flush_pending();
            let probe = engine.probe.take();
            let live = engine.live.values().map(|live| Arc::clone(&live.rpc));
            let helpers = lock(&self.helpers)
                .iter()
                .filter_map(Weak::upgrade)
                .collect::<Vec<_>>();
            live.chain(probe).chain(helpers).collect()
        };
        for rpc in &rpcs {
            rpc.stop();
        }
        let started = Instant::now();
        while started.elapsed() < LEAVING && !rpcs.iter().all(|rpc| rpc.exited()) {
            thread::sleep(Duration::from_millis(10));
        }
    }
}

/// `work`, with a panic as its failure: as an uncaught exception was to the
/// Node provider, it loses what it was doing and nothing else.
pub(crate) fn guarded<T>(work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or_else(|panic| {
        let what = panic
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "a panic".to_owned());
        Err(what)
    })
}

/// The loop: starts up, then takes messages in order and keeps time.
pub(crate) fn dispatch(core: &Arc<Core>, inbox: &Receiver<Value>) {
    if let Err(problem) = guarded(|| {
        core.boot();
        Ok(())
    }) {
        core.fail(&problem, None);
    }
    let mut ticked = Instant::now();
    loop {
        match inbox.recv_timeout(WAKE) {
            Ok(message) => {
                let handled = guarded(|| {
                    commands::handle(core, &message);
                    Ok(())
                });
                if let Err(problem) = handled {
                    core.fail(&problem, None);
                }
            },
            Err(RecvTimeoutError::Timeout) => {},
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if core.stopping() {
            return;
        }
        if ticked.elapsed() >= TICK {
            ticked = Instant::now();
            let ticking = guarded(|| {
                core.tick();
                Ok(())
            });
            if let Err(problem) = ticking {
                core.fail(&problem, None);
            }
        }
        core.lock().store.flush_if_due();
    }
}

/// Publishes what changed, at most once a frame, skipping what is the same.
pub(crate) fn publish(core: &Core, host: &Handle) {
    let mut last = String::new();
    loop {
        core.notify.wait(&core.stopping);
        if core.stopping() {
            return;
        }
        thread::sleep(FRAME);
        core.notify.clear();
        let built = guarded(|| Ok(snapshot::snapshot_of(&mut core.lock(), &core.runtime())));
        let view = match built {
            Ok(view) => view,
            Err(problem) => {
                tracing::error!("pilot: building app.pi: {problem}");
                continue;
            },
        };
        let Some(view) = view else {
            continue;
        };
        let (line, value) = snapshot::encode(view);
        if line != last {
            host.publish_json(&value);
            last = line;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn work_for_one_key_runs_in_the_order_it_was_asked_for() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let core = Core::new(Config {
            agent_dir: root.join("agent"),
            data_dir: root.join("data"),
            home: root.to_owned(),
            pi: None,
            login_shell: false,
            env: Vec::new(),
        })
        .unwrap();
        let (done, order) = mpsc::channel();
        let (first, second) = (done.clone(), done);
        core.work_in_order("t", move |_| {
            thread::sleep(Duration::from_millis(100));
            first.send(1).map_err(|error| error.to_string())
        });
        core.work_in_order("t", move |_| {
            second.send(2).map_err(|error| error.to_string())
        });
        let got: Vec<i32> = order.iter().take(2).collect();
        assert_eq!(got, [1, 2]);
    }
}
