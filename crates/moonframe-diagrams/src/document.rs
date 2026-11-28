//! A diagram as JSON, in the shape `ui.diagram`'s `document` prop takes, so
//! what the provider publishes goes to the view as it is: from Mermaid or
//! DOT text, checked when an agent gives one directly, and patched.

use {
    moonframe_diagram::{
        Diagram, DiagramKind, Direction, EdgeKind, Head, MessageKind, Tone,
        parse::{self, MOST_EDGES, MOST_NODES},
    },
    serde_json::{Map, Value, json},
};

fn tone(t: Option<Tone>) -> Value {
    match t {
        None => Value::Null,
        Some(Tone::Accent) => "accent".into(),
        Some(Tone::Success) => "success".into(),
        Some(Tone::Warning) => "warning".into(),
        Some(Tone::Danger) => "danger".into(),
        Some(Tone::Muted) => "muted".into(),
    }
}

fn head(h: Head) -> &'static str {
    match h {
        Head::None => "none",
        Head::Arrow => "arrow",
        Head::Open => "open",
        Head::Circle => "circle",
        Head::Cross => "cross",
        Head::Triangle => "triangle",
        Head::Diamond => "diamond",
        Head::HollowDiamond => "hollow-diamond",
    }
}

/// Drops the keys whose value is null, so the JSON stays small.
fn compact(mut value: Value) -> Value {
    if let Value::Object(map) = &mut value {
        map.retain(|_, v| !v.is_null());
    }
    value
}

/// The model as a document.
pub fn from_model(d: &Diagram) -> Value {
    let nodes: Vec<Value> = d
        .nodes
        .iter()
        .map(|n| {
            compact(json!({
                "id": n.id, "label": n.label, "sublabel": n.sublabel, "kind": n.kind.name(),
                "tone": tone(n.tone), "fill": n.fill, "group": n.group,
            }))
        })
        .collect();
    let edges: Vec<Value> = d
        .edges
        .iter()
        .map(|e| {
            let kind = match e.kind {
                EdgeKind::Solid => "solid",
                EdgeKind::Dashed => "dashed",
                EdgeKind::Dotted => "dotted",
                EdgeKind::Thick => "thick",
                EdgeKind::Invisible => "invisible",
            };
            compact(json!({
                "id": e.id, "from": e.from, "to": e.to, "label": e.label, "kind": kind,
                "start": head(e.start), "end": head(e.end), "tone": tone(e.tone),
            }))
        })
        .collect();
    let groups: Vec<Value> = d
        .groups
        .iter()
        .map(|g| {
            compact(
                json!({ "id": g.id, "label": g.label, "parent": g.parent, "tone": tone(g.tone) }),
            )
        })
        .collect();
    let messages: Vec<Value> = d
        .messages
        .iter()
        .map(|m| {
            let kind = match m.kind {
                MessageKind::Sync => "sync",
                MessageKind::Async => "async",
                MessageKind::Reply => "reply",
            };
            json!({ "from": m.from, "to": m.to, "label": m.label, "kind": kind })
        })
        .collect();
    let notes: Vec<Value> = d
        .notes
        .iter()
        .map(|n| json!({ "over": n.over, "text": n.text, "before": n.before }))
        .collect();
    let direction = match d.direction {
        Direction::Down => "down",
        Direction::Up => "up",
        Direction::Right => "right",
        Direction::Left => "left",
    };
    let kind = if d.kind == DiagramKind::Sequence {
        "sequence"
    } else {
        "flow"
    };
    let mut doc = json!({ "kind": kind, "direction": direction, "nodes": nodes, "edges": edges });
    for (key, list) in [("groups", groups), ("messages", messages), ("notes", notes)] {
        if !list.is_empty() {
            doc[key] = Value::Array(list);
        }
    }
    doc
}

/// Mermaid or DOT text as a document, or why not.
pub fn from_text(language: &str, text: &str) -> Result<Value, String> {
    match parse::fenced(language, text) {
        Some(Ok(diagram)) => Ok(from_model(&diagram)),
        Some(Err(e)) => Err(e.to_string()),
        None => Err(format!(
            "`{language}` is not a diagram language (mermaid or dot)"
        )),
    }
}

/// A document an agent gave as a graph, held to what a diagram may be:
/// lists where lists go, every node with an `id`, within the limits.
pub fn check(doc: &Value) -> Result<Value, String> {
    let Value::Object(map) = doc else {
        return Err("a graph is an object with `nodes` and `edges`".into());
    };
    let list = |key: &str| match map.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => Ok(items.clone()),
        Some(_) => Err(format!("`{key}` must be a list")),
    };
    let nodes = list("nodes")?;
    let edges = list("edges")?;
    if nodes.len() > MOST_NODES || edges.len() + list("messages")?.len() > MOST_EDGES {
        return Err(format!(
            "more than {MOST_NODES} nodes or {MOST_EDGES} edges"
        ));
    }
    for (i, node) in nodes.iter().enumerate() {
        if !node.get("id").is_some_and(Value::is_string) {
            return Err(format!("node {} has no `id`", i + 1));
        }
    }
    for (i, edge) in edges.iter().enumerate() {
        if !(edge.get("from").is_some_and(Value::is_string)
            && edge.get("to").is_some_and(Value::is_string))
        {
            return Err(format!("edge {} needs `from` and `to`", i + 1));
        }
    }
    let mut out = Map::new();
    for key in [
        "kind",
        "direction",
        "nodes",
        "edges",
        "groups",
        "messages",
        "notes",
    ] {
        if let Some(value) = map.get(key).filter(|v| !v.is_null()) {
            out.insert(key.to_owned(), value.clone());
        }
    }
    Ok(Value::Object(out))
}

/// Applies `ops` to `doc`: `add_nodes`, `add_edges`, `add_groups` (a node
/// or group with an id already there replaces it), `remove_nodes` (and
/// their edges), `remove_edges` (by id, or `from`/`to`), and `set`, fields
/// to change on nodes by id. Returns what changed, for the agent.
pub fn patch(doc: &mut Value, ops: &Value) -> Result<String, String> {
    if !doc.is_object() {
        *doc = json!({ "nodes": [], "edges": [] });
    }
    let mut changed = Vec::new();
    let list = |key: &str| {
        ops.get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let id_of = |v: &Value| v.get("id").and_then(Value::as_str).map(str::to_owned);
    for (key, ops_key) in [("nodes", "add_nodes"), ("groups", "add_groups")] {
        let added = list(ops_key);
        if added.is_empty() {
            continue;
        }
        let items = doc[key].as_array().cloned().unwrap_or_default();
        let mut items: Vec<Value> = items;
        for item in &added {
            let Some(id) = id_of(item) else {
                return Err(format!("every one of `{ops_key}` needs an `id`"));
            };
            match items
                .iter_mut()
                .find(|n| id_of(n).as_deref() == Some(id.as_str()))
            {
                Some(known) => *known = item.clone(),
                None => items.push(item.clone()),
            }
        }
        changed.push(format!("{} {key} added", added.len()));
        doc[key] = Value::Array(items);
    }
    let added_edges = list("add_edges");
    if !added_edges.is_empty() {
        let mut edges = doc["edges"].as_array().cloned().unwrap_or_default();
        for edge in added_edges.iter() {
            if !(edge.get("from").is_some_and(Value::is_string)
                && edge.get("to").is_some_and(Value::is_string))
            {
                return Err("every one of `add_edges` needs `from` and `to`".into());
            }
            let mut edge = edge.clone();
            if id_of(&edge).is_none() {
                edge["id"] = Value::String(format!("e{}", edges.len()));
            }
            edges.push(edge);
        }
        changed.push(format!("{} edges added", added_edges.len()));
        doc["edges"] = Value::Array(edges);
    }
    let removed: Vec<String> = list("remove_nodes")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    if !removed.is_empty() {
        let keep = |v: &Value, key: &str| {
            !v.get(key)
                .and_then(Value::as_str)
                .is_some_and(|id| removed.iter().any(|r| r == id))
        };
        if let Some(nodes) = doc["nodes"].as_array_mut() {
            nodes.retain(|n| keep(n, "id"));
        }
        if let Some(edges) = doc["edges"].as_array_mut() {
            edges.retain(|e| keep(e, "from") && keep(e, "to"));
        }
        changed.push(format!("{} nodes removed", removed.len()));
    }
    let gone_edges = list("remove_edges");
    if !gone_edges.is_empty() {
        let matches = |edge: &Value, gone: &Value| match gone {
            Value::String(id) => edge.get("id").and_then(Value::as_str) == Some(id.as_str()),
            Value::Object(_) => {
                edge.get("from") == gone.get("from") && edge.get("to") == gone.get("to")
            },
            _ => false,
        };
        if let Some(edges) = doc["edges"].as_array_mut() {
            edges.retain(|e| !gone_edges.iter().any(|g| matches(e, g)));
        }
        changed.push(format!("{} edges removed", gone_edges.len()));
    }
    if let Some(Value::Object(sets)) = ops.get("set") {
        let nodes = doc["nodes"]
            .as_array_mut()
            .ok_or("the diagram has no nodes to set")?;
        for (id, fields) in sets {
            let Some(node) = nodes
                .iter_mut()
                .find(|n| n.get("id").and_then(Value::as_str) == Some(id.as_str()))
            else {
                return Err(format!("no node `{id}` to set"));
            };
            if let (Value::Object(node), Value::Object(fields)) = (node, fields) {
                for (key, value) in fields {
                    if key != "id" {
                        node.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        changed.push(format!("{} nodes changed", sets.len()));
    }
    check(doc)?;
    if changed.is_empty() {
        return Err("nothing to change: give add_nodes, add_edges, add_groups, remove_nodes, remove_edges or set".into());
    }
    Ok(changed.join(", "))
}

/// Roughly how many values `value` holds: what a publish is counted in.
pub fn weight(value: &Value) -> usize {
    match value {
        Value::Array(items) => 1 + items.iter().map(weight).sum::<usize>(),
        Value::Object(map) => 1 + map.values().map(weight).sum::<usize>(),
        _ => 1,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn mermaid_becomes_a_document_ui_diagram_takes() {
        let doc = from_text("mermaid", "flowchart LR\n A[Start] -->|go| B{Done?}").unwrap();
        assert_eq!(doc["direction"], "right");
        assert_eq!(doc["nodes"][1]["kind"], "diamond");
        assert_eq!(doc["edges"][0]["label"], "go");
        assert!(from_text("mermaid", "pie\n \"a\": 1").is_err());
    }

    #[test]
    fn a_patch_adds_removes_and_sets() {
        let mut doc = from_text("mermaid", "flowchart TD\n a --> b").unwrap();
        let summary = patch(
            &mut doc,
            &json!({
                "add_nodes": [{ "id": "c", "label": "C" }],
                "add_edges": [{ "from": "b", "to": "c" }],
                "set": { "a": { "tone": "success" } },
            }),
        )
        .unwrap();
        assert!(summary.contains("1 nodes added"), "{summary}");
        assert_eq!(doc["nodes"].as_array().unwrap().len(), 3);
        assert_eq!(doc["nodes"][0]["tone"], "success");
        patch(&mut doc, &json!({ "remove_nodes": ["b"] })).unwrap();
        assert_eq!(
            doc["edges"].as_array().unwrap().len(),
            0,
            "b's edges went with it"
        );
        assert!(patch(&mut doc, &json!({ "set": { "zz": { "label": "x" } } })).is_err());
        assert!(patch(&mut doc, &json!({})).is_err());
    }

    #[test]
    fn a_graph_is_checked() {
        assert!(check(&json!({ "nodes": [{ "id": "a" }], "edges": [] })).is_ok());
        assert!(check(&json!({ "nodes": [{ "label": "no id" }] })).is_err());
        assert!(check(&json!({ "nodes": "a" })).is_err());
        assert!(check(&json!({ "edges": [{ "from": "a" }] })).is_err());
    }
}
