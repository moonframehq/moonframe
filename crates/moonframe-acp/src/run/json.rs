//! A run as `app.acp` shows it, and as it is saved and restored.

use {
    super::{Allow, Run, Scope, Start, Status, Timing, Tool, Transcript},
    serde_json::{Value, json},
    std::path::PathBuf,
};

impl Run {
    /// The run as `app.acp` shows it.
    pub(crate) fn to_json(&self) -> Value {
        let tools: Vec<Value> = self
            .tools
            .iter()
            .rev()
            .take(12)
            .rev()
            .map(|tool| {
                json!({ "id": tool.id, "title": tool.title, "kind": tool.kind, "status": tool.status, "ms": tool.duration() })
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
            // Where it works; none for a run saved before this was kept.
            "cwd": (!self.cwd.as_os_str().is_empty()).then_some(&self.cwd),
            "status": self.status.name(),
            "started": self.started,
            "finished": self.finished,
            "error": self.error,
            "stop_reason": self.stop_reason,
            "message": if self.message.is_empty() { Value::Null } else { json!(self.message) },
            "tools": tools,
            "tool_count": self.tool_count,
            "parts": self.transcript.to_json(|id| self.tools.iter().find(|tool| tool.id == id)),
            "agent_info": self.agent_info,
            "model": self.model,
            "plan": plan,
            "question": question,
            "warning": self.warning,
            "changed": self.changed,
            "undone": self.undone,
            "checking": self.checking,
            "timing": self.timing.to_json(),
        })
    }

    /// A finished run as saved, to show again at the next launch.
    pub(crate) fn saved(&self) -> Value {
        let mut value = self.to_json();
        value["question"] = Value::Null;
        value["kept"] = json!(self.kept);
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
                cwd: text("cwd").map(PathBuf::from).unwrap_or_default(),
                installed: None,
                scope: Scope::default(),
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
        run.warning = text("warning");
        run.checked = true;
        // Stopped by the app quitting: what it left is noted at this launch.
        run.kept = value.get("kept").and_then(Value::as_bool) != Some(false);
        run.changed = value.get("changed").and_then(Value::as_bool) == Some(true);
        run.undone = value.get("undone").and_then(Value::as_bool) == Some(true);
        run.stop_reason = text("stop_reason");
        run.message = text("message").unwrap_or_default();
        run.tool_count = value
            .get("tool_count")
            .and_then(Value::as_u64)
            .and_then(|count| usize::try_from(count).ok())
            .unwrap_or(0);
        run.agent_info = value
            .get("agent_info")
            .filter(|info| info.is_object())
            .cloned();
        run.model = text("model");
        run.timing = Timing::restore(value.get("timing"));
        let (transcript, tools) = Transcript::restore(value.get("parts"));
        run.transcript = transcript;
        run.tools.extend(tools);
        // Runs saved before transcripts: their last tool calls.
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
            if run.tools.iter().any(|known| known.id == field("id")) {
                continue;
            }
            run.tools.push_back(Tool {
                id: field("id"),
                title: field("title"),
                kind: field("kind"),
                status: field("status"),
                paths: Vec::new(),
                command: None,
                input: None,
                began: None,
                ended: None,
                ms: tool.get("ms").and_then(Value::as_u64),
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
