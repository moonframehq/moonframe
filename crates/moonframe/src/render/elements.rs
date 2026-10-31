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
    std::path::{Path, PathBuf},
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
        let children = painter.children(&self.children, place, self.direction, cx);
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
        let element = element.children(children);
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
        let element = match &self.action {
            Some(action) => covering(element, in_drag)
                .cursor_pointer()
                .on_click(place.click(action, cx)),
            None => element,
        };
        with_extras(element, &self.extras, painter, place, cx).into_any_element()
    }
}
/// A control's tooltip, right-click and copy, on the element that draws it.
fn with_extras<'w>(
    mut element: Stateful<Div>,
    extras: &Extras,
    painter: &Painter<'w, '_>,
    place: &Place<'w>,
    cx: Cx,
) -> Stateful<Div> {
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
        let text = SharedString::from(text.clone());
        element = element.tooltip(move |_, cx| {
            let tip = Tip {
                text: text.clone(),
                look: look.clone(),
            };
            cx.new(|_| tip).into()
        });
    }
    element
}

/// A tooltip's content, styled by the `tooltip` rule.
struct Tip {
    text: SharedString,
    look: Resolved,
}

impl Render for Tip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
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
            .child(self.text.clone())
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
        let body = div()
            .id(place.part("body").id())
            .flex()
            .flex_1()
            .min_h_0()
            .arranged(
                &self.frame,
                &look.base,
                Direction::Column,
                (Space::Normal, Space::Normal),
            )
            .children(children);
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
        let group = match self.direction {
            Direction::Row => h_resizable(key),
            Direction::Column => v_resizable(key),
        };
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
                group.child(panel.child(body.children(children)))
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
        element.child(text).into_any_element()
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
            element = element.custom(
                ButtonCustomVariant::new(cx)
                    .color(background)
                    .foreground(foreground.unwrap_or(theme.foreground))
                    .hover(hover)
                    .active(background),
            );
        }
        if let Some(radius) = look.base.radius {
            element = element.rounded(px(radius));
        }
        if self.small {
            element = element.small();
        }
        if let Some(action) = &self.action {
            element = element.on_click(place.click(action, cx));
        }
        if let Some(text) = self.extras.copy.clone() {
            element = element.on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
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
        pill(&self.text, self.tone, &look, place.id(), cx.theme()).into_any_element()
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
        element.child(icon).into_any_element()
    }
}

impl<'w> Draw<'w> for Image {
    /// The picture fills its own box, which carries the look: a `radius`
    /// rounds the picture itself (avatars). Without a file, or with one
    /// GPUI can't read, the box stays empty at its size.
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Image, &self.look, false, place);
        let theme = cx.theme();
        let home = std::env::home_dir();
        let folder = painter.folder(place.owner).unwrap_or(Path::new("."));
        let mut allowed: Vec<PathBuf> = painter
            .workspace
            .roots()
            .layers()
            .map(Path::to_owned)
            .collect();
        allowed.push(crate::paths::images());
        let path = self.resolve(folder, home.as_deref(), &allowed);
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
            return element.into_any_element();
        };
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
        element.into_any_element()
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
        let element = div()
            .flex()
            .min_h_0()
            .min_w_0()
            .gap(px(self.gap.pixels()));
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
        };
        sheet = match self.align {
            Anchor::Start => sheet.left_0(),
            Anchor::End => sheet.right_0(),
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
