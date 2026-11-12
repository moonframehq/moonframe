//! What a click on a picked element sends, as its report says it. The
//! values its mod drew it from come from core ([`ViewContext::report`]).
//!
//! [`ViewContext::report`]: moonframe_core::ViewContext::report

use moonframe_core::{Action, Data, Node};

/// A message an element sends, at most.
const MESSAGE: usize = 300;

/// What clicking, or typing in, `node` sends, as the report says it.
pub(super) fn does(node: &Node) -> Option<String> {
    let action = match node {
        Node::Stack(stack) => stack.action.as_ref(),
        Node::Button(button) => button.action.as_ref(),
        Node::Item(item) => item.action.as_ref(),
        Node::Input(input) => {
            let to = input.to.as_deref();
            let sends: Vec<String> = [
                ("change", &input.change),
                ("submit", &input.submit),
                ("blur", &input.blur),
            ]
            .into_iter()
            .filter_map(|(on, msg)| Some(format!("on {on} sends {}", send(to, msg.as_ref()?))))
            .collect();
            return (!sends.is_empty()).then(|| sends.join("; "));
        },
        _ => None,
    }?;
    Some(click(action))
}

fn click(action: &Action) -> String {
    match action {
        Action::Send { to, msg } => format!("a click sends {}", send(to.as_deref(), msg)),
        Action::Command(command) => format!("a click runs the app command {command:?}"),
        Action::Browser { id, command, .. } => {
            format!("a click does {command:?} in the browser {id:?}")
        },
        Action::Window(name) => format!("a click opens the mod {name} in its own window"),
    }
}

fn send(to: Option<&str>, msg: &Data) -> String {
    let to = to.map_or_else(
        || "its mod's update".to_owned(),
        |to| format!("{to}'s update"),
    );
    format!("{} to {to}", msg.to_luau_line(MESSAGE))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "context/tests.rs"]
mod tests;
