//! What a view's reader notes without refusing: deprecated keys always,
//! and in a strict read, classes and states no rule styles.

use {
    super::*,
    crate::{ViewLevel, diagnostic},
};

/// A class or state an element used, where.
pub(super) struct Used {
    cx_at: Error,
    class: Vec<String>,
    state: Option<String>,
}

impl Tree<'_> {
    /// Notes each deprecated key the element still takes.
    pub(super) fn deprecated(&mut self, kind: Element, fields: &Fields) {
        let keys = diagnostic::deprecated(kind.name(), fields.keys());
        for (key, replacement) in keys {
            let error = fields
                .cx()
                .key(key)
                .fail(format!("`{key}` is deprecated"))
                .with(|detail| {
                    detail.level = ViewLevel::Deprecated;
                    detail.line = fields.line();
                    detail.suggestion = Some(format!("use `{replacement}`"));
                });
            self.problems.push(error);
        }
    }

    /// In a strict read, keeps the element's classes and state to check.
    pub(super) fn used(&mut self, fields: &Fields) {
        let Some((_, used)) = &mut self.strict else {
            return;
        };
        let class: Vec<String> = match fields.raw("class") {
            Some(Value::String(names)) => names
                .to_string_lossy()
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
            Some(Value::Table(names)) => names
                .sequence_values::<String>()
                .filter_map(Result::ok)
                .collect(),
            _ => Vec::new(),
        };
        let state = match fields.raw("state") {
            Some(Value::String(state)) => Some(state.to_string_lossy()),
            _ => None,
        };
        if class.is_empty() && state.is_none() {
            return;
        }
        let cx_at = fields
            .cx()
            .fail("")
            .with(|detail| detail.line = fields.line());
        used.push(Used {
            cx_at,
            class,
            state,
        });
    }

    /// In a strict read, notes each class and state no rule styles.
    pub(super) fn unstyled(&mut self) {
        let Some((sheet, used)) = self.strict.take() else {
            return;
        };
        let rules = [&sheet.rules, &sheet.components];
        for Used {
            cx_at,
            class,
            state,
        } in used
        {
            let unknown_class = class
                .iter()
                .filter(|class| !rules.iter().any(|rules| rules.styles_class(class)))
                .map(|class| {
                    (
                        "class",
                        class.clone(),
                        format!("no rule styles the class `{class}`"),
                    )
                });
            let unknown_state = state
                .filter(|state| !rules.iter().any(|rules| rules.styles_state(state)))
                .map(|state| {
                    (
                        "state",
                        state.clone(),
                        format!("no rule styles the state `{state}`"),
                    )
                });
            for (key, value, problem) in unknown_class.chain(unknown_state) {
                let note = cx_at.clone().with(|detail| {
                    detail.at = format!("{}.{key}", detail.at);
                    detail.problem = problem;
                    detail.key = Some(key.to_owned());
                    detail.value = Some(value);
                    detail.level = ViewLevel::Strict;
                    detail.suggestion = Some(
                        "add a rule for it in style.luau (or the mod's `styles`), or drop it"
                            .to_owned(),
                    );
                });
                self.problems.push(note);
            }
        }
    }
}
