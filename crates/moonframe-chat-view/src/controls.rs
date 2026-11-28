//! What waits on the user (permissions, the queue) and the session's
//! controls: pickers, usage, the plan, activities and slash commands.

use {
    crate::{Limits, text, unframe},
    herdr_chat_protocol::{
        ActivityKind, ConfigOption, EntryKind, OptionCategory, OptionKind, PermissionKind,
        PlanPriority, PlanStatus, Question, Transcript, Usage,
    },
    serde::Serialize,
};

/// A question the agent asks before it acts.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PermissionView {
    /// Answered with `{ answer = chat, permission = id, option }`.
    pub id: u64,
    pub title: String,
    /// The call it is about, and that call's own title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub options: Vec<PermissionChoice>,
    /// The question in full, when the agent asked one with choices:
    /// answered with `{ answer = chat, permission = id, answers }`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<QuestionAsked>,
    /// A guest's prompt waiting for the user to allow it, rather than a
    /// question from the agent: `Bob via tma`, and what they wrote.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest_text: Option<String>,
}

/// A question the agent asks with choices.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QuestionAsked {
    /// A short label for it, such as `Tab detail`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    pub text: String,
    /// Several choices may be picked.
    pub multiple: bool,
    pub options: Vec<QuestionChoice>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QuestionChoice {
    /// What is sent back as the answer.
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PermissionChoice {
    pub id: String,
    pub name: String,
    /// `"allow_once" "allow_always" "reject_once" "reject_always" "other"`.
    pub kind: &'static str,
    /// It lets the call run.
    pub allows: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QueuedView {
    /// Taken back with `{ unqueue = id, chat }`.
    pub id: String,
    pub text: String,
    pub context: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    /// Kept when the turn was stopped: it waits to be sent now
    /// (`{ send_now = id, chat }`) or taken back.
    pub held: bool,
    /// Already sent into the running turn, for the agent to take at its
    /// next step; it can no longer be taken back.
    pub steered: bool,
    /// Who wrote it, for a guest's prompt the user allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlanView {
    pub items: Vec<PlanItem>,
    pub done: usize,
    pub total: usize,
    /// The item in progress, else the first pending one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlanItem {
    pub text: String,
    /// `"pending" "running" "done"`.
    pub status: &'static str,
    /// `"high" "medium" "low"`.
    pub priority: &'static str,
}

/// A setting the user picks: the mode, the model, the effort, or another.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Picker {
    /// The setting's id; `"mode"` for the agent's modes.
    pub id: String,
    pub name: String,
    /// How a choice is sent: `"mode"` as `{ mode = choice, chat }`,
    /// `"setting"` as `{ setting = id, value = choice, chat }`.
    pub send: &'static str,
    /// `"select"`, or `"toggle"` (whose value is a boolean).
    pub kind: &'static str,
    pub current: String,
    pub current_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<ChoiceView>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChoiceView {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The heading it is listed under.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UsageView {
    /// Context window tokens used, of `size`.
    pub used: u64,
    pub size: u64,
    pub percent: u8,
    /// `45.2k / 200k`.
    pub text: String,
    /// What the session cost so far, such as `$0.42`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommandView {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// A sub-agent or background shell still running.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ActivityView {
    pub call_id: String,
    /// `"subagent" "shell" "monitor" "other"`.
    pub kind: &'static str,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub started_ms: u64,
    /// The agent can end it alone: `{ stop_activity = call_id, chat }`.
    pub stoppable: bool,
}

/// The most running activities shown.
const MAX_ACTIVITIES: usize = 16;

pub(crate) fn permissions(transcript: &Transcript, limits: &Limits) -> Vec<PermissionView> {
    transcript
        .permissions
        .iter()
        .take(limits.permissions)
        .map(|permission| {
            let tool = permission.call_id.as_deref().and_then(|call| {
                transcript
                    .entries
                    .iter()
                    .rev()
                    .find_map(|entry| match &entry.kind {
                        EntryKind::Tool(tool) if tool.call_id == call => {
                            text::first_line(&tool.title, 500)
                        },
                        _ => None,
                    })
            });
            PermissionView {
                id: permission.id.0,
                title: text::head(&permission.title, limits.text),
                call_id: permission
                    .call_id
                    .as_deref()
                    .map(|call| text::head(call, 256)),
                tool,
                options: permission
                    .options
                    .iter()
                    .take(16)
                    .map(|option| PermissionChoice {
                        id: text::head(&option.id, 256),
                        name: text::head(&option.name, 200),
                        kind: match option.kind {
                            PermissionKind::AllowOnce => "allow_once",
                            PermissionKind::AllowAlways => "allow_always",
                            PermissionKind::RejectOnce => "reject_once",
                            PermissionKind::RejectAlways => "reject_always",
                            PermissionKind::Other => "other",
                        },
                        allows: option.kind.allows(),
                    })
                    .collect(),
                question: permission
                    .question
                    .as_ref()
                    .map(|question| question_asked(question, limits)),
                guest: permission
                    .guest
                    .as_ref()
                    .map(|prompt| crate::rows::guest_name(&prompt.guest)),
                guest_text: permission
                    .guest
                    .as_ref()
                    .map(|prompt| text::head(&prompt.text, limits.text)),
            }
        })
        .collect()
}

fn question_asked(question: &Question, limits: &Limits) -> QuestionAsked {
    QuestionAsked {
        header: question
            .header
            .as_deref()
            .map(|header| text::head(header, 200)),
        text: text::head(&question.text, limits.text),
        multiple: question.multiple,
        options: question
            .options
            .iter()
            .take(32)
            .map(|option| QuestionChoice {
                // Sent back as it came: a cut label would not match.
                label: option.label.clone(),
                description: option
                    .description
                    .as_deref()
                    .map(|description| text::head(description, 500)),
            })
            .collect(),
    }
}

pub(crate) fn queue(transcript: &Transcript, limits: &Limits) -> Vec<QueuedView> {
    transcript
        .queued
        .iter()
        .take(limits.queue)
        .map(|queued| {
            let (context, prompt) = unframe(&queued.text);
            QueuedView {
                id: queued.id.0.to_string(),
                text: text::head(prompt, limits.text),
                context: context.is_some(),
                files: queued
                    .attachments
                    .iter()
                    .map(|file| text::head(file.name(), 512))
                    .collect(),
                held: queued.held,
                steered: queued.steered,
                guest: queued.guest.as_ref().map(crate::rows::guest_name),
            }
        })
        .collect()
}

pub(crate) fn plan(transcript: &Transcript, limits: &Limits) -> Option<PlanView> {
    if transcript.plan.is_empty() {
        return None;
    }
    let items: Vec<PlanItem> = transcript
        .plan
        .iter()
        .take(limits.plan)
        .map(|entry| PlanItem {
            text: text::head(&entry.content, limits.text.min(2048)),
            status: match entry.status {
                PlanStatus::Pending => "pending",
                PlanStatus::InProgress => "running",
                PlanStatus::Completed => "done",
            },
            priority: match entry.priority {
                PlanPriority::High => "high",
                PlanPriority::Medium => "medium",
                PlanPriority::Low => "low",
            },
        })
        .collect();
    let current = transcript
        .plan
        .iter()
        .find(|entry| entry.status == PlanStatus::InProgress)
        .or_else(|| {
            transcript
                .plan
                .iter()
                .find(|entry| entry.status == PlanStatus::Pending)
        })
        .map(|entry| text::head(&entry.content, 300));
    Some(PlanView {
        done: transcript
            .plan
            .iter()
            .filter(|entry| entry.status == PlanStatus::Completed)
            .count(),
        total: transcript.plan.len(),
        items,
        current,
    })
}

/// The agent's modes, else a setting of the mode category.
pub(crate) fn mode(transcript: &Transcript, limits: &Limits) -> Option<Picker> {
    let Some(modes) = &transcript.modes else {
        return option(transcript, OptionCategory::Mode, limits);
    };
    if modes.available.is_empty() {
        return None;
    }
    Some(Picker {
        id: "mode".to_owned(),
        name: "Mode".to_owned(),
        send: "mode",
        kind: "select",
        current: modes.current.clone(),
        current_name: modes
            .current()
            .map_or_else(|| modes.current.clone(), |mode| mode.name.clone()),
        description: None,
        choices: modes
            .available
            .iter()
            .take(limits.choices)
            .map(|mode| ChoiceView {
                id: mode.id.clone(),
                name: text::head(&mode.name, 200),
                description: mode
                    .description
                    .as_deref()
                    .map(|description| text::head(description, 500)),
                group: None,
            })
            .collect(),
    })
}

/// The first setting of `category`.
pub(crate) fn option(
    transcript: &Transcript,
    category: OptionCategory,
    limits: &Limits,
) -> Option<Picker> {
    transcript
        .option(category)
        .map(|option| picker(option, limits))
}

/// Every setting no picker of its own shows.
pub(crate) fn settings(transcript: &Transcript, limits: &Limits) -> Vec<Picker> {
    let mut shown: Vec<&str> = [OptionCategory::Model, OptionCategory::ThoughtLevel]
        .into_iter()
        .filter_map(|category| transcript.option(category))
        .map(|option| option.id.as_str())
        .collect();
    if transcript.modes.is_none()
        && let Some(mode) = transcript.option(OptionCategory::Mode)
    {
        shown.push(&mode.id);
    }
    transcript
        .options
        .iter()
        // With the agent's own modes, a mode setting would be a second one.
        .filter(|option| !(transcript.modes.is_some() && option.category == OptionCategory::Mode))
        .filter(|option| !shown.contains(&option.id.as_str()))
        .take(16)
        .map(|option| picker(option, limits))
        .collect()
}

fn picker(option: &ConfigOption, limits: &Limits) -> Picker {
    let (kind, current, choices) = match &option.kind {
        OptionKind::Select { choices, current } => (
            "select",
            current.clone(),
            choices
                .iter()
                .take(limits.choices)
                .map(|choice| ChoiceView {
                    id: choice.id.clone(),
                    name: text::head(&choice.name, 200),
                    description: choice
                        .description
                        .as_deref()
                        .map(|description| text::head(description, 500)),
                    group: choice.group.as_deref().map(|group| text::head(group, 200)),
                })
                .collect(),
        ),
        OptionKind::Toggle { current } => ("toggle", current.to_string(), Vec::new()),
    };
    Picker {
        id: option.id.clone(),
        name: text::head(&option.name, 200),
        send: "setting",
        kind,
        current,
        current_name: text::head(&option.current_name(), 200),
        description: option
            .description
            .as_deref()
            .map(|description| text::head(description, 500)),
        choices,
    }
}

pub(crate) fn usage(usage: &Usage) -> UsageView {
    UsageView {
        used: usage.used,
        size: usage.size,
        percent: usage.percent(),
        text: format!(
            "{} / {}",
            text::tokens(usage.used),
            text::tokens(usage.size)
        ),
        cost: usage.cost.as_ref().map(|cost| {
            let amount = format!(
                "{}.{:02}",
                cost.micros / 1_000_000,
                cost.micros % 1_000_000 / 10_000
            );
            match cost.currency.as_str() {
                "USD" => format!("${amount}"),
                "EUR" => format!("€{amount}"),
                "GBP" => format!("£{amount}"),
                other => format!("{amount} {}", text::head(other, 8)),
            }
        }),
    }
}

pub(crate) fn commands(transcript: &Transcript, limits: &Limits) -> Vec<CommandView> {
    transcript
        .commands
        .iter()
        .take(limits.commands)
        .map(|command| CommandView {
            name: text::head(command.name.trim_start_matches('/'), 100),
            description: text::head(&command.description, 500),
            hint: command.hint.as_deref().map(|hint| text::head(hint, 200)),
        })
        .collect()
}

pub(crate) fn activities(transcript: &Transcript) -> Vec<ActivityView> {
    transcript
        .activities
        .iter()
        .filter(|activity| activity.is_running())
        .take(MAX_ACTIVITIES)
        .map(|activity| ActivityView {
            call_id: text::head(&activity.call_id, 256),
            kind: match activity.kind {
                ActivityKind::Subagent => "subagent",
                ActivityKind::Shell => "shell",
                ActivityKind::Monitor => "monitor",
                ActivityKind::Other => "other",
            },
            label: text::head(&activity.label, 300),
            detail: activity
                .detail
                .as_deref()
                .map(|detail| text::head(detail, 300)),
            started_ms: activity.started_ms,
            stoppable: activity.stoppable,
        })
        .collect()
}
