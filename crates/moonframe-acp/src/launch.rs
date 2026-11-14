//! Starting an agent's process for a run, and keeping one warm: when the
//! dialog that starts runs opens, a mod sends `{ warm = true, agent? }`, and
//! the agent starts, shakes hands and opens a session while the user writes.
//! The next run of that agent in the same folder takes it and sends its
//! prompt at once, so starting the agent (seconds, longer through `npx`)
//! and its session (seconds more) are not in the way.
//! A warm agent nobody used is stopped after a while.

use {
    crate::{Acp, Handle, Inbound, Run, Start, agents::Agent, lock, now, run::Allow, tree},
    serde_json::Value,
    std::{
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::Arc,
        time::Duration,
    },
};

/// How long a warm agent waits for a run before it is stopped.
pub(crate) const WARM_FOR: Duration = Duration::from_secs(5 * 60);

impl Acp {
    /// Starts `program` for `run`, whose threads hand what it says to it,
    /// as the warm agent or as one of the runs.
    pub(crate) fn launch(
        &mut self,
        mut run: Run,
        program: &Path,
        args: &[String],
        warm: bool,
    ) -> Result<(), String> {
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(run.cwd())
            .env("PATH", &self.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        tree::own_group(&mut command);
        let child = command
            .spawn()
            .map_err(|error| format!("cannot start {}: {error}", run.agent.title))?;
        self.serial += 1;
        let conn = self.serial;
        run.conn = conn;
        let shared = Arc::clone(&self.state);
        let deliver = move |inbound: Inbound| {
            let mut state = lock(&shared);
            let state = &mut *state;
            let Some(run) = state
                .runs
                .iter_mut()
                .chain(state.warm.iter_mut())
                .find(|run| run.conn == conn)
            else {
                return;
            };
            let was = run.status;
            run.receive(inbound, now());
            let finished = run.status.finished() && !was.finished() && !run.is_warm();
            state.dirty = true;
            if finished {
                state.unsaved = true;
            }
        };
        tracing::info!(
            provider = "acp",
            agent = run.agent.name,
            warm,
            "starting {}",
            program.display()
        );
        // In place before its threads start, so nothing they say is lost.
        let mut state = lock(&self.state);
        let placed = if warm {
            state.warm.insert(run)
        } else {
            state.runs.push(run);
            state
                .runs
                .last_mut()
                .ok_or("the run went missing as it started")?
        };
        if let Err(error) = placed.attach(child, deliver) {
            if warm {
                state.warm = None;
            } else {
                state.runs.pop();
            }
            return Err(error);
        }
        Ok(())
    }

    /// The warm agent, for a run of `agent` in `cwd`, if it fits.
    pub(crate) fn take_warm(&self, agent: &Agent, cwd: &Path) -> Option<Run> {
        lock(&self.state).warm.take_if(|run| {
            run.agent.name == agent.name && run.cwd() == cwd && !run.status.finished()
        })
    }

    /// Starts the agent `message` names (else the default) ahead of a run
    /// of `from`'s, in the folder runs work in, unless it already is.
    pub(crate) fn warm(&mut self, host: &Handle, from: &str, message: &Value) {
        if !self.starters.allows(from) {
            return;
        }
        let name = message
            .get("agent")
            .and_then(Value::as_str)
            .unwrap_or(&self.default_agent);
        let Some(agent) = crate::agents::find(name) else {
            return;
        };
        let cwd = self.cwd.clone();
        {
            let mut state = lock(&self.state);
            let fits = state.warm.as_ref().is_some_and(|run| {
                run.agent.name == agent.name && *run.cwd() == cwd && !run.status.finished()
            });
            if fits {
                return;
            }
            if let Some(mut stale) = state.warm.take() {
                stale.stop(now());
            }
        }
        let (program, args) = match self.locate(agent) {
            crate::agents::Found::Installed(program, args)
            | crate::agents::Found::Fetched(program, args) => (program, args),
            crate::agents::Found::Missing => return,
        };
        let run = Run::new(waiting(agent, cwd), Some(host.clone()));
        if let Err(error) = self.launch(run, &program, &args, true) {
            tracing::warn!(provider = "acp", "cannot warm {}: {error}", agent.title);
        }
    }

    /// Stops a warm agent that died or waited too long.
    pub(crate) fn expire_warm(&self) {
        let mut state = lock(&self.state);
        let now = now();
        let stale = state.warm.as_ref().is_some_and(|run| {
            run.status.finished() || now.saturating_sub(run.started()) > WARM_FOR.as_secs()
        });
        if stale && let Some(mut run) = state.warm.take() {
            run.stop(now);
        }
    }
}

/// A run with no prompt yet, for a warm agent.
fn waiting(agent: &'static Agent, cwd: PathBuf) -> Start {
    Start {
        id: String::new(),
        owner: String::new(),
        request: Value::Null,
        agent,
        text: String::new(),
        thread: None,
        prompt: String::new(),
        allow: Allow::Ask,
        scope: crate::Scope::default(),
        cwd,
        installed: None,
        now: now(),
    }
}
