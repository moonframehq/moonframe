//! On-demand pages of the original text, outside the bounded chat projection.

use {
    crate::Chat,
    herdr_chat_protocol::{EntryKind, ToolContent},
    serde_json::{Value, json},
};

// Even a page consisting entirely of four-byte characters fits a ui.text.
const PAGE_CHARS: usize = 4096;

pub(super) fn page(chat: &Chat, key: &str, page: usize) -> Result<Value, String> {
    let entry_id = key.strip_prefix('e').and_then(|id| id.parse::<u64>().ok());
    let tool = chat
        .transcript
        .as_ref()
        .and_then(|transcript| {
            transcript
                .entries
                .iter()
                .find_map(|entry| match &entry.kind {
                    EntryKind::Tool(tool) if Some(entry.id.0) == entry_id => Some(tool),
                    _ => None,
                })
        })
        .ok_or_else(|| "Tool output is unavailable".to_owned())?;
    let parts = tool.content.iter().filter_map(|part| match part {
        ToolContent::Text { text } if !text.is_empty() => Some(text.as_str()),
        _ => None,
    });
    let (text, more) = text_page(parts, page)?;
    Ok(json!({ "chat": chat.id, "key": key, "page": page, "text": text, "more": more }))
}

fn text_page<'a>(
    parts: impl Iterator<Item = &'a str>,
    page: usize,
) -> Result<(String, bool), String> {
    let mut chars = parts
        .enumerate()
        .flat_map(|(i, part)| (i > 0).then_some('\n').into_iter().chain(part.chars()))
        .skip(page.saturating_mul(PAGE_CHARS));
    let text: String = chars.by_ref().take(PAGE_CHARS).collect();
    if text.is_empty() {
        return Err("Tool output is unavailable".into());
    }
    Ok((text, chars.next().is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_output_across_parts_and_page_boundaries() {
        let first = "📩".repeat(PAGE_CHARS - 1);
        let parts = [first.as_str(), "second\n\npart"];
        assert_eq!(
            text_page(parts.into_iter(), 0),
            Ok((format!("{first}\n"), true))
        );
        assert_eq!(
            text_page(parts.into_iter(), 1),
            Ok(("second\n\npart".into(), false))
        );
        assert!(text_page(parts.into_iter(), 2).is_err());
    }
}
