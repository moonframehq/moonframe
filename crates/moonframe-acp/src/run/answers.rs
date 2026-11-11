//! Reading what agents send over ACP: who they are, the model their
//! session uses, the files a tool call names, and the answers to their
//! permission questions.

use {
    acp::v1 as wire,
    serde_json::{Value, json},
    std::path::PathBuf,
};

/// Who the agent says it is, from its `initialize` answer.
pub(super) fn agent_info(value: &Value) -> Option<Value> {
    let info = value.get("agentInfo")?;
    let text = |key: &str| {
        info.get(key)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
    };
    let name = text("name")?;
    Some(json!({ "name": name, "title": text("title"), "version": text("version") }))
}

/// The model a session says it uses, by its name: the select option of the
/// `model` category in `configOptions`, else the unstable `models` state.
pub(super) fn model_name(value: &Value) -> Option<String> {
    let named = |current: &str, choices: &[&Value]| {
        choices
            .iter()
            .find(|choice| {
                choice
                    .get("value")
                    .or_else(|| choice.get("modelId"))
                    .and_then(Value::as_str)
                    == Some(current)
            })
            .and_then(|choice| choice.get("name").and_then(Value::as_str))
            .unwrap_or(current)
            .to_owned()
    };
    let options = value.get("configOptions").and_then(Value::as_array);
    for option in options.into_iter().flatten() {
        if option.get("category").and_then(Value::as_str) != Some("model") {
            continue;
        }
        let Some(current) = option.get("currentValue").and_then(Value::as_str) else {
            continue;
        };
        // Choices, or groups of them.
        let mut choices = Vec::new();
        for choice in option
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            match choice.get("options").and_then(Value::as_array) {
                Some(group) => choices.extend(group),
                None => choices.push(choice),
            }
        }
        return Some(named(current, &choices));
    }
    let models = value.get("models")?;
    let current = models.get("currentModelId").and_then(Value::as_str)?;
    let choices: Vec<&Value> = models
        .get("availableModels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect();
    Some(named(current, &choices))
}

/// The files a tool call's fields name.
pub(super) fn fields_paths(fields: &wire::ToolCallUpdateFields) -> Vec<PathBuf> {
    crate::guard::paths(
        fields.content.as_deref().unwrap_or_default(),
        fields.locations.as_deref().unwrap_or_default(),
        fields.raw_input.as_ref(),
    )
}

pub(super) fn selected(option: &str) -> Value {
    json!({ "outcome": { "outcome": "selected", "optionId": option } })
}

pub(super) fn cancelled() -> Value {
    json!({ "outcome": { "outcome": "cancelled" } })
}

/// A wire enum's name as the protocol spells it (`in_progress`).
pub(super) fn enum_name(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn the_model_is_named_from_either_way_an_agent_says_it() {
        // An unstable `models` state, as some adapters send.
        let models = json!({ "models": {
            "currentModelId": "default",
            "availableModels": [{ "modelId": "default", "name": "Default (Opus)" }],
        } });
        assert_eq!(model_name(&models).as_deref(), Some("Default (Opus)"));
        // A grouped select, whose value has no name: the value.
        let grouped = json!({ "configOptions": [
            { "id": "mode", "category": "mode", "type": "select", "currentValue": "ask", "options": [] },
            { "id": "model", "category": "model", "type": "select", "currentValue": "x",
              "options": [{ "group": "g", "name": "G", "options": [{ "value": "y", "name": "Y" }] }] },
        ] });
        assert_eq!(model_name(&grouped).as_deref(), Some("x"));
        assert_eq!(model_name(&json!({})), None);
        assert!(agent_info(&json!({ "agentInfo": { "name": "" } })).is_none());
    }
}
