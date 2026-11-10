//! The providers the user let read another app's Keychain item, such as
//! Zed's sign-in. Reading one makes macOS ask, so nothing asks until the
//! user presses Allow (`{ allow = "<id>" }`). The answer is kept across
//! launches, so "Always Allow" shows numbers at startup without asking
//! again; a denial takes the grant back so a later launch does not ask in
//! the background.

use {
    serde::{Deserialize, Serialize},
    serde_json::Value,
    sheprd_usage::{Provider, registry},
    std::{
        collections::{BTreeSet, HashSet},
        fs,
        path::{Path, PathBuf},
    },
};

/// A grants file larger than this is not one: it grants nothing.
const MAX_FILE_BYTES: u64 = 16 * 1024;

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    providers: BTreeSet<String>,
}

#[derive(Default)]
pub(crate) struct Grants {
    granted: HashSet<Provider>,
    path: Option<PathBuf>,
}

/// `[settings] grants`, else
/// `$XDG_STATE_HOME/moonframe/usage/<profile>-keychain.json` (by default
/// under `~/.local/state`), outside the folders the window watches.
pub(crate) fn path(settings: &Value) -> Option<PathBuf> {
    if let Some(path) = settings.get("grants").and_then(Value::as_str) {
        return Some(PathBuf::from(path));
    }
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    let profile = moonframe_providers::profile().unwrap_or("default");
    Some(
        state
            .join("moonframe/usage")
            .join(format!("{profile}-keychain.json")),
    )
}

impl Grants {
    /// What `path` grants; a missing or damaged file grants nothing.
    pub(crate) fn load(path: Option<PathBuf>) -> Self {
        let granted = path
            .as_deref()
            .and_then(read)
            .map(|bytes| parse(&bytes))
            .unwrap_or_default();
        Self { granted, path }
    }

    pub(crate) fn granted(&self) -> &HashSet<Provider> {
        &self.granted
    }

    pub(crate) fn grant(&mut self, provider: Provider) {
        if self.granted.insert(provider) {
            self.save();
        }
    }

    pub(crate) fn revoke(&mut self, provider: Provider) {
        if self.granted.remove(&provider) {
            self.save();
        }
    }

    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let saved = Saved {
            providers: self
                .granted
                .iter()
                .map(|provider| provider.id().to_owned())
                .collect(),
        };
        if let Err(error) = write(path, &saved) {
            tracing::warn!(%error, path = %path.display(), "usage: cannot save Keychain grants");
        }
    }
}

fn read(path: &Path) -> Option<Vec<u8>> {
    let size = fs::metadata(path).ok()?.len();
    if size > MAX_FILE_BYTES {
        tracing::warn!(path = %path.display(), "usage: Keychain grants file too large");
        return None;
    }
    fs::read(path).ok()
}

/// Known providers only: an id this build does not have grants nothing.
pub(crate) fn parse(bytes: &[u8]) -> HashSet<Provider> {
    serde_json::from_slice::<Saved>(bytes)
        .map(|saved| {
            saved
                .providers
                .iter()
                .filter_map(|id| registry::find(id))
                .collect()
        })
        .unwrap_or_default()
}

/// Written beside itself and moved into place, so a crash leaves the old
/// file or the new one, never half of one.
fn write(path: &Path, saved: &Saved) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let partial = path.with_extension("json.partial");
    fs::write(&partial, serde_json::to_vec_pretty(saved)?)?;
    fs::rename(&partial, path)
}
