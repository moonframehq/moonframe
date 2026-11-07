//! Finding a file by a few letters, as a quick open does.

use {
    super::{
        Repo,
        files::{Listing, walk},
    },
    crate::git::git,
    serde_json::json,
    std::time::{Duration, Instant},
};

const MAX_FOUND: usize = 40;
const LISTING_KEPT: Duration = Duration::from_secs(15);

/// Characters in order, the closer together and the nearer the name the
/// better: lower is better, `None` is no match.
fn score(path: &str, query: &str) -> Option<f64> {
    let lower: Vec<char> = path.to_lowercase().chars().collect();
    let wanted: Vec<char> = query.chars().collect();
    let mut at = 0;
    let mut total = 0_usize;
    let mut previous: Option<usize> = None;
    for letter in &wanted {
        let found = at + lower[at..].iter().position(|item| item == letter)?;
        if previous.is_none_or(|previous| found != previous + 1) {
            total += found - at + 1;
        }
        previous = Some(found);
        at = found + 1;
    }
    let name = lower
        .iter()
        .rposition(|item| *item == '/')
        .map_or(0, |slash| slash + 1);
    let in_name = lower[name..]
        .windows(wanted.len().max(1))
        .any(|window| window == wanted.as_slice());
    #[allow(
        clippy::cast_precision_loss,
        reason = "a path's length and a small score"
    )]
    Some(
        total as f64
            + if in_name {
                -100.0
            } else {
                0.0
            }
            + lower.len() as f64 / 100.0,
    )
}

impl Repo {
    fn all_files(&mut self, root: &str) -> Vec<String> {
        if let Some(listing) = &self.listing
            && listing.at == root
            && listing.time.elapsed() < LISTING_KEPT
        {
            return listing.files.clone();
        }
        let tracked = git(&self.runtime, root, &[
            "ls-files",
            "-co",
            "--exclude-standard",
        ]);
        let files = match tracked {
            Ok(text) => text.lines().map(str::to_owned).collect(),
            Err(_) => {
                let mut files = Vec::new();
                walk(root, "", 0, &mut files);
                files
            },
        };
        self.listing = Some(Listing {
            at: root.to_owned(),
            time: Instant::now(),
            files: files.clone(),
        });
        files
    }

    pub(super) fn find(&mut self, query: &str, root: &str) {
        let wanted = query.to_lowercase();
        let mut scored: Vec<(f64, String)> = self
            .all_files(root)
            .into_iter()
            .filter_map(|path| {
                #[allow(clippy::cast_precision_loss, reason = "a path's length")]
                let value = if wanted.is_empty() {
                    Some(path.len() as f64)
                } else {
                    score(&path, &wanted)
                };
                value.map(|value| (value, path))
            })
            .collect();
        scored.sort_by(|a, b| a.0.total_cmp(&b.0));
        let results: Vec<String> = scored
            .into_iter()
            .take(MAX_FOUND)
            .map(|(_, path)| path)
            .collect();
        self.found = json!({ "query": query, "results": results });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_must_come_in_order() {
        assert!(score("src/price.js", "pric").is_some());
        assert!(score("src/price.js", "cirp").is_none());
    }

    #[test]
    fn the_name_beats_the_folders() {
        let in_name = score("src/price.js", "price").unwrap_or(f64::MAX);
        let spread = score("pr/i/c/e/other.js", "price").unwrap_or(f64::MAX);
        assert!(in_name < spread);
    }
}
