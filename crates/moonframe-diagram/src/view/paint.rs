//! One frame: groups, then a sequence's lifelines, edges, nodes, labels,
//! and on top the selection and the gesture under way. Only what falls in
//! the view is painted; text too small to read is left out.

mod shapes;

pub(crate) use shapes::text_width;
use {
    super::{DiagramView, Mode, Tool, camera::Camera, hit, input, scene::Scene, style::Palette},
    crate::{
        geometry::{Point, Rect},
        layout::Layout,
        model::{Diagram, EdgeKind, Head, MessageKind, NodeKind},
    },
    gpui_kit::*,
    shapes::{Stroke, bounds_of},
    std::{collections::HashSet, sync::Arc},
};

/// What a frame needs from the view besides its scene, owned so the paint
/// closure can keep it.
pub(crate) struct Shown {
    diagram: Arc<Diagram>,
    layout: Arc<Layout>,
    selected: HashSet<String>,
    highlight: HashSet<String>,
    hover: Option<String>,
    /// The one chosen node whose handles and ports show.
    handles: Option<String>,
    editing: Option<String>,
    font: f32,
}

impl Shown {
    /// A diagram with nothing chosen, as a picture shows it.
    pub(crate) fn still(diagram: Arc<Diagram>, layout: Arc<Layout>) -> Self {
        Self {
            diagram,
            layout,
            selected: HashSet::new(),
            highlight: HashSet::new(),
            hover: None,
            handles: None,
            editing: None,
            font: 14.0,
        }
    }

    pub(crate) fn of(view: &DiagramView) -> Self {
        let handles =
            (view.mode == Mode::Edit && view.tool == Tool::Select && view.gesture.is_none())
                .then(|| input::alone(view).map(str::to_owned))
                .flatten();
        Self {
            diagram: view.diagram.clone(),
            layout: view.layout.clone(),
            selected: view.selected.clone(),
            highlight: view.highlight.clone(),
            hover: view.hover.clone(),
            handles,
            editing: view.editing.as_ref().map(|(id, ..)| id.clone()),
            font: 14.0,
        }
    }
}

pub(crate) fn paint(
    scene: &Scene,
    shown: &Shown,
    camera: Camera,
    b: Bounds<Pixels>,
    palette: Palette,
    window: &mut Window,
    cx: &mut App,
) {
    let origin = b.origin;
    let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
    let screen = Rect::new(0.0, 0.0, w, h);
    let zoom = camera.zoom;
    let font = shown.font * zoom;
    let on_screen = |r: Rect| camera.rect_to_screen(r);
    window.with_content_mask(Some(ContentMask { bounds: b }), |window| {
        // Groups, under everything.
        for group in &shown.diagram.groups {
            let Some(r) = shown.layout.groups.get(&group.id).map(|r| on_screen(*r)) else {
                continue;
            };
            if !r.overlaps(&screen) {
                continue;
            }
            let tint = group.tone.map_or(palette.quiet, |t| palette.tone(t, cx));
            window.paint_quad(quad(
                bounds_of(origin, r),
                px(10.0 * zoom),
                tint.opacity(0.06),
                px(1.0),
                tint.opacity(0.35),
                BorderStyle::Dashed,
            ));
            let label = Point::new(r.x + 10.0 * zoom, r.y + 12.0 * zoom);
            let width = text_width(&group.label, font * 0.8, window, cx);
            shapes::text(
                &group.label,
                Point::new(label.x + width / 2.0, label.y),
                font * 0.8,
                r.w,
                palette.quiet,
                true,
                origin,
                window,
                cx,
            );
        }

        // A sequence's lifelines and messages.
        for (_, top, bottom) in &shown.layout.lifelines {
            let (a, z) = (camera.to_screen(*top), camera.to_screen(*bottom));
            let stroke = Stroke {
                color: palette.outline,
                width: 1.0,
                dash: Some((4.0, 4.0)),
                radius: 0.0,
            };
            shapes::line(&[a, z], stroke, Head::None, Head::None, origin, window);
        }
        for (message, route) in shown.diagram.messages.iter().zip(&shown.layout.messages) {
            let points: Vec<Point> = route.points.iter().map(|p| camera.to_screen(*p)).collect();
            let (dash, end) = match message.kind {
                MessageKind::Sync => (None, Head::Arrow),
                MessageKind::Async => (None, Head::Open),
                MessageKind::Reply => (Some((5.0, 4.0)), Head::Open),
            };
            let stroke = Stroke {
                color: palette.text.opacity(0.75),
                width: 1.5,
                dash,
                radius: 6.0 * zoom,
            };
            shapes::line(&points, stroke, Head::None, end, origin, window);
            if let Some(label) = route.label {
                let at = camera.to_screen(label);
                shapes::text(
                    &message.label,
                    at,
                    font * 0.86,
                    f32::INFINITY,
                    palette.text,
                    false,
                    origin,
                    window,
                    cx,
                );
            }
        }

        // Edges, under the nodes.
        let mut labels = Vec::new();
        for edge in &shown.diagram.edges {
            if edge.kind == EdgeKind::Invisible {
                continue;
            }
            let Some(route) = scene.routes.get(&edge.id) else {
                continue;
            };
            let points: Vec<Point> = route.points.iter().map(|p| camera.to_screen(*p)).collect();
            let reach = points.iter().fold(None::<Rect>, |r, p| {
                let dot = Rect::new(p.x, p.y, 0.0, 0.0);
                Some(r.map_or(dot, |r| r.union(&dot)))
            });
            if !reach.is_some_and(|r| r.inflate(20.0).overlaps(&screen)) {
                continue;
            }
            let chosen = shown.selected.contains(&edge.id) || shown.highlight.contains(&edge.id);
            let base = edge.tone.map_or(palette.edge, |t| palette.tone(t, cx));
            let color = if chosen {
                palette.accent
            } else {
                base
            };
            let width = match edge.kind {
                EdgeKind::Thick => 3.0,
                _ => 1.5,
            } + if chosen {
                0.5
            } else {
                0.0
            };
            let dash = match edge.kind {
                EdgeKind::Dashed => Some((6.0, 4.0)),
                EdgeKind::Dotted => Some((1.5, 3.5)),
                _ => None,
            };
            let stroke = Stroke {
                color: color.opacity(color.a * scene.edge_alpha),
                width,
                dash,
                radius: (10.0 * zoom).clamp(2.0, 12.0),
            };
            shapes::line(&points, stroke, edge.start, edge.end, origin, window);
            if let (Some(label), Some(text)) =
                (route.label, edge.label.as_deref().filter(|l| !l.is_empty()))
            {
                labels.push((camera.to_screen(label), text, color));
            }
        }

        // Nodes, bottom first.
        for node in &shown.diagram.nodes {
            let Some(world) = scene.rects.get(&node.id) else {
                continue;
            };
            let r = on_screen(*world);
            if !r.inflate(4.0).overlaps(&screen) {
                continue;
            }
            let (fill, outline, text) =
                palette.node(node.kind, node.tone, node.fill.as_deref(), cx);
            let lit = shown.highlight.contains(&node.id);
            let (outline, width) = if lit {
                (palette.accent, 2.0)
            } else {
                (outline, 1.0)
            };
            shapes::node(node.kind, r, zoom, fill, outline, width, origin, window);
            if shown.hover.as_deref() == Some(node.id.as_str())
                && !shown.selected.contains(&node.id)
            {
                window.paint_quad(quad(
                    bounds_of(origin, r.inflate(2.0)),
                    px(6.0 * zoom),
                    transparent_black(),
                    px(1.0),
                    palette.accent.opacity(0.5),
                    BorderStyle::Solid,
                ));
            }
            if shown.editing.as_deref() == Some(node.id.as_str()) {
                continue;
            }
            // The label in the middle (a diamond's in its middle half; a
            // note's from the top), the sublabel under it.
            let room = match node.kind {
                NodeKind::Diamond => r.w * 0.6,
                NodeKind::Circle => r.w * 0.8,
                _ => r.w - 12.0 * zoom,
            };
            let note = node.kind == NodeKind::Note;
            let pad = 10.0 * zoom;
            let middle = if note {
                Point::new(r.center().x, r.y + pad)
            } else {
                r.center()
            };
            let tall = match node.kind {
                NodeKind::Diamond => r.h * 0.55,
                _ => r.h - pad * 2.0,
            };
            if let Some(sub) = &node.sublabel {
                let line = font * 1.3;
                shapes::text(
                    &node.label,
                    Point::new(middle.x, middle.y - line * 0.45),
                    font,
                    room,
                    text,
                    true,
                    origin,
                    window,
                    cx,
                );
                shapes::text(
                    sub,
                    Point::new(middle.x, middle.y + line * 0.55),
                    font * 0.86,
                    room,
                    palette.quiet,
                    false,
                    origin,
                    window,
                    cx,
                );
            } else {
                shapes::paragraph(
                    &node.label,
                    middle,
                    note,
                    font,
                    room,
                    tall,
                    text,
                    true,
                    origin,
                    window,
                    cx,
                );
            }
        }
        // A sequence's notes.
        for (note, rect) in shown.diagram.notes.iter().zip(&shown.layout.notes) {
            let r = on_screen(*rect);
            let (fill, outline, text) = palette.node(NodeKind::Note, None, None, cx);
            shapes::node(NodeKind::Note, r, zoom, fill, outline, 1.0, origin, window);
            shapes::text(
                &note.text,
                r.center(),
                font * 0.9,
                r.w - 10.0,
                text,
                false,
                origin,
                window,
                cx,
            );
        }

        // Edge labels over everything they cross.
        for (center, text, color) in labels {
            let size = font * 0.86;
            if size < 4.0 {
                continue;
            }
            let width = text_width(text, size, window, cx) + 10.0;
            let pill = Rect::new(
                center.x - width / 2.0,
                center.y - size * 0.75,
                width,
                size * 1.5,
            );
            window.paint_quad(quad(
                bounds_of(origin, pill),
                px(size * 0.75),
                palette.background,
                px(0.0),
                transparent_black(),
                BorderStyle::Solid,
            ));
            let quiet = if color == palette.edge {
                palette.quiet
            } else {
                color
            };
            shapes::text(
                text,
                center,
                size,
                f32::INFINITY,
                quiet,
                false,
                origin,
                window,
                cx,
            );
        }

        // The selection: an outline round each chosen node, and the one
        // chosen node's handles and ports.
        for id in &shown.selected {
            if let Some(world) = scene.rects.get(id) {
                let r = on_screen(*world).inflate(3.0);
                window.paint_quad(quad(
                    bounds_of(origin, r),
                    px(6.0),
                    transparent_black(),
                    px(1.5),
                    palette.accent,
                    BorderStyle::Solid,
                ));
            }
        }
        if let Some(world) = shown.handles.as_ref().and_then(|id| scene.rects.get(id)) {
            let r = on_screen(*world);
            for corner in hit::Corner::ALL {
                let c = corner.at(&r);
                let handle = Rect::new(c.x - 4.0, c.y - 4.0, 8.0, 8.0);
                window.paint_quad(quad(
                    bounds_of(origin, handle),
                    px(2.0),
                    palette.background,
                    px(1.5),
                    palette.accent,
                    BorderStyle::Solid,
                ));
            }
            for p in hit::ports(&r, input::PORT_OUT) {
                let dot = Rect::new(p.x - 4.5, p.y - 4.5, 9.0, 9.0);
                window.paint_quad(quad(
                    bounds_of(origin, dot),
                    px(4.5),
                    palette.accent,
                    px(0.0),
                    transparent_black(),
                    BorderStyle::Solid,
                ));
            }
        }

        // The gesture under way.
        for (a, z) in &scene.guides {
            let stroke = Stroke {
                color: palette.accent.opacity(0.8),
                width: 1.0,
                dash: None,
                radius: 0.0,
            };
            shapes::line(
                &[camera.to_screen(*a), camera.to_screen(*z)],
                stroke,
                Head::None,
                Head::None,
                origin,
                window,
            );
        }
        if let Some(area) = scene.marquee.or(scene.creating) {
            let r = on_screen(area);
            window.paint_quad(quad(
                bounds_of(origin, r),
                px(2.0),
                palette.accent.opacity(0.08),
                px(1.0),
                palette.accent.opacity(0.7),
                BorderStyle::Solid,
            ));
        }
        if let Some((from, to, over)) = &scene.link {
            if let Some(target) = over.as_ref().and_then(|id| scene.rects.get(id)) {
                let r = on_screen(*target).inflate(3.0);
                window.paint_quad(quad(
                    bounds_of(origin, r),
                    px(6.0),
                    transparent_black(),
                    px(2.0),
                    palette.accent,
                    BorderStyle::Solid,
                ));
            }
            let stroke = Stroke {
                color: palette.accent,
                width: 2.0,
                dash: Some((6.0, 4.0)),
                radius: 0.0,
            };
            shapes::line(
                &[camera.to_screen(*from), camera.to_screen(*to)],
                stroke,
                Head::None,
                Head::Arrow,
                origin,
                window,
            );
        }
    });
}
