//! `ui.document`: a page of HTML the host shows sealed, with no script and
//! no network, following the app's light or dark look: a briefing, a
//! shared file's preview, a note to edit in place.

use crate::{Data, Look};

/// The most HTML a document takes, in bytes: pictures in it come inline,
/// as `data:` addresses, since it loads nothing else.
pub const MAX_HTML: usize = 8 * 1024 * 1024;

/// A sealed document: its own bytes, shown with its scripts off and every
/// load but the document itself refused, its links told to the mod rather
/// than followed. Kept by its mod and `id`, as a browser is.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub look: Look,
    pub id: String,
    pub source: DocumentSource,
    /// Sent when a link is clicked, with `url` (and `text`) added: the
    /// document never follows one itself; a fragment (`#part`) scrolls it.
    pub link: Option<Data>,
    /// Sent as it scrolls, with `y`, `height` and `content_height` added,
    /// at most every 100 ms and always the last.
    pub scroll: Option<Data>,
    /// Where to scroll, when it changes.
    pub scroll_to: Option<DocumentScrollTo>,
    /// Sent as the selection changes, with `text` (empty once nothing is
    /// selected) and its box, `x`, `y`, `width` and `height`, in points
    /// from the box's top left as it shows.
    pub select: Option<Data>,
    /// Edited in place: the user types in it.
    pub editable: bool,
    /// Sent, while editable, with `html` (the whole document) added, a
    /// little after each edit.
    pub change: Option<Data>,
    /// Sent, while editable, with `html` added as the document loses the
    /// keyboard.
    pub done: Option<Data>,
}

/// What a document shows.
#[derive(Clone, Debug, PartialEq)]
pub enum DocumentSource {
    /// The HTML itself.
    Html(String),
    /// A file of HTML, named as `ui.image` names pictures.
    File(String),
}

/// A document's `scroll_to`, acted on when it changes: a new `seq` asks
/// again for the same place.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentScrollTo {
    pub to: DocumentScroll,
    pub seq: Data,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DocumentScroll {
    /// This many points down: a `y` its `scroll` told.
    Y(f32),
    /// The element with this `id`, brought to the top.
    Id(String),
}
