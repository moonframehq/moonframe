//! A link's tooltip: what the mod's `tooltips` says for its address, else
//! the link's own title (`[text](url "title")`), else none. A wrapped
//! address (a mail scanner's) can show where it leads while the link stays
//! as it is.
//!
//! The text view asks for a tooltip by address alone, and keeps the view
//! it got for that address, so titles are read by address from the
//! document's current text (one shows only while every link to that
//! address carries the same title, never a sibling's), and the tooltip
//! reads them, and the mod's, as it draws.

use {
    ::markdown::{ParseOptions, mdast::Node, to_mdast},
    gpui_kit::{component::tooltip::Tooltip, *},
    std::{
        collections::{BTreeMap, HashMap},
        sync::{
            Arc, PoisonError, RwLock,
            atomic::{AtomicU64, Ordering},
        },
    },
};

/// The document's link titles, by address, as of its latest text.
type Titles = Arc<RwLock<HashMap<String, String>>>;

/// What a document's links show: the mod's `tooltips` and the links'
/// titles, both as of the latest frame.
#[derive(Clone, Default)]
pub(super) struct Tips {
    given: Arc<RwLock<BTreeMap<String, String>>>,
    titles: Titles,
}

/// Keeps a document's [`Tips`] current: the mod's as each frame gives
/// them, the titles read from its current text, off the UI thread.
#[derive(Default)]
pub(super) struct Titling {
    tips: Tips,
    /// The text `titles` are being read from, or were.
    of: Option<String>,
    /// Counts the texts read: a read publishes only while its text is
    /// still the latest, so an older one finishing last changes nothing.
    latest: Arc<AtomicU64>,
    reading: Option<Task<()>>,
}

impl Titling {
    pub(super) fn tips(&self) -> Tips {
        self.tips.clone()
    }

    /// Takes the mod's `given` tooltips, and reads the titles of `text`
    /// again when it changed.
    pub(super) fn follow(&mut self, text: &str, given: &BTreeMap<String, String>, cx: &App) {
        {
            let mut kept = self
                .tips
                .given
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            if *kept != *given {
                kept.clone_from(given);
            }
        }
        if self.of.as_deref() == Some(text) {
            return;
        }
        self.of = Some(text.to_owned());
        let mine = self.latest.fetch_add(1, Ordering::AcqRel) + 1;
        let (titles, latest) = (Arc::clone(&self.tips.titles), Arc::clone(&self.latest));
        // Nothing to read in text with no link or link definition.
        if !text.contains("](") && !text.contains("]:") {
            self.reading = None;
            publish(&titles, &latest, mine, HashMap::new());
            return;
        }
        let text = text.to_owned();
        self.reading = Some(cx.background_spawn(async move {
            publish(&titles, &latest, mine, titles_of(&text));
        }));
    }
}

/// Puts `found`, read from text number `mine`, in `titles`, unless a later
/// text is wanted by now; whether it did.
fn publish(titles: &Titles, latest: &AtomicU64, mine: u64, found: HashMap<String, String>) -> bool {
    let mut titles = titles.write().unwrap_or_else(PoisonError::into_inner);
    // Checked under the lock: a later read publishing meanwhile waits.
    if latest.load(Ordering::Acquire) != mine {
        return false;
    }
    *titles = found;
    true
}

/// Each address's title in `source`, for addresses whose every link has
/// the same one.
fn titles_of(source: &str) -> HashMap<String, String> {
    let mut options = ParseOptions::gfm();
    options.constructs.frontmatter = true;
    let Ok(root) = to_mdast(source, &options) else {
        return HashMap::new();
    };
    let mut links = Vec::new();
    let mut definitions = HashMap::new();
    let mut references = Vec::new();
    gather(&root, &mut links, &mut definitions, &mut references);
    links.extend(
        references
            .iter()
            .filter_map(|identifier| definitions.get(identifier).cloned()),
    );
    // Per address: its one title, or `None` once two of its links differ.
    let mut each: HashMap<String, Option<Option<String>>> = HashMap::new();
    for (url, title) in links {
        each.entry(url)
            .and_modify(|seen| {
                if seen.as_ref() != Some(&title) {
                    *seen = None;
                }
            })
            .or_insert(Some(title));
    }
    each.into_iter()
        .filter_map(|(url, title)| Some((url, title.flatten()?)))
        .collect()
}

/// The links under `node`, with their titles; the link definitions, by
/// identifier; and the identifiers the reference links name.
fn gather(
    node: &Node,
    links: &mut Vec<(String, Option<String>)>,
    definitions: &mut HashMap<String, (String, Option<String>)>,
    references: &mut Vec<String>,
) {
    let titled = |title: &Option<String>| title.clone().filter(|title| !title.is_empty());
    match node {
        Node::Link(link) => links.push((link.url.clone(), titled(&link.title))),
        Node::Definition(definition) => {
            definitions.insert(
                definition.identifier.clone(),
                (definition.url.clone(), titled(&definition.title)),
            );
        },
        Node::LinkReference(reference) => references.push(reference.identifier.clone()),
        _ => {},
    }
    for child in node.children().into_iter().flatten() {
        gather(child, links, definitions, references);
    }
}

/// What the view shows over a link to `url`: the mod's tooltip or the
/// link's title as they are when it draws, or nothing.
pub(super) fn builder(
    tips: Tips,
) -> impl Fn(&SharedString, &mut Window, &mut App) -> AnyView + Send + Sync + 'static {
    move |url, _, cx| {
        cx.new(|_| Tip {
            url: url.to_string(),
            tips: tips.clone(),
            shown: None,
        })
        .into()
    }
}

/// A link's tooltip, which reads what to say each time it draws: the view
/// keeps it while the pointer stays on links to that address.
struct Tip {
    url: String,
    tips: Tips,
    /// The tooltip drawn, and what it says.
    shown: Option<(String, AnyView)>,
}

impl Render for Tip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(text) = text(&self.tips, &self.url) else {
            self.shown = None;
            return div();
        };
        if self.shown.as_ref().is_none_or(|(shown, _)| *shown != text) {
            let tip = Tooltip::new(text.clone()).build(window, cx);
            self.shown = Some((text, tip));
        }
        let tip = self.shown.as_ref().map(|(_, tip)| tip.clone());
        // A name tests find it by.
        div()
            .debug_selector(|| "markdown-link-tooltip".to_owned())
            .children(tip)
    }
}

/// The tooltip of a link to `url`: the mod's, else its title.
fn text(tips: &Tips, url: &str) -> Option<String> {
    let given = tips.given.read().unwrap_or_else(PoisonError::into_inner);
    given.get(url).cloned().or_else(|| {
        tips.titles
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(url)
            .cloned()
    })
}

#[cfg(test)]
mod tests {
    use {
        super::{Tips, publish, text, titles_of},
        std::{collections::HashMap, sync::PoisonError},
    };

    #[test]
    fn the_mods_tooltip_comes_before_the_links_title() {
        let tips = Tips::default();
        tips.titles
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .extend([
                ("https://a.example".to_owned(), "A's title".to_owned()),
                ("https://b.example".to_owned(), "B's title".to_owned()),
            ]);
        tips.given
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert("https://b.example".to_owned(), "Leads to b".to_owned());
        let text = |url| text(&tips, url);
        assert_eq!(text("https://a.example").as_deref(), Some("A's title"));
        assert_eq!(text("https://b.example").as_deref(), Some("Leads to b"));
        assert_eq!(text("https://c.example"), None);
    }

    #[test]
    fn an_address_has_a_title_only_while_all_its_links_agree() {
        let found = titles_of(
            "[a](https://a.example \"A\") and [again](https://a.example \"A\")\n\n\
             [b](https://b.example \"First\") [b2](https://b.example \"Second\")\n\n\
             [c](https://c.example \"C\") [bare](https://c.example)\n\n\
             > [!NOTE]\n> [d](https://d.example \"In a note\")\n\n\
             [e][ref]\n\n[ref]: https://e.example \"By reference\"\n",
        );
        let expected: HashMap<String, String> = [
            ("https://a.example", "A"),
            ("https://d.example", "In a note"),
            ("https://e.example", "By reference"),
        ]
        .into_iter()
        .map(|(url, title)| (url.to_owned(), title.to_owned()))
        .collect();
        assert_eq!(found, expected);
        assert!(titles_of("[a](https://a.example)").is_empty());
    }

    #[test]
    fn an_older_read_finishing_last_leaves_the_newer_titles() {
        let tips = Tips::default();
        let latest = std::sync::atomic::AtomicU64::new(2);
        let titled =
            |title: &str| HashMap::from([("https://a.example".to_owned(), title.to_owned())]);
        // Text 2's read lands first, then text 1's, which is no longer wanted.
        assert!(publish(&tips.titles, &latest, 2, titled("New")));
        assert!(!publish(&tips.titles, &latest, 1, titled("Old")));
        assert_eq!(text(&tips, "https://a.example").as_deref(), Some("New"));
    }
}
