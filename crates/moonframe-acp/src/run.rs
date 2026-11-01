//! One run: an agent process, its ACP session, and what it has said.
//!
//! JSON-RPC 2.0 goes over the agent's standard input and output, one
//! message per line, as in herdr-acp: a reader thread hands each line to
//! the run under the provider's lock, a writer thread writes queued lines
//! in order, so a run never blocks on an agent that stops reading.

use {
    acp::v1 as wire,
    moonframe_providers::Handle,
    serde_json::{Value, json},
    std::{
        collections::{BTreeMap, VecDeque},
        io::{BufRead, BufReader, Read, Write},
        path::PathBuf,
        process::Child,
        sync::mpsc::{Sender, channel},
        thread,
        time::{Duration, Instant},
    },
};

/// The longest line read from an agent.
const MAX_LINE: usize = 16 << 20;
/// What is kept of the agent's latest message, in characters.
const MAX_MESSAGE: usize = 16_000;
/// Tool calls kept per run, newest last.
const MAX_TOOLS: usize = 40;
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
    plan: Vec<(String, String)>,
    question: Option<Question>,
    /// Which permission questions are answered without asking.
    allow: Allow,
    cwd: PathBuf,
    session: Option<String>,
    next: i64,
    pending: BTreeMap<i64, Pending>,
    outbox: Option<Sender<Vec<u8>>>,
    child: Option<Child>,
    log: VecDeque<String>,
    host: Option<Handle>,
}

/// Which permission questions a run answers itself, as the user set for
/// its mod in provider.toml's `[settings] allow`; never a mod's own choice.
/// Each is answered "allow once", never "always": the agent asks again
/// next time, and nothing it was allowed outlives the run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Allow {
    /// Every question waits on the user.
    #[default]
    Ask,
    /// Tools that only look (read, search, think); the rest wait.
    Read,
    /// Every tool.
    All,
}

impl Allow {
    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "ask" => Some(Self::Ask),
            "read" => Some(Self::Read),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    /// Whether a tool of `kind` is answered without asking. A tool whose
    /// kind is unknown counts as one that changes things.
    pub(crate) fn covers(self, kind: Option<&str>) -> bool {
        match self {
            Self::Ask => false,
            Self::Read => matches!(kind, Some("read" | "search" | "think")),
            Self::All => true,
        }
    }
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
    pub(crate) now: u64,
}

impl Run {
    pub(crate) fn new(start: Start, host: Option<Handle>) -> Self {
        Self {
            id: start.id,
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
            plan: Vec::new(),
            question: None,
            allow: start.allow,
            cwd: start.cwd,
            session: None,
            next: 0,
            pending: BTreeMap::new(),
            outbox: None,
            child: None,
            log: VecDeque::new(),
            host,
        }
    }

    pub(crate) fn started(&self) -> u64 {
        self.started
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
            Pending::Initialize => self.request_rpc(
                "session/new",
                &json!({ "cwd": self.cwd, "mcpServers": [] }),
                Pending::NewSession,
            ),
            Pending::NewSession => {
                let Ok(response) = serde_json::from_value::<wire::NewSessionResponse>(value) else {
                    return self.fail("the agent's session was invalid".to_owned(), now);
                };
                let session = response.session_id.0.to_string();
                let prompt = json!({
                    "sessionId": session,
                    "prompt": [{ "type": "text", "text": self.prompt }],
                });
                self.session = Some(session);
                self.status = Status::Running;
                self.request_rpc("session/prompt", &prompt, Pending::Prompt);
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
                    if self.fresh {
                        self.message.clear();
                        self.fresh = false;
                    }
                    self.message.push_str(&text.text);
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
                self.fresh = true;
                self.tool_count += 1;
                if self.tools.len() == MAX_TOOLS {
                    self.tools.pop_front();
                }
                self.tools.push_back(Tool {
                    id: call.tool_call_id.0.to_string(),
                    title: call.title,
                    kind: enum_name(&call.kind),
                    status: enum_name(&call.status),
                });
            },
            wire::SessionUpdate::ToolCallUpdate(update) => {
                let id = update.tool_call_id.0.to_string();
                if let Some(tool) = self.tools.iter_mut().find(|tool| tool.id == id) {
                    if let Some(title) = update.fields.title {
                        tool.title = title;
                    }
                    if let Some(status) = update.fields.status {
                        tool.status = enum_name(&status);
                    }
                    if let Some(kind) = update.fields.kind {
                        tool.kind = enum_name(&kind);
                    }
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
        // Allowed once, never always; without an "allow once" option the
        // user decides.
        if self.allow.covers(kind.as_deref())
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

    /// Stops the agent as the app quits.
    pub(crate) fn stop(&mut self, now: u64) {
        if !self.status.finished() {
            self.finish(Status::Stopped, now);
        }
    }

    pub(crate) fn fail(&mut self, why: String, now: u64) {
        self.error = Some(why);
        self.finish(Status::Failed, now);
    }

    fn finish(&mut self, status: Status, now: u64) {
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
            let reaped = thread::Builder::new()
                .name("acp-reap".to_owned())
                .spawn(move || {
                    let deadline = Instant::now() + GRACE;
                    while Instant::now() < deadline {
                        if matches!(child.try_wait(), Ok(Some(_))) {
                            return;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                });
            if let Err(error) = reaped {
                tracing::warn!(provider = "acp", "cannot reap the agent: {error}");
            }
        }
        if let Some(host) = &self.host {
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

    /// The run as `app.acp` shows it.
    pub(crate) fn to_json(&self) -> Value {
        let tools: Vec<Value> = self
            .tools
            .iter()
            .rev()
            .take(12)
            .rev()
            .map(|tool| {
                json!({ "id": tool.id, "title": tool.title, "kind": tool.kind, "status": tool.status })
            })
            .collect();
        let plan: Vec<Value> = self
            .plan
            .iter()
            .take(30)
            .map(|(content, status)| json!({ "content": content, "status": status }))
            .collect();
        let question = self.question.as_ref().map(|question| {
            let options: Vec<Value> = question
                .options
                .iter()
                .map(|(id, name, kind)| json!({ "id": id, "name": name, "kind": kind }))
                .collect();
            json!({ "title": question.title, "options": options })
        });
        json!({
            "id": self.id,
            "owner": self.owner,
            "agent": self.agent.name,
            "agent_title": self.agent.title,
            "text": self.text,
            "thread": self.thread,
            "status": self.status.name(),
            "started": self.started,
            "finished": self.finished,
            "error": self.error,
            "stop_reason": self.stop_reason,
            "message": if self.message.is_empty() { Value::Null } else { json!(self.message) },
            "tools": tools,
            "tool_count": self.tool_count,
            "plan": plan,
            "question": question,
        })
    }

    /// A finished run as saved, to show again at the next launch.
    pub(crate) fn saved(&self) -> Value {
        let mut value = self.to_json();
        value["question"] = Value::Null;
        value
    }

    /// A saved run back; one that was running when the app quit is stopped.
    pub(crate) fn restore(value: &Value) -> Option<Self> {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        let agent = crate::agents::find(&text("agent")?)?;
        let mut run = Self::new(
            Start {
                id: text("id")?,
                owner: text("owner")?,
                request: Value::Null,
                agent,
                text: text("text").unwrap_or_default(),
                thread: text("thread"),
                prompt: String::new(),
                allow: Allow::Ask,
                cwd: PathBuf::new(),
                now: value.get("started").and_then(Value::as_u64).unwrap_or(0),
            },
            None,
        );
        run.status = Status::parse(&text("status").unwrap_or_default());
        run.finished = value
            .get("finished")
            .and_then(Value::as_u64)
            .or(Some(run.started));
        run.error = text("error");
        run.stop_reason = text("stop_reason");
        run.message = text("message").unwrap_or_default();
        run.tool_count = value
            .get("tool_count")
            .and_then(Value::as_u64)
            .and_then(|count| usize::try_from(count).ok())
            .unwrap_or(0);
        for tool in value
            .get("tools")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let field = |key: &str| {
                tool.get(key)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned()
            };
            run.tools.push_back(Tool {
                id: field("id"),
                title: field("title"),
                kind: field("kind"),
                status: field("status"),
            });
        }
        for entry in value
            .get("plan")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let field = |key: &str| {
                entry
                    .get(key)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned()
            };
            run.plan.push((field("content"), field("status")));
        }
        Some(run)
    }
}

fn selected(option: &str) -> Value {
    json!({ "outcome": { "outcome": "selected", "optionId": option } })
}

fn cancelled() -> Value {
    json!({ "outcome": { "outcome": "cancelled" } })
}

/// A wire enum's name as the protocol spells it (`in_progress`).
fn enum_name(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
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
