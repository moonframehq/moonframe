//! What the view tells the mod, as messages to its `update`.

use {
    moonframe_core::Data,
    moonframe_diagram::{
        NodeKind, Rect,
        view::{DiagramEvent, Tool},
    },
};

fn number(n: f32) -> Data {
    Data::Number(f64::from(n.round()))
}

fn rect(id: &str, r: &Rect) -> Data {
    Data::map([
        ("id", Data::String(id.to_owned())),
        ("x", number(r.x)),
        ("y", number(r.y)),
        ("w", number(r.w)),
        ("h", number(r.h)),
    ])
}

fn ids(list: &[String]) -> Data {
    Data::List(list.iter().cloned().map(Data::String).collect())
}

/// A kind by the name the Luau editor uses for it.
pub(super) fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Circle => "ellipse",
        NodeKind::Note => "sticky",
        NodeKind::Diamond => "diamond",
        other => other.name(),
    }
}

pub(super) fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "select",
        Tool::Hand => "hand",
        Tool::Connect => "connector",
        Tool::Create(kind) => kind_name(kind),
    }
}

pub(super) fn message(event: &DiagramEvent) -> Data {
    let one = |key: &str, value: Data| Data::map([(key.to_owned(), value)]);
    match event {
        DiagramEvent::Select(list) => one("selected", ids(list)),
        DiagramEvent::Move(moved) => one(
            "moved",
            Data::List(moved.iter().map(|(id, r)| rect(id, r)).collect()),
        ),
        DiagramEvent::Resize { id, rect: r } => one("resized", rect(id, r)),
        DiagramEvent::Connect { from, to } => one(
            "connected",
            Data::map([
                ("from", Data::String(from.clone())),
                ("to", Data::String(to.clone())),
            ]),
        ),
        DiagramEvent::Create { kind, rect: r } => {
            let mut created = rect("", r);
            if let Data::Map(map) = &mut created {
                map.remove("id");
                map.insert("kind".to_owned(), Data::String(kind_name(*kind).to_owned()));
            }
            one("created", created)
        },
        DiagramEvent::Delete(list) => one("deleted", ids(list)),
        DiagramEvent::EditLabel { id, label } => one(
            "edited",
            Data::map([
                ("id", Data::String(id.clone())),
                ("label", Data::String(label.clone())),
            ]),
        ),
        DiagramEvent::Open(id) => one("opened", Data::String(id.clone())),
        DiagramEvent::Zoom(zoom) => one("zoom", Data::Number(f64::from(*zoom))),
        DiagramEvent::Tool(tool) => one("tool", Data::String(tool_name(*tool).to_owned())),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_move_reads_as_the_luau_canvas_sends_it() {
        let msg = message(&DiagramEvent::Move(vec![(
            "a".into(),
            Rect::new(10.4, 20.0, 100.0, 40.0),
        )]));
        let moved = msg.get("moved").unwrap();
        let Data::List(list) = moved else {
            panic!("{moved:?}")
        };
        assert_eq!(list[0].get("x").and_then(Data::as_f64), Some(10.0));
        assert_eq!(list[0].get("id").and_then(Data::as_str), Some("a"));
    }
}
