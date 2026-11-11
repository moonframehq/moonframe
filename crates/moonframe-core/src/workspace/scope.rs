//! A miniapp entry's own components and stylesheet: its `use` finds
//! `<entry>/components/<name>.luau` before the app's, and the `rules` of its
//! `<entry>/style.luau` apply only to what it draws, under its own `styles`.

use {
    super::*,
    crate::{
        read::{Cx, Read as _},
        vm::{self, Meter},
    },
    mlua::Value,
    std::fs,
};

/// How far up an instance's owners a scope is looked for.
const MAX_OWNERS: usize = 32;

/// What one entry brings beside its mod.
pub(super) struct Scope {
    library: Library,
    rules: Rules,
    /// Why its stylesheet did not load; the last good rules stay.
    style_error: Option<Error>,
}

impl Workspace {
    /// Reads every entry's components and stylesheet again, over the app's
    /// library and stylesheet, which must be loaded first.
    pub(super) fn load_scopes(&mut self) {
        let mut scopes = BTreeMap::new();
        for (name, entry) in self.roots.external_entries() {
            let previous = self.scopes.remove(name);
            let (rules, style_error) = match read_style(&entry.style_file(), &self.stylesheet) {
                Ok(rules) => (rules, None),
                Err(error) => (
                    previous.map(|scope| scope.rules).unwrap_or_default(),
                    Some(error),
                ),
            };
            scopes.insert(name.clone(), Scope {
                library: self.library.with(entry.components()),
                rules,
                style_error,
            });
        }
        self.scopes = scopes;
    }

    /// The scope the module or instance `key` draws in: its entry's, or that
    /// of whatever drew it.
    fn scope_of(&self, key: &str) -> Option<&Scope> {
        let mut key = key;
        for _ in 0..MAX_OWNERS {
            if let Some(scope) = self.scopes.get(key) {
                return Some(scope);
            }
            let mounted = self.instances.get(key)?;
            key = match &mounted.of {
                InstanceOf::Mod(name) if self.scopes.contains_key(name) => name,
                _ => &mounted.owner,
            };
        }
        None
    }

    /// The components `key` may `use`, and the rules scoped to it.
    pub(super) fn scoped(&self, key: &str) -> (Library, Rules) {
        self.scope_of(key).map_or_else(
            || (self.library.clone(), Rules::default()),
            |scope| (scope.library.clone(), scope.rules.clone()),
        )
    }

    /// Reports external mod revisions after load and first view validation.
    pub fn on_entry_status(&mut self, callback: crate::entries::EntryStatuses) {
        self.entry_statuses = Some(callback);
        self.report_entries();
    }

    pub(super) fn report_entries(&self) {
        let Some(report) = &self.entry_statuses else {
            return;
        };
        for name in self.roots.external_entries().keys() {
            if let Some(status) = self.entry_status(name) {
                report(status);
            }
        }
    }

    /// The entry `name`'s status: its mod's load and view, its stylesheet,
    /// and what its views drew wrong.
    pub(super) fn status_of(&self, name: &str) -> crate::entries::EntryStatus {
        use crate::entries::{EntryKind, EntryState, EntryStatus};
        let slot = self.mods.get(name);
        let style_error = self
            .scopes
            .get(name)
            .and_then(|scope| scope.style_error.as_ref());
        let error = slot
            .and_then(Slot::error)
            .or(style_error)
            .map(ToString::to_string);
        EntryStatus {
            name: name.to_owned(),
            kind: EntryKind::Mod,
            state: if error.is_none() {
                EntryState::Ok
            } else if slot.is_some_and(|slot| slot.module.is_some()) {
                EntryState::Stale
            } else {
                EntryState::Failed
            },
            error,
            revision: self.roots.external_entries().get(name).map_or(0, revision),
            view_errors: self.entry_view_errors(name),
        }
    }
}

/// A hash of every file the entry's mod is made of.
fn revision(entry: &crate::entries::Entry) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let files = [entry.mod_file(), entry.style_file()]
        .into_iter()
        .chain(entry.components().into_values());
    for path in files {
        path.hash(&mut hash);
        fs::read(&path).unwrap_or_default().hash(&mut hash);
    }
    hash.finish()
}

/// The `rules` of an entry's `style.luau`, read against the app's colors;
/// none without the file.
pub(crate) fn read_style(path: &Path, sheet: &Stylesheet) -> Result<Rules, Error> {
    let within = |error: Error| Error::new(format!("{}: {error}", path.display()));
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Rules::default());
        },
        Err(error) => return Err(within(Error::new(format!("cannot read: {error}")))),
    };
    (|| {
        let meter = Meter::default();
        let lua = vm::open("style", &meter, &Library::default(), vm::SMALL_MEMORY)?;
        let value = vm::text(&lua, &source, "style")
            .set_environment(vm::own_globals(&lua)?)
            .eval::<Value>()?;
        let Value::Table(table) = value else {
            return Err(Error::new("style.luau must return style { rules = { ... } }"));
        };
        for pair in table.pairs::<Value, Value>() {
            let (key, _) = pair?;
            if key.as_string().is_none_or(|key| key != "rules") {
                return Err(Error::new(format!(
                    "a miniapp's style.luau takes only `rules`, scoped to its mod, not `{}`; colors and fonts belong to the app",
                    key.to_string().unwrap_or_default()
                )));
            }
        }
        let rules = table.raw_get::<Value>("rules")?;
        if rules.is_nil() {
            return Ok(Rules::default());
        }
        Rules::read(&rules, &Cx::root(sheet.names()).key("rules"))
    })()
    .map_err(within)
}
