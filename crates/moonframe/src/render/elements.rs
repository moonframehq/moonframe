//! How each element draws itself.

use {
    super::{
        Backdrop, Cx, Draw, KeyLayer, POPOVER_BACKDROP, POPOVER_CONTENT, Painter, Place, Shown,
        styling::{FrameExt as _, LookExt as _, StyleExt as _},
    },
    crate::color::ThemeColor,
    gpui_kit::{
        component::{
            ActiveTheme, Disableable as _, Icon as GpuiIcon, Sizable as _, Theme,
            button::{Button as GpuiButton, ButtonCustomVariant, ButtonVariants as _},
            h_flex, h_resizable, resizable_panel, v_resizable,
        },
        *,
    },
    moonframe_core::{
        Align, Anchor, Badge, Button, Case, Color, Direction, Extras, Fit, Icon, Image, Item,
        Justify, Look, Paint, Panel, Part, Popover, Region, Resolved, Side, Sides, Space, Split,
        Stack, States, Text, Token, Tone, TransitionKind, Variant,
    },
    std::path::Path,
};

/// The smallest a pane gets when a module gives a `max` and no `min`.
const PANE_MIN: f32 = 80.;
/// What a sliding pane with neither `size` nor `max` slides across.
const PANE_SLIDE: f32 = 240.;
/// How far a popover's content travels as it slides in or out.
const POPOVER_SLIDE: f32 = 8.;

impl<'w> Draw<'w> for Stack {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let in_drag = painter.in_drag;
        painter.in_drag |= self.drag;
        let children = painter.keyed_children(&self.children, place, self.direction, cx);
        painter.in_drag = in_drag;
        let part = match self.direction {
            Direction::Row => Part::Row,
            Direction::Column => Part::Column,
        };
        let look = painter.look(part, &self.look, self.selected, place);
        let theme = cx.theme();
        let element = div()
            .id(place.id())
            .flex()
            .look(&look, theme)
            .placed(&self.frame, &look.base, theme)
            .arranged(
                &self.frame,
                &look.base,
                self.direction,
                (Space::Normal, Space::None),
            );
        let element = match self.align {
            Align::Stretch => element.items_stretch(),
            Align::Start => element.items_start(),
            Align::Center => element.items_center(),
            Align::End => element.items_end(),
        };
        let element = match self.justify {
            Justify::Start => element.justify_start(),
            Justify::Center => element.justify_center(),
            Justify::End => element.justify_end(),
            Justify::Between => element.justify_between(),
        };
        let element = if self.wrap {
            element.flex_wrap()
        } else {
            element
        };
        let handle = painter.motion.scroll(&place.path);
        let element = element.children(children).scrollbars(
            &self.frame,
            self.direction,
            &handle,
            place.part("scrollbar").id(),
        );
        // Hugging only says something across a column.
        let element = if self.frame.hug
            && self.frame.self_align.is_none()
            && place.parent == Direction::Column
        {
            element.self_start()
        } else {
            element
        };
        let element = if self.drag {
            draggable(element, cx)
        } else {
            element
        };
        let element = match &self.pointer {
            Some(pointer) => super::pointer::attach(element, pointer, place, cx),
            None => element,
        };
        let element = match &self.action {
            Some(action) => covering(element, in_drag)
                .cursor_pointer()
                .on_click(place.click(action, cx)),
            None => element,
        };
        with_extras(element, &self.extras, painter, place, cx).into_any_element()
    }
}
/// An element's tooltip, right-click and copy, on the element that draws it.
fn with_extras<'w, E: StatefulInteractiveElement + Styled>(
    mut element: E,
    extras: &Extras,
    painter: &Painter<'w, '_>,
    place: &Place<'w>,
    cx: Cx,
) -> E {
    if let Some(text) = &extras.copy {
        let text = text.clone();
        element = element.cursor_pointer().on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
        });
    }
    if let Some(action) = &extras.context {
        element = element.on_mouse_down(MouseButton::Right, place.context(action, cx));
    }
    if let Some(text) = &extras.tooltip {
        let look = painter.look(Part::Tooltip, &Look::default(), false, place);
        // The rule's delay (`delay`, or its transition's) is how long the
        // pointer rests before the tooltip shows; else GPUI's own wait.
        let transition = look.base.transition;
        let wait = look.base.delay.or(transition
            .map(|transition| transition.delay)
            .filter(|delay| !delay.is_zero()));
        if let Some(wait) = wait {
            element = element.tooltip_show_delay(wait);
        }
        // Read durations already run at `animation_speed`.
        let motion = transition.filter(|transition| {
            transition.kind != TransitionKind::None && !transition.duration.is_zero()
        });
        let text = SharedString::from(text.clone());
        element = element.tooltip(move |_, cx| {
            let tip = Tip {
                text: text.clone(),
                look: look.clone(),
                motion,
            };
            cx.new(|_| tip).into()
        });
    }
    element
}

/// How far a sliding tooltip rises into place, in pixels.
const TIP_RISE: f32 = 4.;

/// A tooltip's content, styled by the `tooltip` rule, and eased in by its
/// `transition` as it shows: faded, risen a little, or both.
struct Tip {
    text: SharedString,
    look: Resolved,
    /// How it eases in, `None` when it shows at once.
    motion: Option<moonframe_core::Transition>,
}

impl Render for Tip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let tip = div()
            .id("tooltip")
            .px_2()
            .py_1()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .text_xs()
            .shadow_md()
            .look(&self.look, theme)
            .edges(self.look.base.border, &self.look.base, theme)
            .padded(&self.look.base.padding)
            .child(self.text.clone());
        let Some(transition) = self.motion else {
            return tip.into_any_element();
        };
        let opacity = self.look.base.opacity.unwrap_or(1.0);
        let curve = transition.curve;
        let kind = transition.kind;
        tip.relative()
            .with_animation(
                "tooltip-in",
                Animation::new(transition.duration).with_easing(move |time| curve.at(time)),
                move |tip, amount| {
                    let tip = match kind {
                        TransitionKind::Slide | TransitionKind::SlideFade => {
                            tip.top(px(TIP_RISE * (1.0 - amount)))
                        },
                        _ => tip,
                    };
                    match kind {
                        TransitionKind::Fade | TransitionKind::SlideFade => {
                            tip.opacity(opacity * amount)
                        },
                        _ => tip,
                    }
                },
            )
            .into_any_element()
    }
}

/// Makes a stack move the window as a title bar does: drag it to move,
/// double-click to zoom, and on Linux right-click for the window menu.
/// Windows does all of that itself for a caption area.
fn draggable(element: Stateful<Div>, cx: Cx) -> Stateful<Div> {
    element
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, window, _| {
                this.drag = event.click_count < 2;
                if event.click_count == 2 {
                    if cfg!(target_os = "macos") {
                        // Zoom or minimize, as the user's settings say.
                        window.titlebar_double_click();
                    } else if !cfg!(target_os = "windows") {
                        window.zoom_window();
                    }
                }
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _: &MouseUpEvent, _, _| this.drag = false),
        )
        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, _| {
            if this.drag && event.pressed_button == Some(MouseButton::Left) {
                this.drag = false;
                window.start_window_move();
            } else if event.pressed_button.is_none() {
                this.drag = false;
            }
        }))
        .on_mouse_down(MouseButton::Right, |event, window, _| {
            if cfg!(any(target_os = "linux", target_os = "freebsd")) {
                window.show_window_menu(event.position);
            }
        })
}

/// A control inside a drag area covers it, so pressing the control clicks
/// it rather than moving the window (Windows asks the drag area's hitbox
/// under the pointer, and the area's listeners check it too).
fn covering<E: InteractiveElement>(element: E, in_drag: bool) -> E {
    if in_drag {
        element.occlude()
    } else {
        element
    }
}

impl<'w> Draw<'w> for Panel {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let actions = painter.children(&self.actions, &place.part("actions"), Direction::Row, cx);
        let children = painter.children(&self.children, place, Direction::Column, cx);
        let look = painter.look(Part::Panel, &self.look, false, place);
        let header_look = painter.look(Part::PanelHeader, &Look::default(), false, place);
        let theme = cx.theme();
        let header = (self.title.is_some() || !actions.is_empty()).then(|| {
            let title = div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.muted_foreground)
                .typography(&header_look.base, theme)
                .children(self.title.clone());
            h_flex()
                .id(place.part("header").id())
                .flex_shrink_0()
                .gap_1()
                .px_2()
                .min_h(px(34.))
                .border_b_1()
                .border_color(theme.border)
                .look(&header_look, theme)
                .padded(&header_look.base.padding)
                .edges(header_look.base.border, &header_look.base, theme)
                .child(title)
                .children(actions)
        });
        let body_place = place.part("body");
        let handle = painter.motion.scroll(&body_place.path);
        let body = div()
            .id(body_place.id())
            .flex()
            .flex_1()
            .min_h_0()
            .arranged(
                &self.frame,
                &look.base,
                Direction::Column,
                (Space::Normal, Space::Normal),
            )
            .children(children)
            .scrollbars(
                &self.frame,
                Direction::Column,
                &handle,
                body_place.part("scrollbar").id(),
            );
        // Padding and gap belong to the body, under the header.
        let mut outer = look.clone();
        outer.base.padding = Sides::default();
        outer.base.gap = None;
        let element = div()
            .id(place.id())
            .flex()
            .flex_col()
            .look(&outer, theme)
            .placed(&self.frame, &look.base, theme);
        let element = if look.base.radius.is_some() {
            element.overflow_hidden()
        } else {
            element
        };
        element.children(header).child(body).into_any_element()
    }
}

/// A split handle with no hairline: panes meet with nothing between them,
/// and a pill shows only while the handle is hovered or dragged.
fn bare_handle() -> base::ResizeHandleRenderer {
    use base::ResizeHandleState;
    std::rc::Rc::new(|handle, _window, cx| {
        let (length, opacity) = match handle.state() {
            ResizeHandleState::Idle => return Some(div().into_any_element()),
            ResizeHandleState::Hovered => (px(20.), 0.35),
            ResizeHandleState::Pressed => (px(28.), 0.6),
            ResizeHandleState::Dragging => (px(44.), 0.9),
        };
        let pill = div()
            .flex_none()
            .rounded_full()
            .bg(cx.theme().muted_foreground)
            .opacity(opacity);
        let pill = match handle.axis() {
            Axis::Horizontal => pill.w(px(3.)).h(length),
            _ => pill.h(px(3.)).w(length),
        };
        Some(
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(pill)
                .into_any_element(),
        )
    })
}

impl<'w> Draw<'w> for Split {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        // Each pane's look and how much of it shows, before the group: which
        // panes take space decides the group's key.
        let panes: Vec<_> = self
            .panes
            .iter()
            .enumerate()
            .map(|(index, pane)| {
                let place = place.child(index, Direction::Column);
                let look = painter.look(Part::Pane, &pane.look, false, &place);
                let shown = painter.shown(&place, pane.visible, look.base.transition.as_ref());
                (pane, place, look, shown)
            })
            .collect();
        // A group remembers its panes' sizes by key: a hidden pane as 0, and
        // a moving one as wherever it was when the move began. Keying it by
        // each pane's phase starts it afresh when one changes, so a pane
        // opens at its `size` once it has moved.
        let phases: String = panes
            .iter()
            .map(|(_, _, _, shown)| match shown {
                Shown::Settled(false) => '0',
                Shown::Settled(true) => '1',
                Shown::Moving(_) => 'm',
            })
            .collect();
        let key = format!(
            "{}#{phases}",
            self.id
                .clone()
                .unwrap_or_else(|| format!("{}:{}", place.owner, place.path)),
        );
        let mut group = match self.direction {
            Direction::Row => h_resizable(key),
            Direction::Column => v_resizable(key),
        };
        if !self.divider {
            group = group.with_handle_appearance(bare_handle());
        }
        let group = panes
            .into_iter()
            .fold(group, |group, (pane, place, look, shown)| {
                let children = painter.children(&pane.children, &place, Direction::Column, cx);
                let transition = look.base.transition;
                let theme = cx.theme();
                let mut panel = resizable_panel().visible(!shown.hidden());
                if let Some(size) = pane.size {
                    panel = panel.size(px(size));
                }
                if pane.min.is_some() || pane.max.is_some() {
                    let min = px(pane.min.unwrap_or(PANE_MIN));
                    let max = pane.max.map_or(Pixels::MAX, px);
                    panel = panel.size_range(min..max);
                }
                if pane.fixed {
                    panel = panel.fixed();
                }
                let amount = shown.moving();
                let kind = transition.map(|transition| transition.kind);
                if let Some(amount) = amount
                    && matches!(
                        kind,
                        Some(TransitionKind::Slide | TransitionKind::SlideFade)
                    )
                {
                    // The pane's size clamps to its range, so a range of one
                    // size sets it, whatever the group remembers.
                    let size = px(pane.size.or(pane.max).unwrap_or(PANE_SLIDE) * amount);
                    panel = panel.size_range(size..size);
                }
                let mut body = div()
                    .id(place.id())
                    .size_full()
                    .flex()
                    .look(&look, theme)
                    .edges(look.base.border, &look.base, theme)
                    .arranged(
                        &pane.frame,
                        &look.base,
                        Direction::Column,
                        (Space::Normal, Space::None),
                    );
                if let Some(surface) = pane.frame.surface {
                    body = body.bg(surface.hsla(theme));
                }
                if let Some(amount) = amount
                    && matches!(kind, Some(TransitionKind::Fade | TransitionKind::SlideFade))
                {
                    body = body.opacity(amount);
                }
                let handle = painter.motion.scroll(&place.path);
                let body = body.children(children).scrollbars(
                    &pane.frame,
                    Direction::Column,
                    &handle,
                    place.part("scrollbar").id(),
                );
                group.child(panel.child(body))
            });
        div()
            .flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .size_full()
            .child(group)
            .into_any_element()
    }
}

impl<'w> Draw<'w> for Text {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Text, &self.look, false, place);
        let theme = cx.theme();
        let mut element = div()
            .id(place.id())
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .padded(&look.base.padding);
        if let Some(tone) = self.tone {
            element = element.text_color(tone.hsla(theme));
        }
        if let Some(size) = self.size {
            element = element.text_scaled(size, theme);
        }
        if let Some(weight) = self.weight {
            element = element.weighted(weight);
        }
        if self.strike {
            element = element.line_through();
        }
        if self.truncate {
            element = element.min_w_0().truncate();
        }
        if !self.wrap {
            element = element.whitespace_nowrap().flex_shrink_0();
        }
        if let Some(lines) = self.lines {
            element = element.min_w_0().line_clamp(usize::from(lines));
        }
        let text = match look.base.case {
            Some(Case::Upper) => self.text.to_uppercase(),
            Some(Case::Lower) => self.text.to_lowercase(),
            Some(Case::None) | None => self.text.clone(),
        };
        with_extras(element.child(text), &self.extras, painter, place, cx).into_any_element()
    }
}

impl<'w> Draw<'w> for Button {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let states = States {
            disabled: self.disabled,
            ..States::default()
        };
        let look = painter.look_in(Part::Button, &self.look, states, place);
        let theme = cx.theme();
        let mut element = GpuiButton::new(place.id());
        if !self.label.is_empty() {
            element = element.label(self.label.clone());
        }
        if let Some(icon) = &self.icon {
            element = element.icon(GpuiIcon::default().path(icon.path()));
        }
        let mut element = match self.variant {
            Variant::Normal => element,
            Variant::Primary => element.primary(),
            Variant::Ghost => element.ghost(),
            Variant::Outline => element.outline(),
            Variant::Danger => element.danger(),
        };
        // A background or text color from the stylesheet makes it a custom
        // button; otherwise the variant decides.
        let background = look.base.background.map(|color| color.hsla(theme));
        let foreground = look.base.color.map(|color| color.hsla(theme));
        if background.is_some() || foreground.is_some() {
            let background = background.unwrap_or(theme.secondary);
            let hover = look
                .hover
                .background
                .map_or(background.opacity(0.85), |color| color.hsla(theme));
            element = element
                .custom(
                    ButtonCustomVariant::new(cx)
                        .color(background)
                        .foreground(foreground.unwrap_or(theme.foreground))
                        .hover(hover)
                        .active(background),
                )
                // gpui-component paints a custom button at rest as a 20% tint of
                // its color; the button's own style goes on last, so the color
                // the stylesheet asked for is what shows.
                .bg(background);
        }
        // A button takes one radius: its top left's.
        if let Some(radius) = look.base.radius {
            element = element.rounded(px(radius.top_left));
        }
        if self.small {
            element = element.small();
        }
        // One handler for both: a button keeps only the last `on_click`.
        let send = self.action.as_ref().map(|action| place.click(action, cx));
        let copy = self.extras.copy.clone();
        if send.is_some() || copy.is_some() {
            element = element.on_click(move |event, window, cx| {
                if let Some(text) = &copy {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                }
                if let Some(send) = &send {
                    send(event, window, cx);
                }
            });
        }
        let element = element.disabled(self.disabled);
        if self.extras.tooltip.is_none() && self.extras.context.is_none() && !painter.in_drag {
            return element.into_any_element();
        }
        // The tooltip and right-click go on a box around the button, which
        // also keeps its clicks from a drag area.
        let extras = Extras {
            copy: None,
            ..self.extras.clone()
        };
        let wrapper = covering(div().id(place.part("control").id()), painter.in_drag)
            .flex_shrink_0()
            .child(element);
        with_extras(wrapper, &extras, painter, place, cx).into_any_element()
    }
}

impl<'w> Draw<'w> for Item {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let mut look = painter.look(Part::Item, &self.look, self.selected, place);
        // The default hover goes through the look, which sets the element's
        // one hover style.
        look.hover.background.get_or_insert(Color {
            paint: Paint::Token(Token::ListHover),
            alpha: 1.0,
        });
        let theme = cx.theme();
        let mut element = h_flex()
            .id(place.id())
            .flex_shrink_0()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(theme.radius);
        if self.selected {
            element = element.bg(theme.list_active);
        }
        element = element
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .padded(&look.base.padding);
        if let Some(gap) = look.base.gap {
            element = element.gap(px(gap));
        }
        let detail = self.detail.clone().map(|detail| {
            div()
                .flex_shrink_0()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(detail)
        });
        let badge = self.badge.as_deref().map(|text| {
            pill(
                text,
                None,
                &Resolved::default(),
                place.part("badge").id(),
                theme,
            )
        });
        let element = element
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(self.label.clone()),
            )
            .children(detail)
            .children(badge);
        let element = match &self.action {
            Some(action) => covering(element, painter.in_drag)
                .cursor_pointer()
                .on_click(place.click(action, cx)),
            None => element,
        };
        with_extras(element, &self.extras, painter, place, cx).into_any_element()
    }
}

impl<'w> Draw<'w> for Badge {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Badge, &self.look, false, place);
        let pill = pill(&self.text, self.tone, &look, place.id(), cx.theme());
        with_extras(pill, &self.extras, painter, place, cx).into_any_element()
    }
}

impl<'w> Draw<'w> for Icon {
    /// The SVG takes the size and color of the text around it, so the box
    /// carries the stylesheet's look and the icon follows.
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Icon, &self.look, false, place);
        let theme = cx.theme();
        let mut element = div()
            .id(place.id())
            .flex()
            .flex_shrink_0()
            .items_center()
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .padded(&look.base.padding);
        if let Some(tone) = self.tone {
            element = element.text_color(tone.hsla(theme));
        }
        if let Some(size) = self.size {
            element = element.text_scaled(size, theme);
        }
        let mut icon = GpuiIcon::default().path(self.name.path());
        if self.spin
            && let Some(turned) = painter.spin()
        {
            icon = icon.rotate(radians(turned * std::f32::consts::TAU));
        }
        with_extras(element.child(icon), &self.extras, painter, place, cx).into_any_element()
    }
}

impl<'w> Draw<'w> for Image {
    /// The picture fills its own box, which carries the look: a `radius`
    /// rounds the picture itself (avatars). Without a file, or with one
    /// GPUI can't read, the box stays empty at its size.
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Image, &self.look, false, place);
        let theme = cx.theme();
        let folder = painter.folder(place.owner).unwrap_or(Path::new("."));
        let workspace = painter.workspace;
        let owner = place.owner;
        let path = super::images::resolve(self, owner, folder, || {
            workspace
                .roots()
                .layers()
                .map(Path::to_owned)
                .chain([crate::paths::images()])
                .chain(super::images::granted(owner))
                .collect()
        });
        let fit = match self.fit {
            Fit::Contain => ObjectFit::Contain,
            Fit::Cover => ObjectFit::Cover,
            Fit::Fill => ObjectFit::Fill,
            Fit::ScaleDown => ObjectFit::ScaleDown,
            Fit::None => ObjectFit::None,
        };
        let Some(path) = path else {
            // Not a picture the view may show: the empty box.
            let mut element = div()
                .id(place.id())
                .flex_shrink_0()
                .look(&look, theme)
                .edges(look.base.border, &look.base, theme);
            if let Some(width) = self.width {
                element = element.w(px(width));
            }
            if let Some(height) = self.height {
                element = element.h(px(height));
            }
            return with_extras(element, &self.extras, painter, place, cx).into_any_element();
        };
        if super::viewer::wanted(self) {
            return super::viewer::draw(self, &path, painter, place, cx);
        }
        let mut element = img(path)
            .object_fit(fit)
            .id(place.id())
            .flex_shrink_0()
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme);
        if let Some(width) = self.width {
            element = element.w(px(width));
        }
        if let Some(height) = self.height {
            element = element.h(px(height));
        }
        with_extras(element, &self.extras, painter, place, cx).into_any_element()
    }
}

/// A badge's rounded label; a tone written on it outranks the stylesheet.
fn pill(
    text: &str,
    tone: Option<Tone>,
    look: &Resolved,
    id: ElementId,
    theme: &Theme,
) -> Stateful<Div> {
    let color = match tone {
        None | Some(Tone::Normal) => theme.muted_foreground,
        Some(tone) => tone.hsla(theme),
    };
    let element = div()
        .id(id)
        .flex_shrink_0()
        .px(px(6.))
        .rounded_full()
        .text_xs()
        .bg(color.opacity(0.15))
        .text_color(color)
        .look(look, theme)
        .edges(look.base.border, &look.base, theme)
        .padded(&look.base.padding);
    match tone {
        Some(_) => element.bg(color.opacity(0.15)).text_color(color),
        None => element,
    }
    .child(text.to_owned())
}

impl<'w> Draw<'w> for Region {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, _: &Place<'w>, cx: Cx) -> AnyElement {
        let members = painter.region(&self.name, cx);
        let element = div().flex().min_h_0().min_w_0().gap(px(self.gap.pixels()));
        let (element, members) = match self.direction {
            // A row of mods, such as a status bar, never spills onto its
            // neighbours: it takes room in proportion to what it holds, a
            // member that does not fit wraps onto a line of its own below,
            // which is clipped away, and a first member wider than the row
            // is cut. Each member is as tall as the row, so only the first
            // line shows; the row needs a parent of definite height.
            Direction::Row => (
                element
                    .flex_auto()
                    .h_full()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .content_start()
                    .overflow_hidden(),
                members
                    .into_iter()
                    .map(|member| {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .h_full()
                            .min_w_0()
                            .overflow_hidden()
                            .child(member)
                            .into_any_element()
                    })
                    .collect(),
            ),
            Direction::Column => (element.flex_1().flex_col(), members),
        };
        match self.justify {
            Justify::Start => element.justify_start(),
            Justify::Center => element.justify_center(),
            Justify::End => element.justify_end(),
            Justify::Between => element.justify_between(),
        }
        .children(members)
        .into_any_element()
    }
}

/// A line across the parent's axis, in the stylesheet's `divider` color.
pub(super) fn divider(painter: &Painter<'_, '_>, place: &Place, cx: Cx) -> AnyElement {
    let look = painter.look(Part::Divider, &Look::default(), false, place);
    let theme = cx.theme();
    let color = look
        .base
        .background
        .map_or(theme.border, |color| color.hsla(theme));
    match place.parent {
        Direction::Row => div().w(px(1.)).h_full(),
        Direction::Column => div().h(px(1.)).w_full(),
    }
    .bg(color)
    .flex_shrink_0()
    .into_any_element()
}

impl<'w> Draw<'w> for Popover {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let anchor = painter.children(&self.anchor, place, Direction::Row, cx);
        let container = div()
            .id(place.id())
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .children(anchor);
        let content_place = place.part("content");
        let look = painter.look(Part::Popover, &self.look, false, &content_place);
        let transition = look.base.transition;
        let shown = painter.shown(&content_place, self.open, transition.as_ref());
        if self.open {
            if let Some(msg) = &self.dismiss {
                painter.backdrops.push(Backdrop {
                    priority: painter.layer + POPOVER_BACKDROP,
                    owner: place.owner,
                    to: self.to.clone(),
                    msg: msg.clone(),
                });
            }
            if !self.keys.is_empty() || self.dismiss.is_some() {
                painter.key_layers.push(KeyLayer {
                    priority: painter.layer + POPOVER_CONTENT,
                    owner: place.owner.to_owned(),
                    to: self.to.clone(),
                    keys: self.keys.clone(),
                    dismiss: self.dismiss.clone(),
                    modal: false,
                });
            }
        }
        if shown.hidden() {
            return container.into_any_element();
        }
        let content = self
            .content
            .draw(painter, &content_place.child(0, Direction::Column), cx);
        let theme = cx.theme();
        let kind = transition.map(|transition| transition.kind);
        let moving = shown.moving();
        // Sliding starts it a little toward its anchor; it rises into place.
        let shift = match (moving, kind) {
            (Some(amount), Some(TransitionKind::Slide | TransitionKind::SlideFade)) => {
                POPOVER_SLIDE * (1.0 - amount)
            },
            _ => 0.0,
        };
        let mut sheet = div()
            .id(content_place.id())
            .absolute()
            .flex()
            .flex_col()
            .occlude()
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .padded(&look.base.padding);
        sheet = match self.side {
            Side::Top => sheet.bottom_full().mb(px(self.gap - shift)),
            Side::Bottom => sheet.top_full().mt(px(self.gap - shift)),
            Side::Left => sheet.right_full().mr(px(self.gap - shift)),
            Side::Right => sheet.left_full().ml(px(self.gap - shift)),
        };
        // Above or below, it lines up with the anchor's left or right edge;
        // beside it, with its top or bottom.
        sheet = match (self.side, self.align) {
            (Side::Top | Side::Bottom, Anchor::Start) => sheet.left_0(),
            (Side::Top | Side::Bottom, Anchor::End) => sheet.right_0(),
            (Side::Left | Side::Right, Anchor::Start) => sheet.top_0(),
            (Side::Left | Side::Right, Anchor::End) => sheet.bottom_0(),
        };
        if let Some(width) = self.width {
            sheet = sheet.w(px(width));
        }
        if let Some(gap) = look.base.gap {
            sheet = sheet.gap(px(gap));
        }
        if let (Some(amount), Some(TransitionKind::Fade | TransitionKind::SlideFade)) =
            (moving, kind)
        {
            sheet = sheet.opacity(amount);
        }
        container
            .child(
                deferred(sheet.child(content).child(crate::browser::cover()))
                    .with_priority(painter.layer + POPOVER_CONTENT),
            )
            .into_any_element()
    }
}
