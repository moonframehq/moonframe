//! How each element draws itself.

use {
    super::{
        Cx, Draw, Painter, Place, Shown,
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
        Align, Anchor, Badge, Button, Color, Direction, Icon, Item, Justify, Look, Paint, Panel,
        Part, Popover, Region, Resolved, Side, Sides, Space, Split, Stack, Text, Token, Tone,
        TransitionKind, Variant,
    },
};

/// The smallest a pane gets when a module gives a `max` and no `min`.
const PANE_MIN: f32 = 80.;
/// What a sliding pane with neither `size` nor `max` slides across.
const PANE_SLIDE: f32 = 240.;
/// How far a popover's content travels as it slides in or out.
const POPOVER_SLIDE: f32 = 8.;

impl<'w> Draw<'w> for Stack {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let children = painter.children(&self.children, place, self.direction, cx);
        let part = match self.direction {
            Direction::Row => Part::Row,
            Direction::Column => Part::Column,
        };
        let look = painter.look(part, &self.look, false, place);
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
        match &self.action {
            Some(action) => element
                .cursor_pointer()
                .on_click(place.click(action, cx))
                .into_any_element(),
            None => element.into_any_element(),
        }
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
        element.child(self.text.clone()).into_any_element()
    }
}

impl<'w> Draw<'w> for Button {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Button, &self.look, false, place);
        let theme = cx.theme();
        let element = GpuiButton::new(place.id()).label(self.label.clone());
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
        element.disabled(self.disabled).into_any_element()
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
        match &self.action {
            Some(action) => element
                .cursor_pointer()
                .on_click(place.click(action, cx))
                .into_any_element(),
            None => element.into_any_element(),
        }
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
        element
            .child(GpuiIcon::default().path(self.name.path()))
            .into_any_element()
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
            .flex_1()
            .min_h_0()
            .min_w_0()
            .gap(px(self.gap.pixels()));
        let element = match self.direction {
            Direction::Row => element.flex_row().items_center(),
            Direction::Column => element.flex_col(),
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
        if self.open
            && let Some(msg) = &self.dismiss
        {
            painter.backdrop = Some((place.owner, msg.clone()));
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
            .child(deferred(sheet.child(content)).with_priority(2))
            .into_any_element()
    }
}
