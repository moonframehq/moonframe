//! Drawing an entry's view with data, as a check does: with no data, with
//! each `app` case (a provider's sample, say), from each state, and after
//! each message, collecting what each draw got wrong.

use {
    super::{
        Entry,
        check::{Diagnostic, diagnose},
    },
    crate::{Data, ViewDiagnostic, ViewLevel, Workspace},
    std::collections::BTreeSet,
};

/// What [`check_in`](super::check_in) draws an entry's view with.
#[derive(Clone, Debug, Default)]
pub struct CheckOptions {
    /// The name the host gives the entry, `<namespace>:<id>`, so its view
    /// finds its provider's data at `app["user:notes"]`; `check:entry`
    /// when unset.
    pub name: Option<String>,
    /// The app it runs in: its components and stylesheet, and the rest of
    /// its views. Without it a `use` of a component the entry does not
    /// bring is a warning.
    pub roots: Option<crate::Roots>,
    /// Fake `app` tables, each a map of keys such as
    /// `{ ["user:notes"] = <what its provider publishes> }`. The view is
    /// drawn with none, then with each in turn.
    pub app: Vec<Data>,
    /// States to draw the mod from, each as `init(previous)` gets it, in
    /// every `app` case; the mod's own `init` first.
    pub state: Vec<Data>,
    /// Window sizes, `(width, height)` in pixels, for what lays the view
    /// out (`render_entry`); a view reads no size, so a check draws once.
    pub sizes: Vec<(f32, f32)>,
    /// Messages its `update` gets, one after another, the view drawn after
    /// each, in every case.
    pub messages: Vec<Data>,
    /// Also refuses what draws but is likely wrong: classes and states no
    /// rule styles, and deprecated keys, as errors.
    pub strict: bool,
    /// The host capabilities a provider may declare, by name, for
    /// `check_entry` to check `provider.toml` against.
    pub capabilities: Vec<String>,
    /// A `luau-lsp` binary: `check_entry` then also type-checks the Luau
    /// against `types/moonframe.d.luau`, statically, with line numbers.
    pub luau_lsp: Option<std::path::PathBuf>,
}

/// Draws the entry's mod in each case `options` gives; what each draw got
/// wrong, each problem once.
pub(super) fn run(
    workspace: &mut Workspace,
    entry: &Entry,
    options: &CheckOptions,
    lenient: bool,
) -> Vec<Diagnostic> {
    let name = entry.name.as_str();
    let mut found = Found {
        file: entry.mod_file(),
        strict: options.strict,
        lenient,
        seen: BTreeSet::new(),
        diagnostics: Vec::new(),
    };
    if options.strict {
        workspace.set_strict(true);
    }
    let cases = std::iter::once(("no data".to_owned(), None)).chain(
        options
            .app
            .iter()
            .enumerate()
            .map(|(index, app)| (format!("app case {}", index + 1), Some(app))),
    );
    let mut keys: Vec<String> = Vec::new();
    for (label, app) in cases {
        for key in keys.drain(..) {
            workspace.clear_host(&key, None);
        }
        match app {
            Some(Data::Map(app)) => {
                for (key, value) in app {
                    workspace.set_host(key, value.clone());
                    keys.push(key.clone());
                }
            },
            Some(_) => {
                found.push_message(&label, "an `app` case is a table of keys");
                continue;
            },
            None => {},
        }
        let states = std::iter::once(None).chain(options.state.iter().map(Some));
        for (index, state) in states.enumerate() {
            let label = match state {
                None => label.clone(),
                Some(_) => format!("{label}, state {index}"),
            };
            workspace.restart(name, state.cloned());
            found.collect(workspace, name, &label);
            for (step, message) in options.messages.iter().enumerate() {
                workspace.send(crate::SHELL, name, message.clone());
                found.collect(
                    workspace,
                    name,
                    &format!("{label}, after message {}", step + 1),
                );
            }
        }
    }
    found.diagnostics
}

/// What the draws found so far.
struct Found {
    file: std::path::PathBuf,
    strict: bool,
    lenient: bool,
    /// Each problem by what it says, so a case repeating one adds nothing.
    seen: BTreeSet<String>,
    diagnostics: Vec<Diagnostic>,
}

impl Found {
    fn collect(&mut self, workspace: &Workspace, name: &str, case: &str) {
        let Some(slot) = workspace.slot(name) else {
            self.push_message(case, "the mod did not load");
            return;
        };
        if let Some(error) = slot.error() {
            let message = error.to_string();
            if self.seen.insert(message.clone()) {
                let mut found = diagnose(&self.file, &format!("{message} [{case}]"), self.lenient);
                if let Some(line) = line_of(&message, name) {
                    found = found.at_line(line);
                }
                self.diagnostics.push(found);
            }
        }
        for view in workspace.entry_view_errors(name) {
            self.push_view(&view, case);
        }
    }

    fn push_view(&mut self, view: &ViewDiagnostic, case: &str) {
        let mut text = view.path.clone();
        if let Some(key) = &view.key {
            text = format!("{text}.{key}");
        }
        text = format!("{text}: {}", view.message);
        if let Some(value) = &view.value {
            text = format!("{text} (got {value})");
        }
        if let Some(suggestion) = &view.suggestion {
            text = format!("{text}; {suggestion}");
        }
        if !self.seen.insert(format!("{}|{text}", view.module)) {
            return;
        }
        if view.module != view.module.split('#').next().unwrap_or_default() {
            text = format!("{} {text}", view.module);
        }
        let message = format!("{text} [{case}]");
        let error = match view.level {
            ViewLevel::Broken | ViewLevel::Strict => true,
            ViewLevel::Deprecated => self.strict,
        };
        let mut found = if error {
            Diagnostic::error(&self.file, message)
        } else {
            Diagnostic::warning(&self.file, message)
        };
        if let Some(line) = view.line {
            found = found.at_line(line);
        }
        self.diagnostics.push(found);
    }

    fn push_message(&mut self, case: &str, message: &str) {
        self.diagnostics
            .push(Diagnostic::error(&self.file, format!("{message} [{case}]")));
    }
}

/// The line of the module `name` a Luau error names: `user:notes:12: ...`.
fn line_of(message: &str, name: &str) -> Option<u32> {
    let marker = format!("{name}:");
    message.match_indices(&marker).find_map(|(at, _)| {
        let rest = &message[at + marker.len()..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        rest[digits.len()..]
            .starts_with(':')
            .then(|| digits.parse().ok())
            .flatten()
    })
}
