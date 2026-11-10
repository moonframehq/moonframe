//! Checking a miniapp entry without running it: what a host's
//! `<app> miniapp check` reports. It reads the entry's `main.luau`,
//! `style.luau` and `components/*.luau` as the window would, loading the
//! mod and drawing its first view headlessly. Its provider is the
//! provider crate's to check (`moonframe_providers::check_entry`).

use {
    super::{CheckOptions, Entry, RootPolicy},
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
    /// The line in `file`, when known.
    pub line: Option<u32>,
    pub message: String,
}

impl Diagnostic {
    pub fn error(file: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            file: file.into(),
            line: None,
            message: message.into(),
        }
    }

    pub fn warning(file: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            file: file.into(),
            line: None,
            message: message.into(),
        }
    }

    /// The same, at `line` of its file.
    #[must_use]
    pub fn at_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{}", self.file.display())?;
        if let Some(line) = self.line {
            write!(f, ":{line}")?;
        }
        write!(f, ": {level}: {}", self.message)
    }
}

/// Checks the entry at `path` on its own, with no data: a `use` of a
/// component it does not bring is a warning, as the host app may bring
/// it. [`check_in`] takes the app, data to draw with, and more.
pub fn check(path: &Path, policy: &RootPolicy) -> Vec<Diagnostic> {
    check_in(path, policy, &CheckOptions::default())
}

/// Checks the entry at `path` as the app on `options.roots` would load it
/// (its components over the app's, its rules against the app's colors),
/// drawing its view with each case `options` gives: see [`CheckOptions`].
pub fn check_in(path: &Path, policy: &RootPolicy, options: &CheckOptions) -> Vec<Diagnostic> {
    let app = options.roots.as_ref();
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    if !path.is_dir() {
        return vec![Diagnostic::error(&path, "no such folder")];
    }
    let name = options.name.as_deref().unwrap_or("check:entry");
    let Some((namespace, _)) = name.split_once(':').filter(|_| super::module_name(name)) else {
        return vec![Diagnostic::error(
            &path,
            format!("`{name}` is no entry name: use <namespace>:<id>"),
        )];
    };
    let entry = Entry {
        name: name.to_owned(),
        namespace: namespace.to_owned(),
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
    let mut workspace = Workspace::open(roots.clone(), false);
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
        found.extend(super::scenarios::run(
            &mut workspace,
            &entry,
            options,
            lenient,
        ));
    }
    found.sort_by(|a, b| (a.severity, &a.file, a.line).cmp(&(b.severity, &b.file, b.line)));
    found.dedup();
    found
}

/// An error, or a warning for a component only the host app can bring.
pub(super) fn diagnose(file: &Path, message: &str, lenient: bool) -> Diagnostic {
    if lenient && message.contains("no component named") {
        Diagnostic::warning(
            file,
            format!("{message} (not in the miniapp: the host app must bring it)"),
        )
    } else {
        Diagnostic::error(file, message)
    }
}
