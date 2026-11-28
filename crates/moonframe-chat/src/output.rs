//! On-demand pages of the original text, outside the bounded chat projection.
//!
//! Page byte boundaries are computed once per tool entry revision outside the
//! shared chat lock so paging later pages is O(1) without rescanning earlier
//! characters or stalling other chats.

use {
    crate::{Chat, State, lock},
    herdr_chat_protocol::{Change, EntryId, EntryKind, ToolContent},
    serde_json::{Value, json},
    std::{collections::HashMap, sync::Mutex},
};

// Even a page consisting entirely of four-byte characters fits a ui.text.
const PAGE_CHARS: usize = 4096;

/// Tracks transcript mutations that can invalidate a cached tool output.
#[derive(Default)]
pub(crate) struct Revisions {
    epoch: u64,
    entries: HashMap<EntryId, u64>,
}

impl Revisions {
    #[cfg_attr(not(unix), allow(dead_code))]
    pub(crate) fn on_snapshot(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.entries.clear();
    }

    #[cfg_attr(not(unix), allow(dead_code))]
    pub(crate) fn on_change(&mut self, change: &Change) {
        match change {
            Change::Replace { entry } => {
                let rev = self.entries.entry(entry.id).or_default();
                *rev = rev.wrapping_add(1);
            },
            Change::Truncate { before } => {
                self.entries.retain(|id, _| id >= before);
            },
            _ => {},
        }
    }

    pub(super) fn for_entry(&self, id: EntryId) -> (u64, u64) {
        (self.epoch, self.entries.get(&id).copied().unwrap_or(0))
    }
}

/// Result of inspecting a chat under the shared lock.
pub(super) enum Lookup {
    Cached,
    Fresh {
        chat_id: String,
        entry_id: EntryId,
        revision: (u64, u64),
        parts: Vec<String>,
    },
}

/// Pre-indexed tool output text cached on the provider thread outside the lock.
pub(crate) struct Cached {
    chat_id: String,
    entry_id: EntryId,
    revision: (u64, u64),
    text: String,
    /// Byte offset of each page start in `text`, ending with `text.len()`.
    pages: Vec<usize>,
}

impl Cached {
    pub(super) fn chat_id(&self) -> &str {
        &self.chat_id
    }

    pub(super) fn matches(&self, chat_id: &str, entry_id: EntryId, revision: (u64, u64)) -> bool {
        self.chat_id == chat_id && self.entry_id == entry_id && self.revision == revision
    }

    pub(super) fn build(
        chat_id: String,
        entry_id: EntryId,
        revision: (u64, u64),
        parts: Vec<String>,
    ) -> Result<Self, String> {
        let (text, pages) = index_parts(&parts)?;
        Ok(Self {
            chat_id,
            entry_id,
            revision,
            text,
            pages,
        })
    }

    pub(super) fn page(&self, key: &str, page: usize) -> Result<Value, String> {
        let (text, more) = slice_page(&self.text, &self.pages, page)?;
        Ok(json!({
            "chat": self.chat_id,
            "key": key,
            "page": page,
            "text": text,
            "more": more,
        }))
    }
}

/// Reads one page of a tool entry's original text, holding `shared` only long
/// enough to verify ownership and inspect or copy raw parts.
pub(super) fn page(
    shared: &Mutex<State>,
    cache: &mut Option<Cached>,
    owner: &str,
    chat_id: &str,
    key: &str,
    page: usize,
) -> Result<Value, String> {
    let found = {
        let state = lock(shared);
        state
            .chat(chat_id)
            .filter(|chat| chat.owner == owner)
            .ok_or_else(|| "no such chat".to_owned())
            .and_then(|chat| lookup(chat, key, cache.as_ref()))
    }?;
    match found {
        Lookup::Cached => cache
            .as_ref()
            .ok_or_else(|| "Tool output is unavailable".to_owned())
            .and_then(|cached| cached.page(key, page)),
        Lookup::Fresh {
            chat_id,
            entry_id,
            revision,
            parts,
        } => {
            let cached = cache.insert(Cached::build(chat_id, entry_id, revision, parts)?);
            cached.page(key, page)
        },
    }
}

/// Checks the cache or copies the raw text parts while holding the shared lock;
/// joining and UTF-8 page indexing happen afterwards outside the lock.
pub(super) fn lookup(chat: &Chat, key: &str, cached: Option<&Cached>) -> Result<Lookup, String> {
    let entry_id = key
        .strip_prefix('e')
        .and_then(|id| id.parse::<u64>().ok())
        .map(EntryId)
        .ok_or_else(|| "Tool output is unavailable".to_owned())?;
    let tool = chat
        .transcript
        .as_ref()
        .and_then(|transcript| transcript.entry(entry_id))
        .and_then(|entry| match &entry.kind {
            EntryKind::Tool(tool) => Some(tool),
            _ => None,
        })
        .ok_or_else(|| "Tool output is unavailable".to_owned())?;
    let revision = chat.output_revisions.for_entry(entry_id);
    if cached.is_some_and(|cached| cached.matches(&chat.id, entry_id, revision)) {
        return Ok(Lookup::Cached);
    }
    let parts: Vec<String> = tool
        .content
        .iter()
        .filter_map(|part| match part {
            ToolContent::Text { text } if !text.is_empty() => Some(text.clone()),
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        return Err("Tool output is unavailable".to_owned());
    }
    Ok(Lookup::Fresh {
        chat_id: chat.id.clone(),
        entry_id,
        revision,
        parts,
    })
}

fn index_parts<S: AsRef<str>>(parts: &[S]) -> Result<(String, Vec<usize>), String> {
    let total_bytes: usize = parts
        .iter()
        .map(|part| part.as_ref().len())
        .sum::<usize>()
        .saturating_add(parts.len().saturating_sub(1));
    if total_bytes == 0 {
        return Err("Tool output is unavailable".to_owned());
    }
    let mut text = String::with_capacity(total_bytes);
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str(part.as_ref());
    }
    let mut pages = vec![0];
    for (chars, (byte_idx, _)) in text.char_indices().enumerate() {
        if chars > 0 && chars % PAGE_CHARS == 0 {
            pages.push(byte_idx);
        }
    }
    pages.push(text.len());
    Ok((text, pages))
}

fn slice_page<'a>(text: &'a str, pages: &[usize], page: usize) -> Result<(&'a str, bool), String> {
    let Some(&start) = pages.get(page) else {
        return Err("Tool output is unavailable".to_owned());
    };
    let Some(&end) = pages.get(page + 1) else {
        return Err("Tool output is unavailable".to_owned());
    };
    let slice = &text[start..end];
    if slice.is_empty() {
        return Err("Tool output is unavailable".to_owned());
    }
    Ok((slice, page + 2 < pages.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_output_across_parts_and_page_boundaries() -> Result<(), String> {
        let first = "📩".repeat(PAGE_CHARS - 1);
        let (text, pages) = index_parts(&[first.as_str(), "second\n\npart"])?;
        assert_eq!(
            slice_page(&text, &pages, 0),
            Ok((format!("{first}\n").as_str(), true))
        );
        assert_eq!(slice_page(&text, &pages, 1), Ok(("second\n\npart", false)));
        assert!(slice_page(&text, &pages, 2).is_err());
        Ok(())
    }

    #[test]
    fn exact_page_multiple_has_no_trailing_empty_page() -> Result<(), String> {
        let exact = "a".repeat(PAGE_CHARS * 2);
        let (text, pages) = index_parts(&[exact.as_str()])?;
        assert_eq!(pages.len(), 3);
        let (page0, more0) = slice_page(&text, &pages, 0)?;
        assert_eq!(page0.len(), PAGE_CHARS);
        assert!(more0);
        let (page1, more1) = slice_page(&text, &pages, 1)?;
        assert_eq!(page1.len(), PAGE_CHARS);
        assert!(!more1);
        assert!(slice_page(&text, &pages, 2).is_err());
        Ok(())
    }

    #[test]
    fn revisions_invalidate_only_on_snapshot_or_matching_replace() {
        let mut revisions = Revisions::default();
        let initial = revisions.for_entry(EntryId(2));
        revisions.on_change(&Change::Append {
            id: EntryId(3),
            text: "more".to_owned(),
        });
        assert_eq!(revisions.for_entry(EntryId(2)), initial);

        revisions.on_change(&Change::Replace {
            entry: herdr_chat_protocol::Entry {
                id: EntryId(5),
                kind: EntryKind::Agent {
                    text: "other".to_owned(),
                },
            },
        });
        assert_eq!(revisions.for_entry(EntryId(2)), initial);

        revisions.on_change(&Change::Replace {
            entry: herdr_chat_protocol::Entry {
                id: EntryId(2),
                kind: EntryKind::Agent {
                    text: "updated".to_owned(),
                },
            },
        });
        let replaced = revisions.for_entry(EntryId(2));
        assert_ne!(replaced, initial);

        revisions.on_snapshot();
        assert_ne!(revisions.for_entry(EntryId(2)), replaced);
    }
}
