//! Which permission questions a run answers itself, as the user set for its
//! mod in provider.toml's `[settings] allow`; never a mod's own choice.
//! Each is answered "allow once", never "always": the agent asks again next
//! time, and nothing it was allowed outlives the run.

use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Allow {
    /// Every question waits on the user.
    #[default]
    Ask,
    /// Tools that only look (read, search, think); the rest wait.
    Read,
    /// Tools that only look, edits of files in the app's writable folder
    /// (which the app keeps, so a run can be undone), and the app's own
    /// check command; the rest wait.
    App,
    /// Every tool.
    All,
}

/// What a run may touch under [`Allow::App`].
#[derive(Clone, Debug, Default)]
pub(crate) struct Scope {
    /// Where the agent works: relative paths are from here.
    pub(crate) cwd: PathBuf,
    /// The folders it may edit in, resolved.
    pub(crate) writable: Vec<PathBuf>,
    /// Commands it may run as they are: the app's check.
    pub(crate) commands: Vec<String>,
}

/// One tool call asking permission.
pub(crate) struct Call<'a> {
    pub(crate) kind: Option<&'a str>,
    pub(crate) paths: &'a [PathBuf],
    pub(crate) command: Option<&'a str>,
    /// Other files named in its input, or unknown if it may name others.
    pub(crate) targets: Option<&'a [PathBuf]>,
}

impl Allow {
    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "ask" => Some(Self::Ask),
            "read" => Some(Self::Read),
            "app" => Some(Self::App),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    /// Its name in provider.toml.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Read => "read",
            Self::App => "app",
            Self::All => "all",
        }
    }

    /// Whether a tool of `kind` is answered without asking, whatever it
    /// works on. A tool whose kind is unknown counts as one that changes
    /// things.
    pub(crate) fn covers(self, kind: Option<&str>) -> bool {
        match self {
            Self::Ask => false,
            Self::Read | Self::App => matches!(kind, Some("read" | "search" | "think")),
            Self::All => true,
        }
    }

    /// Whether `call` is answered without asking, within `scope`.
    pub(crate) fn covers_call(self, call: &Call<'_>, scope: &Scope) -> bool {
        if self.covers(call.kind) {
            return true;
        }
        if self != Self::App {
            return false;
        }
        match call.kind {
            // Only when every file it touches is known: a move's other
            // end, or a patch naming files under keys not read here, would
            // otherwise go unchecked. A move always asks.
            Some("edit" | "delete") => call.targets.is_some_and(|named| {
                let paths: Vec<&Path> = call
                    .paths
                    .iter()
                    .map(PathBuf::as_path)
                    .chain(named.iter().map(PathBuf::as_path))
                    .collect();
                !paths.is_empty() && paths.iter().all(|path| scope.holds(&scope.cwd.join(path)))
            }),
            Some("execute") => call
                .command
                .and_then(|command| bare(command, &scope.cwd))
                .is_some_and(|command| scope.commands.iter().any(|known| known.trim() == command)),
            _ => false,
        }
    }
}

impl Scope {
    /// Whether `path` is in a writable folder: no `..`, links followed,
    /// and nothing that says what runs or what is allowed.
    pub(crate) fn holds(&self, path: &Path) -> bool {
        if path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return false;
        }
        let path = resolved(path);
        self.writable.iter().any(|folder| {
            path.strip_prefix(folder)
                .is_ok_and(|rest| !rest.as_os_str().is_empty() && !guarded(rest))
        })
    }
}

/// Whether `rest`, a path in a writable folder, configures what runs or
/// what is allowed: a provider (whose `provider.toml` holds this very
/// `allow`, and may start a program), or the app's manifests. An agent
/// editing one could give itself more, so the user is always asked.
fn guarded(rest: &Path) -> bool {
    let names: Vec<String> = rest
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    names.iter().any(|name| name == "providers")
        || names
            .last()
            .is_some_and(|name| matches!(name.as_str(), "moonframe.toml" | "packages.toml"))
}

/// `path` with its nearest folder that exists resolved: a file being made
/// goes by where it will be.
fn resolved(path: &Path) -> PathBuf {
    path.ancestors()
        .find_map(|folder| {
            let rest = path.strip_prefix(folder).ok()?;
            Some(folder.canonicalize().ok()?.join(rest))
        })
        .unwrap_or_else(|| path.to_owned())
}

/// The command at the heart of `command`, as agents wrap it: after a `cd`
/// into the folder they work in, and before `2>&1` and a pipe into `tail`
/// or `head`. None when anything else is chained to it.
fn bare<'a>(command: &'a str, cwd: &Path) -> Option<&'a str> {
    // One line, as the check is: a newline would run another command.
    if command.trim().chars().any(char::is_control) {
        return None;
    }
    let mut command = command.trim();
    for cd in [
        format!("cd {} && ", cwd.display()),
        format!("cd '{}' && ", cwd.display()),
    ] {
        if let Some(rest) = command.strip_prefix(&cd) {
            command = rest.trim();
        }
    }
    if let Some((before, after)) = command.rsplit_once('|')
        && viewer(after)
    {
        command = before.trim();
    }
    if let Some(rest) = command.strip_suffix("2>&1") {
        command = rest.trim();
    }
    Some(command)
}

/// Whether `piped`, what a command's output is piped into, only shows the
/// end or start of it: `tail` or `head`, with a count and nothing else.
fn viewer(piped: &str) -> bool {
    // Plain words on one line: a newline would start another command.
    if !piped
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-')
    {
        return false;
    }
    let mut words = piped.split(' ').filter(|word| !word.is_empty());
    let program = words.next();
    let mut counted = false;
    matches!(program, Some("tail" | "head"))
        && words.all(|word| {
            // `-20`, `-n20`, or `-n` then `20`; a bare word is a file.
            let count = |text: &str| !text.is_empty() && text.chars().all(|c| c.is_ascii_digit());
            let ok = match word.strip_prefix('-') {
                Some("n") => !counted,
                Some(rest) => count(rest.strip_prefix('n').unwrap_or(rest)),
                None => std::mem::take(&mut counted),
            };
            counted = word == "-n";
            ok
        })
}

/// The files an edit's `input` names besides those the protocol shows: the
/// keys of a `changes` map (a patch over several files). None when it has
/// anything else that could name a file, so the user is asked.
pub(crate) fn input_targets(input: Option<&serde_json::Value>) -> Option<Vec<PathBuf>> {
    /// Keys that name the file edited (read by `guard::paths`) or hold
    /// only text.
    const KNOWN: [&str; 10] = [
        "abs_path",
        "content",
        "description",
        "file_path",
        "new_string",
        "notebook_path",
        "old_string",
        "path",
        "replace_all",
        "edits",
    ];
    let Some(input) = input else {
        return Some(Vec::new());
    };
    let fields = input.as_object()?;
    let mut named = Vec::new();
    for (key, value) in fields {
        match key.as_str() {
            "changes" => {
                for (file, change) in value.as_object()? {
                    if !text_only(change) {
                        return None;
                    }
                    named.push(PathBuf::from(file));
                }
            },
            // Several edits of the one file: text only.
            "edits" => {
                for edit in value.as_array()? {
                    let only_text = edit.as_object()?.keys().all(|key| {
                        matches!(key.as_str(), "old_string" | "new_string" | "replace_all")
                    });
                    if !only_text {
                        return None;
                    }
                }
            },
            key if KNOWN.contains(&key) => {},
            _ => return None,
        }
    }
    Some(named)
}

/// Whether a patch's `change` to one file holds only its text, every value
/// of a known shape: a move to another file, at any depth, is refused.
fn text_only(change: &serde_json::Value) -> bool {
    use serde_json::Value;
    let Some(fields) = change.as_object() else {
        return false;
    };
    fields.iter().all(|(key, value)| match key.as_str() {
        "type" | "content" | "unified_diff" => value.is_string(),
        // `{ type = "update", move_path = nil }`, as Codex sends it.
        "kind" => match value {
            Value::String(_) => true,
            Value::Object(kind) => kind.iter().all(|(key, value)| match key.as_str() {
                "type" => value.is_string(),
                "move_path" => value.is_null(),
                _ => false,
            }),
            _ => false,
        },
        _ => false,
    })
}

/// The shell command a tool call's input names, as agents send it: a
/// string, or words.
pub(crate) fn command_of(input: Option<&serde_json::Value>) -> Option<String> {
    let command = input?.get("command")?;
    match command {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Array(words) => {
            let words: Vec<&str> = words.iter().filter_map(serde_json::Value::as_str).collect();
            // `bash -lc "<command>"`, as Codex sends it: only a shell, with
            // only the flag that runs a command. Anything else is compared
            // whole, so it never passes for the check.
            let shell = |word: &str| {
                matches!(
                    Path::new(word).file_name().and_then(|name| name.to_str()),
                    Some("sh" | "bash" | "zsh")
                )
            };
            match words.as_slice() {
                [program, flag, command] if shell(program) && matches!(*flag, "-c" | "-lc") => {
                    Some((*command).to_owned())
                },
                _ => Some(words.join(" ")),
            }
        },
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, serde_json::json};

    fn scope(dir: &Path) -> Scope {
        Scope {
            cwd: dir.to_owned(),
            writable: vec![dir.join("app").canonicalize().unwrap()],
            commands: vec!["/bin/notes --check /repo/app".to_owned()],
        }
    }

    fn call<'a>(kind: &'a str, paths: &'a [PathBuf], command: Option<&'a str>) -> Call<'a> {
        Call {
            kind: Some(kind),
            paths,
            command,
            targets: Some(&[]),
        }
    }

    #[test]
    fn edits_whose_files_are_not_all_known_ask() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app/mods")).unwrap();
        let scope = scope(dir.path());
        let inside = [PathBuf::from("app/style.luau")];
        let allowed = |kind: &'static str, input: Option<&serde_json::Value>| {
            let named = input_targets(input);
            let call = Call {
                kind: Some(kind),
                paths: &inside,
                command: None,
                targets: named.as_deref(),
            };
            Allow::App.covers_call(&call, &scope)
        };
        let edit = json!({ "file_path": "app/style.luau", "old_string": "a", "new_string": "b" });
        assert!(allowed("edit", Some(&edit)));
        let multi = json!({ "file_path": "app/style.luau", "edits": [{ "old_string": "a", "new_string": "b" }] });
        assert!(allowed("edit", Some(&multi)));
        let patch = json!({ "changes": {
            "app/a.luau": { "type": "update", "unified_diff": "@@" },
            "app/b.luau": { "type": "add", "content": "x" },
            "app/c.luau": { "kind": { "type": "update", "move_path": null }, "unified_diff": "@@" },
        } });
        assert!(allowed("edit", Some(&patch)));
        // A patch reaching outside, a key not read here, an edit naming
        // another file, and any move ask.
        for input in [
            json!({ "changes": { "app/a.luau": {}, "crates/lib.rs": {} } }),
            json!({ "changes": { "app/a.luau": { "type": "update", "move_path": "crates/x" } } }),
            json!({ "changes": { "app/a.luau": { "kind": { "type": "update", "move_path": "crates/x" } } } }),
            json!({ "changes": { "app/a.luau": { "content": { "move_path": "crates/x" } } } }),
            json!({ "file_path": "app/style.luau", "new_path": "crates/lib.rs" }),
            json!({ "file_path": "app/style.luau", "edits": [{ "file_path": "crates/x" }] }),
            json!("app/style.luau"),
        ] {
            assert!(!allowed("edit", Some(&input)), "{input}");
        }
        let moved = json!({ "path": "app/style.luau" });
        assert!(!allowed("move", Some(&moved)));
    }

    #[test]
    fn the_app_level_edits_the_app_folder_and_runs_its_check_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app/mods")).unwrap();
        let scope = scope(dir.path());
        let app = Allow::App;
        let inside = [PathBuf::from("app/mods/new/main.luau")];
        let absolute = [dir.path().join("app/style.luau")];
        let outside = [PathBuf::from("crates/lib.rs")];
        let escaping = [PathBuf::from("app/../crates/lib.rs")];
        let mixed = [inside[0].clone(), outside[0].clone()];
        assert!(app.covers_call(&call("edit", &inside, None), &scope));
        assert!(app.covers_call(&call("delete", &absolute, None), &scope));
        assert!(!app.covers_call(&call("edit", &outside, None), &scope));
        assert!(!app.covers_call(&call("edit", &escaping, None), &scope));
        assert!(!app.covers_call(&call("edit", &mixed, None), &scope));
        // Nothing that says what runs or what is allowed, nor the folder.
        for guarded in [
            "app/providers/acp/provider.toml",
            "app/Providers/mine/main.wasm",
            "app/moonframe.toml",
            "app/packages.toml",
            "app",
        ] {
            let paths = [PathBuf::from(guarded)];
            assert!(
                !app.covers_call(&call("edit", &paths, None), &scope),
                "{guarded}"
            );
        }
        // An edit that names no file could be anywhere.
        assert!(!app.covers_call(&call("edit", &[], None), &scope));
        assert!(app.covers_call(&call("read", &outside, None), &scope));

        let check = "/bin/notes --check /repo/app";
        for command in [
            check.to_owned(),
            format!("{check} 2>&1"),
            format!("{check} 2>&1 | tail -20"),
            format!("cd {} && {check}", dir.path().display()),
        ] {
            assert!(
                app.covers_call(&call("execute", &[], Some(&command)), &scope),
                "{command}"
            );
        }
        for command in [
            format!("{check}; rm -rf ~"),
            format!("{check} | sh"),
            format!("{check} | tailscale up"),
            format!("{check}\nrm -rf x"),
            format!("{check} | head -c /etc/passwd"),
            format!("{check} && curl x"),
            "just check".to_owned(),
        ] {
            assert!(
                !app.covers_call(&call("execute", &[], Some(&command)), &scope),
                "{command}"
            );
        }
        // Other levels are as they were.
        assert!(!Allow::Read.covers_call(&call("edit", &inside, None), &scope));
        assert!(Allow::All.covers_call(&call("execute", &[], Some("rm x")), &scope));
        for level in [Allow::Ask, Allow::Read, Allow::App, Allow::All] {
            assert_eq!(Allow::parse(level.name()), Some(level));
        }
    }

    #[test]
    fn commands_come_as_text_or_words() {
        assert_eq!(
            command_of(Some(&json!({ "command": "just check" }))).as_deref(),
            Some("just check")
        );
        assert_eq!(
            command_of(Some(&json!({ "command": ["bash", "-lc", "x --check"] }))).as_deref(),
            Some("x --check")
        );
        assert_eq!(
            command_of(Some(&json!({ "command": ["ls", "-la"] }))).as_deref(),
            Some("ls -la")
        );
        assert_eq!(command_of(Some(&json!({ "path": "x" }))), None);
        // Only a shell's own command flag is unwrapped.
        assert_eq!(
            command_of(Some(&json!({ "command": ["ssh", "-t", "x --check"] }))).as_deref(),
            Some("ssh -t x --check")
        );
        assert_eq!(
            command_of(Some(&json!({ "command": ["bash", "-x", "x --check"] }))).as_deref(),
            Some("bash -x x --check")
        );
        for (piped, shown) in [
            ("tail -20", true),
            ("tail -n 20", true),
            ("head -n20", true),
            ("tail", true),
            ("tailscale up", false),
            ("tail -f", false),
            ("head -c /etc/x", false),
            ("tail 20", false),
            ("tail -5\nrm -rf x", false),
            ("tail\t-5", false),
        ] {
            assert_eq!(viewer(piped), shown, "{piped}");
        }
    }
}
