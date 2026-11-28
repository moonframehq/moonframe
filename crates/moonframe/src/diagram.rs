//! `ui.diagram`: moonframe-diagram's view, kept by mod and `id` like any
//! hosted view. The mod gives the document and the selection as props and
//! hears each edit as a message to its `update`, so it keeps the document
//! and its undo; the view never changes the document by itself.
//!
//! The user copies a diagram as Mermaid with ⌘C in it; a mod cannot put
//! anything on the clipboard by itself.
//!
//! Props ([`read`]): `mermaid` or `dot` text, `shapes` and `links` (an
//! editor's document, every shape placed) or `document` (nodes, edges,
//! groups, a sequence's messages and notes, laid out when unplaced), `mode`, `tool`,
//! `selected`, `highlight`, `camera`, `scroll`. Messages ([`tell`]):
//! `selected`, `moved`, `resized`, `created`, `connected`, `deleted`,
//! `edited`, `opened`, `zoom`, `tool`, the names the Luau `diagram`
//! component sends, so a mod can use either.

#[cfg(all(test, feature = "capture"))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod bench;
mod export;
#[cfg(all(test, feature = "capture"))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod export_tests;
mod read;
mod tell;

use {
    crate::hosted::{HostView, Messenger, Registry, ViewInit},
    gpui_kit::{component::ActiveTheme as _, *},
    moonframe_core::Data,
    moonframe_diagram::view::{DiagramEvent, DiagramView},
};

/// Registers the view as `diagram`, which `ui.diagram` names.
pub(crate) fn register(registry: &mut Registry) {
    registry.insert("diagram", DiagramHost::new);
}

pub(crate) struct DiagramHost {
    view: Entity<DiagramView>,
    /// The document as the mod last gave it, so a new selection alone does
    /// not lay it out again.
    document: Data,
    /// A document to show, waiting for a frame with a window to measure
    /// its labels in.
    waiting: Option<moonframe_diagram::Diagram>,
    /// Why the `mermaid` or `dot` text given is not a diagram.
    problem: Option<String>,
    /// The last `camera` request acted on, by its `seq`.
    camera_seq: Option<Data>,
    /// The last `export` request acted on, by its `seq`.
    export_seq: Option<Data>,
    /// Exports asked for, done once the view has laid the document out;
    /// a new document turns them down, as theirs is gone.
    asked: Vec<read::Hand>,
    /// Exports made, so each writes a file of its own.
    exports: u64,
    messenger: Messenger,
    /// The view's `id`, naming its exported files.
    name: String,
    _events: Subscription,
    /// Draws again when a background layout lands, for the exports
    /// waiting on it.
    _laid_out: Subscription,
}

impl DiagramHost {
    fn new(init: &ViewInit, _: &mut Window, cx: &mut Context<Self>) -> Self {
        let view = cx.new(DiagramView::new);
        let messenger: Messenger = init.messenger();
        let tells = messenger.clone();
        let events = cx.subscribe(&view, move |_, _, event: &DiagramEvent, _| {
            tells.send(tell::message(event))
        });
        let laid_out = cx.observe(&view, |host: &mut Self, _, cx| {
            if !host.asked.is_empty() {
                cx.notify();
            }
        });
        let mut host = Self {
            view,
            document: Data::Nil,
            waiting: None,
            problem: None,
            camera_seq: None,
            export_seq: None,
            asked: Vec::new(),
            exports: 0,
            messenger,
            name: format!("{}-{}", init.owner(), init.id()),
            _events: events,
            _laid_out: laid_out,
        };
        host.set_props(init.props(), cx);
        host
    }
}

impl HostView for DiagramHost {
    fn set_props(&mut self, props: &Data, cx: &mut Context<Self>) {
        let props = read::props(props);
        if props.document != self.document {
            self.document = props.document.clone();
            for read::Hand::Export { format, seq } in std::mem::take(&mut self.asked) {
                self.messenger.send(export::told(
                    Err("the diagram changed before it was laid out".to_owned()),
                    &format,
                    seq,
                ));
            }
            match props.diagram {
                Ok(diagram) => {
                    self.problem = None;
                    self.waiting = Some(diagram);
                },
                Err(problem) => self.problem = Some(problem),
            }
        }
        let camera = props
            .camera
            .filter(|(seq, _)| Some(seq) != self.camera_seq.as_ref());
        if let Some((seq, _)) = &camera {
            self.camera_seq = Some(seq.clone());
        }
        if let Some((seq, format)) = props
            .export
            .filter(|(seq, _)| Some(seq) != self.export_seq.as_ref())
        {
            self.export_seq = Some(seq.clone());
            self.asked.push(read::Hand::Export { format, seq });
        }
        self.view.update(cx, |view, cx| {
            view.set_mode(props.mode, cx);
            view.set_tool(props.tool, cx);
            if view.selected() != props.selected.as_slice() {
                view.set_selected(props.selected, cx);
            }
            view.set_highlight(props.highlight, cx);
            view.set_scroll_needs_focus(props.scroll_needs_focus);
            match camera.map(|(_, ask)| ask) {
                Some(read::Camera::Fit) => view.fit(cx),
                Some(read::Camera::ZoomBy(by)) => view.zoom_by(by, cx),
                Some(read::Camera::ZoomTo(to)) => view.zoom_to(to, cx),
                None => {},
            }
        });
        cx.notify();
    }
}

impl Render for DiagramHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(diagram) = self.waiting.take() {
            self.view
                .update(cx, |view, cx| view.set_diagram(diagram, window, cx));
        }
        // An export waits for the layout of the document it was asked
        // with; only the SVG text is made here, the file in the background.
        // Text that is no diagram has none to export: the view still shows
        // the last one that was.
        if !self.view.read(cx).is_laying_out() {
            for hand in std::mem::take(&mut self.asked) {
                let read::Hand::Export { format, seq } = hand;
                if let Some(problem) = &self.problem {
                    self.messenger
                        .send(export::told(Err(problem.clone()), &format, seq));
                    continue;
                }
                let view = self.view.read(cx);
                let svg = moonframe_diagram::view::export_svg(
                    view.diagram(),
                    view.layout(),
                    &view.highlighted(),
                    window,
                    cx,
                );
                self.exports += 1;
                let name = format!("{}-{}", self.name, self.exports);
                let (messenger, renderer) = (self.messenger.clone(), cx.svg_renderer());
                cx.background_spawn(async move {
                    let done = export::write(&svg, &format, &name, &renderer);
                    messenger.send(export::told(done, &format, seq));
                })
                .detach();
            }
        }
        let problem = self.problem.clone().map(|problem| {
            let theme = cx.theme();
            div()
                .absolute()
                .top_2()
                .left_2()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(theme.background)
                .text_color(theme.danger)
                .text_sm()
                .child(problem)
        });
        div()
            .relative()
            .size_full()
            .child(self.view.clone())
            .children(problem)
    }
}
