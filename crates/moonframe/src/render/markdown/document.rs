//! One document on screen: the text view's state, kept across frames, and
//! what the mod asked of it (`find`, `reveal`, `outline`, `section`), carried
//! out as the view's parse lands. Laid out by the window, as only then is
//! there a window to keep state in.

use {
    super::{blocks, outline::Outline, search, send},
    crate::app::MoonframeApp,
    gpui_kit::{
        component::text::{
            MarkdownExtensions, RangeHighlight, RenderedText, TextView, TextViewMotion,
            TextViewState, TextViewStyle,
        },
        *,
    },
    moonframe_core::{Data, Markdown, Reveal, Style},
    std::{
        ops::Range,
        path::PathBuf,
        sync::{Arc, Mutex},
        time::Duration,
    },
};

/// How long streamed text takes to fade in, and the wait between its words.
const FADE: Duration = Duration::from_millis(240);
const FADE_STAGGER: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Colors {
    pub(super) find: Hsla,
    pub(super) current: Hsla,
}

/// What the document's custom blocks read as they draw, which happens after
/// this frame's `Draw` and off its stack: they run in the text view.
pub(super) struct Shared {
    pub(super) owner: String,
    pub(super) to: Option<String>,
    pub(super) app: WeakEntity<MoonframeApp>,
    pub(super) link: Option<Data>,
    pub(super) image: Option<Data>,
    pub(super) images: bool,
    pub(super) folder: PathBuf,
    pub(super) roots: Vec<PathBuf>,
    pub(super) alert: Style,
    pub(super) state: Option<WeakEntity<TextViewState>>,
    pub(super) scroll: bool,
}

pub(super) type SharedRef = Arc<Mutex<Shared>>;

/// What a search last found, and for what, so the same one isn't run twice.
#[derive(PartialEq)]
struct Searched {
    text: RenderedText,
    query: String,
    index: Option<usize>,
    colors: Colors,
}

/// What is kept for a document between frames.
struct Kept {
    shared: SharedRef,
    extensions: MarkdownExtensions,
    /// The text last handed to the view.
    text: String,
    /// The mod's wishes, as of the last frame.
    find: Option<String>,
    find_index: Option<usize>,
    found: Option<Data>,
    outline_msg: Option<Data>,
    section_msg: Option<Data>,
    colors: Option<Colors>,
    searched: Option<Searched>,
    count: Option<usize>,
    /// The match shown last, so moving on scrolls and a retyped query
    /// doesn't scroll back.
    revealed_match: Option<usize>,
    /// The last `reveal` taken, and one waiting for its text to parse.
    reveal: Option<Reveal>,
    pending: Option<usize>,
    /// One post-layout callback at a time, waiting for the list's items.
    revealing: bool,
    outline: Arc<Outline>,
    /// The text `outline` is of.
    outlined: Option<String>,
    outlining: Option<Task<()>>,
    section: Option<usize>,
    tracking: bool,
    _observing: Option<Subscription>,
}

/// A document, drawn when the window lays it out.
#[derive(IntoElement)]
pub(super) struct Document {
    pub(super) id: ElementId,
    pub(super) owner: String,
    pub(super) app: WeakEntity<MoonframeApp>,
    pub(super) markdown: Markdown,
    pub(super) folder: PathBuf,
    pub(super) roots: Vec<PathBuf>,
    pub(super) code: StyleRefinement,
    pub(super) inline_code: StyleRefinement,
    pub(super) quote: StyleRefinement,
    pub(super) alert: Style,
    pub(super) colors: Colors,
}

impl RenderOnce for Document {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let key = SharedString::from(format!("{}", self.id));
        let text = self.markdown.text.clone();
        let state = window.use_keyed_state(SharedString::from(format!("{key}/state")), cx, {
            let text = text.clone();
            move |_, cx| TextViewState::markdown(&text, cx)
        });
        let shared: SharedRef = Arc::new(Mutex::new(Shared {
            owner: self.owner.clone(),
            to: self.markdown.to.clone(),
            app: self.app.clone(),
            link: self.markdown.link.clone(),
            image: self.markdown.image.clone(),
            images: self.markdown.images,
            folder: self.folder.clone(),
            roots: self.roots.clone(),
            alert: self.alert.clone(),
            state: Some(state.downgrade()),
            scroll: self.markdown.scroll,
        }));
        let kept = window.use_keyed_state(SharedString::from(format!("{key}/kept")), cx, {
            let shared = shared.clone();
            let text = text.clone();
            move |_, _| Kept::new(shared, text)
        });
        // Custom blocks hold the first `Shared`; each frame fills it anew.
        let shared = kept.update(cx, |kept, _| {
            let current = kept.shared.clone();
            if !Arc::ptr_eq(&current, &shared)
                && let (Ok(mut into), Ok(mut from)) = (current.lock(), shared.lock())
            {
                std::mem::swap(&mut *into, &mut *from);
            }
            current
        });
        state.update(cx, |state, cx| state.set_text(&text, cx));
        kept.update(cx, |kept, cx| {
            kept.take(&self.markdown, self.colors, &state, window, cx);
            kept.settle(&state, window, cx);
            let rendered = state.read(cx).rendered_text();
            if kept.pending.is_some() && !kept.revealing && rendered.source() == kept.text {
                // Schedule from render, rather than the parse observer: the
                // list will lay out this version before the next frame.
                kept.reveal_after_layout(&state, rendered, window, cx);
            }
        });
        let extensions = kept.read(cx).extensions.clone();
        let mut view = TextView::new(&state)
            // A preview is for looking at: a click on it belongs to its card.
            .selectable(self.markdown.lines.is_none())
            .scrollable(self.markdown.scroll)
            .markdown_extensions(extensions)
            .image_source(blocks::image_source(&shared))
            .code_block_actions(blocks::code_actions)
            .style(TextViewStyle {
                code_block: self.code,
                ..TextViewStyle::default()
            })
            .inline_code_style(self.inline_code)
            .blockquote_style(self.quote)
            .on_link_click(blocks::link_click(&shared));
        if let Some(lines) = self.markdown.lines {
            view = view.max_lines(lines).on_link_click(|_, _, _, cx| {
                // TextView stops mouse-up propagation before calling us.
                // In a preview the original click belongs to its card.
                cx.propagate();
            });
        }
        if self.markdown.stream {
            view = view.motion(
                TextViewMotion::default()
                    .with_stream_fade(FADE)
                    .with_stream_fade_stagger(FADE_STAGGER),
            );
        }
        if self.markdown.scroll {
            view.size_full().into_any_element()
        } else {
            view.w_full().into_any_element()
        }
    }
}

impl Kept {
    fn new(shared: SharedRef, text: String) -> Self {
        Self {
            extensions: blocks::extensions(&shared),
            shared,
            text,
            find: None,
            find_index: None,
            found: None,
            outline_msg: None,
            section_msg: None,
            colors: None,
            searched: None,
            count: None,
            revealed_match: None,
            reveal: None,
            pending: None,
            revealing: false,
            outline: Arc::default(),
            outlined: None,
            outlining: None,
            section: None,
            tracking: false,
            _observing: None,
        }
    }

    /// Takes this frame's props, and starts what they ask for.
    fn take(
        &mut self,
        markdown: &Markdown,
        colors: Colors,
        state: &Entity<TextViewState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.text.clone_from(&markdown.text);
        self.find.clone_from(&markdown.find);
        self.find_index = markdown.find_index;
        self.found.clone_from(&markdown.found);
        self.outline_msg.clone_from(&markdown.outline);
        self.section_msg.clone_from(&markdown.section);
        self.colors = Some(colors);
        if markdown.reveal != self.reveal {
            self.reveal = markdown.reveal;
            self.pending = markdown.reveal.map(|reveal| reveal.at);
        }
        if self._observing.is_none() {
            // A parse lands in the view's own time, not in a frame of ours.
            self._observing = Some(cx.observe_in(state, window, |kept, state, window, cx| {
                kept.settle(&state, window, cx);
            }));
        }
        let wants_outline = self.outline_msg.is_some() || self.section_msg.is_some();
        if wants_outline && self.outlined.as_deref() != Some(self.text.as_str()) {
            self.outlined = Some(self.text.clone());
            let text = self.text.clone();
            let parsed = cx.background_spawn(async move { Outline::parse(&text) });
            self.outlining = Some(cx.spawn_in(window, async move |kept, cx| {
                let outline = parsed.await;
                kept.update_in(cx, |kept, window, cx| kept.outlined_as(outline, window, cx))
                    .ok();
            }));
        }
        if self.section_msg.is_some() && !self.tracking {
            self.tracking = true;
            let kept = cx.entity().downgrade();
            state
                .read(cx)
                .list_state()
                .set_scroll_handler(move |event, window, cx| {
                    let block = event.visible_range.start;
                    kept.update(cx, |kept, cx| kept.scrolled(block, window, cx))
                        .ok();
                });
        }
    }

    /// Takes a finished outline, and tells the mod if its headings moved.
    fn outlined_as(&mut self, outline: Outline, window: &mut Window, cx: &mut Context<Self>) {
        if outline.headings == self.outline.headings {
            self.outline = Arc::new(outline);
            return;
        }
        self.outline = Arc::new(outline);
        self.section = None;
        if let Some(msg) = &self.outline_msg {
            let headings = self.outline.headings.iter().map(|h| h.data()).collect();
            let msg = Markdown::note(msg, [("headings", Data::List(headings))]);
            self.tell(msg, window, cx);
        }
    }

    /// A scrolling document reached top-level block `block`.
    fn scrolled(&mut self, block: usize, window: &mut Window, cx: &mut Context<Self>) {
        let section = self.outline.section(block);
        if section == self.section {
            return;
        }
        self.section = section;
        if let (Some(msg), Some(section)) = (&self.section_msg, section) {
            let msg = Markdown::note(msg, [("heading", Data::Integer(section as i64 + 1))]);
            self.tell(msg, window, cx);
        }
    }

    fn tell(&self, msg: Data, window: &mut Window, cx: &mut App) {
        let Ok(shared) = self.shared.lock() else {
            return;
        };
        send(
            &shared.app,
            &shared.owner,
            shared.to.clone(),
            msg,
            window,
            cx,
        );
    }

    /// Updates the search and current section as the view's parse lands.
    fn settle(
        &mut self,
        state: &Entity<TextViewState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rendered = state.read(cx).rendered_text();
        self.search(state, rendered, window, cx);
        // A reveal moves the view without a scroll event.
        if self.section_msg.is_some() {
            let block = state.read(cx).list_state().logical_scroll_top().item_ix;
            self.scrolled(block, window, cx);
        }
    }

    /// Parsing precedes layout. `TextView` resets its list when the block
    /// count changes, so a jump before that reset would be lost.
    fn reveal_after_layout(
        &mut self,
        state: &Entity<TextViewState>,
        rendered: RenderedText,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        self.revealing = true;
        let kept = cx.entity().downgrade();
        let state = state.downgrade();
        window.on_next_frame(move |window, cx| {
            let Some(state) = state.upgrade() else {
                return;
            };
            kept.update(cx, |kept, cx| {
                kept.revealing = false;
                let current = state.read(cx).rendered_text();
                // A newer parse or request may have arrived in the meantime.
                if current == rendered
                    && current.source() == kept.text
                    && let Some(at) = kept.pending.take()
                {
                    let scroll = kept.shared.lock().is_ok_and(|shared| shared.scroll);
                    let outline = Outline::parse(current.source());
                    go_to(&state, &current, &outline, at, scroll, cx);
                }
                kept.settle(&state, window, cx);
                if kept.pending.is_some() {
                    cx.notify();
                }
            })
            .ok();
        });
    }

    fn search(
        &mut self,
        state: &Entity<TextViewState>,
        rendered: RenderedText,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(colors) = self.colors else {
            return;
        };
        let query = self.find.clone().unwrap_or_default();
        let wanted = Searched {
            text: rendered.clone(),
            query: query.clone(),
            index: self.find_index,
            colors,
        };
        if self.searched.as_ref() == Some(&wanted) {
            return;
        }
        // Nothing wanted and nothing marked: leave the view alone.
        if query.is_empty() && self.searched.is_none() {
            return;
        }
        self.searched = Some(wanted);
        let found = search::matches(rendered.as_str(), &query);
        let current = self
            .find_index
            .and_then(|index| index.checked_sub(1))
            .filter(|index| *index < found.len());
        let marks = found.iter().enumerate().map(|(index, range)| {
            let color = if Some(index) == current {
                colors.current
            } else {
                colors.find
            };
            RangeHighlight::new(range.clone(), color)
        });
        let marks: Vec<_> = marks.collect();
        let moved = current != self.revealed_match;
        self.revealed_match = current;
        state.update(cx, |state, cx| {
            state.set_range_highlights(marks, cx).ok();
            if moved && let Some(range) = current.and_then(|index| found.get(index)) {
                state.reveal_range(range.clone(), cx).ok();
            }
        });
        if self.count != Some(found.len()) {
            self.count = Some(found.len());
            if let Some(msg) = &self.found {
                let msg = Markdown::note(msg, [("count", Data::Integer(found.len() as i64))]);
                self.tell(msg, window, cx);
            }
        }
    }
}

/// Takes the reader to the byte `at` of the source. A scrolling document
/// puts its block at the top, as a jump to a heading should; `reveal_range`
/// would stop as soon as the place shows, at the bottom edge.
pub(super) fn go_to(
    state: &Entity<TextViewState>,
    rendered: &RenderedText,
    outline: &Outline,
    at: usize,
    scroll: bool,
    cx: &mut App,
) {
    if scroll {
        state.read(cx).list_state().scroll_to(ListOffset {
            item_ix: outline.block_at(at),
            offset_in_item: px(0.),
        });
        state.update(cx, |_, cx| cx.notify());
    } else if let Some(range) = place(rendered, at) {
        state.update(cx, |state, cx| state.reveal_range(range, cx).ok());
    }
}

/// The start of what is rendered from `at` in the source on: a heading's
/// marks render nothing, so the line is asked for, not one byte.
fn place(rendered: &RenderedText, at: usize) -> Option<Range<usize>> {
    let source = rendered.source();
    let at = (0..=at.min(source.len()))
        .rev()
        .find(|at| source.is_char_boundary(*at))?;
    let end = source[at..].find('\n').map_or(source.len(), |end| at + end);
    let range = rendered
        .range_for_source(at..end)
        .or_else(|| rendered.range_for_source(at..source.len()))?;
    Some(range.start..range.start)
}
