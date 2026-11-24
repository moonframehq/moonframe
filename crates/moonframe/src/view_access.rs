//! `moonframe.view`, the capability that lets a provider read a mod's view
//! and act on it as a person would: `tree` answers the view in the front
//! main window as roles and names, each element with a `ref`; `act` clicks
//! one, fills a field, presses a key or chooses an option through the
//! element's own messages, then answers the view again with what the mod
//! reported wrong. The app's own providers reach any mod; a miniapp's
//! provider only its own namespace's.

use {
    moonframe_core::{ViewAct, ViewNode, ViewTree},
    moonframe_providers::{Bounds, Caller, HostCapability},
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

/// The capability's name, in provider.toml's `capabilities`.
pub(crate) const NAME: &str = "moonframe.view";

/// How long the window has for each step of a call.
const WAIT: Duration = Duration::from_secs(5);

/// The bounds of a call: an act and a read, each with its wait.
pub(crate) fn bounds() -> Bounds {
    Bounds {
        max_bytes: 2 * 1024 * 1024,
        timeout: 2 * WAIT + Duration::from_secs(1),
    }
}

/// `tree`'s arguments.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Look {
    #[serde(rename = "mod")]
    name: String,
}

/// `act`'s arguments.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Act {
    #[serde(rename = "mod")]
    name: String,
    #[serde(rename = "ref")]
    reference: String,
    action: String,
    /// The text to fill, the key to press or the option to choose.
    #[serde(default)]
    value: Option<String>,
}

impl Act {
    fn act(&self) -> Result<ViewAct, String> {
        let value = || {
            self.value
                .clone()
                .ok_or_else(|| format!("`{}` needs a `value`", self.action))
        };
        Ok(match self.action.as_str() {
            "click" => ViewAct::Click,
            "fill" => ViewAct::Fill(value()?),
            "press" => ViewAct::Press(value()?),
            "select" => ViewAct::Select(value()?),
            other => {
                return Err(format!(
                    "`{other}` is no action; use click, fill, press or select"
                ));
            },
        })
    }
}

/// What `tree` and `act` answer.
#[derive(Debug, Serialize)]
struct Answer {
    #[serde(rename = "mod")]
    name: String,
    /// One line per element: `button "Save" [e0.3]`, indented by depth.
    tree: String,
    nodes: Vec<Element>,
    truncated: bool,
    /// What the mod reported wrong, as `name: problem`.
    problems: Vec<String>,
}

/// One element, as `nodes` lists it.
#[derive(Debug, Serialize)]
struct Element {
    #[serde(rename = "ref")]
    reference: String,
    role: &'static str,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    selected: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    disabled: bool,
    depth: usize,
}

impl Answer {
    fn new(name: String, tree: &ViewTree, problems: Vec<String>) -> Self {
        Self {
            name,
            tree: tree.lines(),
            nodes: tree.nodes.iter().map(Element::from).collect(),
            truncated: tree.truncated,
            problems,
        }
    }
}

impl From<&ViewNode> for Element {
    fn from(node: &ViewNode) -> Self {
        Self {
            reference: node.reference.clone(),
            role: node.role,
            name: node.name.clone(),
            value: node.value.clone(),
            selected: node.selected,
            disabled: node.disabled,
            depth: node.depth,
        }
    }
}

/// Whether `caller` may reach the mod `name`: the app's own providers any,
/// a miniapp's only those of its namespace.
fn reaches(caller: &Caller, name: &str) -> Result<(), String> {
    match &caller.namespace {
        None => Ok(()),
        Some(namespace)
            if name
                .strip_prefix(namespace.as_str())
                .is_some_and(|rest| rest.starts_with(':')) =>
        {
            Ok(())
        },
        Some(namespace) => Err(format!(
            "a `{namespace}` provider reaches only `{namespace}:` mods, not `{name}`"
        )),
    }
}

/// The capability providers call.
pub(crate) struct View;

impl HostCapability for View {
    fn call(&self, caller: &Caller, method: &str, args: &str) -> Result<String, String> {
        let answer = match method {
            "tree" => {
                let look: Look = serde_json::from_str(args)
                    .map_err(|error| format!("tree's arguments: {error}"))?;
                reaches(caller, &look.name)?;
                read(look.name)?
            },
            "act" => {
                let asked: Act = serde_json::from_str(args)
                    .map_err(|error| format!("act's arguments: {error}"))?;
                reaches(caller, &asked.name)?;
                let act = asked.act()?;
                let (name, reference) = (asked.name.clone(), asked.reference);
                crate::app::on_front(WAIT, move |app, window, cx| {
                    app.act_on_view(&name, &reference, &act, window, cx)
                })?;
                read(asked.name)?
            },
            other => {
                return Err(format!(
                    "`{NAME}` has no method `{other}`; it has `tree` and `act`"
                ));
            },
        };
        serde_json::to_string(&answer).map_err(|error| error.to_string())
    }
}

/// The mod `name`'s view in the front main window, and its problems.
fn read(name: String) -> Result<Answer, String> {
    crate::app::on_front(WAIT, move |app, _, _| {
        let tree = app.workspace.view_tree(&name)?;
        let problems = app.view_problems(&name);
        Ok(Answer::new(name, &tree, problems))
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn caller(namespace: Option<&str>) -> Caller {
        Caller {
            provider: "test".to_owned(),
            folder: "/p".into(),
            root: "/".into(),
            namespace: namespace.map(str::to_owned),
            capabilities: vec![NAME.to_owned()],
        }
    }

    #[test]
    fn a_miniapps_provider_reaches_only_its_own_mods() {
        assert!(reaches(&caller(None), "settings").is_ok());
        let miniapp = caller(Some("acme"));
        assert!(reaches(&miniapp, "acme:garden").is_ok());
        assert!(reaches(&miniapp, "acme:garden#2").is_ok());
        for other in ["settings", "acmex:garden", "acme"] {
            assert!(
                reaches(&miniapp, other)
                    .unwrap_err()
                    .contains("reaches only `acme:`"),
                "{other}"
            );
        }
        let refused = View
            .call(&miniapp, "tree", r#"{ "mod": "settings" }"#)
            .unwrap_err();
        assert!(refused.contains("reaches only"), "{refused}");
    }

    #[test]
    fn acts_are_read_from_their_arguments() {
        let act = |args: &str| {
            serde_json::from_str::<Act>(args)
                .map_err(|error| error.to_string())
                .and_then(|asked| asked.act())
        };
        assert_eq!(
            act(r#"{ "mod": "m", "ref": "e0", "action": "click" }"#),
            Ok(ViewAct::Click)
        );
        assert_eq!(
            act(r#"{ "mod": "m", "ref": "e0", "action": "fill", "value": "hi" }"#),
            Ok(ViewAct::Fill("hi".to_owned()))
        );
        assert!(
            act(r#"{ "mod": "m", "ref": "e0", "action": "press" }"#)
                .unwrap_err()
                .contains("needs a `value`")
        );
        assert!(
            act(r#"{ "mod": "m", "ref": "e0", "action": "hover" }"#)
                .unwrap_err()
                .contains("no action")
        );
        assert!(
            View.call(&caller(None), "look", "{}")
                .unwrap_err()
                .contains("`tree` and `act`")
        );
    }
}
