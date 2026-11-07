//! A thread with pi running: follows pi's event stream and keeps what the
//! window needs between session writes: the message streaming in, the tools
//! at work, what is queued, and what extensions are asking for.

use {
    crate::{
        rpc::Rpc,
        value::{as_list, display, lock, now_millis},
    },
    serde_json::{Value, json},
    std::{
        collections::{BTreeMap, HashMap},
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{
            Arc, Condvar, Mutex,
            mpsc::{self, Sender},
        },
        thread,
        time::{Duration, Instant},
    },
};

/// What a tool is doing, between pi's session writes.
#[derive(Clone, Debug)]
pub(crate) enum ToolState {
    Working {
        content: Value,
    },
    Done {
        content: Value,
        details: Value,
        failed: bool,
    },
}

/// An extension's question.
#[derive(Clone, Debug)]
pub(crate) struct Dialog {
    pub(crate) id: Value,
    /// As the window draws it.
    pub(crate) shown: Value,
    /// When it stops waiting, if it does.
    pub(crate) until: Option<Instant>,
}

/// What the engine does about an event, once the thread has taken it in.
#[derive(Debug)]
pub(crate) enum Hook {
    Entry(Value),
    Reload,
    TurnStarted(Value),
    Settled {
        aborted: bool,
    },
    Renamed,
    Waiting(String),
    Toast {
        level: String,
        title: String,
        body: String,
    },
}

/// Set once: a thread's first load, which others wait on.
#[derive(Default)]
pub(crate) struct Latch {
    outcome: Mutex<Option<Result<(), String>>>,
    set: Condvar,
}

impl Latch {
    pub(crate) fn finish(&self, outcome: Result<(), String>) {
        *lock(&self.outcome) = Some(outcome);
        self.set.notify_all();
    }

    pub(crate) fn wait(&self) -> Result<(), String> {
        let mut outcome = lock(&self.outcome);
        loop {
            if let Some(outcome) = outcome.as_ref() {
                return outcome.clone();
            }
            outcome = self
                .set
                .wait(outcome)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}

type Job = Box<dyn FnOnce() + Send>;

/// Work done in order on a thread of its own, for as long as this lives: a
/// run's before and after take git, and pi's events must not wait for that.
pub(crate) struct Serial(Sender<Job>);

impl Default for Serial {
    fn default() -> Self {
        let (jobs, queue) = mpsc::channel::<Job>();
        thread::spawn(move || {
            for job in queue {
                if catch_unwind(AssertUnwindSafe(job)).is_err() {
                    tracing::error!("pilot: a thread's job panicked");
                }
            }
        });
        Self(jobs)
    }
}

impl Serial {
    pub(crate) fn push(&self, job: impl FnOnce() + Send + 'static) {
        let _ = self.0.send(Box::new(job));
    }
}

/// One thread's pi.
pub(crate) struct Live {
    pub(crate) id: String,
    pub(crate) cwd: String,
    pub(crate) rpc: Arc<Rpc>,
    pub(crate) ready: Arc<Latch>,
    pub(crate) jobs: Serial,
    pub(crate) running: bool,
    pub(crate) partial: Option<Value>,
    pub(crate) tools: HashMap<String, ToolState>,
    pub(crate) queued: Value,
    pub(crate) dialogs: Vec<Dialog>,
    pub(crate) statuses: BTreeMap<String, Value>,
    pub(crate) widgets: BTreeMap<String, Value>,
    pub(crate) retry: Option<Value>,
    pub(crate) compacting: bool,
    /// Words sent, until pi takes them as a prompt.
    pub(crate) sending: Option<String>,
    pub(crate) editor_text: Option<Value>,
    pub(crate) state: Option<Value>,
    pub(crate) levels: Vec<Value>,
    pub(crate) commands: Vec<Value>,
    /// The run under way, by its prompt's timestamp.
    pub(crate) turn: Option<String>,
    /// The tree that run started from.
    pub(crate) before: Option<String>,
    /// The tree taken just ahead of sending a prompt.
    pub(crate) next_before: Option<String>,
}

fn apply_delta(partial: &mut Value, update: &Value) {
    let Some(index) = update["contentIndex"]
        .as_u64()
        .and_then(|index| usize::try_from(index).ok())
    else {
        return;
    };
    let Value::Array(content) = &mut partial["content"] else {
        return;
    };
    if content.len() <= index {
        content.resize(index + 1, Value::Null);
    }
    let block = &mut content[index];
    let delta = update["delta"].as_str().unwrap_or_default();
    match update["type"].as_str().unwrap_or_default() {
        "text_start" => *block = json!({ "type": "text", "text": "" }),
        "text_delta" => append(block, "text", delta),
        "text_end" => block["text"] = update["content"].clone(),
        "thinking_start" => *block = json!({ "type": "thinking", "thinking": "" }),
        "thinking_delta" => append(block, "thinking", delta),
        "thinking_end" => block["thinking"] = update["content"].clone(),
        "toolcall_start" => {
            *block = json!({
                "type": "toolCall",
                "id": update["id"],
                "name": update["toolName"],
                "arguments": {},
            });
        },
        "toolcall_end" => *block = update["toolCall"].clone(),
        _ => {},
    }
}

fn append(block: &mut Value, key: &str, delta: &str) {
    if let Some(Value::String(text)) = block.get_mut(key) {
        text.push_str(delta);
    }
}

impl Live {
    pub(crate) fn new(id: String, cwd: String, rpc: Arc<Rpc>) -> Self {
        Self {
            id,
            cwd,
            rpc,
            ready: Arc::default(),
            jobs: Serial::default(),
            running: false,
            partial: None,
            tools: HashMap::new(),
            queued: json!({ "steering": [], "followUp": [] }),
            dialogs: Vec::new(),
            statuses: BTreeMap::new(),
            widgets: BTreeMap::new(),
            retry: None,
            compacting: false,
            sending: None,
            editor_text: None,
            state: None,
            levels: Vec::new(),
            commands: Vec::new(),
            turn: None,
            before: None,
            next_before: None,
        }
    }

    /// Takes in one of pi's events; what comes back is for the engine.
    pub(crate) fn receive(&mut self, event: &Value) -> Vec<Hook> {
        self.rpc.touch();
        let mut hooks = Vec::new();
        match event["type"].as_str().unwrap_or_default() {
            "agent_start" => self.running = true,
            "message_start" => self.message_start(&event["message"], &mut hooks),
            "message_update" => {
                if let Some(partial) = &mut self.partial {
                    apply_delta(partial, &event["assistantMessageEvent"]);
                }
            },
            "message_end" => {
                let role = event["message"]["role"].as_str();
                if role == Some("assistant") {
                    self.partial = None;
                }
                if role != Some("system") {
                    hooks.push(Hook::Entry(event["message"].clone()));
                }
            },
            "tool_execution_start" => {
                self.set_tool(event, ToolState::Working {
                    content: Value::Null,
                });
            },
            "tool_execution_update" => {
                let content = event["partialResult"]["content"].clone();
                self.set_tool(event, ToolState::Working { content });
            },
            "tool_execution_end" => {
                let state = ToolState::Done {
                    content: event["result"]["content"].clone(),
                    details: event["result"]["details"].clone(),
                    failed: event["isError"].as_bool().unwrap_or(false),
                };
                self.set_tool(event, state);
            },
            "queue_update" => {
                let list = |key: &str| Value::Array(as_list(&event[key]).to_vec());
                self.queued = json!({ "steering": list("steering"), "followUp": list("followUp") });
            },
            "compaction_start" => self.compacting = true,
            "compaction_end" => {
                self.compacting = false;
                hooks.push(Hook::Reload);
            },
            "auto_retry_start" => {
                self.retry = Some(json!({
                    "attempt": event["attempt"],
                    "max": event["maxAttempts"],
                    "error": event["errorMessage"],
                }));
            },
            "auto_retry_end" => self.retry = None,
            "agent_settled" => {
                self.running = false;
                self.partial = None;
                self.retry = None;
                self.sending = None;
                self.tools.clear();
                let aborted = event["aborted"].as_bool().unwrap_or(false);
                hooks.push(Hook::Settled { aborted });
            },
            "session_info_changed" => hooks.push(Hook::Renamed),
            "thinking_level_changed" => {
                if let Some(state) = &mut self.state {
                    state["thinkingLevel"] = event["level"].clone();
                }
            },
            "extension_ui_request" => self.extension_request(event, &mut hooks),
            "extension_error" => hooks.push(Hook::Toast {
                level: "error".to_owned(),
                title: "An extension failed".to_owned(),
                body: display(&event["error"]),
            }),
            _ => {},
        }
        hooks
    }

    fn set_tool(&mut self, event: &Value, state: ToolState) {
        self.tools.insert(display(&event["toolCallId"]), state);
    }

    fn message_start(&mut self, message: &Value, hooks: &mut Vec<Hook>) {
        match message["role"].as_str() {
            Some("assistant") => {
                let mut partial = message.clone();
                partial["content"] = json!([]);
                self.partial = Some(partial);
            },
            Some("user") => {
                self.sending = None;
                self.running = true;
                hooks.push(Hook::TurnStarted(message.clone()));
            },
            _ => {},
        }
    }

    fn extension_request(&mut self, request: &Value, hooks: &mut Vec<Hook>) {
        match request["method"].as_str().unwrap_or_default() {
            kind @ ("select" | "confirm" | "input" | "editor") => {
                let shown = json!({
                    "id": request["id"],
                    "kind": kind,
                    "title": display(&request["title"]),
                    "message": display(&request["message"]),
                    "options": as_list(&request["options"]),
                    "placeholder": display(&request["placeholder"]),
                    "prefill": display(&request["prefill"]),
                });
                let until = request["timeout"]
                    .as_u64()
                    .filter(|timeout| *timeout > 0)
                    .map(|timeout| Instant::now() + Duration::from_millis(timeout));
                self.dialogs.push(Dialog {
                    id: request["id"].clone(),
                    shown,
                    until,
                });
                let title = request["title"]
                    .as_str()
                    .unwrap_or("An extension needs you");
                hooks.push(Hook::Waiting(title.to_owned()));
            },
            "notify" => hooks.push(Hook::Toast {
                level: request["notifyType"].as_str().unwrap_or("info").to_owned(),
                title: "pi".to_owned(),
                body: display(&request["message"]),
            }),
            "setStatus" => keep(&mut self.statuses, request, "statusKey", "statusText"),
            "setWidget" => keep(&mut self.widgets, request, "widgetKey", "widgetLines"),
            "set_editor_text" => {
                self.editor_text = Some(json!({ "text": request["text"], "seq": now_millis() }));
            },
            _ => {},
        }
    }

    /// Drops the questions that stopped waiting; whether any did.
    pub(crate) fn expire_dialogs(&mut self, now: Instant) -> bool {
        let before = self.dialogs.len();
        self.dialogs
            .retain(|dialog| dialog.until.is_none_or(|until| until > now));
        self.dialogs.len() != before
    }

    /// Sends the user's answer to a question.
    pub(crate) fn answer(&mut self, id: &Value, fields: serde_json::Map<String, Value>) {
        let mut record = fields;
        record.insert("type".to_owned(), json!("extension_ui_response"));
        record.insert("id".to_owned(), id.clone());
        self.rpc.write(&Value::Object(record));
        self.dialogs.retain(|dialog| dialog.id != *id);
    }
}

fn keep(list: &mut BTreeMap<String, Value>, request: &Value, key: &str, value: &str) {
    let name = display(&request[key]);
    let given = &request[value];
    let empty = given.is_null() || *given == json!("") || *given == json!(false);
    if empty {
        list.remove(&name);
    } else {
        list.insert(name, given.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deltas_build_up_the_streaming_message() {
        let mut partial = json!({ "content": [] });
        let step = |partial: &mut Value, update: Value| apply_delta(partial, &update);
        step(
            &mut partial,
            json!({ "type": "text_start", "contentIndex": 0 }),
        );
        step(
            &mut partial,
            json!({ "type": "text_delta", "contentIndex": 0, "delta": "he" }),
        );
        step(
            &mut partial,
            json!({ "type": "text_delta", "contentIndex": 0, "delta": "llo" }),
        );
        assert_eq!(partial["content"][0]["text"], "hello");
        step(
            &mut partial,
            json!({ "type": "text_end", "contentIndex": 0, "content": "Hello" }),
        );
        assert_eq!(partial["content"][0]["text"], "Hello");
        step(
            &mut partial,
            json!({ "type": "toolcall_start", "contentIndex": 2, "id": "t", "toolName": "read" }),
        );
        assert_eq!(partial["content"][1], Value::Null);
        assert_eq!(partial["content"][2]["name"], "read");
        let call =
            json!({ "type": "toolCall", "id": "t", "name": "read", "arguments": { "path": "a" } });
        step(
            &mut partial,
            json!({ "type": "toolcall_end", "contentIndex": 2, "toolCall": call }),
        );
        assert_eq!(partial["content"][2]["arguments"]["path"], "a");
    }

    #[test]
    fn a_delta_for_nothing_is_ignored() {
        let mut partial = json!({ "content": [] });
        apply_delta(
            &mut partial,
            &json!({ "type": "text_delta", "contentIndex": 0, "delta": "x" }),
        );
        assert_eq!(partial["content"][0], Value::Null);
    }

    #[test]
    fn jobs_run_in_order_and_a_bad_one_stops_nothing() {
        let (done, heard) = mpsc::channel();
        let serial = Serial::default();
        let first = done.clone();
        serial.push(move || first.send(1).unwrap_or(()));
        serial.push(|| panic!("a bad job"));
        serial.push(move || done.send(2).unwrap_or(()));
        let order: Vec<i32> = heard.iter().take(2).collect();
        assert_eq!(order, [1, 2]);
    }
}
