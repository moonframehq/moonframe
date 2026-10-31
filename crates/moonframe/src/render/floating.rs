//! Overlays: content floating over the whole window, such as a dialog, a
//! command palette, a sheet along an edge, or a toast in a corner. Each
//! open overlay is drawn where the view puts it (so its content's paths and
//! the inspector see it there), but painted at the window's root, one layer
//! above the last, by [`Painter::shell`].

use {
    super::{
        Cx, Draw, KeyLayer, OVERLAY_BASE, OVERLAY_STEP, Painter, Place,
        styling::{LookExt as _, StyleExt as _},
    },
    crate::color::ThemeColor,
    gpui_kit::{component::ActiveTheme, *},
    moonframe_core::{Action, Direction, Look, Overlay, Part, Placement, TransitionKind},
};

/// How dark the window behind a backdrop gets, unless the stylesheet's
/// `overlay-backdrop` rule sets a background.
const BACKDROP_ALPHA: f32 = 0.35;
/// How far a floating overlay's content travels as it slides in or out.
const RISE: f32 = 12.;
/// What a sheet with no `width` (or `height`) slides across.
const SHEET_SLIDE: f32 = 320.;
/// Space between a floating overlay and the window's edge, and how far a
/// `top` one hangs down; the `overlay` rule's `margin` changes it.
const EDGE: f32 = 16.;
const HANG: f32 = 80.;
/// The content's padding unless the stylesheet sets one.
const PADDING: f32 = 16.;

impl<'w> Draw<'w> for Overlay {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let content_place = place.part("content");
        let look = painter.look(Part::Overlay, &self.look, false, &content_place);
        let transition = look.base.transition;
        let shown = painter.shown(&content_place, self.open, transition.as_ref());
        // In the component browser it is drawn in place, as a card, so a
        // preview shows it without covering the browser.
        if painter.preview {
            if !self.open {
                return div().into_any_element();
            }
            let content =
                self.content
                    .draw(painter, &content_place.child(0, Direction::Column), cx);
            return sheet(self, &look, &content_place, None, cx)
                .child(content)
                .into_any_element();
        }
        // Nothing takes its place in the layout where the view puts it.
        let placeholder = div().into_any_element();
        if shown.hidden() {
            return placeholder;
        }
        let index = painter.overlays.len();
        painter.overlays.push(None);
        let priority = OVERLAY_BASE + OVERLAY_STEP * index;
        if self.open {
            painter.key_layers.push(KeyLayer {
                priority,
                owner: place.owner.to_owned(),
                to: self.to.clone(),
                keys: self.keys.clone(),
                dismiss: self.dismiss.clone(),
                modal: self.backdrop,
            });
        }
        // Popovers inside it open above it.
        let outer = std::mem::replace(&mut painter.layer, priority);
        let content = self
            .content
            .draw(painter, &content_place.child(0, Direction::Column), cx);
        painter.layer = outer;

        let theme = cx.theme();
        let kind = transition.map(|transition| transition.kind);
        let moving = shown.moving();
        let mut layer = div()
            .id(place.part("layer").id())
            .absolute()
            .inset_0()
            .flex();
        layer = match self.place {
            Placement::Center => layer.flex_col().items_center().justify_center(),
            Placement::Top => layer.flex_col().items_center().justify_start(),
            Placement::Bottom => layer.flex_col().items_stretch().justify_end(),
            Placement::Left => layer.flex_row().items_stretch().justify_start(),
            Placement::Right => layer.flex_row().items_stretch().justify_end(),
            Placement::TopStart => layer.flex_col().items_start().justify_start(),
            Placement::TopEnd => layer.flex_col().items_end().justify_start(),
            Placement::BottomStart => layer.flex_col().items_start().justify_end(),
            Placement::BottomEnd => layer.flex_col().items_end().justify_end(),
        };
        if self.backdrop {
            let backdrop = painter.look(
                Part::OverlayBackdrop,
                &Look::default(),
                false,
                &place.part("backdrop"),
            );
            let mut color = backdrop
                .base
                .background
                .map_or(black().opacity(BACKDROP_ALPHA), |color| color.hsla(theme));
            if let Some(opacity) = backdrop.base.opacity {
                color = color.opacity(opacity);
            }
            // It fades with the content, whichever way the content moves.
            if let Some(amount) = moving {
                color = color.opacity(amount);
            }
            layer = layer.bg(color);
        }
        // Only an open overlay catches the pointer: one closing lets clicks
        // through at once.
        if self.open && (self.backdrop || self.dismiss.is_some()) {
            layer = layer.occlude();
            if let Some(msg) = &self.dismiss {
                let action = Action::Send {
                    to: self.to.clone(),
                    msg: msg.clone(),
                };
                let owner = place.owner.to_owned();
                layer = layer.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                        this.dispatch(&owner, &action, window, cx)
                    }),
                );
            }
        }
        let mut sheet = sheet(self, &look, &content_place, moving.zip(kind), cx);
        if self.open {
            sheet = sheet.occlude();
        }
        // A page under it hides while it shows: under the whole backdrop,
        // else under the content alone.
        if self.backdrop {
            layer = layer.child(crate::browser::cover());
        }
        painter.overlays[index] = Some(
            deferred(layer.child(sheet.child(content).child(crate::browser::cover())))
                .with_priority(priority)
                .into_any_element(),
        );
        placeholder
    }
}

/// The box around an overlay's content: its defaults, then the `overlay`
/// rule, sized and, while it moves, slid and faded by its transition.
fn sheet(
    overlay: &Overlay,
    look: &moonframe_core::Resolved,
    place: &Place,
    moving: Option<(f32, TransitionKind)>,
    cx: Cx,
) -> Stateful<Div> {
    let theme = cx.theme();
    let mut sheet = div()
        .id(place.id())
        .flex()
        .flex_col()
        .flex_shrink_0()
        .p(px(PADDING))
        .bg(theme.background)
        .text_color(theme.foreground)
        .border_color(theme.border)
        .shadow_lg();
    sheet = match overlay.place {
        Placement::Bottom => sheet.border_t_1(),
        Placement::Left => sheet.border_r_1(),
        Placement::Right => sheet.border_l_1(),
        // Floating boxes are rounded a step more than controls.
        _ => sheet.border_1().rounded(theme.radius * 1.5),
    };
    sheet = match overlay.place {
        Placement::Top => sheet.mt(px(HANG)).mx(px(EDGE)),
        Placement::Bottom | Placement::Left | Placement::Right => sheet,
        _ => sheet.m(px(EDGE)),
    };
    sheet = sheet
        .look(look, theme)
        .edges(look.base.border, &look.base, theme)
        .padded(&look.base.padding);
    if let Some(gap) = look.base.gap {
        sheet = sheet.gap(px(gap));
    }
    if let Some(width) = overlay.width {
        sheet = sheet.w(px(width));
    }
    if let Some(height) = overlay.height {
        sheet = sheet.h(px(height));
    }
    let Some((amount, kind)) = moving else {
        return sheet;
    };
    if matches!(kind, TransitionKind::Slide | TransitionKind::SlideFade) {
        // A sheet slides in from its edge, across its own size; a
        // floating box rises (or drops) a little into place.
        let away = 1.0 - amount;
        let across = |size: Option<f32>| size.unwrap_or(SHEET_SLIDE) * away;
        let (x, y) = match overlay.place {
            Placement::Left => (-across(overlay.width), 0.),
            Placement::Right => (across(overlay.width), 0.),
            Placement::Bottom => (0., across(overlay.height)),
            Placement::Top | Placement::TopStart | Placement::TopEnd => (0., -RISE * away),
            Placement::Center | Placement::BottomStart | Placement::BottomEnd => (0., RISE * away),
        };
        sheet = sheet.relative().left(px(x)).top(px(y));
    }
    if matches!(kind, TransitionKind::Fade | TransitionKind::SlideFade) {
        sheet = sheet.opacity(look.base.opacity.unwrap_or(1.0) * amount);
    }
    sheet
}
