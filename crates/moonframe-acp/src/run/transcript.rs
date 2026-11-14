//! What a run said, in order: the agent's text and its tool calls as they
//! came, so an app can show the whole reply rather than its last message.
//! Text chunks join the text before them; tool calls in a row form one
//! group. The oldest parts go first past the limits.

use {
    super::Tool,
    serde_json::{Value, json},
};

/// The most text kept across a run's parts, in bytes.
const MAX_TEXT: usize = 64 << 10;
/// The most tool calls kept across a run's parts.
const MAX_CALLS: usize = 200;

#[derive(Debug, PartialEq, Eq)]
enum Part {
    Text(String),
    /// Tool calls by id, whose details the run keeps.
    Tools(Vec<String>),
}

#[derive(Debug, Default)]
pub(crate) struct Transcript {
    parts: Vec<Part>,
    text: usize,
    calls: usize,
}

impl Transcript {
    /// A chunk of the agent's message.
    pub(crate) fn text(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }
        match self.parts.last_mut() {
            Some(Part::Text(text)) => text.push_str(chunk),
            _ => self.parts.push(Part::Text(chunk.to_owned())),
        }
        self.text += chunk.len();
        self.trim();
    }

    /// A tool call the agent started.
    pub(crate) fn tool(&mut self, id: &str) {
        match self.parts.last_mut() {
            Some(Part::Tools(ids)) => ids.push(id.to_owned()),
            _ => self.parts.push(Part::Tools(vec![id.to_owned()])),
        }
        self.calls += 1;
        self.trim();
    }

    /// Drops the oldest parts, and then the start of the oldest text,
    /// until it fits.
    fn trim(&mut self) {
        while (self.text > MAX_TEXT || self.calls > MAX_CALLS) && self.parts.len() > 1 {
            match self.parts.remove(0) {
                Part::Text(text) => self.text -= text.len(),
                Part::Tools(ids) => self.calls -= ids.len(),
            }
        }
        if let Some(Part::Text(text)) = self.parts.first_mut()
            && self.text > MAX_TEXT
        {
            let over = self.text - MAX_TEXT;
            let cut = (over..text.len())
                .find(|at| text.is_char_boundary(*at))
                .unwrap_or(text.len());
            text.drain(..cut);
            self.text -= cut;
        }
    }

    /// The parts as `app.acp` shows them: `{ kind = "text", text }` and
    /// `{ kind = "tools", tools = { { id, title, kind, status } } }`, with
    /// each call's details from `tools`.
    pub(crate) fn to_json<'a>(&self, tools: impl Fn(&str) -> Option<&'a Tool>) -> Value {
        let parts: Vec<Value> = self
            .parts
            .iter()
            .map(|part| match part {
                Part::Text(text) => json!({ "kind": "text", "text": text }),
                Part::Tools(ids) => {
                    let calls: Vec<Value> = ids
                        .iter()
                        .map(|id| match tools(id) {
                            Some(tool) => json!({
                                "id": tool.id,
                                "title": tool.title,
                                "kind": tool.kind,
                                "status": tool.status,
                                "ms": tool.duration(),
                            }),
                            None => json!({ "id": id, "title": "", "kind": "", "status": "" }),
                        })
                        .collect();
                    json!({ "kind": "tools", "tools": calls })
                },
            })
            .collect();
        Value::Array(parts)
    }

    /// The parts back from what `to_json` made, with the tool calls they
    /// name, for the run to keep.
    pub(crate) fn restore(value: Option<&Value>) -> (Self, Vec<Tool>) {
        let mut transcript = Self::default();
        let mut tools = Vec::new();
        for part in value.and_then(Value::as_array).into_iter().flatten() {
            match part.get("kind").and_then(Value::as_str) {
                Some("text") => {
                    transcript.text(part.get("text").and_then(Value::as_str).unwrap_or(""));
                },
                Some("tools") => {
                    for call in part
                        .get("tools")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        let field = |key: &str| {
                            call.get(key)
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_owned()
                        };
                        transcript.tool(&field("id"));
                        tools.push(Tool {
                            id: field("id"),
                            title: field("title"),
                            kind: field("kind"),
                            status: field("status"),
                            paths: Vec::new(),
                            command: None,
                            targets: None,
                            began: None,
                            ended: None,
                            ms: call.get("ms").and_then(Value::as_u64),
                        });
                    }
                },
                _ => {},
            }
        }
        (transcript, tools)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn tool(id: &str, title: &str) -> Tool {
        Tool {
            id: id.to_owned(),
            title: title.to_owned(),
            kind: "read".to_owned(),
            status: "completed".to_owned(),
            paths: Vec::new(),
            command: None,
            targets: None,
            began: None,
            ended: None,
            ms: None,
        }
    }

    #[test]
    fn text_and_tool_calls_keep_their_order() {
        let mut transcript = Transcript::default();
        transcript.text("Let me ");
        transcript.text("look.");
        transcript.tool("t1");
        transcript.tool("t2");
        transcript.text("Found it.");
        let known = [tool("t1", "Read a"), tool("t2", "Read b")];
        let json = transcript.to_json(|id| known.iter().find(|tool| tool.id == id));
        assert_eq!(json[0], json!({ "kind": "text", "text": "Let me look." }));
        assert_eq!(json[1]["kind"], "tools");
        assert_eq!(json[1]["tools"][0]["title"], "Read a");
        assert_eq!(json[1]["tools"][1]["title"], "Read b");
        assert_eq!(json[2], json!({ "kind": "text", "text": "Found it." }));

        let (back, tools) = Transcript::restore(Some(&json));
        assert_eq!(back.parts, transcript.parts);
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[1].title, "Read b");
    }

    #[test]
    fn the_oldest_parts_go_first() {
        let mut transcript = Transcript::default();
        transcript.text("old");
        transcript.tool("t1");
        transcript.text(&"é".repeat(MAX_TEXT / 2));
        transcript.text("x");
        let json = transcript.to_json(|_| None);
        // The first text and the tools after it went; what is left starts
        // on a character.
        assert_eq!(json.as_array().unwrap().len(), 1);
        let text = json[0]["text"].as_str().unwrap();
        assert!(text.len() <= MAX_TEXT);
        assert!(text.ends_with("éx"));
    }
}
