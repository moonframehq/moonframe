//! What takes time: starting up, keeping pis running, and a thread's life
//! from prompt to settled. These run on workers and wait on pi and git, so
//! each takes the engine's lock only for the moments it reads or writes it.

use {
    crate::{
        config::real_path,
        core::Core,
        engine::{self, Branch, Engine, PiStatus},
        git,
        live::{Hook, Latch, Live},
        resources::config_stamp,
        rpc::{Rpc, Start},
        runtime::{Runtime, find_pi},
        schedule::{next_run, skip_missed},
        store::{Meta, Worktree},
        value::{as_list, display, lock, now_seconds},
    },
    serde_json::{Value, json},
    std::{
        path::{Path, PathBuf},
        sync::{Arc, OnceLock},
        time::{Duration, Instant},
    },
    uuid::Uuid,
};

/// How long a thread's pi stays once nobody uses it.
const IDLE_LIMIT: Duration = Duration::from_secs(600);
/// The pis running at once, unless all are busy.
const MAX_LIVE: usize = 12;
const SCAN_EVERY: Duration = Duration::from_secs(5);

/// A running thread, as a worker holds it: the lock isn't.
#[derive(Clone)]
pub(crate) struct LiveRef {
    pub(crate) id: String,
    pub(crate) rpc: Arc<Rpc>,
}

/// What a new thread starts with.
#[derive(Default)]
pub(crate) struct NewThread {
    pub(crate) workspace: Option<String>,
    pub(crate) worktree: bool,
    pub(crate) prompt: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) select: bool,
    pub(crate) model: Option<Value>,
    pub(crate) thinking: Option<String>,
}

pub(crate) fn pi_path(status: &PiStatus) -> Option<PathBuf> {
    match status {
        PiStatus::Ready { path, .. } => Some(path.clone()),
        _ => None,
    }
}

impl Core {
    // Starting up -----------------------------------------------------

    pub(crate) fn boot(self: &Arc<Self>) {
        let mut runtime = Runtime::clone(&self.runtime());
        if self.config.login_shell {
            runtime.adopt_login(&self.config);
            self.set_runtime(runtime.clone());
        }
        let status = match find_pi(&self.config, &runtime) {
            Ok(found) => PiStatus::Ready {
                path: found.path,
                version: found.version,
            },
            Err(reason) => {
                tracing::error!("pilot: {reason}");
                PiStatus::Missing { reason }
            },
        };
        {
            let mut engine = self.lock();
            engine.pi = status;
            let now = now_seconds();
            if skip_missed(&mut engine.data_mut().schedules, now) {
                engine.store.save();
            }
        }
        self.scan();
        let (selected, workspaces) = {
            let mut engine = self.lock();
            let selected = engine.selected().map(str::to_owned);
            if let Some(id) = &selected {
                engine.load_entries(id);
            }
            engine.stamp = config_stamp(&self.config);
            let paths = engine
                .data()
                .workspaces
                .iter()
                .map(|item| item.path.clone());
            (selected, paths.collect::<Vec<_>>())
        };
        self.changed();
        self.start_probe();
        for workspace in workspaces {
            self.refresh_branch(workspace);
        }
        if let Some(id) = selected {
            self.keep_live(id);
        }
    }

    /// The catalogue of models and commands comes from a pi of its own, so
    /// it is there before any thread runs.
    pub(crate) fn start_probe(self: &Arc<Self>) {
        let (old, path, cwd) = {
            let mut engine = self.lock();
            let cwd = engine
                .data()
                .workspaces
                .first()
                .map(|item| item.path.clone());
            (engine.probe.take(), pi_path(&engine.pi), cwd)
        };
        if let Some(old) = old {
            old.stop();
        }
        let Some(path) = path.filter(|_| !self.stopping()) else {
            return;
        };
        let cwd = cwd.unwrap_or_else(|| self.config.home.to_string_lossy().into_owned());
        let core = Arc::clone(self);
        let spawned = Rpc::spawn(Start {
            program: &path,
            args: &["--no-session".to_owned()],
            cwd: &cwd,
            runtime: &self.runtime(),
            on_event: Box::new(|_| {}),
            on_exit: Box::new(move |reason, said| {
                if !said.is_empty() {
                    tracing::warn!("pilot: pi catalogue stopped ({reason}): {said}");
                }
                let mut engine = core.lock();
                if engine.probe.as_ref().is_some_and(|probe| probe.exited()) {
                    engine.probe = None;
                }
            }),
        });
        let probe = match spawned {
            Ok(probe) => probe,
            Err(problem) => {
                tracing::warn!("pilot: pi catalogue: {problem}");
                return;
            },
        };
        {
            // Checked as it is kept, under the lock shutdown takes to
            // collect what to stop: one or the other sees it.
            let mut engine = self.lock();
            if self.stopping() {
                drop(engine);
                probe.stop();
                return;
            }
            engine.probe = Some(Arc::clone(&probe));
        }
        self.work(move |core| {
            let asked = probe
                .send("get_available_models", json!({}))
                .and_then(|models| Ok((models, probe.send("get_commands", json!({}))?)));
            let (models, commands) = match asked {
                Ok(answers) => answers,
                Err(problem) => {
                    tracing::warn!("pilot: pi catalogue: {problem}");
                    return Ok(());
                },
            };
            let mut engine = core.lock();
            // What a pi replaced since says is out of date.
            let current = engine
                .probe
                .as_ref()
                .is_some_and(|now| Arc::ptr_eq(now, &probe));
            if current {
                engine.models = as_list(&models["models"]).to_vec();
                engine.commands = as_list(&commands["commands"]).to_vec();
                engine.changed();
            }
            Ok(())
        });
    }

    pub(crate) fn tick(self: &Arc<Self>) {
        let now = Instant::now();
        let (selected, changed_stamp) = {
            let mut engine = self.lock();
            let running = engine.live.values().any(|live| live.running);
            let mut asked = running;
            for live in engine.live.values_mut() {
                asked |= live.expire_dialogs(now);
            }
            if asked {
                engine.changed();
            }
            let stamp = config_stamp(&self.config);
            let moved = stamp != engine.stamp;
            engine.stamp = stamp;
            (engine.selected().map(str::to_owned), moved)
        };
        if self.scan_due(now) {
            self.scan();
        }
        if changed_stamp {
            self.config_changed();
        }
        self.reap_idle(now, selected.as_deref());
        self.run_due(now_seconds());
    }

    fn scan_due(&self, now: Instant) -> bool {
        let mut scanned = lock(&self.scanned);
        let due = scanned.is_none_or(|at| now.duration_since(at) >= SCAN_EVERY);
        if due {
            *scanned = Some(now);
        }
        due
    }

    /// pi reads its settings when it starts: idle threads start again on
    /// their next use, and the catalogue at once.
    pub(crate) fn config_changed(self: &Arc<Self>) {
        self.start_probe();
        let selected = {
            let mut engine = self.lock();
            let idle: Vec<String> = engine
                .live
                .values()
                .filter(|live| !live.running && live.sending.is_none())
                .map(|live| live.id.clone())
                .collect();
            for id in idle {
                self.stop_live(&mut engine, &id);
            }
            engine.resources_asked += 1;
            engine.selected().map(str::to_owned)
        };
        if let Some(id) = selected {
            let core = Arc::clone(self);
            std::thread::spawn(move || {
                let _ = core.ensure_live(&id);
            });
        }
        self.changed();
    }

    fn reap_idle(&self, now: Instant, selected: Option<&str>) {
        let mut engine = self.lock();
        let quiet = |live: &Live| {
            now.duration_since(live.rpc.touched()) > IDLE_LIMIT
                && !live.running
                && live.dialogs.is_empty()
                && Some(live.id.as_str()) != selected
        };
        let idle: Vec<String> = engine
            .live
            .values()
            .filter(|live| quiet(live))
            .map(|live| live.id.clone())
            .collect();
        for id in idle {
            self.stop_live(&mut engine, &id);
        }
    }

    /// Stops a thread's pi; it leaves without a word.
    pub(crate) fn stop_live(&self, engine: &mut Engine, id: &str) {
        if let Some(live) = engine.live.remove(id) {
            lock(&self.urgent).live.remove(id);
            live.rpc.stop();
        }
    }

    // Workspaces and the threads in them ------------------------------

    pub(crate) fn refresh_branch(self: &Arc<Self>, cwd: String) {
        self.work(move |core| {
            let runtime = core.runtime();
            let branch = git::current_branch(&runtime, &cwd);
            let git = git::repo_root(&runtime, &cwd).is_some();
            let mut engine = core.lock();
            engine.branches.insert(cwd, Branch { branch, git });
            engine.changed();
            Ok(())
        });
    }

    /// Adopts sessions pi wrote in a workspace, the CLI's included, and
    /// keeps each thread's summary current. Reads outside the lock: a long
    /// session takes a while to parse.
    pub(crate) fn scan(&self) {
        let folders = self.lock().known_folders();
        let found = folders
            .into_iter()
            .map(|cwd| {
                let list = self.sessions.sessions_in(&self.config, &cwd);
                (cwd, list)
            })
            .collect();
        self.lock().merge_sessions(found);
    }

    pub(crate) fn refresh_summary(&self, id: &str) {
        let file = self.lock().thread(id).and_then(|meta| meta.file.clone());
        let fresh = file.and_then(|file| self.sessions.summary(Path::new(&file)));
        if let Some(fresh) = fresh {
            self.lock().summaries.insert(id.to_owned(), fresh);
        }
    }

    pub(crate) fn select(self: &Arc<Self>, id: Option<String>) {
        {
            let mut engine = self.lock();
            if id.as_ref().is_some_and(|id| engine.thread(id).is_none()) {
                return;
            }
            engine.data_mut().selected.clone_from(&id);
            lock(&self.urgent).selected.clone_from(&id);
            if let Some(id) = &id {
                if let Some(meta) = engine.thread_mut(id) {
                    meta.unread = false;
                    meta.failed = false;
                }
                engine.load_entries(id);
            }
            engine.store.save();
            engine.changed();
        }
        if let Some(id) = id {
            self.keep_live(id);
        }
    }

    /// Starts a thread's pi in the background; a failure is a toast on it.
    fn keep_live(self: &Arc<Self>, id: String) {
        self.work(move |core| {
            if let Err(problem) = core.ensure_live(&id) {
                core.fail(&problem, Some(&id));
            }
            Ok(())
        });
    }

    // Running pi ------------------------------------------------------

    /// The thread's pi, started if need be, once it has loaded.
    pub(crate) fn ensure_live(self: &Arc<Self>, id: &str) -> Result<LiveRef, String> {
        let (rpc, ready) = {
            let mut engine = self.lock();
            if let Some(live) = engine.live.get(id) {
                (Arc::clone(&live.rpc), Arc::clone(&live.ready))
            } else {
                self.start_live(&mut engine, id)?
            }
        };
        ready.wait()?;
        let mut engine = self.lock();
        let file = engine.live.get(id).and_then(|live| {
            let state = live.state.as_ref()?;
            state["sessionFile"].as_str().map(str::to_owned)
        });
        if let Some(meta) = engine.thread_mut(id)
            && file.is_some()
            && meta.file != file
        {
            meta.file = file;
            engine.store.save();
        }
        Ok(LiveRef {
            id: id.to_owned(),
            rpc,
        })
    }

    fn start_live(
        self: &Arc<Self>,
        engine: &mut Engine,
        id: &str,
    ) -> Result<(Arc<Rpc>, Arc<Latch>), String> {
        if self.stopping() {
            return Err("Pilot is closing.".to_owned());
        }
        let meta = engine.thread(id).ok_or("That thread is gone.")?;
        let path = pi_path(&engine.pi).ok_or("pi was not found.")?;
        if !Path::new(&meta.cwd).exists() {
            return Err(format!("Its folder is gone: {}", meta.cwd));
        }
        let args = launch_args(engine, meta, id);
        let cwd = meta.cwd.clone();
        self.make_room(engine);
        let (events, exit) = (Arc::clone(self), Arc::clone(self));
        let (first, last) = (id.to_owned(), id.to_owned());
        let shared = Arc::new(OnceLock::<Arc<Rpc>>::new());
        let (for_events, for_exit) = (Arc::clone(&shared), Arc::clone(&shared));
        let rpc = Rpc::spawn(Start {
            program: &path,
            args: &args,
            cwd: &cwd,
            runtime: &self.runtime(),
            on_event: Box::new(move |record| events.pi_event(&first, &for_events, &record)),
            on_exit: Box::new(move |reason, said| exit.ended(&last, &for_exit, &reason, &said)),
        })?;
        let _ = shared.set(Arc::clone(&rpc));
        let live = Live::new(id.to_owned(), cwd, Arc::clone(&rpc));
        let ready = Arc::clone(&live.ready);
        engine.live.insert(id.to_owned(), live);
        lock(&self.urgent)
            .live
            .insert(id.to_owned(), Arc::clone(&rpc));
        let (loading, latch, thread) = (Arc::clone(&rpc), Arc::clone(&ready), id.to_owned());
        self.work(move |core| {
            latch.finish(core.load_live(&thread, &loading));
            Ok(())
        });
        Ok((rpc, ready))
    }

    fn make_room(&self, engine: &mut Engine) {
        if engine.live.len() < MAX_LIVE {
            return;
        }
        let selected = engine.selected().map(str::to_owned);
        let idle = engine
            .live
            .values()
            .filter(|live| !live.running && live.dialogs.is_empty())
            .filter(|live| Some(&live.id) != selected.as_ref())
            .min_by_key(|live| live.rpc.touched())
            .map(|live| live.id.clone());
        if let Some(id) = idle {
            self.stop_live(engine, &id);
        }
    }

    fn load_live(&self, id: &str, rpc: &Arc<Rpc>) -> Result<(), String> {
        self.refresh(id, rpc)?;
        let levels = rpc.send("get_available_thinking_levels", json!({}));
        let commands = rpc.send("get_commands", json!({}));
        let list = |answer: Result<Value, String>, key: &str| {
            answer.map_or_else(|_| Vec::new(), |answer| as_list(&answer[key]).to_vec())
        };
        let mut engine = self.lock();
        if let Some(live) = same_live(&mut engine, id, rpc) {
            live.levels = list(levels, "levels");
            live.commands = list(commands, "commands");
        }
        engine.changed();
        Ok(())
    }

    /// Asks pi where the thread stands.
    pub(crate) fn refresh(&self, id: &str, rpc: &Arc<Rpc>) -> Result<(), String> {
        let state = rpc.send("get_state", json!({}))?;
        let mut engine = self.lock();
        if let Some(live) = same_live(&mut engine, id, rpc) {
            live.state = Some(state);
        }
        engine.changed();
        Ok(())
    }

    /// One of pi's events, from its reader thread. With the lock held, the
    /// pi that was starting has its place in the list.
    fn pi_event(self: &Arc<Self>, id: &str, cell: &OnceLock<Arc<Rpc>>, event: &Value) {
        let mut engine = self.lock();
        let Some(rpc) = cell.get() else {
            return;
        };
        let Some(live) = same_live(&mut engine, id, rpc) else {
            return;
        };
        let hooks = live.receive(event);
        for hook in hooks {
            self.apply_hook(&mut engine, id, rpc, hook);
        }
        engine.changed();
    }

    fn apply_hook(self: &Arc<Self>, engine: &mut Engine, id: &str, rpc: &Arc<Rpc>, hook: Hook) {
        match hook {
            Hook::Entry(message) => engine.push_message(id, message),
            Hook::Reload => engine.load_entries(id),
            Hook::Renamed => {
                let thread = id.to_owned();
                self.work(move |core| {
                    core.refresh_summary(&thread);
                    Ok(())
                });
            },
            Hook::Waiting(title) => {
                let name = engine.title_of(id);
                engine.emit("waiting", Some(id), &name, &title, "info");
            },
            Hook::Toast { level, title, body } => {
                engine.emit("toast", Some(id), &title, &body, &level);
            },
            Hook::TurnStarted(message) => {
                let (core, thread, rpc) = (Arc::clone(self), id.to_owned(), Arc::clone(rpc));
                if let Some(live) = engine.live.get(id) {
                    live.jobs
                        .push(move || core.turn_started(&thread, &rpc, &message));
                }
            },
            Hook::Settled { aborted } => {
                let (core, thread, rpc) = (Arc::clone(self), id.to_owned(), Arc::clone(rpc));
                if let Some(live) = engine.live.get(id) {
                    live.jobs.push(move || core.settled(&thread, &rpc, aborted));
                }
            },
        }
    }

    fn ended(&self, id: &str, cell: &OnceLock<Arc<Rpc>>, reason: &str, said: &str) {
        let mut engine = self.lock();
        let Some(rpc) = cell.get() else {
            return;
        };
        // Stopped on purpose: it is out of the list already, or all are going.
        let Some(live) = same_live(&mut engine, id, rpc).filter(|_| !self.stopping()) else {
            return;
        };
        let was_running = live.running;
        engine.live.remove(id);
        lock(&self.urgent).live.remove(id);
        let mut lines = said.lines().filter(|line| !line.starts_with("Warning:"));
        let detail = lines.next_back().unwrap_or(reason).to_owned();
        if was_running {
            engine.mark_failed(id);
            engine.emit("toast", Some(id), "pi stopped", &detail, "error");
        }
        tracing::warn!("pilot: pi for {id} ended: {reason}");
        engine.changed();
    }

    // Asking pi -------------------------------------------------------

    /// Makes a thread, returning its id. What it was asked starts it with
    /// `deliver`, which gets the id.
    pub(crate) fn create_thread(
        self: &Arc<Self>,
        options: NewThread,
        deliver: impl FnOnce(&Arc<Self>, &str) -> Result<(), String>,
    ) -> Result<String, String> {
        let (given, chats) = {
            let engine = self.lock();
            let first = engine
                .data()
                .workspaces
                .first()
                .map(|item| item.path.clone());
            let chats = engine.chats.clone();
            let given = options
                .workspace
                .clone()
                .or(first)
                .unwrap_or_else(|| chats.clone());
            (given, chats)
        };
        // pi names a session folder after the real path, links resolved.
        let path = real_path(&given)?;
        let id = Uuid::new_v4().to_string();
        let mut cwd = path.clone();
        let mut worktree = None;
        // A choice left over from a project: chats are not a repository.
        if options.worktree && path != chats {
            let words = options.title.clone().or_else(|| options.prompt.clone());
            let words = words.filter(|text| !text.is_empty());
            let words = words.unwrap_or_else(|| "thread".to_owned());
            let (made, branch) = git::add_worktree(&self.config, &self.runtime(), &path, &words)?;
            cwd.clone_from(&made);
            self.refresh_branch(made.clone());
            worktree = Some(Worktree { path: made, branch });
        }
        let prompt = options.prompt.as_deref().unwrap_or_default();
        let title = options
            .title
            .clone()
            .or_else(|| engine::first_words(prompt));
        let mut meta = engine::new_meta(
            id.clone(),
            cwd,
            path,
            title,
            options.model.clone(),
            options.thinking.clone(),
        );
        meta.worktree = worktree;
        self.lock().insert_thread(meta);
        if options.select {
            self.select(Some(id.clone()));
        }
        deliver(self, &id)?;
        Ok(id)
    }

    /// Sends a prompt, as it is or as a follow-up or steering during a run.
    pub(crate) fn prompt(
        self: &Arc<Self>,
        id: &str,
        text: &str,
        images: Vec<Value>,
        behavior: Option<&str>,
    ) -> Result<(), String> {
        let thread = self.ensure_live(id)?;
        thread.rpc.touch();
        let mut fields = json!({ "message": text });
        if !images.is_empty() {
            fields["images"] = Value::Array(images);
        }
        let cwd = {
            let mut engine = self.lock();
            let live = same_live(&mut engine, id, &thread.rpc).ok_or("That thread stopped.")?;
            if live.running {
                let kind = if behavior == Some("steer") {
                    "steer"
                } else {
                    "followUp"
                };
                fields["streamingBehavior"] = json!(kind);
                None
            } else {
                live.sending = Some(text.to_owned());
                let cwd = live.cwd.clone();
                engine.changed();
                Some(cwd)
            }
        };
        if let Some(cwd) = cwd {
            let before = git::snapshot(&self.runtime(), &cwd);
            let mut engine = self.lock();
            if let Some(live) = same_live(&mut engine, id, &thread.rpc) {
                live.next_before = before;
            }
        }
        self.changed();
        thread.rpc.send("prompt", fields).inspect_err(|_| {
            let mut engine = self.lock();
            if let Some(live) = same_live(&mut engine, id, &thread.rpc) {
                live.sending = None;
            }
        })?;
        Ok(())
    }

    // Scheduled tasks -------------------------------------------------

    fn run_due(self: &Arc<Self>, now: f64) {
        let due: Vec<String> = {
            let mut engine = self.lock();
            let mut due = Vec::new();
            for task in &mut engine.data_mut().schedules {
                if task.get("enabled") != Some(&json!(true)) {
                    continue;
                }
                let next = task
                    .get("next")
                    .and_then(Value::as_f64)
                    .filter(|next| *next != 0.0);
                let next = next.unwrap_or_else(|| {
                    let next = next_run(task, now);
                    task.insert("next".to_owned(), json!(next));
                    next
                });
                if next <= now {
                    let next = next_run(task, now);
                    task.insert("next".to_owned(), json!(next));
                    if let Some(id) = task.get("id").and_then(Value::as_str) {
                        due.push(id.to_owned());
                    }
                }
            }
            due
        };
        for id in due {
            self.work(move |core| core.run_task(&id));
        }
    }

    pub(crate) fn run_task(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let task = {
            let mut engine = self.lock();
            let schedules = &mut engine.data_mut().schedules;
            let Some(task) = schedules
                .iter_mut()
                .find(|task| task.get("id") == Some(&json!(id)))
            else {
                return Ok(());
            };
            task.insert("last_run".to_owned(), json!(now_seconds()));
            task.clone()
        };
        let text = |key: &str| {
            let value = task.get(key).and_then(Value::as_str);
            value.filter(|text| !text.is_empty()).map(str::to_owned)
        };
        let prompt = text("prompt");
        let words = prompt.clone().unwrap_or_default();
        let options = NewThread {
            workspace: text("workspace"),
            worktree: task.get("worktree") == Some(&json!(true)),
            prompt,
            title: text("name"),
            select: false,
            ..NewThread::default()
        };
        let thread = self.create_thread(options, |core, thread| {
            if words.is_empty() {
                return Ok(());
            }
            core.prompt(thread, &words, Vec::new(), None)
        })?;
        let mut engine = self.lock();
        let schedules = &mut engine.data_mut().schedules;
        if let Some(task) = schedules
            .iter_mut()
            .find(|task| task.get("id") == Some(&json!(id)))
        {
            task.insert("last_thread".to_owned(), json!(thread));
        }
        engine.store.save();
        engine.changed();
        Ok(())
    }
}

/// The live thread `id`, if it is still the one with this `rpc`.
pub(crate) fn same_live<'a>(
    engine: &'a mut Engine,
    id: &str,
    rpc: &Arc<Rpc>,
) -> Option<&'a mut Live> {
    engine
        .live
        .get_mut(id)
        .filter(|live| Arc::ptr_eq(&live.rpc, rpc))
}

/// What pi starts a thread with: its session if it has a file, or a new one
/// under the thread's id, as pi writes no file until the model first answers.
fn launch_args(engine: &Engine, meta: &Meta, id: &str) -> Vec<String> {
    let file = meta.file.as_ref().filter(|file| Path::new(file).exists());
    if let Some(file) = file {
        return vec!["--session".to_owned(), file.clone()];
    }
    let mut args = vec!["--session-id".to_owned(), id.to_owned()];
    let prefs = &engine.data().prefs;
    let model = meta.model.as_ref().or_else(|| prefs.get("last_model"));
    if let Some(model) = model.filter(|model| model.is_object()) {
        let (provider, name) = (&model["provider"], &model["id"]);
        args.push("--model".to_owned());
        args.push(format!("{}/{}", display(provider), display(name)));
    }
    let chosen = meta.thinking.clone();
    let remembered = || {
        prefs
            .get("last_thinking")
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    if let Some(thinking) = chosen.or_else(remembered) {
        args.push("--thinking".to_owned());
        args.push(thinking);
    }
    args
}
