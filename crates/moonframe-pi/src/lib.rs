//! Pilot's two native providers: [`pi`], threads of the
//! [pi](https://github.com/earendil-works/pi) coding agent, and [`repo`], the
//! checkout of the thread on screen. Both run programs with the user's
//! access; `pi` also reads and writes pi's settings and credentials under
//! its agent folder. Neither needs anything installed but pi and git.
//!
//! Each thread is a `pi --mode rpc` process over the session files pi keeps
//! itself, so what happens here carries over to the pi CLI and back. What
//! the app adds to pi (projects, pins, schedules, each run's before and
//! after) is in `state.json` under `PILOT_DATA`, or
//! `~/.local/share/moonframe/pilot`.
//!
//! See `snapshot.rs` for what `app.pi` holds, `commands.rs` for the messages
//! it takes, `core.rs` for how it runs without blocking on pi, and `repo.rs`
//! for `app.repo`.

// keep-sorted start
mod commands;
mod config;
mod core;
mod engine;
mod flows;
mod git;
mod limits;
mod live;
mod repo;
mod resources;
mod rpc;
mod run;
mod runtime;
mod schedule;
mod sessions;
mod snapshot;
mod store;
mod timeline;
mod turns;
mod value;
// keep-sorted end

pub use config::Config;
use {
    crate::core::Core,
    moonframe_core::Data,
    moonframe_providers::{Handle, Native, to_json},
    serde_json::Value,
    std::{
        sync::{
            Arc,
            mpsc::{self, Sender},
        },
        thread,
    },
};

/// The `pi` provider, configured from the environment.
pub fn pi() -> Box<dyn Native> {
    pi_with(Config::from_env())
}

/// The `pi` provider, with `config`: how tests keep to folders of their own.
pub fn pi_with(config: Config) -> Box<dyn Native> {
    Box::new(Provider {
        config: Some(config),
        running: None,
    })
}

/// The `repo` provider.
pub fn repo() -> Box<dyn Native> {
    repo::native()
}

struct Running {
    core: Arc<Core>,
    inbox: Sender<Value>,
}

struct Provider {
    config: Option<Config>,
    running: Option<Running>,
}

impl Native for Provider {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let config = self.config.take().unwrap_or_else(Config::from_env);
        let core = Core::new(config)?;
        let (inbox, messages) = mpsc::channel();
        let looping = Arc::clone(&core);
        thread::spawn(move || core::dispatch(&looping, &messages));
        let publishing = Arc::clone(&core);
        let host = host.clone();
        thread::spawn(move || core::publish(&publishing, &host));
        self.running = Some(Running { core, inbox });
        Ok(())
    }

    /// Everything here keeps its own time, on its own threads.
    fn tick(&mut self, _host: &Handle) {}

    fn receive(&mut self, _host: &Handle, _from: &str, message: Data) {
        let Some(running) = &self.running else {
            return;
        };
        let message = to_json(&message);
        if message.get("abort").is_some() {
            let thread = message["thread"].as_str().map(str::to_owned);
            running.core.abort(thread);
        } else {
            let _ = running.inbox.send(message);
        }
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            drop(running.inbox);
            running.core.shutdown();
        }
    }
}
