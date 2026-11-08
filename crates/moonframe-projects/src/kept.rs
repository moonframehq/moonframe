//! What the app asked for, kept across restarts (an edit to the provider's
//! folder, an update), so a file being read or a project being shown stays
//! so; the mods asked once and will not ask again. In the cache, one file
//! per app, as a change to the provider's folder would restart it.

use {
    crate::scope_known,
    serde_json::{Value, json},
    std::{
        env, fs,
        path::{Path, PathBuf},
    },
};

pub(crate) struct Kept {
    pub(crate) select: Option<String>,
    pub(crate) scope: String,
    /// The open folders; a message may have named one by a number.
    pub(crate) open: Vec<Value>,
    pub(crate) read: Option<String>,
}

/// `$XDG_CACHE_HOME/moonframe/projects/<profile>.json`. Outside a profile,
/// `default`; byte by byte, so a name's file is the one it always was.
pub(crate) fn file(home: &str, profile: Option<&str>) -> PathBuf {
    let cache = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| Path::new(home).join(".cache"));
    let name: String = profile
        .unwrap_or("default")
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' => byte as char,
            _ => '-',
        })
        .collect();
    cache
        .join("moonframe/projects")
        .join(format!("{name}.json"))
}

impl Kept {
    pub(crate) fn restore(file: &Path) -> Self {
        let kept: Value = fs::read(file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Value::Null);
        let word = |key: &str| kept.get(key).and_then(Value::as_str).map(str::to_owned);
        let open = kept.get("open").and_then(Value::as_array).cloned();
        let open = open.unwrap_or_default().into_iter();
        Self {
            select: word("select").filter(|path| Path::new(path).is_dir()),
            scope: word("scope")
                .filter(|scope| scope_known(scope))
                .unwrap_or_else(|| "uncommitted".to_owned()),
            open: open
                .filter(|path| !(path.is_array() || path.is_object() || path.is_boolean()))
                .collect(),
            read: word("read"),
        }
    }

    /// Best effort: it only saves asking again.
    pub(crate) fn keep(&self, file: &Path) {
        let text = json!({ "select": self.select, "scope": self.scope, "open": self.open,
            "read": self.read });
        let temporary = file.with_extension("json.tmp");
        if let Some(folder) = file.parent() {
            let _ = fs::create_dir_all(folder);
        }
        if fs::write(&temporary, text.to_string()).is_ok() {
            let _ = fs::rename(&temporary, file);
        }
    }
}
