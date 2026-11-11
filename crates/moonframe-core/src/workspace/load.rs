//! Reading the app's files: the stylesheet, the components and their
//! demos, the shell, the mods and the instances.

use {
    super::*,
    crate::{
        component::Demo,
        vm::{self, Meter},
    },
    mlua::{Table, Value},
    std::fs,
};

impl Workspace {
    /// Reads every module again, adds mods that appeared, drops those that
    /// went, and keeps each module's state across the reload.
    pub fn reload(&mut self) {
        self.reloading = true;
        self.style_generation += 1;
        self.load_style();
        self.library = Library::read(self.roots.entries("components", "luau"));
        self.components = self.library.catalogue(&self.stylesheet);
        let mut rules = Rules::default();
        for component in &self.components {
            rules.layer(&component.rules);
        }
        self.stylesheet.components = rules;
        self.load_scopes();
        self.load_demos();
        match self.roots.file("main.luau") {
            Some(_) => {
                let candidates = self.roots.candidates("main.luau");
                self.shell.load_layered(SHELL, &candidates, &self.library);
            },
            None => {
                self.shell.load_error = Some(Error::new(format!("no main.luau in {}", self.roots)));
            },
        }
        let found = self.roots.folders("mods", "main.luau");
        for name in found.keys() {
            if !self.mods.contains_key(name) {
                let restored = self.restore(name);
                self.mods.entry(name.clone()).or_default().restored = restored;
            }
            let (library, scoped) = self.scoped(name);
            let Some(slot) = self.mods.get_mut(name) else {
                continue;
            };
            slot.scoped = scoped;
            if valid_name(name) {
                // Highest layer first, so the folder found above leads.
                let candidates = self.roots.mod_candidates(name);
                slot.load_layered(name, &candidates, &library);
            } else {
                slot.load_error = Some(Error::new(format!(
                    "`{name}` is not a mod name: use a-z, 0-9, - and _, and not `{SHELL}`"
                )));
            }
        }
        self.mods.retain(|name, _| found.contains_key(name));
        // Instances run the new source too, keeping their state.
        let keys: Vec<String> = self.instances.keys().cloned().collect();
        for key in keys {
            self.load_instance(&key);
        }
        self.settle();
        self.reloading = false;
        self.report_entries();
    }

    /// Starts the mod `name` again from `state`, as `init(previous)` gets
    /// it, or from its own `init`, and draws: what a check or a headless
    /// render starts a mod from.
    pub fn restart(&mut self, name: &str, state: Option<Data>) {
        let (library, scoped) = self.scoped(name);
        let candidates = self.roots.mod_candidates(name);
        let strict = self.strict;
        let Some(slot) = self.mods.get_mut(name) else {
            return;
        };
        *slot = Slot {
            restored: state,
            scoped,
            strict,
            ..Slot::default()
        };
        slot.load_layered(name, &candidates, &library);
        self.settle();
    }

    /// Reads every view strictly from now on, as a check does: classes and
    /// states no rule styles are noted.
    pub fn set_strict(&mut self, strict: bool) {
        if self.strict != strict {
            self.strict = strict;
            self.reload();
        }
    }

    /// Loads (or reloads) the instance at `key` from its mod or component.
    pub(super) fn load_instance(&mut self, key: &str) {
        let (library, scoped) = self.scoped(key);
        let Some(mounted) = self.instances.get_mut(key) else {
            return;
        };
        mounted.slot.scoped = scoped;
        match &mounted.of {
            InstanceOf::Mod(name) => {
                let candidates = self.roots.mod_candidates(name);
                if candidates.is_empty() {
                    mounted.slot.load_error = Some(Error::new(format!("no mod named `{name}`")));
                } else {
                    mounted.slot.load_layered(key, &candidates, &library);
                }
            },
            InstanceOf::Component(name) => {
                let name = name.clone();
                mounted.slot.load_component(key, &name, &library);
            },
        }
    }

    /// Reads every layer's `style.luau`, each laid over the ones below it
    /// ([`layer`]); without one the app uses no rules. A broken stylesheet
    /// keeps the previous one, like a broken module.
    fn load_style(&mut self) {
        let files = self.roots.files("style.luau");
        if files.is_empty() {
            self.stylesheet = Stylesheet::default();
            self.style_error = None;
            return;
        }
        let loaded = (|| {
            let meter = Meter::default();
            let lua = vm::open("style", &meter, &Library::default(), vm::SMALL_MEMORY)?;
            let merged = lua.create_table()?;
            for path in &files {
                let within = |error: Error| Error::new(format!("{}: {error}", path.display()));
                let source = fs::read_to_string(path)
                    .map_err(|error| Error::new(format!("cannot read: {error}")))
                    .map_err(within)?;
                // Each file gets globals of its own and the whole budget,
                // so a package's stylesheet cannot replace `style` for the
                // layers above it.
                meter.reset();
                let value = vm::text(&lua, &source, "style")
                    .set_environment(vm::own_globals(&lua)?)
                    .eval::<Value>()
                    .map_err(|error| within(error.into()))?;
                match value {
                    Value::Table(sheet) => layer(&merged, &sheet)?,
                    // Reading it says what is wrong.
                    other => return Stylesheet::read(&other).map_err(within),
                }
            }
            Stylesheet::read(&Value::Table(merged))
        })();
        match loaded {
            Ok(sheet) => {
                self.stylesheet = sheet;
                self.style_error = None;
            },
            Err(error) => self.style_error = Some(error),
        }
    }

    /// Every component, for the window's list.
    pub fn components(&self) -> &[Component] {
        &self.components
    }

    pub fn stylesheet(&self) -> &Stylesheet {
        &self.stylesheet
    }

    /// Loads every component's demo, its file or one made from its example,
    /// keeping each one's state across reloads.
    fn load_demos(&mut self) {
        let files = self.roots.entries("components/demos", "luau");
        let mut found = BTreeSet::new();
        for component in &mut self.components {
            let demo = match files.get(&component.name) {
                Some(path) => match fs::read_to_string(path) {
                    Ok(source) => Some(Demo {
                        source,
                        path: Some(path.clone()),
                    }),
                    Err(error) => {
                        component.preview = Err(Error::new(format!(
                            "cannot read {}: {error}",
                            path.display()
                        )));
                        None
                    },
                },
                None => component.demo.take(),
            };
            component.demo = demo;
            let Some(demo) = &component.demo else {
                continue;
            };
            let name = format!("{DEMO}{}", component.name);
            found.insert(name.clone());
            let slot = self.demos.entry(name.clone()).or_default();
            slot.load_source(&name, &demo.source, demo.path.as_deref(), &self.library);
        }
        self.demos.retain(|name, _| found.contains(name));
    }
}

/// How deep each stylesheet key merges when one layer goes over another:
/// a color or speed replaces the one of its name, and a rule's properties
/// replace those of the same rule, so a user's `panel = { radius = 4 }`
/// keeps the app's panel background, and `mono = { size = 14 }` keeps its
/// family. Any other key replaces the value below.
const MERGED: &[(&str, usize)] = &[
    ("colors", 1),
    ("speeds", 1),
    ("looks", 1),
    ("mono", 1),
    ("rules", 2),
];

/// Lays the stylesheet `sheet` over `merged`.
fn layer(merged: &Table, sheet: &Table) -> mlua::Result<()> {
    for pair in sheet.pairs::<Value, Value>() {
        let (key, value) = pair?;
        let depth = key
            .as_string()
            .and_then(|key| key.to_str().ok())
            .and_then(|key| MERGED.iter().find(|(name, _)| *name == &*key))
            .map_or(0, |(_, depth)| *depth);
        merge(merged, key, value, depth)?;
    }
    Ok(())
}

/// Sets `into[key]` to `value`, merging tables `depth` levels down.
fn merge(into: &Table, key: Value, value: Value, depth: usize) -> mlua::Result<()> {
    match (depth, into.raw_get::<Value>(key.clone())?, value) {
        (1.., Value::Table(below), Value::Table(above)) => {
            for pair in above.pairs::<Value, Value>() {
                let (key, value) = pair?;
                merge(&below, key, value, depth - 1)?;
            }
            Ok(())
        },
        (_, _, value) => into.raw_set(key, value),
    }
}

fn valid_name(name: &str) -> bool {
    name != SHELL && crate::entries::module_name(name)
}
