//! What a coding agent reads to build in an app started outside this
//! repository: the Luau API's types and the Moonframe skills. `moonframe
//! init` and `moonframe app new` write them, and they follow Moonframe's
//! version, so writing them again refreshes them.

use std::{fs, path::Path};

const TYPES: (&str, &str) = (
    "types/moonframe.d.luau",
    include_str!("../../../types/moonframe.d.luau"),
);
const SKILL: (&str, &str) = (
    ".claude/skills/moonframe/SKILL.md",
    include_str!("../../../.claude/skills/moonframe/SKILL.md"),
);
const COMPONENTS_SKILL: (&str, &str) = (
    ".claude/skills/moonframe-components/SKILL.md",
    include_str!("../../../.claude/skills/moonframe-components/SKILL.md"),
);
const APPS_SKILL: (&str, &str) = (
    ".claude/skills/moonframe-apps/SKILL.md",
    include_str!("../../../.claude/skills/moonframe-apps/SKILL.md"),
);

/// For a crate on the library (`moonframe init`).
pub(crate) const LIBRARY: &[(&str, &str)] = &[TYPES, SKILL, COMPONENTS_SKILL];

/// For an app run as a profile (`moonframe app new`), which also needs the
/// guide to profiles, `moonframe.toml` and bundles.
pub(crate) const PROFILE: &[(&str, &str)] = &[TYPES, SKILL, COMPONENTS_SKILL, APPS_SKILL];

/// What `refresh` did to one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Current,
    Updated,
    Wrote,
}

impl Outcome {
    pub(crate) fn verb(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Updated => "updated",
            Self::Wrote => "wrote",
        }
    }
}

/// Writes `files` (path relative to `dir`, text) into `dir`, replacing any
/// older copy, and says what it did to each.
pub(crate) fn refresh(
    dir: &Path,
    files: &[(&'static str, &str)],
) -> Result<Vec<(&'static str, Outcome)>, String> {
    files
        .iter()
        .map(|&(relative, text)| {
            let path = dir.join(relative);
            if fs::read_to_string(&path).is_ok_and(|old| old == text) {
                return Ok((relative, Outcome::Current));
            }
            let outcome = if path.exists() {
                Outcome::Updated
            } else {
                Outcome::Wrote
            };
            write(&path, text)?;
            Ok((relative, outcome))
        })
        .collect()
}

/// Writes `text` to `path`, making its folders.
pub(crate) fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn writes_then_refreshes() {
        let dir = tempfile::tempdir().unwrap();
        let files = [("a/b.md", "one")];
        assert_eq!(refresh(dir.path(), &files).unwrap(), [(
            "a/b.md",
            Outcome::Wrote
        )]);
        assert_eq!(refresh(dir.path(), &files).unwrap(), [(
            "a/b.md",
            Outcome::Current
        )]);
        fs::write(dir.path().join("a/b.md"), "old").unwrap();
        assert_eq!(refresh(dir.path(), &files).unwrap(), [(
            "a/b.md",
            Outcome::Updated
        )]);
        assert_eq!(
            fs::read_to_string(dir.path().join("a/b.md")).unwrap(),
            "one"
        );
    }
}
