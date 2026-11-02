//! The `chat` native provider: conversations with coding agents, each run
//! by herdr-gpui's `herdr-acp` program with no view of its own, attached
//! over herdr's chat protocol, and shared as `app.chat` for mods to draw.
//!
//! A chat is one agent session that takes prompt after prompt. Its program
//! runs in a process group of its own, so it outlives the app: the next
//! launch attaches to it again and gets only what it missed. A chat whose
//! program ended keeps its agent session's id, so it can be resumed.
//!
//! Messages (only from mods in provider.toml's `[settings] starters`; a chat
//! obeys only the mod that started it). Each may carry an `id`, which its
//! answer `{ reply = id, action, ok, error? }` carries back:
//! - `{ start = "<agent>", cwd?, load? }` starts a chat in `cwd` (else the
//!   app's repository), continuing the agent's session `load` if given;
//!   answered with `chat`.
//! - `{ prompt = "<text>", chat, files? }` sends a prompt, or queues it
//!   behind the running turn; `files` are absolute paths sent with it.
//! - `{ cancel = chat }`, `{ answer = chat, permission, option }`,
//!   `{ mode = "<id>", chat }`, `{ setting = "<id>", value, chat }` (a
//!   choice's id, or a boolean), `{ unqueue = "<queued id>", chat }`,
//!   `{ auto_accept = bool, chat }`: herdr's commands of the same names.
//! - `{ resume = chat }` starts an ended chat again on its agent session;
//!   `{ close = chat }` ends its program and forgets it.
//! - `{ focus = chat }` (anyone): the chat a view shows, whose transcript
//!   is shared whole; the others share their last few entries.
//! - Anything else looks for agents again and publishes.
//!
//! `app.chat` is `{ available, reason?, program, agents, chats }`; a chat is
//! `{ id, owner, agent, cwd, created, status, error?, session?, transcript? }`
//! with `status` `"starting" "live" "reconnecting" "ended" "failed"` and
//! `transcript` herdr's (see `project`).

mod program;
mod project;

#[cfg(unix)]
mod link;

use {
    herdr_chat_protocol::{
        Attachment, Command, CommandId, Cursor, EndpointId, OptionValue, PermissionId, Transcript,
    },
    moonframe_core::Data,
    moonframe_providers::{Handle, Mods, Native, owner, to_json},
    serde_json::{Value, json},
    std::{
        collections::HashMap,
        ffi::OsString,
        path::{Path, PathBuf},
        sync::{Arc, Mutex, MutexGuard, PoisonError},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    },
};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

/// Chats kept, ended ones dropped oldest first.
const MAX_CHATS: usize = 24;
/// The longest prompt a mod may send.
const MAX_PROMPT: usize = 256 * 1024;

/// Makes the provider, for the app's registry.
pub fn native() -> Box<dyn Native> {
    Box::new(Chats::default())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Starting,
    Live,
    Reconnecting,
    Ended(String),
    Failed(String),
}

pub(crate) struct Chat {
    pub(crate) id: String,
    pub(crate) owner: String,
    pub(crate) agent: String,
    pub(crate) cwd: String,
    pub(crate) created: u64,
    pub(crate) pid: Option<u32>,
    pub(crate) endpoint: Option<EndpointId>,
    pub(crate) session: Option<String>,
    pub(crate) phase: Phase,
    pub(crate) transcript: Option<Transcript>,
    pub(crate) cursor: Option<Cursor>,
    #[cfg(unix)]
    pub(crate) writer: Option<UnixStream>,
    pub(crate) closing: bool,
}

/// A command sent and not yet acknowledged: whom to answer.
pub(crate) struct Pending {
    pub(crate) owner: String,
    pub(crate) reply: Value,
    pub(crate) action: &'static str,
}

#[derive(Default)]
pub(crate) struct State {
    pub(crate) chats: Vec<Chat>,
    pub(crate) pending: HashMap<CommandId, Pending>,
    pub(crate) agents: Vec<String>,
    pub(crate) focus: Option<String>,
    /// Something changed since the last publish.
    pub(crate) dirty: bool,
    /// The chats' records changed since they were last saved.
    pub(crate) unsaved: bool,
}

impl State {
    pub(crate) fn chat(&self, id: &str) -> Option<&Chat> {
        self.chats.iter().find(|chat| chat.id == id)
    }

    pub(crate) fn chat_mut(&mut self, id: &str) -> Option<&mut Chat> {
        self.chats.iter_mut().find(|chat| chat.id == id)
    }
}

pub(crate) type Shared = Arc<Mutex<State>>;

pub(crate) fn lock(shared: &Mutex<State>) -> MutexGuard<'_, State> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn send(host: &Handle, to: &str, answer: &Value) {
    if let Ok(data) = moonframe_providers::from_json(answer) {
        host.send(to, data);
    }
}

#[derive(Default)]
pub struct Chats {
    state: Shared,
    starters: Mods,
    program: Option<PathBuf>,
    /// The folders agents are looked for in.
    path: OsString,
    /// Where a chat without `cwd` starts.
    cwd: PathBuf,
    /// Where the chats' records are saved, and their programs' logs go.
    store: Option<PathBuf>,
    default_agent: String,
    serial: u64,
}

impl Native for Chats {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let settings = to_json(host.settings());
        self.starters = Mods::from_settings(host.settings(), "starters");
        let home = std::env::var_os("HOME").map(PathBuf::from);
        self.path = moonframe_providers::program::search_path(
            std::env::var_os("PATH").as_deref(),
            home.as_deref(),
        );
        let setting = settings
            .get("program")
            .and_then(Value::as_str)
            .map(|path| expand(path, home.as_deref()));
        self.program = program::find(setting.as_deref(), &self.path, home.as_deref());
        self.cwd = match settings.get("cwd").and_then(Value::as_str) {
            Some(cwd) => expand(cwd, home.as_deref()),
            None => repository(host.dir()).unwrap_or_else(|| home.clone().unwrap_or_default()),
        };
        self.default_agent = settings
            .get("agent")
            .and_then(Value::as_str)
            .unwrap_or("claude")
            .to_owned();
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|home| home.join(".local/state")));
        let profile = moonframe_providers::profile().unwrap_or("default");
        self.store = match settings.get("store").and_then(Value::as_str) {
            Some(store) => Some(PathBuf::from(store)),
            None => state.map(|state| state.join("moonframe/chat").join(format!("{profile}.json"))),
        };
        self.restore(host);
        self.look_for_agents();
        lock(&self.state).dirty = true;
        self.publish(host);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        self.publish(host);
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        let message = to_json(&message);
        let reply = message.get("id").cloned();
        let result = self.handle(host, from, &message);
        let answer = result.and_then(|(action, result)| match result {
            Ok(Some(extra)) => Some((action, extra)),
            // Answered when the program acknowledges it.
            Ok(None) => None,
            Err(error) => Some((action, json!({ "ok": false, "error": error }))),
        });
        if let Some((action, mut answer)) = answer {
            answer["action"] = json!(action);
            answer["reply"] = reply.unwrap_or_else(|| json!(action));
            if answer.get("ok").is_none() {
                answer["ok"] = json!(true);
            }
            send(host, owner(from), &answer);
        }
        lock(&self.state).dirty = true;
        self.publish(host);
    }
}

type Handled = Option<(&'static str, Result<Option<Value>, String>)>;

impl Chats {
    fn handle(&mut self, host: &Handle, from: &str, message: &Value) -> Handled {
        let text = |key: &str| message.get(key).and_then(Value::as_str);
        if let Some(chat) = text("focus") {
            lock(&self.state).focus = Some(chat.to_owned());
            return None;
        }
        let action = [
            "start",
            "prompt",
            "cancel",
            "answer",
            "mode",
            "setting",
            "unqueue",
            "auto_accept",
            "resume",
            "close",
        ]
        .into_iter()
        .find(|key| message.get(*key).is_some());
        let Some(action) = action else {
            self.look_for_agents();
            return None;
        };
        if !self.starters.allows(from) {
            let why = format!(
                "`{from}` may not drive chats: only the mods in providers/chat's `starters` may"
            );
            return Some((action, Err(why)));
        }
        let owner = owner(from);
        let result = match action {
            "start" => self
                .start_chat(host, owner, message)
                .map(|chat| Some(json!({ "chat": chat }))),
            "resume" => self.resume(host, owner, text("resume").unwrap_or_default()),
            "close" => self.close(owner, text("close").unwrap_or_default()),
            _ => {
                let chat = match action {
                    "cancel" | "answer" => text(action),
                    _ => text("chat"),
                };
                match command(action, message) {
                    Ok(command) => {
                        let reply = message.get("id").cloned().unwrap_or_else(|| json!(action));
                        self.command(owner, chat.unwrap_or_default(), command, reply, action)
                    },
                    Err(error) => Err(error),
                }
            },
        };
        Some((action, result))
    }

    fn start_chat(
        &mut self,
        host: &Handle,
        owner: &str,
        message: &Value,
    ) -> Result<String, String> {
        let agent = message
            .get("start")
            .and_then(Value::as_str)
            .filter(|agent| !agent.is_empty())
            .unwrap_or(&self.default_agent)
            .to_owned();
        if !agent
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!("`{agent}` is not an agent's name"));
        }
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let cwd = match message.get("cwd").and_then(Value::as_str) {
            Some(cwd) => expand(cwd, home.as_deref()),
            None => self.cwd.clone(),
        };
        if !cwd.is_absolute() || !cwd.is_dir() {
            return Err(format!("{} is not a folder", cwd.display()));
        }
        let load = message
            .get("load")
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.serial += 1;
        let id = format!("{}-{}", now_ms(), self.serial);
        {
            let mut state = lock(&self.state);
            state.chats.insert(0, Chat {
                id: id.clone(),
                owner: owner.to_owned(),
                agent: agent.clone(),
                cwd: cwd.display().to_string(),
                created: now_ms(),
                pid: None,
                endpoint: None,
                session: load.clone(),
                phase: Phase::Starting,
                transcript: None,
                cursor: None,
                #[cfg(unix)]
                writer: None,
                closing: false,
            });
            trim(&mut state);
            state.focus = Some(id.clone());
            state.unsaved = true;
        }
        self.launch(host, &id, &agent, &cwd, load);
        Ok(id)
    }

    /// Starts the chat's program on a thread, then attaches to it.
    fn launch(&self, host: &Handle, id: &str, agent: &str, cwd: &Path, load: Option<String>) {
        let Some(program) = self.program.clone() else {
            let mut state = lock(&self.state);
            if let Some(chat) = state.chat_mut(id) {
                chat.phase = Phase::Failed("herdr-acp is not installed".to_owned());
            }
            return;
        };
        let shared = Arc::clone(&self.state);
        let host = host.clone();
        let (id, agent, cwd, path) = (
            id.to_owned(),
            agent.to_owned(),
            cwd.to_owned(),
            self.path.clone(),
        );
        let log = self.store.as_ref().and_then(|store| {
            let dir = store.parent()?.join("logs");
            std::fs::create_dir_all(&dir).ok()?;
            Some(dir.join(format!("{id}.log")))
        });
        let spawned = thread::Builder::new()
            .name("chat-start".to_owned())
            .spawn(move || {
                let started = program::spawn(&program::Spawn {
                    program: &program,
                    agent: &agent,
                    cwd: &cwd,
                    load: load.as_deref(),
                    path: &path,
                    log: log.as_deref(),
                });
                let mut state = lock(&shared);
                let Some(chat) = state.chat_mut(&id) else {
                    if let Ok((pid, _)) = started {
                        program::end(pid);
                    }
                    return;
                };
                match started {
                    Ok((pid, endpoint)) => {
                        chat.pid = Some(pid);
                        chat.endpoint = Some(endpoint.clone());
                        chat.phase = Phase::Reconnecting;
                        state.unsaved = true;
                        state.dirty = true;
                        drop(state);
                        attach(shared, host, id, endpoint);
                    },
                    Err(error) => {
                        chat.phase = Phase::Failed(error);
                        state.dirty = true;
                    },
                }
            });
        if let Err(error) = spawned {
            tracing::warn!("chat: cannot start a chat: {error}");
        }
    }

    fn resume(&mut self, host: &Handle, owner: &str, id: &str) -> Result<Option<Value>, String> {
        let (agent, cwd, session) = {
            let mut state = lock(&self.state);
            let chat = state
                .chat_mut(id)
                .filter(|chat| chat.owner == owner)
                .ok_or_else(|| "no such chat".to_owned())?;
            if !matches!(chat.phase, Phase::Ended(_) | Phase::Failed(_)) {
                return Err("the chat is still running".to_owned());
            }
            chat.phase = Phase::Starting;
            chat.cursor = None;
            (
                chat.agent.clone(),
                PathBuf::from(&chat.cwd),
                chat.session.clone(),
            )
        };
        self.launch(host, id, &agent, &cwd, session);
        Ok(Some(json!({ "chat": id })))
    }

    fn close(&mut self, owner: &str, id: &str) -> Result<Option<Value>, String> {
        let mut state = lock(&self.state);
        let at = state
            .chats
            .iter()
            .position(|chat| chat.id == id && chat.owner == owner)
            .ok_or_else(|| "no such chat".to_owned())?;
        let mut chat = state.chats.remove(at);
        chat.closing = true;
        end_chat(&mut chat);
        state.unsaved = true;
        Ok(Some(json!({ "chat": id })))
    }

    #[cfg(unix)]
    fn command(
        &self,
        owner: &str,
        id: &str,
        command: Command,
        reply: Value,
        action: &'static str,
    ) -> Result<Option<Value>, String> {
        let mut state = lock(&self.state);
        let chat = state
            .chat_mut(id)
            .filter(|chat| chat.owner == owner)
            .ok_or_else(|| "no such chat".to_owned())?;
        let writer = chat
            .writer
            .as_mut()
            .ok_or_else(|| "the chat is not attached yet".to_owned())?;
        let command_id = CommandId::random();
        if !link::command(writer, command_id, command) {
            return Err("the chat's program did not take it".to_owned());
        }
        let owner = owner.to_owned();
        state.pending.insert(command_id, Pending {
            owner,
            reply,
            action,
        });
        // Answered when the program acknowledges it.
        Ok(None)
    }

    #[cfg(not(unix))]
    fn command(
        &self,
        _owner: &str,
        _id: &str,
        _command: Command,
        _reply: Value,
        _action: &'static str,
    ) -> Result<Option<Value>, String> {
        Err("chats need a Unix system for now".to_owned())
    }

    fn look_for_agents(&self) {
        let Some(program) = self.program.clone() else {
            return;
        };
        let shared = Arc::clone(&self.state);
        let path = self.path.clone();
        let _ = thread::Builder::new()
            .name("chat-agents".to_owned())
            .spawn(move || {
                let agents = program::agents(&program, &path);
                let mut state = lock(&shared);
                state.agents = agents;
                state.dirty = true;
            });
    }

    fn publish(&self, host: &Handle) {
        let mut state = lock(&self.state);
        if state.unsaved {
            state.unsaved = false;
            self.save(&state);
        }
        if !std::mem::take(&mut state.dirty) {
            return;
        }
        let focus = state.focus.clone();
        let chats: Vec<Value> = state
            .chats
            .iter()
            .map(|chat| {
                let entries = if focus.as_deref() == Some(&chat.id) {
                    project::FULL_ENTRIES
                } else {
                    project::SHORT_ENTRIES
                };
                let (status, error) = match &chat.phase {
                    Phase::Starting => ("starting", None),
                    Phase::Live => ("live", None),
                    Phase::Reconnecting => ("reconnecting", None),
                    Phase::Ended(why) => ("ended", Some(why.clone())),
                    Phase::Failed(why) => ("failed", Some(why.clone())),
                };
                json!({
                    "id": chat.id,
                    "owner": chat.owner,
                    "agent": chat.agent,
                    "cwd": chat.cwd,
                    "created": chat.created,
                    "status": status,
                    "error": error,
                    "session": chat.session,
                    "transcript": chat
                        .transcript
                        .as_ref()
                        .map(|transcript| project::transcript(transcript, entries)),
                })
            })
            .collect();
        let program = self.program.as_ref().map(|path| path.display().to_string());
        let mut published = json!({
            "available": program.is_some(),
            "program": program,
            "agents": state.agents,
            "default": self.default_agent,
            "cwd": self.cwd.display().to_string(),
            "chats": chats,
        });
        if self.program.is_none() {
            published["reason"] =
                json!("herdr-acp was not found: install Herdr, or set `program` in providers/chat");
        }
        drop(state);
        host.publish_json(&published);
    }

    fn save(&self, state: &State) {
        let Some(store) = &self.store else {
            return;
        };
        let records: Vec<Value> = state
            .chats
            .iter()
            .map(|chat| {
                json!({
                    "id": chat.id, "owner": chat.owner, "agent": chat.agent, "cwd": chat.cwd,
                    "created": chat.created, "pid": chat.pid,
                    "endpoint": chat.endpoint.as_ref().map(EndpointId::as_str),
                    "session": chat.session,
                    "ended": matches!(chat.phase, Phase::Ended(_) | Phase::Failed(_)),
                })
            })
            .collect();
        if let Some(dir) = store.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let partial = store.with_extension("json.partial");
        if std::fs::write(&partial, json!({ "chats": records }).to_string()).is_ok() {
            let _ = std::fs::rename(&partial, store);
        }
    }

    /// The chats saved at the last launch: attached again where their
    /// program still runs, ended (and resumable) where it does not.
    fn restore(&mut self, host: &Handle) {
        let Some(saved) = self
            .store
            .as_ref()
            .and_then(|store| std::fs::read_to_string(store).ok())
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        else {
            return;
        };
        let records = saved
            .get("chats")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut attach_to = Vec::new();
        let mut state = lock(&self.state);
        for record in records {
            let text = |key: &str| record.get(key).and_then(Value::as_str).map(str::to_owned);
            let (Some(id), Some(owner), Some(agent), Some(cwd)) =
                (text("id"), text("owner"), text("agent"), text("cwd"))
            else {
                continue;
            };
            let endpoint =
                text("endpoint").and_then(|endpoint| endpoint.parse::<EndpointId>().ok());
            let running = endpoint.as_ref().is_some_and(program::alive)
                && record.get("ended") != Some(&Value::Bool(true));
            if running && let Some(endpoint) = &endpoint {
                attach_to.push((id.clone(), endpoint.clone()));
            }
            state.chats.push(Chat {
                id,
                owner,
                agent,
                cwd,
                created: record
                    .get("created")
                    .and_then(Value::as_u64)
                    .unwrap_or_default(),
                pid: record
                    .get("pid")
                    .and_then(Value::as_u64)
                    .and_then(|pid| u32::try_from(pid).ok()),
                endpoint,
                session: text("session"),
                phase: if running {
                    Phase::Reconnecting
                } else {
                    Phase::Ended("the chat ended while the app was closed".to_owned())
                },
                transcript: None,
                cursor: None,
                #[cfg(unix)]
                writer: None,
                closing: false,
            });
        }
        drop(state);
        for (id, endpoint) in attach_to {
            attach(Arc::clone(&self.state), host.clone(), id, endpoint);
        }
    }
}

#[cfg(unix)]
fn attach(shared: Shared, host: Handle, id: String, endpoint: EndpointId) {
    link::attach(shared, host, id, endpoint);
}

#[cfg(not(unix))]
fn attach(shared: Shared, _host: Handle, id: String, _endpoint: EndpointId) {
    let mut state = lock(&shared);
    if let Some(chat) = state.chat_mut(&id) {
        chat.phase = Phase::Failed("chats need a Unix system for now".to_owned());
    }
}

/// Ends a chat's program, when it is still the one the chat started.
fn end_chat(chat: &mut Chat) {
    #[cfg(unix)]
    if let Some(writer) = chat.writer.take() {
        let _ = writer.shutdown(std::net::Shutdown::Both);
    }
    let alive = chat.endpoint.as_ref().is_some_and(program::alive);
    if alive && let Some(pid) = chat.pid {
        program::end(pid);
    }
}

/// Drops the oldest ended chats past [`MAX_CHATS`].
fn trim(state: &mut State) {
    while state.chats.len() > MAX_CHATS {
        let Some(at) = state
            .chats
            .iter()
            .rposition(|chat| matches!(chat.phase, Phase::Ended(_) | Phase::Failed(_)))
        else {
            break;
        };
        state.chats.remove(at);
    }
}

/// The herdr command a message asks for.
fn command(action: &str, message: &Value) -> Result<Command, String> {
    let text = |key: &str| message.get(key).and_then(Value::as_str);
    Ok(match action {
        "prompt" => {
            let text = text("prompt").unwrap_or_default();
            if text.trim().is_empty() {
                return Err("nothing to send".to_owned());
            }
            if text.len() > MAX_PROMPT {
                return Err(format!("a prompt is at most {MAX_PROMPT} bytes"));
            }
            let attachments = message
                .get("files")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|path| Attachment {
                    path: path.to_owned(),
                })
                .collect();
            Command::Prompt {
                text: text.to_owned(),
                attachments,
            }
        },
        "cancel" => Command::Cancel,
        "answer" => Command::Answer {
            option: text("option").ok_or("which option?")?.to_owned(),
            permission: PermissionId(
                message
                    .get("permission")
                    .and_then(Value::as_u64)
                    .ok_or("which permission?")?,
            ),
        },
        "mode" => Command::SetMode {
            id: text("mode").unwrap_or_default().to_owned(),
        },
        "setting" => Command::SetOption {
            id: text("setting").unwrap_or_default().to_owned(),
            value: match message.get("value") {
                Some(Value::Bool(value)) => OptionValue::Toggle { value: *value },
                Some(Value::String(id)) => OptionValue::Choice { id: id.clone() },
                _ => return Err("a setting's value is a choice's id or a boolean".to_owned()),
            },
        },
        "unqueue" => Command::Unqueue {
            id: serde_json::from_value(json!(text("unqueue").unwrap_or_default()))
                .map_err(|_| "no such queued prompt".to_owned())?,
        },
        "auto_accept" => Command::SetAutoAccept {
            on: message
                .get("auto_accept")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        _ => return Err(format!("unknown action `{action}`")),
    })
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

fn expand(path: &str, home: Option<&Path>) -> PathBuf {
    match (path.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ if path == "~" => home.map(Path::to_path_buf).unwrap_or_default(),
        _ => PathBuf::from(path),
    }
}

/// The git repository holding the app the provider folder `dir` is in.
fn repository(dir: &Path) -> Option<PathBuf> {
    let start = dir.join("..").join("..");
    let start = std::path::absolute(&start).unwrap_or(start);
    let start = start.canonicalize().unwrap_or(start);
    start
        .ancestors()
        .find(|folder| folder.join(".git").exists())
        .map(Path::to_path_buf)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
