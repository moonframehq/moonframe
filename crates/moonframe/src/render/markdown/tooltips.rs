//! A link's tooltip: what the mod's `tooltips` says for its address, else
//! the link's own title (`[text](url "title")`), else none. A wrapped
//! address (a mail scanner's) can show where it leads while the link stays
//! as it is.

use {
    gpui_kit::{
        component::{
            text::{MarkdownNode, MarkdownParseContext, MarkdownPlugin, markdown_ast::Node},
            tooltip::Tooltip,
        },
        *,
    },
    std::{
        collections::{BTreeMap, HashMap},
        sync::{Arc, Mutex},
    },
};

/// The titles of the document's links, by address, as its text parses.
pub(super) type Titles = Arc<Mutex<HashMap<String, String>>>;

/// Notes each link's title as the text parses, and leaves the link to
/// whatever draws it.
pub(super) struct LinkTitles(pub(super) Titles);

impl MarkdownPlugin for LinkTitles {
    fn name(&self) -> &str {
        "moonframe-link-titles"
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        if let Node::Link(link) = node
            && let Some(title) = link.title.as_ref().filter(|title| !title.is_empty())
            && let Ok(mut titles) = self.0.lock()
        {
            titles.insert(link.url.clone(), title.clone());
        }
        None
    }
}

/// What the view shows over a link to `url`: an empty view when nothing
/// names it.
pub(super) fn builder(
    given: BTreeMap<String, String>,
    titles: Titles,
) -> impl Fn(&SharedString, &mut Window, &mut App) -> AnyView + Send + Sync + 'static {
    move |url, window, cx| match text(&given, &titles, url) {
        Some(text) => {
            let tip = Tooltip::new(text).build(window, cx);
            cx.new(|_| Tip(tip)).into()
        },
        None => cx.new(|_| Empty).into(),
    }
}

/// The tooltip, under a name tests find it by.
struct Tip(AnyView);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "markdown-link-tooltip".to_owned())
            .child(self.0.clone())
    }
}

/// The tooltip of a link to `url`: the mod's, else its title.
fn text(given: &BTreeMap<String, String>, titles: &Titles, url: &str) -> Option<String> {
    given.get(url).cloned().or_else(|| {
        titles
            .lock()
            .ok()
            .and_then(|titles| titles.get(url).cloned())
    })
}

#[cfg(test)]
mod tests {
    use {
        super::{Titles, text},
        std::collections::BTreeMap,
    };

    #[test]
    fn the_mods_tooltip_comes_before_the_links_title() {
        let titles = Titles::default();
        if let Ok(mut into) = titles.lock() {
            into.extend([
                ("https://a.example".to_owned(), "A's title".to_owned()),
                ("https://b.example".to_owned(), "B's title".to_owned()),
            ]);
        }
        let given = BTreeMap::from([("https://b.example".to_owned(), "Leads to b".to_owned())]);
        let text = |url| text(&given, &titles, url);
        assert_eq!(text("https://a.example").as_deref(), Some("A's title"));
        assert_eq!(text("https://b.example").as_deref(), Some("Leads to b"));
        assert_eq!(text("https://c.example"), None);
    }
}
