//! Keeps agents out of the installed app. Its files are read and copied
//! from, never changed: that breaks the bundle's signature, and the next
//! update takes the change away. A tool that says it writes there is
//! refused. A shell command names no paths the protocol shows, so after
//! each run the seal itself is checked.

use {
    acp::v1 as wire,
    moonframe_providers::Installed,
    serde_json::Value,
    std::{
        path::{Path, PathBuf},
        process::Command,
    },
};

/// The keys agents' file tools name their file with.
const PATH_KEYS: [&str; 4] = ["abs_path", "file_path", "notebook_path", "path"];

/// The files a tool call says it works on.
pub(crate) fn paths(
    content: &[wire::ToolCallContent],
    locations: &[wire::ToolCallLocation],
    input: Option<&Value>,
) -> Vec<PathBuf> {
    let diffs = content.iter().filter_map(|content| match content {
        wire::ToolCallContent::Diff(diff) => Some(diff.path.clone()),
        _ => None,
    });
    let located = locations.iter().map(|location| location.path.clone());
    let named = PATH_KEYS
        .iter()
        .filter_map(|key| input?.get(key)?.as_str())
        .map(PathBuf::from);
    diffs.chain(located).chain(named).collect()
}

/// The first of `paths` (from `cwd`) a tool of `kind` may not touch: any in
/// the bundle, unless the tool only looks. As with `allow`, a tool of no
/// known kind counts as one that changes things.
pub(crate) fn refused<'a>(
    installed: &Installed,
    kind: Option<&str>,
    paths: &'a [PathBuf],
    cwd: &Path,
) -> Option<&'a PathBuf> {
    if crate::run::Allow::Read.covers(kind) {
        return None;
    }
    paths.iter().find(|path| installed.holds(&cwd.join(path)))
}

/// Whether a signed `.app` no longer matches its signature. One never
/// signed has none to break, and elsewhere there is no bundle to verify.
pub(crate) fn broken(bundle: &Path) -> bool {
    if bundle.extension().is_none_or(|found| found != "app") {
        return false;
    }
    let Ok(output) = Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(bundle)
        .output()
    else {
        return false;
    };
    !output.status.success()
        && !String::from_utf8_lossy(&output.stderr).contains("not signed at all")
}
