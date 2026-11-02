//! Popovers: content floating beside an anchor, such as a dropdown's menu,
//! a context menu or a submenu. The content opens on the side the view asks
//! for unless only the other side has room in the window; while open, a
//! click outside sends `dismiss`, and with `anchor_clicks` the anchor keeps
//! its own clicks.

use {
    super::{
        Backdrop, Cx, Draw, KeyLayer, POPOVER_BACKDROP, POPOVER_CONTENT, Painter, Place,
        styling::{LookExt as _, StyleExt as _},
    },
    crate::motion::Room,
    gpui_kit::{component::ActiveTheme, *},
    moonframe_core::{Anchor, Direction, Part, Popover, Side, TransitionKind},
    std::rc::Rc,
};

/// How far a popover's content travels as it slides in or out.
const POPOVER_SLIDE: f32 = 8.;

impl<'w> Draw<'w> for Popover {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let anchor = painter.children(&self.anchor, place, Direction::Row, cx);
        // Above the click-outside layer, the anchor takes its own clicks.
        let lifted = self.open && self.anchor_clicks && self.dismiss.is_some();
        let anchor: Vec<AnyElement> = if lifted {
            let priority = painter.layer + POPOVER_CONTENT;
            anchor
                .into_iter()
                .map(|element| deferred(element).with_priority(priority).into_any_element())
                .collect()
        } else {
            anchor
        };
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
        let room = painter.motion.room(&content_place.path);
        let side = if room.flipped.get() {
            opposite(self.side)
        } else {
            self.side
        };
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
        sheet = match side {
            Side::Top => sheet.bottom_full().mb(px(self.gap - shift)),
            Side::Bottom => sheet.top_full().mt(px(self.gap - shift)),
            Side::Left => sheet.right_full().mr(px(self.gap - shift)),
            Side::Right => sheet.left_full().ml(px(self.gap - shift)),
        };
        // Above or below, it lines up with the anchor's left or right edge;
        // beside it, with its top or bottom.
        sheet = match (side, self.align) {
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
        if !room.decided.get() {
            sheet = sheet.opacity(0.);
        }
        let sheet = sheet
            .child(content)
            .child(crate::browser::cover())
            .child(measure_content(room.clone(), self.side, self.gap));
        container
            .child(measure_anchor(room))
            .child(deferred(sheet).with_priority(painter.layer + POPOVER_CONTENT))
            .into_any_element()
    }
}

/// The side across the anchor from `side`.
fn opposite(side: Side) -> Side {
    match side {
        Side::Top => Side::Bottom,
        Side::Bottom => Side::Top,
        Side::Left => Side::Right,
        Side::Right => Side::Left,
    }
}

/// Whether content of `size` fits on `side` of `anchor`, `gap` away from
/// it, inside a window of `viewport`.
fn fits(
    side: Side,
    anchor: Bounds<Pixels>,
    size: Size<Pixels>,
    gap: f32,
    viewport: Size<Pixels>,
) -> bool {
    let gap = px(gap);
    match side {
        Side::Top => anchor.top() - gap - size.height >= px(0.),
        Side::Bottom => anchor.bottom() + gap + size.height <= viewport.height,
        Side::Left => anchor.left() - gap - size.width >= px(0.),
        Side::Right => anchor.right() + gap + size.width <= viewport.width,
    }
}

/// A layer over the anchor's box that records where it is in the window.
fn measure_anchor(room: Rc<Room>) -> impl IntoElement {
    canvas(
        move |bounds, _, _| room.anchor.set(Some(bounds)),
        |_, (), _, _| {},
    )
    .absolute()
    .size_full()
}

/// A layer over the content's box: it opens on the side asked for unless
/// only the other has room. A changed side draws again the next frame.
fn measure_content(room: Rc<Room>, side: Side, gap: f32) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let Some(anchor) = room.anchor.get() else {
                return;
            };
            let viewport = window.viewport_size();
            let flipped = !fits(side, anchor, bounds.size, gap, viewport)
                && fits(opposite(side), anchor, bounds.size, gap, viewport);
            if flipped != room.flipped.get() || !room.decided.get() {
                room.flipped.set(flipped);
                room.decided.set(true);
                window.request_animation_frame();
            }
        },
        |_, (), _, _| {},
    )
    .absolute()
    .size_full()
}

#[cfg(test)]
mod tests {
    use {
        super::{fits, opposite},
        gpui_kit::{Bounds, Pixels, point, px, size},
        moonframe_core::Side,
    };

    fn anchor(x: f32, y: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(100.), px(20.)))
    }

    #[test]
    fn content_opens_where_the_window_has_room() {
        let window = size(px(800.), px(600.));
        let menu = size(px(200.), px(150.));
        // Room on the right of an anchor in the middle; none at the edge.
        assert!(fits(Side::Right, anchor(300., 100.), menu, 8., window));
        assert!(!fits(Side::Right, anchor(600., 100.), menu, 8., window));
        assert!(fits(Side::Left, anchor(600., 100.), menu, 8., window));
        // Below near the bottom does not fit; above does.
        assert!(!fits(Side::Bottom, anchor(300., 500.), menu, 4., window));
        assert!(fits(Side::Top, anchor(300., 500.), menu, 4., window));
        assert_eq!(opposite(Side::Right), Side::Left);
        assert_eq!(opposite(Side::Top), Side::Bottom);
    }
}
