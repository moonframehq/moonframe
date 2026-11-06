//! `syntax` on a `ui.text` in a build without highlighting: the text stays
//! plain, as for a language with no grammar.

use gpui_kit::{StyledText, component::Theme};

pub(super) fn styled(_: Option<&str>, _: &str, _: &Theme) -> Option<StyledText> {
    None
}
