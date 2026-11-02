//! Mods and components mounted with state of their own, and what they
//! offer a tab bar.

use super::*;

/// What a component or mod offers a tab bar: the `+` menu lists it, and a
/// new tab of it starts with `props`.
#[derive(Clone, Debug, PartialEq)]
pub struct TabKind {
    pub title: Option<String>,
    pub icon: Option<IconName>,
    pub props: Data,
    /// The prop a tab hands it as a message for reports, such as a browser's
    /// `navigate`: what comes back (`url`, `title`) is kept in the tab.
    pub report: Option<String>,
}

/// What an instance runs: a mod, as `ui.mount { "explorer", id = ... }`
/// mounts it, or a component that keeps state, as `use` mounts it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstanceOf {
    Mod(String),
    Component(String),
}

/// A mod or component instance in a view: its own state, kept by the app
/// under [`Instance::key`], and drawn where this node is.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub of: InstanceOf,
    pub id: String,
    /// What it is drawn with, as its `init`, `update` and view see it.
    pub props: Data,
}

impl Instance {
    /// Where the app keeps it. A mod instance is the app's own (`explorer#2`,
    /// the same wherever it is mounted); a component's belongs to the module
    /// that drew it (`editor/tabbed#main`), so two mods' tab bars differ.
    pub fn key(&self, owner: &str) -> String {
        match &self.of {
            // A preview's instances stay inside it: never a real one's state.
            InstanceOf::Mod(name) if owner.starts_with(crate::workspace::DEMO) => {
                format!("{owner}/{name}#{}", self.id)
            },
            InstanceOf::Mod(name) => format!("{name}#{}", self.id),
            InstanceOf::Component(name) => format!("{owner}/{name}#{}", self.id),
        }
    }
}
