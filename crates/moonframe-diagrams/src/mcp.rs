//! `moonframe mcp diagrams`: a Model Context Protocol server over stdio
//! (JSON-RPC 2.0, one message a line) whose tools let an agent show the
//! user diagrams beside the chat and keep them up to date as it works:
//! `diagram_show`, `diagram_patch`, `diagram_highlight`, `diagram_close`.
//! Each diagram is written whole to a file in a folder the app's
//! `diagrams` provider watches.

use {
    crate::{
        document,
        store::{self, Shown},
    },
    serde_json::{Value, json},
    std::{
        io::{BufRead, Write},
        path::{Path, PathBuf},
    },
};

/// The protocol version answered when the client asks for none we know.
const PROTOCOL: &str = "2025-06-18";
/// The longest line read: a request bigger than any diagram.
const MOST_LINE: usize = 4 << 20;

fn tools() -> Value {
    let graph = json!({
        "type": "object",
        "description": "Nodes and edges, laid out by the app: { direction: down|right, nodes: [{ id, label, sublabel?, kind?: box|rounded|pill|diamond|circle|cylinder|note|text, tone?: accent|success|warning|danger|muted, group? }], edges: [{ from, to, label?, kind?: solid|dashed|dotted|thick }], groups?: [{ id, label, parent? }] }"
    });
    json!([
        {
            "name": "diagram_show",
            "description": "Show the user a diagram beside the chat. Use it when relations matter: architecture, data flow, dependencies, states, a sequence of calls. Give Mermaid (flowchart, sequenceDiagram, stateDiagram, classDiagram, erDiagram) or DOT text, or a graph; the app lays it out. Showing an id again replaces it; to change part of it use diagram_patch.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "A short name for this diagram, letters, digits, - and _: the same id updates it." },
                    "title": { "type": "string" },
                    "mermaid": { "type": "string" },
                    "dot": { "type": "string" },
                    "graph": graph,
                },
                "required": ["id"],
            },
        },
        {
            "name": "diagram_patch",
            "description": "Change a shown diagram in place as your work moves on: add or remove nodes and edges, or set fields (label, tone, kind) on nodes. Nodes already there keep their place.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "add_nodes": { "type": "array", "items": { "type": "object" } },
                    "add_edges": { "type": "array", "items": { "type": "object" } },
                    "add_groups": { "type": "array", "items": { "type": "object" } },
                    "remove_nodes": { "type": "array", "items": { "type": "string" } },
                    "remove_edges": { "type": "array", "description": "Edge ids, or { from, to }." },
                    "set": { "type": "object", "description": "Fields to change, by node id: { \"api\": { \"tone\": \"success\" } }." },
                },
                "required": ["id"],
            },
        },
        {
            "name": "diagram_highlight",
            "description": "Point at part of a shown diagram: the nodes and edges you are talking about or working on, with a short note. An empty list clears it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "nodes": { "type": "array", "items": { "type": "string" } },
                    "edges": { "type": "array", "items": { "type": "string" } },
                    "note": { "type": "string" },
                },
                "required": ["id"],
            },
        },
        {
            "name": "diagram_close",
            "description": "Stop showing a diagram.",
            "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] },
        },
    ])
}

/// The server's state: where its diagrams go and for whom. What it shows
/// is the folder's: the app closes a diagram by removing its file.
pub struct Server {
    dir: PathBuf,
    agent: Option<String>,
    /// Whose diagrams these are, stamped on each it writes and named in
    /// each answer: the app's agent run when the app started the agent,
    /// else this server's own, as each agent session starts one. A chat
    /// draws a diagram only while the latest to write it is its session.
    run: String,
}

impl Server {
    pub fn new(dir: &Path, agent: Option<String>) -> Self {
        Self {
            dir: dir.to_owned(),
            agent,
            run: session(),
        }
    }

    /// Stamps its diagrams with the agent run `run`.
    pub fn for_run(mut self, run: Option<String>) -> Self {
        if let Some(run) = run.filter(|r| !r.is_empty()) {
            self.run = run;
        }
        self
    }

    /// Serves requests from `input` until it ends.
    pub fn serve(&mut self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(answer) = self.answer(&line) {
                writeln!(output, "{answer}")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    /// The answer to one line, `None` for a notification.
    pub fn answer(&mut self, line: &str) -> Option<Value> {
        if line.len() > MOST_LINE {
            return Some(error(Value::Null, -32600, "request too large"));
        }
        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return Some(error(Value::Null, -32700, &e.to_string())),
        };
        let id = request.get("id").cloned()?;
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => {
                let version = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or(PROTOCOL);
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "moonframe-diagrams", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "Show diagrams to the user with diagram_show when how things relate matters, then keep them current with diagram_patch and diagram_highlight.",
                })
            },
            "ping" => json!({}),
            "tools/list" => json!({ "tools": tools() }),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
                match self.call(name, &arguments) {
                    // The run last: a chat reads the diagram from the
                    // answer's first `id` and the run from this line.
                    Ok(text) => {
                        let text = format!("{text}\n[run {}]", self.run);
                        json!({ "content": [{ "type": "text", "text": text }] })
                    },
                    Err(text) => {
                        json!({ "content": [{ "type": "text", "text": text }], "isError": true })
                    },
                }
            },
            other => return Some(error(id, -32601, &format!("no method `{other}`"))),
        };
        Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }

    fn call(&self, tool: &str, args: &Value) -> Result<String, String> {
        let id = store::safe_id(args.get("id").and_then(Value::as_str).unwrap_or_default())?;
        let text = |key: &str| args.get(key).and_then(Value::as_str).map(str::to_owned);
        match tool {
            "diagram_show" => {
                let (document, source) = if let Some(mermaid) = text("mermaid") {
                    (
                        document::from_text("mermaid", &mermaid)?,
                        Some(("mermaid".to_owned(), mermaid)),
                    )
                } else if let Some(dot) = text("dot") {
                    (
                        document::from_text("dot", &dot)?,
                        Some(("dot".to_owned(), dot)),
                    )
                } else if let Some(graph) = args.get("graph") {
                    (document::check(graph)?, None)
                } else {
                    return Err("give `mermaid`, `dot` or `graph`".into());
                };
                let revision = self.known(&id).map_or(1, |s| s.revision + 1);
                let nodes = document["nodes"].as_array().map_or(0, Vec::len);
                let shown = Shown {
                    title: text("title").unwrap_or_else(|| id.clone()),
                    id: id.clone(),
                    agent: self.agent.clone(),
                    run: Some(self.run.clone()),
                    revision,
                    updated: store::now(),
                    document,
                    highlight: Vec::new(),
                    note: None,
                    source,
                };
                self.save(&shown)?;
                Ok(format!("Showing `{id}` ({nodes} nodes) to the user."))
            },
            "diagram_patch" => {
                let mut shown = self
                    .known(&id)
                    .ok_or_else(|| format!("no diagram `{id}`: show it first"))?;
                let summary = document::patch(&mut shown.document, args)?;
                shown.revision += 1;
                shown.updated = store::now();
                // Patched, it no longer matches the text it came from.
                shown.source = None;
                self.save(&shown)?;
                Ok(format!("`{id}`: {summary}."))
            },
            "diagram_highlight" => {
                let mut shown = self
                    .known(&id)
                    .ok_or_else(|| format!("no diagram `{id}`: show it first"))?;
                let ids = |key: &str| -> Vec<String> {
                    args.get(key)
                        .and_then(Value::as_array)
                        .map(|l| {
                            l.iter()
                                .filter_map(|v| v.as_str().map(str::to_owned))
                                .collect()
                        })
                        .unwrap_or_default()
                };
                shown.highlight = ids("nodes").into_iter().chain(ids("edges")).collect();
                shown.note = text("note").filter(|n| !n.is_empty());
                shown.revision += 1;
                shown.updated = store::now();
                let count = shown.highlight.len();
                self.save(&shown)?;
                Ok(if count == 0 {
                    format!("`{id}`: highlight cleared.")
                } else {
                    format!("`{id}`: {count} highlighted.")
                })
            },
            "diagram_close" => {
                store::remove(&self.dir, &id).map_err(|e| e.to_string())?;
                Ok(format!("`{id}` closed."))
            },
            other => Err(format!("no tool `{other}`")),
        }
    }

    /// The diagram `id` as its file has it now: none once the user closed
    /// it, and one a server before this one showed.
    fn known(&self, id: &str) -> Option<Shown> {
        store::read(&store::path(&self.dir, id)).ok()
    }

    /// Writes `shown` as this server's: its run is now the one to draw it.
    fn save(&self, shown: &Shown) -> Result<(), String> {
        let mut shown = shown.clone();
        shown.run = Some(self.run.clone());
        store::write(&self.dir, &shown).map_err(|e| format!("cannot write the diagram: {e}"))
    }
}

/// A run for a server the app did not name one: unique to this process.
fn session() -> String {
    let micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_micros());
    static SERVERS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = SERVERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("session-{micros}-{}-{serial}", std::process::id())
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn call(server: &mut Server, id: u64, tool: &str, args: Value) -> Value {
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": tool, "arguments": args } });
        server.answer(&line.to_string()).unwrap()["result"].clone()
    }

    #[test]
    fn an_agent_shows_patches_highlights_and_closes() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = Server::new(dir.path(), Some("claude".into()));
        let init = server.answer(r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert!(
            server
                .answer(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .is_none()
        );
        let list = server
            .answer(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
            .unwrap();
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        for tool in [
            "diagram_show",
            "diagram_patch",
            "diagram_highlight",
            "diagram_close",
        ] {
            assert!(names.contains(&tool), "{tool} in {names:?}");
        }

        let shown = call(
            &mut server,
            2,
            "diagram_show",
            json!({ "id": "arch", "title": "Architecture", "mermaid": "flowchart LR\n api --> db" }),
        );
        assert!(shown.get("isError").is_none(), "{shown}");
        let file = store::read(&store::path(dir.path(), "arch")).unwrap();
        assert_eq!(file.title, "Architecture");
        assert_eq!(file.agent.as_deref(), Some("claude"));

        call(
            &mut server,
            3,
            "diagram_patch",
            json!({ "id": "arch", "add_nodes": [{ "id": "cache", "label": "Redis" }], "add_edges": [{ "from": "api", "to": "cache" }] }),
        );
        call(
            &mut server,
            4,
            "diagram_highlight",
            json!({ "id": "arch", "nodes": ["cache"], "note": "new" }),
        );
        let file = store::read(&store::path(dir.path(), "arch")).unwrap();
        assert_eq!(file.revision, 3);
        assert_eq!(file.document["nodes"].as_array().unwrap().len(), 3);
        assert_eq!(file.highlight, vec!["cache"]);

        // A server started again picks the diagram up from its file.
        let mut again = Server::new(dir.path(), None);
        let patched = call(
            &mut again,
            5,
            "diagram_patch",
            json!({ "id": "arch", "remove_nodes": ["cache"] }),
        );
        assert!(patched.get("isError").is_none(), "{patched}");

        // Patched by another session, the diagram is that session's now,
        // and each answer names the run.
        let file = store::read(&store::path(dir.path(), "arch")).unwrap();
        assert_eq!(file.run.as_deref(), Some(again.run.as_str()));
        assert_ne!(again.run, server.run);
        let text = patched["content"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("`arch`: "), "{text}");
        assert!(text.ends_with(&format!("\n[run {}]", again.run)), "{text}");

        call(&mut again, 6, "diagram_close", json!({ "id": "arch" }));
        assert!(!store::path(dir.path(), "arch").exists());
    }

    #[test]
    fn a_diagram_the_user_closed_stays_closed() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = Server::new(dir.path(), None);
        call(
            &mut server,
            1,
            "diagram_show",
            json!({ "id": "arch", "mermaid": "graph TD; a-->b" }),
        );
        // The app's Close removes the file, not the server's memory.
        store::remove(dir.path(), "arch").unwrap();
        for (n, tool, args) in [
            (
                2,
                "diagram_patch",
                json!({ "id": "arch", "remove_nodes": ["b"] }),
            ),
            (
                3,
                "diagram_highlight",
                json!({ "id": "arch", "nodes": ["a"] }),
            ),
        ] {
            assert_eq!(call(&mut server, n, tool, args)["isError"], true, "{tool}");
            assert!(!store::path(dir.path(), "arch").exists(), "{tool}");
        }
    }

    #[test]
    fn mistakes_come_back_to_the_agent_as_tool_errors() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = Server::new(dir.path(), None);
        let bad = call(
            &mut server,
            1,
            "diagram_show",
            json!({ "id": "x", "mermaid": "flowchart TD\n A ->> B" }),
        );
        assert_eq!(bad["isError"], true);
        assert!(
            bad["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("line 2")
        );
        assert_eq!(
            call(
                &mut server,
                2,
                "diagram_show",
                json!({ "id": "../etc", "mermaid": "graph TD; a-->b" })
            )["isError"],
            true
        );
        assert_eq!(
            call(
                &mut server,
                3,
                "diagram_patch",
                json!({ "id": "nope", "set": {} })
            )["isError"],
            true
        );
        let unknown = server
            .answer(r#"{"jsonrpc":"2.0","id":4,"method":"resources/list"}"#)
            .unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
    }
}
