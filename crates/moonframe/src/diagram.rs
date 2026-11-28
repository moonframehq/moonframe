//! `ui.diagram`: moonframe-diagram's view, kept by mod and `id` like any
//! hosted view. The mod gives the document and the selection as props and
//! hears each edit as a message to its `update`, so it keeps the document
//! and its undo; the view never changes the document by itself.
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
    /// The last `export` and `copy` requests acted on, by their `seq`.
    export_seq: Option<Data>,
    copy_seq: Option<Data>,
    /// Exports and copies asked for, done once the frame has laid the
    /// document out.
    asked: Vec<read::Hand>,
    messenger: Messenger,
    /// The view's `id`, naming its exported files.
    name: String,
    _events: Subscription,
}

impl DiagramHost {
    fn new(init: &ViewInit, _: &mut Window, cx: &mut Context<Self>) -> Self {
        let view = cx.new(DiagramView::new);
        let messenger: Messenger = init.messenger();
        let tells = messenger.clone();
        let events = cx.subscribe(&view, move |_, _, event: &DiagramEvent, _| {
            tells.send(tell::message(event))
        });
        let mut host = Self {
            view,
            document: Data::Nil,
            waiting: None,
            problem: None,
            camera_seq: None,
            export_seq: None,
            copy_seq: None,
            asked: Vec::new(),
            messenger,
            name: format!("{}-{}", init.owner(), init.id()),
            _events: events,
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
            self.export_seq = Some(seq);
            self.asked.push(read::Hand::Export(format));
        }
        if let Some(seq) = props.copy.filter(|seq| Some(seq) != self.copy_seq.as_ref()) {
            self.copy_seq = Some(seq);
            self.asked.push(read::Hand::Copy);
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
        for hand in std::mem::take(&mut self.asked) {
            let view = self.view.read(cx);
            let (diagram, layout, lit) = (
                view.diagram().clone(),
                view.layout().clone(),
                view.highlighted(),
            );
            match hand {
                read::Hand::Export(format) => {
                    let done = export::file(&diagram, &layout, &lit, &format, &self.name, cx);
                    self.messenger.send(export::told(done, &format));
                },
                read::Hand::Copy => export::copy(&diagram, cx),
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
