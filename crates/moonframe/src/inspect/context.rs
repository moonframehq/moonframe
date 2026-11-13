//! What a click on a picked element sends, as its report says it. The
//! values its mod drew it from come from core ([`ViewContext::report`]).
//!
//! [`ViewContext::report`]: moonframe_core::ViewContext::report

use moonframe_core::{Action, Data, Input, Node};

/// A message an element sends, at most.
const MESSAGE: usize = 300;
/// What an input's message holds in place of its text and caret.
const TEXT: &str = "<text>";
const CARET: &str = "<caret byte>";

/// What clicking, or typing in, `node` sends, as the report says it.
pub(super) fn does(node: &Node) -> Option<String> {
    let action = match node {
        Node::Stack(stack) => stack.action.as_ref(),
        Node::Button(button) => button.action.as_ref(),
        Node::Item(item) => item.action.as_ref(),
        Node::Input(input) => return typing(input),
        _ => None,
    }?;
    Some(click(action))
}

/// What an input sends as it is typed in, submitted and left, in the
/// shape `update` gets: the host adds the text as `value` (wrapping a
/// message that is not a table as `{ msg, value }`), and on change where
/// the caret is.
fn typing(input: &Input) -> Option<String> {
    typed(input.to.as_deref(), [
        ("change", input.change.as_ref()),
        ("submit", input.submit.as_ref()),
        ("blur", input.blur.as_ref()),
    ])
}

/// [`typing`], from the mod the field sends `to` and its messages by event.
fn typed(to: Option<&str>, events: [(&str, Option<&Data>); 3]) -> Option<String> {
    let sends: Vec<String> = events
        .into_iter()
        .filter_map(|(on, msg)| {
            let mut sent = Input::message(msg?, TEXT);
            if on == "change"
                && let Data::Map(fields) = &mut sent
            {
                fields.insert("caret".to_owned(), Data::from(CARET));
            }
            Some(format!("on {on} sends {}", send(to, &sent)))
        })
        .collect();
    (!sends.is_empty()).then(|| {
        format!(
            "{} ({TEXT} and {CARET} stand for the field's own)",
            sends.join("; ")
        )
    })
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
