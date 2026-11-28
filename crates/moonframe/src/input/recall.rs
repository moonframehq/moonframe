//! A field's `history`: Up with the caret on the first line recalls the
//! entry before the one shown, Down on the last line the one after, and
//! Down past the newest puts back what was being written. Typing anything
//! else ends the recall, so the next Up starts again from the newest.

/// Where a field is in its history while it shows a recalled entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Recall {
    /// The entry shown, from 0 (the oldest).
    at: usize,
    /// What the field held before the first Up.
    draft: String,
    /// The text recalled: an edit that leaves something else ends it.
    shown: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Older,
    Newer,
}

/// The text Up or Down puts in a field holding `value` with the caret
/// `caret` bytes in, moving `recall`; `None` when the key should move the
/// caret instead (not on the first or last line, or nothing to recall).
pub(crate) fn step(
    history: &[String],
    recall: &mut Option<Recall>,
    step: Step,
    value: &str,
    caret: usize,
) -> Option<String> {
    let caret = caret.min(value.len());
    if history.is_empty() {
        return None;
    }
    // A recall the user typed over has ended.
    if recall.as_ref().is_some_and(|recall| recall.shown != value) {
        *recall = None;
    }
    match step {
        Step::Older => {
            if value
                .get(..caret)
                .is_some_and(|before| before.contains('\n'))
            {
                return None;
            }
            let (at, draft) = match recall.take() {
                Some(recall) => (recall.at.saturating_sub(1), recall.draft),
                None => (history.len() - 1, value.to_owned()),
            };
            let shown = history.get(at)?.clone();
            *recall = Some(Recall {
                at,
                draft,
                shown: shown.clone(),
            });
            Some(shown)
        },
        Step::Newer => {
            if value.get(caret..).is_some_and(|after| after.contains('\n')) {
                return None;
            }
            let current = recall.take()?;
            if current.at + 1 >= history.len() {
                return Some(current.draft);
            }
            let at = current.at + 1;
            let shown = history.get(at)?.clone();
            *recall = Some(Recall {
                at,
                draft: current.draft,
                shown: shown.clone(),
            });
            Some(shown)
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn history() -> Vec<String> {
        vec!["first".to_owned(), "second\nline".to_owned()]
    }

    #[test]
    fn up_recalls_older_entries_and_down_comes_back_to_the_draft() {
        let history = history();
        let mut recall = None;
        let shown = step(&history, &mut recall, Step::Older, "draft", 5).unwrap();
        assert_eq!(shown, "second\nline");
        // The caret lands at the end, on the last line: Up there moves it.
        assert_eq!(
            step(&history, &mut recall, Step::Older, &shown, shown.len()),
            None
        );
        let shown = step(&history, &mut recall, Step::Older, &shown, 0).unwrap();
        assert_eq!(shown, "first");
        // The oldest stays.
        assert_eq!(
            step(&history, &mut recall, Step::Older, &shown, 0).as_deref(),
            Some("first")
        );
        let shown = step(&history, &mut recall, Step::Newer, "first", 5).unwrap();
        assert_eq!(shown, "second\nline");
        assert_eq!(
            step(&history, &mut recall, Step::Newer, &shown, shown.len()).as_deref(),
            Some("draft")
        );
        assert_eq!(recall, None);
        assert_eq!(step(&history, &mut recall, Step::Newer, "draft", 5), None);
    }

    #[test]
    fn typing_over_a_recalled_entry_ends_the_recall() {
        let history = history();
        let mut recall = None;
        step(&history, &mut recall, Step::Older, "", 0).unwrap();
        // Edited: Down has nothing to come back to, Up starts at the newest.
        assert_eq!(step(&history, &mut recall, Step::Newer, "changed", 7), None);
        assert_eq!(
            step(&history, &mut recall, Step::Older, "changed", 0).as_deref(),
            Some("second\nline")
        );
        assert_eq!(
            step(&history, &mut recall, Step::Newer, "second\nline", 11).as_deref(),
            Some("changed")
        );
    }

    #[test]
    fn up_below_the_first_line_or_with_no_history_moves_the_caret() {
        let mut recall = None;
        assert_eq!(step(&[], &mut recall, Step::Older, "", 0), None);
        assert_eq!(step(&history(), &mut recall, Step::Older, "a\nb", 3), None);
    }
}
