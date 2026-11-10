//! Checking a miniapp entry without running it: what a host's
//! `<app> miniapp check` reports. It reads the entry's `main.luau`,
//! `style.luau` and `components/*.luau` as the window would, loading the
//! mod and drawing its first view headlessly. Its provider is the
//! provider crate's to check (`moonframe_providers::check_entry`).

use {
    super::{Entry, RootPolicy},
    crate::{
        Roots, Workspace,
        component::Library,
        vm::{self, Meter},
        workspace::read_style,
    },
    std::{
        collections::BTreeMap,
        fmt,
        path::{Path, PathBuf},
    },
};

/// How bad a [`Diagnostic`] is: an error stops the entry from loading.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

/// One problem in an entry, and the file it is in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub file: PathBuf,
    pub message: String,
}

impl Diagnostic {
    pub fn error(file: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            file: file.into(),
            message: message.into(),
        }
    }

    pub fn warning(file: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            file: file.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{}: {level}: {}", self.file.display(), self.message)
    }
}

/// Checks the entry at `path` on its own: a `use` of a component it does
/// not bring is a warning, as the host app may bring it. Use [`check_in`]
/// to resolve those against the app.
pub fn check(path: &Path, policy: &RootPolicy) -> Vec<Diagnostic> {
    check_in(path, policy, None)
}

/// Checks the entry at `path` as the app on `app` would load it: its
/// components over the app's, its rules against the app's colors.
pub fn check_in(path: &Path, policy: &RootPolicy, app: Option<&Roots>) -> Vec<Diagnostic> {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    if !path.is_dir() {
        return vec![Diagnostic::error(&path, "no such folder")];
    }
    let entry = Entry {
        name: "check:entry".to_owned(),
        namespace: "check".to_owned(),
        path: path.clone(),
        root: path.parent().unwrap_or(&path).to_owned(),
        policy: policy.clone(),
    };
    let has_mod = entry.mod_file().is_file();
    if !has_mod && !entry.provider_dir().join("provider.toml").is_file() {
        return vec![Diagnostic::error(
            &path,
            "a miniapp needs main.luau, provider/provider.toml, or both",
        )];
    }
    let mut roots = app
        .cloned()
        .unwrap_or_else(|| Roots::new(Vec::<PathBuf>::new()));
    roots.set_entries(BTreeMap::from([(entry.name.clone(), entry.clone())]));
    let workspace = Workspace::open(roots.clone(), false);
    let lenient = app.is_none();
    let mut found = Vec::new();
    if let Err(error) = read_style(&entry.style_file(), workspace.stylesheet()) {
        found.push(Diagnostic::error(entry.style_file(), error.to_string()));
    }
    let library = Library::read(if lenient {
        BTreeMap::new()
    } else {
        roots.entries("components", "luau")
    })
    .with(entry.components());
    for (name, file) in entry.components() {
        let meter = Meter::default();
        let loaded = vm::open(
            &format!("components/{name}"),
            &meter,
            &library,
            vm::SMALL_MEMORY,
        )
        .and_then(|lua| library.definition(&lua, &name).map(drop));
        if let Err(error) = loaded {
            found.push(diagnose(&file, &error.to_string(), lenient));
        }
    }
    if has_mod {
        match workspace.slot(&entry.name) {
            Some(slot) => {
                if let Some(error) = slot.error() {
                    found.push(diagnose(&entry.mod_file(), &error.to_string(), lenient));
                }
                for broken in slot.broken() {
                    found.push(Diagnostic::error(entry.mod_file(), broken.to_string()));
                }
            },
            None => found.push(Diagnostic::error(entry.mod_file(), "did not load")),
        }
    }
    found.sort_by(|a, b| (a.severity, &a.file).cmp(&(b.severity, &b.file)));
    found.dedup();
    found
}

/// An error, or a warning for a component only the host app can bring.
fn diagnose(file: &Path, message: &str, lenient: bool) -> Diagnostic {
    if lenient && message.contains("no component named") {
        Diagnostic::warning(
            file,
            format!("{message} (not in the miniapp: the host app must bring it)"),
        )
    } else {
        Diagnostic::error(file, message)
    }
}
