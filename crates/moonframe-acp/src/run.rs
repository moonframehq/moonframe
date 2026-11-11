//! One run: an agent process, its ACP session, and what it has said.
//!
//! JSON-RPC 2.0 goes over the agent's standard input and output, one
//! message per line, as in herdr-acp: a reader thread hands each line to
//! the run under the provider's lock, a writer thread writes queued lines
//! in order, so a run never blocks on an agent that stops reading.

pub(crate) use crate::allow::Allow;
use {
    crate::allow::{Call, Scope},
    acp::v1 as wire,
    moonframe_providers::{Handle, Installed},
    serde_json::{Value, json},
    std::{
        collections::{BTreeMap, VecDeque},
        io::{BufRead, BufReader, Read, Write},
        path::PathBuf,
        process::Child,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{Sender, channel},
        },
        thread,
        time::{Duration, Instant},
    },
    transcript::Transcript,
};

mod answers;
mod json;
mod timing;
mod transcript;

use {
    answers::{agent_info, cancelled, enum_name, fields_paths, model_name, selected},
    timing::Timing,
};

/// The longest line read from an agent.
const MAX_LINE: usize = 16 << 20;
/// What is kept of the agent's latest message, in characters.
const MAX_MESSAGE: usize = 16_000;
/// Tool calls kept per run, newest last: the transcript shows them.
const MAX_TOOLS: usize = 200;
/// Lines of the agent's stderr kept, to explain a failure.
const MAX_LOG: usize = 20;
/// How long a finished agent has to exit before it is killed.
const GRACE: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Status {
    /// Starting the agent, its handshake and its session.
    Starting,
    Running,
    /// Waiting on the user to answer a permission question.
    Waiting,
    Done,
    Failed,
    Cancelled,
    /// The app quit while it ran.
    Stopped,
}

impl Status {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Stopped => "stopped",
        }
    }

    pub(crate) fn parse(name: &str) -> Self {
        match name {
            "done" => Self::Done,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Stopped,
        }
    }

    pub(crate) fn finished(self) -> bool {
        !matches!(self, Self::Starting | Self::Running | Self::Waiting)
    }
}

#[derive(Debug)]
pub(crate) struct Tool {
    id: String,
    title: String,
    kind: String,
    status: String,
    /// The files it said it works on, for the permission question it asks.
    paths: Vec<PathBuf>,
    /// The shell command it runs, if it said.
    command: Option<String>,
    /// Other files its input names, or unknown if it may name others.
    targets: Option<Vec<PathBuf>>,
    /// When it started and ended, in milliseconds from the run's start.
    began: Option<u64>,
    ended: Option<u64>,
    /// How long it took, for one restored from a saved run.
    ms: Option<u64>,
}

impl Tool {
    /// How long it took, once it ended.
    pub(crate) fn duration(&self) -> Option<u64> {
        self.ms
            .or_else(|| Some(self.ended?.saturating_sub(self.began?)))
    }
}

/// A permission question the agent asked, waiting on the user.
#[derive(Debug)]
pub(crate) struct Question {
    rpc: Value,
    title: String,
    options: Vec<(String, String, String)>,
}

#[derive(Debug)]
enum Pending {
    Initialize,
    NewSession,
    Prompt,
}

/// A line read from the agent, or the end of its output.
pub(crate) enum Inbound {
    Line(Vec<u8>),
    Log(String),
    Closed(Option<String>),
}

pub(crate) struct Run {
    pub(crate) id: String,
    /// Which agent process it talks to, for what that process says to find
    /// it: a warm agent's run gets its id only when a mod starts it.
    pub(crate) conn: u64,
    /// The agent opened its session before any prompt came: a warm agent,
    /// waiting for a run.
    ready: bool,
    /// The mod that started it: the only one that may stop it, answer it
    /// or forget it.
    pub(crate) owner: String,
    /// The mod's own id for it, echoed in replies.
    request: Value,
    pub(crate) agent: &'static crate::agents::Agent,
    /// The user's words, shown back.
    text: String,
    /// The run this one follows up, the first of a conversation: a mod
    /// shows them together.
    thread: Option<String>,
    /// What the agent is sent: the mod's instructions, the words, context.
    prompt: String,
    pub(crate) status: Status,
    started: u64,
    finished: Option<u64>,
    error: Option<String>,
    stop_reason: Option<String>,
    /// The agent's latest message: its answer, once it ends.
    message: String,
    /// A tool ran since the message last grew: the next chunk starts anew.
    fresh: bool,
    tools: VecDeque<Tool>,
    tool_count: usize,
    /// Everything the agent said and the tools it called, in order.
    transcript: Transcript,
    /// Who the agent says it is (`initialize`'s `agentInfo`).
    agent_info: Option<Value>,
    /// The model its session says it uses, by name.
    model: Option<String>,
    plan: Vec<(String, String)>,
    question: Option<Question>,
    /// Which permission questions are answered without asking.
    allow: Allow,
    cwd: PathBuf,
    /// The app as installed, which the agent may not write in.
    installed: Option<Installed>,
    /// What [`Allow::App`] lets it touch.
    scope: Scope,
    /// How long each part of it took.
    pub(crate) timing: Timing,
    /// What the user should know beyond the agent's answer.
    warning: Option<String>,
    /// Finished, and the bundle's signature was checked after it.
    checked: bool,
    /// No agent of this run is left to write: it has none, or it exited.
    /// A finished run's agent gets a moment to exit, so this comes after.
    exited: Arc<AtomicBool>,
    /// Finished, and what it changed was taken note of.
    kept: bool,
    /// It left the app's folders different from how it found them.
    changed: bool,
    /// The app was taken back to before it.
    undone: bool,
    /// The app is being checked after it.
    pub(crate) checking: bool,
    session: Option<String>,
    next: i64,
    pending: BTreeMap<i64, Pending>,
    outbox: Option<Sender<Vec<u8>>>,
    child: Option<Child>,
    /// The agent's process id, which leads its process group.
    pid: Option<u32>,
    log: VecDeque<String>,
    host: Option<Handle>,
}

/// What a mod asked to start.
pub(crate) struct Start {
    pub(crate) id: String,
    pub(crate) owner: String,
    pub(crate) request: Value,
    pub(crate) agent: &'static crate::agents::Agent,
    pub(crate) text: String,
    pub(crate) thread: Option<String>,
    pub(crate) prompt: String,
    pub(crate) allow: Allow,
    pub(crate) cwd: PathBuf,
    pub(crate) installed: Option<Installed>,
    pub(crate) scope: Scope,
    pub(crate) now: u64,
}

impl Run {
    pub(crate) fn new(start: Start, host: Option<Handle>) -> Self {
        Self {
            id: start.id,
            conn: 0,
            ready: false,
            owner: start.owner,
            request: start.request,
            agent: start.agent,
            text: start.text,
            thread: start.thread,
            prompt: start.prompt,
            status: Status::Starting,
            started: start.now,
            finished: None,
            error: None,
            stop_reason: None,
            message: String::new(),
            fresh: false,
            tools: VecDeque::new(),
            tool_count: 0,
            transcript: Transcript::default(),
            agent_info: None,
            model: None,
            plan: Vec::new(),
            question: None,
            allow: start.allow,
            cwd: start.cwd,
            installed: start.installed,
            scope: start.scope,
            timing: Timing::default(),
            warning: None,
            checked: false,
            exited: Arc::new(AtomicBool::new(true)),
            kept: false,
            changed: false,
            undone: false,
            checking: false,
            session: None,
            next: 0,
            pending: BTreeMap::new(),
            outbox: None,
            child: None,
            pid: None,
            log: VecDeque::new(),
            host,
        }
    }

    /// Makes a warm agent's run the one a mod started: it takes `start`'s
    /// place, keeping the agent and what it said as it started.
    pub(crate) fn adopt(&mut self, start: Start) {
        self.id = start.id;
        self.owner = start.owner;
        self.request = start.request;
        self.text = start.text;
        self.thread = start.thread;
        self.prompt = start.prompt;
        self.allow = start.allow;
        self.scope = start.scope;
        self.installed = start.installed;
        self.started = start.now;
        self.timing = Timing::warm(self.ready);
        if std::mem::take(&mut self.ready) {
            self.send_prompt();
        }
    }

    /// A warm agent: started ahead of a run that has not come yet.
    pub(crate) fn is_warm(&self) -> bool {
        self.prompt.is_empty()
    }

    pub(crate) fn cwd(&self) -> &std::path::Path {
        &self.cwd
    }

    /// Sends the run's prompt in its open session.
    fn send_prompt(&mut self) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let prompt = json!({
            "sessionId": session,
            "prompt": [{ "type": "text", "text": self.prompt }],
        });
        self.status = Status::Running;
        self.request_rpc("session/prompt", &prompt, Pending::Prompt);
    }

    fn open_session(&mut self) {
        self.request_rpc(
            "session/new",
            &json!({ "cwd": self.cwd, "mcpServers": [] }),
            Pending::NewSession,
        );
    }

    pub(crate) fn started(&self) -> u64 {
        self.started
    }

    /// The run's id, once, when it finished with the signature unchecked.
    pub(crate) fn take_unchecked(&mut self) -> Option<String> {
        if !self.status.finished() || self.checked {
            return None;
        }
        self.checked = true;
        Some(self.id.clone())
    }

    /// The run's id, once, when it finished with its changes not noted and
    /// its agent can write no more: only then is what it left final, and
    /// `changed`, which allows an undo, comes from this.
    pub(crate) fn take_unkept(&mut self) -> Option<String> {
        let stopped = self.status.finished() && self.exited.load(Ordering::Acquire);
        if !stopped || self.kept {
            return None;
        }
        self.kept = true;
        Some(self.id.clone())
    }

    pub(crate) fn set_changed(&mut self, changed: bool) {
        self.changed = changed;
    }

    pub(crate) fn set_undone(&mut self) {
        self.undone = true;
    }

    /// Finished with changes that have not been undone.
    pub(crate) fn undoable(&self) -> bool {
        self.status.finished() && self.changed && !self.undone
    }

    pub(crate) fn warn(&mut self, warning: String) {
        self.warning = Some(warning);
    }

    /// Takes the agent's pipes and starts the session: the threads hand
    /// what it says to `deliver`.
    pub(crate) fn attach(
        &mut self,
        mut child: Child,
        deliver: impl Fn(Inbound) + Send + Sync + Clone + 'static,
    ) -> Result<(), String> {
        let (Some(mut input), Some(output), Some(errors)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill();
            return Err("the agent's pipes are missing".to_owned());
        };
        let (outbox, queue) = channel::<Vec<u8>>();
        let spawned = thread::Builder::new()
            .name("acp-writer".to_owned())
            .spawn(move || {
                for line in queue {
                    if input.write_all(&line).and_then(|()| input.flush()).is_err() {
                        return;
                    }
                }
            })
            .and_then(|_| {
                let deliver = deliver.clone();
                thread::Builder::new()
                    .name("acp-reader".to_owned())
                    .spawn(move || read_lines(output, &deliver))
            })
            .and_then(|_| {
                thread::Builder::new()
                    .name("acp-stderr".to_owned())
                    .spawn(move || {
                        moonframe_providers::lines::log_lines(errors, |line| {
                            deliver(Inbound::Log(line));
                        });
                    })
            });
        if let Err(error) = spawned {
            let _ = child.kill();
            return Err(format!("cannot start the agent's threads: {error}"));
        }
        self.outbox = Some(outbox);
        self.exited.store(false, Ordering::Release);
        self.pid = Some(child.id());
        let now = self.timing.now();
        self.timing.spawned.get_or_insert(now);
        self.child = Some(child);
        self.request_rpc(
            "initialize",
            &json!({
                "protocolVersion": 1,
                // The agent uses its own tools for files and commands.
                "clientCapabilities": {
                    "fs": { "readTextFile": false, "writeTextFile": false },
                    "terminal": false,
                },
                "clientInfo": { "name": "moonframe", "version": env!("CARGO_PKG_VERSION") },
            }),
            Pending::Initialize,
        );
        Ok(())
    }

    pub(crate) fn receive(&mut self, inbound: Inbound, now: u64) {
        match inbound {
            Inbound::Line(line) => self.line(&line, now),
            Inbound::Log(line) => {
                tracing::info!(provider = "acp", agent = self.agent.name, "{line}");
                if self.log.len() == MAX_LOG {
                    self.log.pop_front();
                }
                self.log.push_back(line);
            },
            Inbound::Closed(problem) => {
                if !self.status.finished() {
                    let last = self.log.iter().rev().find(|line| !line.trim().is_empty());
                    let why = problem
                        .or_else(|| last.cloned())
                        .unwrap_or_else(|| "it exited".to_owned());
                    self.fail(format!("{} stopped: {why}", self.agent.title), now);
                }
            },
        }
    }

    fn line(&mut self, line: &[u8], now: u64) {
        let Ok(Value::Object(mut message)) = serde_json::from_slice::<Value>(line) else {
            return;
        };
        let id = message.remove("id");
        let params = message.remove("params").unwrap_or(Value::Null);
        if let Some(Value::String(method)) = message.remove("method") {
            match id {
                Some(id) => self.agent_request(id, &method, params),
                None if method == "session/update" => self.update(params),
                None => {},
            }
            return;
        }
        let Some(id) = id.as_ref().and_then(Value::as_i64) else {
            return;
        };
        let Some(pending) = self.pending.remove(&id) else {
            return;
        };
        let result = match message.remove("error") {
            Some(error) => Err(serde_json::from_value::<wire::Error>(error)
                .unwrap_or_else(|_| wire::Error::internal_error())),
            None => Ok(message.remove("result").unwrap_or(Value::Null)),
        };
        self.response(pending, result, now);
    }

    fn response(&mut self, pending: Pending, result: Result<Value, wire::Error>, now: u64) {
        if self.status.finished() {
            return;
        }
        let value = match result {
            Ok(value) => value,
            Err(error) => {
                let why = if error.code == wire::Error::auth_required().code {
                    format!(
                        "{} needs you to sign in: {}",
                        self.agent.title, self.agent.login
                    )
                } else {
                    error.message
                };
                return self.fail(why, now);
            },
        };
        match pending {
            Pending::Initialize => {
                let at = self.timing.now();
                self.timing.initialized.get_or_insert(at);
                self.agent_info = agent_info(&value);
                self.open_session();
            },
            Pending::NewSession => {
                if let Some(model) = model_name(&value) {
                    self.model = Some(model);
                }
                let Ok(response) = serde_json::from_value::<wire::NewSessionResponse>(value) else {
                    return self.fail("the agent's session was invalid".to_owned(), now);
                };
                self.session = Some(response.session_id.0.to_string());
                let at = self.timing.now();
                self.timing.session.get_or_insert(at);
                // A warm agent waits for its run's prompt.
                if self.prompt.is_empty() {
                    self.ready = true;
                } else {
                    self.send_prompt();
                }
            },
            Pending::Prompt => {
                let reason = serde_json::from_value::<wire::PromptResponse>(value)
                    .map(|response| response.stop_reason);
                match reason {
                    Ok(wire::StopReason::EndTurn) => self.finish(Status::Done, now),
                    Ok(wire::StopReason::Cancelled) => self.finish(Status::Cancelled, now),
                    Ok(other) => {
                        let name = enum_name(&other);
                        self.stop_reason = Some(name.clone());
                        self.fail(format!("{} stopped early: {name}", self.agent.title), now);
                    },
                    Err(_) => self.finish(Status::Done, now),
                }
            },
        }
    }

    fn update(&mut self, params: Value) {
        // An update from a newer protocol revision may not decode; it is
        // skipped rather than ending the run.
        let Ok(notification) = serde_json::from_value::<wire::SessionNotification>(params) else {
            return;
        };
        if self.session.as_deref() != Some(&*notification.session_id.0) {
            return;
        }
        match notification.update {
            wire::SessionUpdate::AgentMessageChunk(chunk) => {
                if let wire::ContentBlock::Text(text) = chunk.content {
                    let at = self.timing.now();
                    self.timing.first_token.get_or_insert(at);
                    if self.fresh {
                        self.message.clear();
                        self.fresh = false;
                    }
                    self.message.push_str(&text.text);
                    self.transcript.text(&text.text);
                    if self.message.len() > MAX_MESSAGE {
                        let cut = self.message.len() - MAX_MESSAGE;
                        let cut = (cut..self.message.len())
                            .find(|at| self.message.is_char_boundary(*at))
                            .unwrap_or(self.message.len());
                        self.message.drain(..cut);
                    }
                }
            },
            wire::SessionUpdate::ToolCall(call) => {
                let at = self.timing.now();
                self.timing.first_token.get_or_insert(at);
                self.fresh = true;
                self.tool_count += 1;
                if self.tools.len() == MAX_TOOLS {
                    self.tools.pop_front();
                }
                self.transcript.tool(&call.tool_call_id.0);
                self.tools.push_back(Tool {
                    id: call.tool_call_id.0.to_string(),
                    title: call.title,
                    kind: enum_name(&call.kind),
                    status: enum_name(&call.status),
                    paths: crate::guard::paths(
                        &call.content,
                        &call.locations,
                        call.raw_input.as_ref(),
                    ),
                    command: crate::allow::command_of(call.raw_input.as_ref()),
                    targets: crate::allow::input_targets(call.raw_input.as_ref()),
                    began: Some(at),
                    ended: None,
                    ms: None,
                });
            },
            wire::SessionUpdate::ToolCallUpdate(update) => {
                let id = update.tool_call_id.0.to_string();
                if let Some(tool) = self.tools.iter_mut().find(|tool| tool.id == id) {
                    tool.paths.extend(fields_paths(&update.fields));
                    if let Some(title) = update.fields.title {
                        tool.title = title;
                    }
                    if update.fields.raw_input.is_some() {
                        tool.command = crate::allow::command_of(update.fields.raw_input.as_ref())
                            .or(tool.command.take());
                        tool.targets =
                            crate::allow::input_targets(update.fields.raw_input.as_ref());
                    }
                    if let Some(status) = update.fields.status {
                        tool.status = enum_name(&status);
                        if matches!(tool.status.as_str(), "completed" | "failed") {
                            tool.ended.get_or_insert(self.timing.now());
                        }
                    }
                    if let Some(kind) = update.fields.kind {
                        tool.kind = enum_name(&kind);
                    }
                }
            },
            wire::SessionUpdate::ConfigOptionUpdate(update) => {
                let options = json!({ "configOptions": update.config_options });
                if let Some(model) = model_name(&options) {
                    self.model = Some(model);
                }
            },
            wire::SessionUpdate::Plan(plan) => {
                self.plan = plan
                    .entries
                    .into_iter()
                    .map(|entry| (entry.content, enum_name(&entry.status)))
                    .collect();
            },
            _ => {},
        }
    }

    fn agent_request(&mut self, id: Value, method: &str, params: Value) {
        if method != "session/request_permission" {
            return self.respond(
                &id,
                Err(json!({ "code": -32601, "message": "Method not found" })),
            );
        }
        let Ok(request) = serde_json::from_value::<wire::RequestPermissionRequest>(params) else {
            return self.respond(
                &id,
                Err(json!({ "code": -32602, "message": "Invalid params" })),
            );
        };
        let call = request.tool_call.tool_call_id.0.to_string();
        // The kind the question says, else the one its tool call said.
        let kind = request
            .tool_call
            .fields
            .kind
            .as_ref()
            .map(enum_name)
            .or_else(|| {
                self.tools
                    .iter()
                    .find(|tool| tool.id == call && !tool.kind.is_empty())
                    .map(|tool| tool.kind.clone())
            });
        let mut paths = fields_paths(&request.tool_call.fields);
        if let Some(tool) = self.tools.iter().find(|tool| tool.id == call) {
            paths.extend(tool.paths.iter().cloned());
        }
        let refused = self.installed.as_ref().and_then(|installed| {
            crate::guard::refused(installed, kind.as_deref(), &paths, &self.cwd)
        });
        let refused = refused.cloned();
        let title = request
            .tool_call
            .fields
            .title
            .or_else(|| {
                self.tools
                    .iter()
                    .find(|tool| tool.id == call)
                    .map(|tool| tool.title.clone())
            })
            .unwrap_or_else(|| "Use a tool".to_owned());
        let options: Vec<(String, String, String)> = request
            .options
            .into_iter()
            .map(|option| {
                (
                    option.option_id.0.to_string(),
                    option.name,
                    enum_name(&option.kind),
                )
            })
            .collect();
        // Whatever `allow` says, and without asking: no answer makes this
        // safe. The agent's instructions say where to write instead.
        if let Some(path) = refused {
            self.warning = Some(format!(
                "The agent tried to change {}, which ships in the signed app, and was stopped.",
                path.display()
            ));
            let rejected = options.iter().find(|option| option.2 == "reject_once");
            return self.respond(
                &id,
                Ok(rejected.map_or_else(cancelled, |(option, ..)| selected(option))),
            );
        }
        // Allowed once, never always; without an "allow once" option the
        // user decides.
        let command = crate::allow::command_of(request.tool_call.fields.raw_input.as_ref())
            .or_else(|| {
                self.tools
                    .iter()
                    .find(|tool| tool.id == call)
                    .and_then(|tool| tool.command.clone())
            });
        let targets = request
            .tool_call
            .fields
            .raw_input
            .as_ref()
            .map(|input| crate::allow::input_targets(Some(input)))
            .unwrap_or_else(|| {
                self.tools
                    .iter()
                    .find(|tool| tool.id == call)
                    .map(|tool| tool.targets.clone())
                    .unwrap_or_else(|| crate::allow::input_targets(None))
            });
        let asked = Call {
            kind: kind.as_deref(),
            paths: &paths,
            command: command.as_deref(),
            targets: targets.as_deref(),
        };
        if self.allow.covers_call(&asked, &self.scope)
            && let Some((option, ..)) = options.iter().find(|option| option.2 == "allow_once")
        {
            return self.respond(&id, Ok(selected(option)));
        }
        // A question already waiting is withdrawn: the agent asks again.
        if let Some(question) = self.question.take() {
            self.respond(&question.rpc, Ok(cancelled()));
        }
        self.question = Some(Question {
            rpc: id,
            title,
            options,
        });
        if !self.status.finished() {
            self.status = Status::Waiting;
        }
    }

    /// The user's answer to the waiting question: an option's id, or none
    /// to refuse. False when there is no such question or option, or the
    /// option is one that allows always: answers come from a mod, which
    /// could pick it without the user, and the agent would keep it past
    /// the run. Allowing is once at most, as with `[settings] allow`.
    pub(crate) fn answer(&mut self, option: Option<&str>) -> bool {
        let Some(question) = self.question.as_ref() else {
            return false;
        };
        let outcome = match option {
            Some(option) => match question.options.iter().find(|known| known.0 == option) {
                Some((.., kind)) if kind == "allow_always" => return false,
                Some(_) => selected(option),
                None => return false,
            },
            None => cancelled(),
        };
        let rpc = question.rpc.clone();
        self.question = None;
        self.respond(&rpc, Ok(outcome));
        if self.status == Status::Waiting {
            self.status = Status::Running;
        }
        true
    }

    pub(crate) fn cancel(&mut self, now: u64) {
        if self.status.finished() {
            return;
        }
        if let Some(question) = self.question.take() {
            self.respond(&question.rpc, Ok(cancelled()));
        }
        if let Some(session) = self.session.clone() {
            self.notify("session/cancel", &json!({ "sessionId": session }));
        }
        self.finish(Status::Cancelled, now);
    }

    /// Stops the agent as the app quits: its group is asked at once, as no
    /// thread outlives the app to ask later.
    pub(crate) fn stop(&mut self, now: u64) {
        if !self.status.finished() {
            if let Some(pid) = self.pid.filter(|_| self.child.is_some()) {
                crate::tree::signal(pid, false);
            }
            self.finish(Status::Stopped, now);
        }
    }

    /// The agent still running, if any: its process id and whether it has
    /// exited since.
    pub(crate) fn agent(&self) -> Option<(u32, Arc<AtomicBool>)> {
        let running = self.child.is_some() || !self.exited.load(Ordering::Acquire);
        self.pid
            .filter(|_| running)
            .map(|pid| (pid, Arc::clone(&self.exited)))
    }

    pub(crate) fn fail(&mut self, why: String, now: u64) {
        self.error = Some(why);
        self.finish(Status::Failed, now);
    }

    fn finish(&mut self, status: Status, now: u64) {
        let at = self.timing.now();
        self.timing.ended.get_or_insert(at);
        self.status = status;
        self.finished = Some(now);
        self.question = None;
        self.pending.clear();
        // Closing the queue ends the writer, and with it the agent's input.
        self.outbox = None;
        if let Some(mut child) = self.child.take() {
            // Given a moment to read what it was last sent (a cancel) and
            // exit with its input closed, then killed; on a thread of its
            // own, so an agent slow to die never holds the provider's lock.
            let (exited, pid) = (Arc::clone(&self.exited), self.pid);
            let reaped = thread::Builder::new()
                .name("acp-reap".to_owned())
                .spawn(move || {
                    let deadline = Instant::now() + GRACE;
                    while Instant::now() < deadline {
                        if matches!(child.try_wait(), Ok(Some(_))) {
                            exited.store(true, Ordering::Release);
                            return;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                    if let Some(pid) = pid {
                        crate::tree::signal(pid, true);
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                    exited.store(true, Ordering::Release);
                });
            if let Err(error) = reaped {
                tracing::warn!(provider = "acp", "cannot reap the agent: {error}");
                self.exited.store(true, Ordering::Release);
            }
        }
        // A warm agent's run has no mod to tell.
        if let Some(host) = self.host.as_ref().filter(|_| !self.owner.is_empty()) {
            let mut answer = json!({
                "reply": self.request,
                "action": "finished",
                "ok": status == Status::Done,
                "run": self.id,
                "status": status.name(),
            });
            match &self.error {
                Some(error) => answer["error"] = json!(error),
                None => answer["message"] = json!(self.message),
            }
            if let Ok(data) = moonframe_providers::from_json(&answer) {
                host.send(&self.owner, data);
            }
        }
    }

    fn request_rpc(&mut self, method: &str, params: &Value, pending: Pending) {
        self.next += 1;
        self.pending.insert(self.next, pending);
        self.send(
            &json!({ "jsonrpc": "2.0", "id": self.next, "method": method, "params": params }),
        );
    }

    fn notify(&self, method: &str, params: &Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    fn respond(&self, id: &Value, result: Result<Value, Value>) {
        let message = match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        };
        self.send(&message);
    }

    fn send(&self, message: &Value) {
        let Some(outbox) = &self.outbox else {
            return;
        };
        if let Ok(mut line) = serde_json::to_vec(message) {
            line.push(b'\n');
            let _ = outbox.send(line);
        }
    }
}

/// Reads the agent's output a line at a time until it ends.
fn read_lines(output: impl Read, deliver: &impl Fn(Inbound)) {
    let mut reader = BufReader::new(output);
    let mut line = Vec::new();
    let problem = loop {
        line.clear();
        match (&mut reader)
            .take(MAX_LINE as u64 + 1)
            .read_until(b'\n', &mut line)
        {
            Ok(0) => break None,
            Ok(_) if line.len() > MAX_LINE => {
                break Some("it sent a line too long to read".to_owned());
            },
            Ok(_) => deliver(Inbound::Line(std::mem::take(&mut line))),
            Err(error) => break Some(error.to_string()),
        }
    };
    deliver(Inbound::Closed(problem));
}
