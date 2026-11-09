//! A Time Machine-style ruler. The native view owns hover/focus and animation;
//! Luau only hears committed selections, so pointer frames never run scripts.

// keep-sorted start
mod geometry;
mod paint;
mod text;
// keep-sorted end

use {
    super::{
        Cx, Draw, Painter, Place,
        pointer::Sender,
        styling::{LookExt as _, StyleExt as _},
    },
    crate::color::ThemeColor as _,
    geometry::{Tick, capacity, nearest, position, ticks},
    gpui_kit::{component::ActiveTheme as _, *},
    moonframe_core::{Part, Resolved, RevisionTimeline, TimelineSide},
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
        time::{Duration, Instant},
    },
};

impl<'w> Draw<'w> for RevisionTimeline {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        TimelineElement {
            key: self
                .id
                .as_ref()
                .map_or_else(
                    || place.path.clone(),
                    |id| format!("{} revision-timeline {id}", place.owner),
                )
                .into(),
            model: self.clone(),
            look: painter.look(Part::RevisionTimeline, &self.look, false, place),
            sender: Sender {
                app: cx.entity().downgrade(),
                owner: place.owner.to_owned(),
                to: self.to.clone(),
            },
            duration: painter
                .workspace
                .stylesheet()
                .scaled(Duration::from_millis(85)),
        }
        .into_any_element()
    }
}

#[derive(IntoElement)]
struct TimelineElement {
    key: SharedString,
    model: RevisionTimeline,
    look: Resolved,
    sender: Sender,
    duration: Duration,
}

impl RenderOnce for TimelineElement {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let view = window.use_keyed_state(self.key.clone(), cx, |_, cx| TimelineView {
            model: Rc::new(self.model.clone()),
            sender: self.sender.clone(),
            focus: cx.focus_handle(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            painted: Rc::new(RefCell::new(Vec::new())),
            selected: self.model.selected.clone(),
            start: 0,
            reveal: true,
            target: None,
            pointer_y: None,
            wheel: 0.0,
            lens: 0.0,
            strength: 0.0,
            last: Instant::now(),
            duration: self.duration,
            accent: cx.theme().primary,
        });
        let theme = cx.theme();
        let accent = self
            .look
            .base
            .color
            .map_or(theme.primary, |color| color.hsla(theme));
        view.update(cx, |view, cx| {
            if view.model.as_ref() != &self.model {
                if view.model.selected != self.model.selected
                    || view.model.entries != self.model.entries
                {
                    view.selected = self.model.selected.clone();
                    view.reveal = true;
                }
                if view.model.entries != self.model.entries || self.model.disabled {
                    view.target = None;
                    view.pointer_y = None;
                }
                view.model = Rc::new(self.model.clone());
                cx.notify();
            }
            view.sender = self.sender;
            view.duration = self.duration;
            view.accent = accent;
        });
        let theme = cx.theme();
        let mut frame = div()
            .id(self.key)
            .relative()
            .min_h_0()
            .overflow_hidden()
            .look(&self.look, theme)
            .edges(self.look.base.border, &self.look.base, theme)
            .w(px(self
                .model
                .width
                .or(self.look.base.width)
                .unwrap_or(240.0)));
        if self.model.grow {
            frame = frame.flex_1().h_full();
        } else {
            frame = frame
                .h(px(self
                    .model
                    .height
                    .or(self.look.base.height)
                    .unwrap_or(420.0)))
                .flex_shrink_0();
        }
        frame.child(view)
    }
}

struct TimelineView {
    model: Rc<RevisionTimeline>,
    sender: Sender,
    focus: FocusHandle,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    painted: Rc<RefCell<Vec<Tick>>>,
    selected: Option<String>,
    start: usize,
    reveal: bool,
    target: Option<f32>,
    pointer_y: Option<f32>,
    wheel: f32,
    lens: f32,
    strength: f32,
    last: Instant,
    duration: Duration,
    accent: Hsla,
}

impl TimelineView {
    fn selected_index(&self) -> Option<usize> {
        self.model
            .entries
            .iter()
            .position(|entry| Some(&entry.id) == self.selected.as_ref())
    }

    fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.disabled {
            return;
        }
        let Some(entry) = self.model.entries.get(index) else {
            return;
        };
        // Keyboard navigation also reveals an unchanged selection after scrolling away.
        self.reveal = true;
        cx.notify();
        if self.selected.as_ref() == Some(&entry.id) {
            return;
        }
        self.selected = Some(entry.id.clone());
        if let Some(message) = self.model.selection_message(index) {
            self.sender.send(message, window, cx);
        }
    }

    fn over_rail(&self, point: Point<Pixels>) -> bool {
        if !self.model.preview {
            return true;
        }
        let bounds = self.bounds.get();
        let distance = if self.model.side == TimelineSide::Left {
            point.x - bounds.left()
        } else {
            bounds.right() - point.x
        };
        // The card begins 12px past the longest painted tick, itself inset 8px.
        // Use its actual anchor so the preview never becomes a selection target.
        f32::from(distance)
            < 20.0
                + paint::rail_width(
                    &self.model,
                    &self.painted.borrow(),
                    self.selected_index(),
                    f32::from(bounds.size.width),
                )
    }

    fn point(&mut self, point: Point<Pixels>, cx: &mut Context<Self>) {
        if self.model.disabled {
            return;
        }
        if !self.over_rail(point) {
            if self.target.take().is_some() {
                self.pointer_y = None;
                self.last = Instant::now();
                cx.notify();
            }
            return;
        }
        let y = f32::from(point.y - self.bounds.get().origin.y);
        self.pointer_y = Some(y);
        let target = position(&self.painted.borrow(), y);
        if self.target != target {
            if self.target.is_none() {
                self.last = Instant::now();
                self.lens = target.unwrap_or(0.0);
            }
            self.target = target;
            cx.notify();
        }
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.disabled || self.model.entries.is_empty() {
            return;
        }
        let current = self.selected_index().unwrap_or(0);
        let last = self.model.entries.len() - 1;
        let next = match event.keystroke.key.as_str() {
            "home" => 0,
            "end" => last,
            "up" if event.keystroke.modifiers.shift
                && self.model.entries[current].day.is_some() =>
            {
                (0..current)
                    .rev()
                    .find(|i| self.model.entries[*i].day != self.model.entries[current].day)
                    .unwrap_or(0)
            },
            "down"
                if event.keystroke.modifiers.shift && self.model.entries[current].day.is_some() =>
            {
                (current + 1..=last)
                    .find(|i| self.model.entries[*i].day != self.model.entries[current].day)
                    .unwrap_or(last)
            },
            "up" => current.saturating_sub(1),
            "down" => {
                if self.selected_index().is_none() {
                    0
                } else {
                    (current + 1).min(last)
                }
            },
            _ => return,
        };
        self.target = None;
        self.pointer_y = None;
        self.select(next, window, cx);
        cx.stop_propagation();
    }
}

impl Render for TimelineView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f32();
        self.last = now;
        let blend = if self.duration.is_zero() {
            1.0
        } else {
            1.0 - (-elapsed / self.duration.as_secs_f32()).exp()
        };
        let target_strength = if self.target.is_some() && !self.model.disabled {
            1.0
        } else {
            0.0
        };
        let target = self.target.unwrap_or(self.lens);
        self.lens += (target - self.lens) * blend;
        self.strength += (target_strength - self.strength) * blend;
        let moving =
            (self.lens - target).abs() > 0.002 || (self.strength - target_strength).abs() > 0.002;
        if !moving {
            self.lens = target;
            self.strength = target_strength;
        }
        let bounds = self.bounds.clone();
        let painted = self.painted.clone();
        let model = self.model.clone();
        let selected = self.selected_index();
        let pointer_y = self.pointer_y;
        let (lens, strength) = (self.lens, self.strength);
        let weak = cx.entity().downgrade();
        let weak_paint = weak.clone();
        let start = self.start;
        let reveal = self.reveal;
        self.reveal = false;
        let accent = self.accent;
        let drawing = canvas(
            move |area, _, _| {
                bounds.set(area);
            },
            move |area, (), window, cx| {
                let height = f32::from(area.size.height);
                let count = capacity(height, &model.metrics);
                let mut start = start.min(model.entries.len().saturating_sub(count));
                if reveal && let Some(selected) = selected {
                    if selected < start {
                        start = selected;
                    }
                    if selected >= start + count {
                        start = selected + 1 - count;
                    }
                }
                let ticks = ticks(
                    model.entries.len(),
                    start,
                    height,
                    lens,
                    strength,
                    &model.metrics,
                );
                let hover = pointer_y.and_then(|y| nearest(&ticks, y));
                paint::ruler(&model, &ticks, selected, hover, area, accent, window, cx);
                *painted.borrow_mut() = ticks;
                // Keep the viewport chosen after layout, without another redraw.
                let _ = weak_paint.update(cx, |view, _| view.start = start);
                if moving {
                    window.on_next_frame(move |_, cx| {
                        let _ = weak.update(cx, |_, cx| cx.notify());
                    });
                }
            },
        )
        .size_full();
        div()
            .id("ruler")
            .debug_selector(|| "revision-timeline".into())
            .size_full()
            .overflow_hidden()
            .track_focus(&self.focus)
            .cursor_pointer()
            .on_mouse_move(
                cx.listener(|view, event: &MouseMoveEvent, _, cx| view.point(event.position, cx)),
            )
            .on_hover(cx.listener(|view, hovered, _, cx| {
                if !hovered {
                    view.target = None;
                    view.pointer_y = None;
                    view.last = Instant::now();
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, event: &MouseDownEvent, window, cx| {
                    if view.model.disabled || !view.over_rail(event.position) {
                        return;
                    }
                    view.focus.focus(window, cx);
                    let y = f32::from(event.position.y - view.bounds.get().origin.y);
                    let index = nearest(&view.painted.borrow(), y);
                    if let Some(index) = index {
                        view.select(index, window, cx);
                    }
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(Self::key))
            .on_scroll(cx.listener(|view, event: &ScrollEvent, _, cx| {
                if view.model.disabled {
                    return;
                }
                let delta = f32::from(event.delta.pixel_delta(px(18.0)).y);
                view.wheel += delta;
                let spacing = view.model.metrics.spacing;
                if view.wheel.abs() < spacing {
                    cx.stop_propagation();
                    return;
                }
                let count = capacity(
                    f32::from(view.bounds.get().size.height),
                    &view.model.metrics,
                );
                let max = view.model.entries.len().saturating_sub(count);
                let step = (view.wheel.abs() / spacing).floor() as usize;
                view.start = if view.wheel > 0.0 {
                    view.start.saturating_sub(step)
                } else {
                    view.start.saturating_add(step).min(max)
                };
                view.wheel %= spacing;
                view.target = None;
                view.pointer_y = None;
                cx.notify();
                cx.stop_propagation();
            }))
            .child(drawing)
    }
}
