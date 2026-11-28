//! The herdr command each message a mod sends asks for.

use {
    herdr_chat_protocol::{
        Agent, Attachment, Command, Device, OptionValue, Origin, PermissionId, Via,
    },
    serde_json::{Value, json},
};

/// The longest prompt a mod may send.
const MAX_PROMPT: usize = 256 * 1024;
/// The longest context a mod may put before a prompt.
pub(crate) const MAX_CONTEXT: usize = 64 * 1024;

/// The actions a message may ask for, by the key that names each; the
/// first key a message carries is its action.
pub(crate) const ACTIONS: &[&str] = &[
    "tool_output",
    "start",
    "prompt",
    "cancel",
    "answer",
    "mode",
    "setting",
    "unqueue",
    "send_now",
    "stop_activity",
    "agent",
    "auto_accept",
    "resume",
    "close",
];

/// The chat a command message is for: `cancel` and `answer` name it in
/// their own key, the others in `chat`.
pub(crate) fn chat_of<'a>(action: &str, message: &'a Value) -> Option<&'a str> {
    let key = match action {
        "cancel" | "answer" => action,
        _ => "chat",
    };
    message.get(key).and_then(Value::as_str)
}

/// The herdr command a message asks for.
pub(crate) fn command(
    action: &str,
    message: &Value,
    context: Option<&str>,
) -> Result<Command, String> {
    let text = |key: &str| message.get(key).and_then(Value::as_str);
    Ok(match action {
        "prompt" => prompt(message, context)?,
        "cancel" => Command::Cancel,
        "answer" => answer(message)?,
        "mode" => Command::SetMode {
            id: text("mode").unwrap_or_default().to_owned(),
        },
        "setting" => Command::SetOption {
            id: text("setting").unwrap_or_default().to_owned(),
            value: match message.get("value") {
                Some(Value::Bool(value)) => OptionValue::Toggle { value: *value },
                Some(Value::String(id)) => OptionValue::Choice { id: id.clone() },
                _ => return Err("a setting's value is a choice's id or a boolean".to_owned()),
            },
        },
        "unqueue" => Command::Unqueue {
            id: queued(text("unqueue"))?,
        },
        "send_now" => Command::SendNow {
            id: queued(text("send_now"))?,
        },
        "stop_activity" => Command::StopActivity {
            call_id: text("stop_activity")
                .filter(|call| !call.is_empty())
                .ok_or("which activity?")?
                .to_owned(),
        },
        "agent" => {
            let name = text("agent").unwrap_or_default();
            Command::SetAgent {
                agent: serde_json::from_value::<Agent>(json!(name))
                    .map_err(|_| format!("`{name}` is not an agent the chat knows"))?,
            }
        },
        "auto_accept" => Command::SetAutoAccept {
            on: message
                .get("auto_accept")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        _ => return Err(format!("unknown action `{action}`")),
    })
}

/// A prompt, with the app's context framed before it and the files it
/// carries; typed in this app, on the machine the agent runs on.
fn prompt(message: &Value, context: Option<&str>) -> Result<Command, String> {
    let text = message
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err("nothing to send".to_owned());
    }
    if text.len() > MAX_PROMPT {
        return Err(format!("a prompt is at most {MAX_PROMPT} bytes"));
    }
    let text = match context {
        Some(context) => moonframe_chat_view::frame(context, text),
        None => text.to_owned(),
    };
    let attachments = message
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|path| Attachment {
            path: path.to_owned(),
        })
        .collect();
    Ok(Command::Prompt {
        text,
        attachments,
        origin: Some(Origin::new(Device::Desktop, Via::Local)),
    })
}

/// An answer to a permission request: one of its options, or, to a
/// question, `answers`, the options' labels or the user's own words.
fn answer(message: &Value) -> Result<Command, String> {
    let permission = PermissionId(
        message
            .get("permission")
            .and_then(Value::as_u64)
            .ok_or("which permission?")?,
    );
    if let Some(answers) = message.get("answers") {
        let answers: Vec<String> = answers
            .as_array()
            .ok_or("`answers` is a list of texts")?
            .iter()
            .map(|answer| answer.as_str().map(str::to_owned))
            .collect::<Option<_>>()
            .ok_or("`answers` is a list of texts")?;
        return Ok(Command::AnswerQuestion {
            answers,
            permission,
        });
    }
    Ok(Command::Answer {
        option: message
            .get("option")
            .and_then(Value::as_str)
            .ok_or("which option?")?
            .to_owned(),
        permission,
    })
}

/// A queued prompt's id, as the view gives it.
fn queued(id: Option<&str>) -> Result<herdr_chat_protocol::CommandId, String> {
    serde_json::from_value(json!(id.unwrap_or_default()))
        .map_err(|_| "no such queued prompt".to_owned())
}

/// A message's `context`, at most [`MAX_CONTEXT`] bytes.
pub(crate) fn context_of(message: &Value) -> Result<Option<String>, String> {
    match message.get("context") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(context)) if context.len() > MAX_CONTEXT => {
            Err(format!("a context is at most {MAX_CONTEXT} bytes"))
        },
        Some(Value::String(context)) => Ok(Some(context.clone()).filter(|c| !c.trim().is_empty())),
        Some(_) => Err("a context is text".to_owned()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
