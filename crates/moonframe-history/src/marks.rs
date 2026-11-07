//! Moments to come back to: a name for the snapshot a folder was at, such
//! as when an agent's run began. Unlike a snapshot a mark exists even when
//! nothing changed since the last one, so a run that began right after
//! another still has a start of its own.

use crate::{Error, History, Result};

/// Where marks are kept, each named after what it marks.
const MARKS: &str = "refs/moonframe/marks/";

impl History {
    /// Names the folder as it is now `name`, taking a snapshot of it saying
    /// `reason` if it changed. A name given again moves to now.
    pub fn mark(&self, name: &str, reason: &str) -> Result<()> {
        let reference = reference(name)?;
        let _lock = self.lock()?;
        self.capture_now(reason)?;
        let cursor = self
            .cursor()
            .ok_or_else(|| Error("the folder has no snapshot to mark".to_owned()))?;
        self.git
            .run(&["update-ref", "-m", reason, &reference, &cursor])
            .map(drop)
    }

    /// The snapshot marked `name`, if there is such a mark.
    pub fn marked(&self, name: &str) -> Option<String> {
        let reference = reference(name).ok()?;
        self.git.ask(&["rev-parse", "--verify", "-q", &reference])
    }

    /// Every mark's name, last in sort order first.
    pub fn marks(&self) -> Result<Vec<String>> {
        let listed = self.git.run(&[
            "for-each-ref",
            "--sort=-refname",
            "--format=%(refname)",
            MARKS,
        ])?;
        Ok(listed
            .lines()
            .filter_map(|line| line.strip_prefix(MARKS))
            .map(str::to_owned)
            .collect())
    }

    /// Whether the folder has moved from the mark `name`.
    pub fn moved_from(&self, name: &str) -> bool {
        self.marked(name)
            .is_some_and(|marked| self.cursor().as_deref() != Some(marked.as_str()))
    }

    /// Whether `id` names a snapshot in this history.
    pub fn has(&self, id: &str) -> bool {
        self.resolve(id).is_ok()
    }
}

/// The ref of the mark `name`, which must be fit to be one.
fn reference(name: &str) -> Result<String> {
    let fits = !name.is_empty()
        && !name.starts_with(['.', '-'])
        && !name.ends_with('.')
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    fits.then(|| format!("{MARKS}{name}"))
        .ok_or_else(|| Error(format!("cannot name a mark `{name}`")))
}
