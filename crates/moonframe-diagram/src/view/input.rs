//! The pointer and the keys: a press starts a gesture (pan, move, resize,
//! marquee, connect, create), moves anywhere in the window carry it on,
//! and letting go ends it with one event for the owner. The phase 0 Luau
//! editor is the behaviour this keeps.

use {
    super::{
        DiagramEvent, DiagramView, Mode, Tool,
        camera::Camera,
        hit::{self, Corner, Hit},
        snap,
    },
    crate::{
        geometry::{Point, Rect, bounds},
        layout::node_size,
        model::{Node, NodeKind},
    },
    gpui_kit::*,
    std::collections::HashSet,
};

/// Screen pixels a press must travel to be a drag, not a click.
const SLOP: f32 = 3.0;
/// Screen pixels within which a line, handle or snap counts.
const REACH: f32 = 6.0;
/// The least size a node is resized to, in diagram pixels.
const LEAST: f32 = 16.0;
/// How far out from a node's side its connector ports sit, on screen.
pub(crate) const PORT_OUT: f32 = 12.0;

pub(crate) enum Gesture {
    Pan {
        start: Point,
        camera: Camera,
    },
    Move {
        ids: Vec<String>,
        start: Point,
        delta: Point,
        guides: Vec<(Point, Point)>,
        travelled: bool,
        pressed: String,
    },
    Resize {
        id: String,
        corner: Corner,
        start: Point,
        original: Rect,
        now: Rect,
    },
    Marquee {
        start: Point,
        end: Point,
        add: bool,
    },
    Connect {
        from: String,
        end: Point,
        over: Option<String>,
    },
    Create {
        kind: NodeKind,
        start: Point,
        end: Point,
    },
}

/// A window position in the view, from its corner.
fn local(view: &DiagramView, position: gpui_kit::Point<Pixels>) -> Point {
    let origin = view.bounds.get().origin;
    Point::new(
        f32::from(position.x - origin.x),
        f32::from(position.y - origin.y),
    )
}

/// The one chosen node, whose handles and ports show.
pub(crate) fn alone(view: &DiagramView) -> Option<&str> {
    let mut nodes = view
        .selected
        .iter()
        .filter(|id| view.layout.nodes.contains_key(*id));
    let first = nodes.next()?;
    nodes.next().is_none().then_some(first.as_str())
}

fn order(view: &DiagramView) -> Vec<String> {
    view.diagram.nodes.iter().map(|n| n.id.clone()).collect()
}

pub(crate) fn down(
    view: &mut DiagramView,
    event: &MouseDownEvent,
    window: &mut Window,
    cx: &mut Context<DiagramView>,
) {
    view.focus.focus(window, cx);
    cx.stop_propagation();
    let screen = local(view, event.position);
    let world = view.camera.to_world(screen);
    let (rects, _) = view.eased();
    let routes: std::collections::HashMap<_, _> = view
        .layout
        .edges
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let edit = view.mode == Mode::Edit;
    let handles = if edit && view.tool == Tool::Select {
        alone(view).map(str::to_owned)
    } else {
        None
    };
    let found = hit::hit(
        &order(view),
        &rects,
        &routes,
        world,
        REACH / view.camera.zoom,
        handles.as_deref(),
        PORT_OUT / view.camera.zoom,
    );
    let pan = Gesture::Pan {
        start: screen,
        camera: view.camera,
    };
    let shift = event.modifiers.shift;
    if event.button == MouseButton::Middle || (edit && view.tool == Tool::Hand) {
        view.gesture = Some(pan);
    } else if !edit {
        match found {
            Hit::Node(id) if event.click_count >= 2 => cx.emit(DiagramEvent::Open(id)),
            Hit::Node(id) | Hit::Edge(id) => choose(view, id, shift, cx),
            _ => view.gesture = Some(pan),
        }
    } else {
        match (view.tool, found) {
            (Tool::Create(kind), _) => {
                view.gesture = Some(Gesture::Create {
                    kind,
                    start: world,
                    end: world,
                })
            },
            (Tool::Connect, Hit::Node(from)) | (_, Hit::Port(from)) => {
                view.gesture = Some(Gesture::Connect {
                    from,
                    end: world,
                    over: None,
                })
            },
            (Tool::Connect, _) => {},
            (_, Hit::Handle(id, corner)) => {
                if let Some(original) = rects.get(&id).copied() {
                    view.gesture = Some(Gesture::Resize {
                        id,
                        corner,
                        start: world,
                        original,
                        now: original,
                    });
                }
            },
            (_, Hit::Node(id)) if event.click_count >= 2 => view.edit_label(id, window, cx),
            (_, Hit::Node(id)) => {
                if shift {
                    choose(view, id.clone(), true, cx);
                } else if !view.selected.contains(&id) {
                    view.select(vec![id.clone()], cx);
                }
                let ids: Vec<String> = view
                    .selected_order
                    .iter()
                    .filter(|s| rects.contains_key(*s))
                    .cloned()
                    .collect();
                view.gesture = Some(Gesture::Move {
                    ids,
                    start: world,
                    delta: Point::default(),
                    guides: Vec::new(),
                    travelled: false,
                    pressed: id,
                });
            },
            (_, Hit::Edge(id)) => choose(view, id, shift, cx),
            (_, Hit::Nothing) => {
                if !shift {
                    view.select(Vec::new(), cx);
                }
                view.gesture = Some(Gesture::Marquee {
                    start: world,
                    end: world,
                    add: shift,
                });
            },
        }
    }
    cx.notify();
}

/// Chooses `id` alone, or with shift adds or removes it.
fn choose(view: &mut DiagramView, id: String, shift: bool, cx: &mut Context<DiagramView>) {
    let mut ids = if shift {
        view.selected_order.clone()
    } else {
        Vec::new()
    };
    if let Some(at) = ids.iter().position(|s| *s == id) {
        ids.remove(at);
    } else {
        ids.push(id);
    }
    view.select(ids, cx);
}

/// A move of the pointer while a gesture is under way, anywhere in the
/// window.
fn dragged(view: &mut DiagramView, event: &MouseMoveEvent, cx: &mut Context<DiagramView>) {
    let screen = local(view, event.position);
    let world = view.camera.to_world(screen);
    let zoom = view.camera.zoom;
    let (rects, _) = view.eased();
    let visible = view.camera.visible(
        f32::from(view.bounds.get().size.width),
        f32::from(view.bounds.get().size.height),
    );
    match &mut view.gesture {
        Some(Gesture::Pan { start, camera }) => {
            view.camera.x = camera.x - (screen.x - start.x) / camera.zoom;
            view.camera.y = camera.y - (screen.y - start.y) / camera.zoom;
        },
        Some(Gesture::Move {
            ids,
            start,
            delta,
            guides,
            travelled,
            ..
        }) => {
            let raw = Point::new(world.x - start.x, world.y - start.y);
            *travelled |= raw.x.hypot(raw.y) * zoom > SLOP;
            let moving: HashSet<&String> = ids.iter().collect();
            let own: Vec<Rect> = ids
                .iter()
                .filter_map(|id| rects.get(id))
                .map(|r| r.translate(raw.x, raw.y))
                .collect();
            let others: Vec<&Rect> = rects
                .iter()
                .filter(|(id, r)| !moving.contains(id) && r.overlaps(&visible))
                .map(|(_, r)| r)
                .collect();
            let snapped = match bounds(&own) {
                Some(selection) if !event.modifiers.alt => {
                    snap::snap(selection, others, REACH / zoom)
                },
                _ => snap::Snapped {
                    dx: 0.0,
                    dy: 0.0,
                    guides: Vec::new(),
                },
            };
            *delta = Point::new(raw.x + snapped.dx, raw.y + snapped.dy);
            *guides = snapped.guides;
        },
        Some(Gesture::Resize {
            corner,
            start,
            original,
            now,
            ..
        }) => {
            *now = corner.drag(original, world.x - start.x, world.y - start.y, LEAST);
        },
        Some(Gesture::Marquee { end, .. }) | Some(Gesture::Create { end, .. }) => *end = world,
        Some(Gesture::Connect { from, end, over }) => {
            *end = world;
            *over = view
                .diagram
                .nodes
                .iter()
                .rev()
                .find(|n| n.id != *from && rects.get(&n.id).is_some_and(|r| r.contains(world)))
                .map(|n| n.id.clone());
        },
        None => return,
    }
    cx.notify();
}

/// Letting go: the gesture's one event.
fn released(view: &mut DiagramView, cx: &mut Context<DiagramView>) {
    let Some(gesture) = view.gesture.take() else {
        return;
    };
    let (rects, _) = view.eased();
    match gesture {
        Gesture::Pan { .. } => view.tell_zoom(cx),
        Gesture::Move {
            ids,
            delta,
            travelled: true,
            ..
        } => {
            let moved: Vec<(String, Rect)> = ids
                .iter()
                .filter_map(|id| {
                    rects
                        .get(id)
                        .map(|r| (id.clone(), round(r.translate(delta.x, delta.y))))
                })
                .collect();
            view.pending.extend(moved.iter().cloned());
            cx.emit(DiagramEvent::Move(moved));
        },
        Gesture::Move { pressed, .. } => {
            // A click on one of several chosen nodes chooses it alone.
            if view.selected.len() > 1 {
                view.select(vec![pressed], cx);
            }
        },
        Gesture::Resize {
            id, now, original, ..
        } if now != original => {
            let rect = round(now);
            view.pending.insert(id.clone(), rect);
            cx.emit(DiagramEvent::Resize { id, rect });
        },
        Gesture::Resize { .. } => {},
        Gesture::Marquee { start, end, add } => {
            let area = Rect::from_corners(start, end);
            if area.w * view.camera.zoom > SLOP || area.h * view.camera.zoom > SLOP {
                let keep: HashSet<String> = if add {
                    view.selected.clone()
                } else {
                    HashSet::new()
                };
                let ids = hit::within(&rects, &area, &order(view), &keep);
                view.select(ids, cx);
            }
        },
        Gesture::Connect {
            from,
            over: Some(to),
            ..
        } => cx.emit(DiagramEvent::Connect { from, to }),
        Gesture::Connect { .. } => {},
        Gesture::Create { kind, start, end } => {
            let drawn = Rect::from_corners(start, end);
            let rect = if drawn.w * view.camera.zoom > SLOP * 2.0
                && drawn.h * view.camera.zoom > SLOP * 2.0
            {
                drawn
            } else {
                // A click: the kind's own size, centred on it.
                let mut node = Node::new("", default_label(kind));
                node.kind = kind;
                Rect::centered(
                    start,
                    node_size(
                        &node,
                        &crate::layout::Approx,
                        &crate::layout::Options::default(),
                    ),
                )
            };
            cx.emit(DiagramEvent::Create {
                kind,
                rect: round(rect),
            });
            view.tool = Tool::Select;
            cx.emit(DiagramEvent::Tool(Tool::Select));
        },
    }
    cx.notify();
}

fn default_label(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Diamond => "Decision?",
        NodeKind::Note => "Note",
        NodeKind::Text => "Text",
        _ => "Box",
    }
}

fn round(r: Rect) -> Rect {
    Rect::new(r.x.round(), r.y.round(), r.w.round(), r.h.round())
}

/// Window-wide listeners for the gesture under way, registered as the
/// canvas paints, so a drag goes on outside the view.
pub(crate) fn listen_while_dragging(view: &Entity<DiagramView>, window: &mut Window) {
    let moves = view.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
        if phase == DispatchPhase::Bubble && moves.read(cx).gesture.is_some() {
            moves.update(cx, |view, cx| dragged(view, event, cx));
        }
    });
    let ups = view.clone();
    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
        if phase == DispatchPhase::Bubble && ups.read(cx).gesture.is_some() {
            ups.update(cx, released);
        }
    });
}

/// The pointer over the view with no button held: what it would take.
pub(crate) fn hover(
    view: &mut DiagramView,
    event: &MouseMoveEvent,
    _: &mut Window,
    cx: &mut Context<DiagramView>,
) {
    if view.gesture.is_some() || event.pressed_button.is_some() {
        return;
    }
    let world = view.camera.to_world(local(view, event.position));
    let (rects, _) = view.eased();
    let routes = std::collections::HashMap::new();
    let handles = if view.mode == Mode::Edit && view.tool == Tool::Select {
        alone(view).map(str::to_owned)
    } else {
        None
    };
    let found = hit::hit(
        &order(view),
        &rects,
        &routes,
        world,
        REACH / view.camera.zoom,
        handles.as_deref(),
        PORT_OUT / view.camera.zoom,
    );
    let (hovered, cursor) = match found {
        Hit::Handle(_, Corner::TopLeft | Corner::BottomRight) => {
            (None, CursorStyle::ResizeUpLeftDownRight)
        },
        Hit::Handle(..) => (None, CursorStyle::ResizeUpRightDownLeft),
        Hit::Port(_) => (None, CursorStyle::Crosshair),
        Hit::Node(id) => (
            Some(id),
            if view.mode == Mode::Edit {
                CursorStyle::Arrow
            } else {
                CursorStyle::PointingHand
            },
        ),
        _ => (None, CursorStyle::Arrow),
    };
    if hovered != view.hover || cursor != view.cursor_now {
        view.hover = hovered;
        view.cursor_now = cursor;
        cx.notify();
    }
}

pub(crate) fn cursor(view: &DiagramView) -> CursorStyle {
    match (&view.gesture, view.mode, view.tool) {
        (Some(Gesture::Pan { .. }), ..) => CursorStyle::ClosedHand,
        (Some(Gesture::Move { .. }), ..) => CursorStyle::ClosedHand,
        (None, Mode::Edit, Tool::Hand) => CursorStyle::OpenHand,
        (None, Mode::Edit, Tool::Create(_) | Tool::Connect) => CursorStyle::Crosshair,
        _ => view.cursor_now,
    }
}

pub(crate) fn scroll(
    view: &mut DiagramView,
    event: &ScrollEvent,
    window: &mut Window,
    cx: &mut Context<DiagramView>,
) {
    if view.scroll_needs_focus && !view.focus.is_focused(window) {
        return;
    }
    cx.stop_propagation();
    let delta = match event.delta {
        ScrollDelta::Pixels(p) => Point::new(f32::from(p.x), f32::from(p.y)),
        ScrollDelta::Lines(l) => Point::new(l.x * 20.0, l.y * 20.0),
    };
    if event.modifiers.platform || event.modifiers.control {
        let at = local(view, event.position);
        view.camera
            .zoom_at(view.camera.zoom * (delta.y * 0.004).exp(), at);
        view.tell_zoom(cx);
    } else {
        view.camera.x -= delta.x / view.camera.zoom;
        view.camera.y -= delta.y / view.camera.zoom;
    }
    cx.notify();
}

pub(crate) fn pinch(
    view: &mut DiagramView,
    event: &PinchEvent,
    window: &mut Window,
    cx: &mut Context<DiagramView>,
) {
    if view.scroll_needs_focus && !view.focus.is_focused(window) {
        return;
    }
    cx.stop_propagation();
    let at = local(view, event.position);
    view.camera
        .zoom_at(view.camera.zoom * (1.0 + event.delta), at);
    view.tell_zoom(cx);
    cx.notify();
}

pub(crate) fn key(
    view: &mut DiagramView,
    event: &KeyDownEvent,
    _: &mut Window,
    cx: &mut Context<DiagramView>,
) {
    if view.editing.is_some() {
        return;
    }
    let keys = &event.keystroke;
    let secondary = keys.modifiers.platform || keys.modifiers.control;
    let edit = view.mode == Mode::Edit;
    let step = if keys.modifiers.shift {
        10.0
    } else {
        1.0
    };
    let nudge = match keys.key.as_str() {
        "up" => Some((0.0, -step)),
        "down" => Some((0.0, step)),
        "left" => Some((-step, 0.0)),
        "right" => Some((step, 0.0)),
        _ => None,
    };
    match keys.key.as_str() {
        "backspace" | "delete" if edit && !view.selected.is_empty() => {
            cx.emit(DiagramEvent::Delete(view.selected_order.clone()))
        },
        "escape" if view.gesture.is_some() => view.gesture = None,
        "escape" => view.select(Vec::new(), cx),
        "a" if secondary => {
            let all = order(view);
            view.select(all, cx);
        },
        // The user's own copy: the diagram as Mermaid. (A mod can't put
        // it on the clipboard by itself.)
        "c" if secondary => cx.write_to_clipboard(ClipboardItem::new_string(
            crate::write::mermaid(&view.diagram),
        )),
        "0" if secondary => view.zoom_to(1.0, cx),
        "=" | "+" if secondary => view.zoom_by(1.25, cx),
        "-" if secondary => view.zoom_by(0.8, cx),
        _ if edit && nudge.is_some() && !view.selected.is_empty() => {
            let (dx, dy) = nudge.unwrap_or_default();
            let (rects, _) = view.eased();
            let moved: Vec<(String, Rect)> = view
                .selected_order
                .iter()
                .filter_map(|id| rects.get(id).map(|r| (id.clone(), r.translate(dx, dy))))
                .collect();
            if !moved.is_empty() {
                view.pending.extend(moved.iter().cloned());
                cx.emit(DiagramEvent::Move(moved));
            }
        },
        _ => return,
    }
    cx.stop_propagation();
    cx.notify();
}
