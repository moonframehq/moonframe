//! `ui.list`: rows, of one height or each its own, in a box that scrolls,
//! of which only those the view built are drawn. Where the box is scrolled
//! says which rows show; when they are not all built, the painter asks the
//! mod for them once the frame is drawn, and the next frame has them.

use {
    super::{
        Cx, Draw, Painter, Place,
        styling::{FrameExt as _, LookExt as _},
    },
    gpui_kit::{component::ActiveTheme, *},
    moonframe_core::{Direction, List, ListWindow, Part, Space},
};

mod range;

/// Rows built beyond those showing, on each side, so scrolling a little
/// never waits for the mod.
const OVERSCAN: usize = 16;
/// The height assumed before the list has been laid out once.
const FIRST_HEIGHT: f32 = 800.;

impl<'w> Draw<'w> for List {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::List, &self.look, false, place);
        let handle = painter.motion.scroll(&place.path);
        let rows_of = &self.row_height;
        // Last frame's layout: where it is scrolled, and how tall it is.
        let scrolled = (-f32::from(handle.offset().y)).max(0.0);
        let height = f32::from(handle.bounds().size.height);
        let height = if height > 0.0 {
            height
        } else {
            FIRST_HEIGHT
        };
        let shown = rows_of.shown(scrolled, height, self.count);
        let built = self.start..self.start + self.rows.len();
        if shown.start < built.start || shown.end > built.end {
            painter
                .list_requests
                .push((place.owner.to_owned(), self.id.clone(), ListWindow {
                    start: shown.start.saturating_sub(OVERSCAN),
                    len: shown.len() + 2 * OVERSCAN,
                }));
        }
        let rows: Vec<AnyElement> = painter.faded(look.base.opacity, |painter| {
            self.rows
                .iter()
                .enumerate()
                .map(|(offset, node)| {
                    let index = self.start + offset;
                    let child = node.draw(painter, &place.child(index, Direction::Column), cx);
                    div()
                        .absolute()
                        .top(px(rows_of.top(index)))
                        .left_0()
                        .right_0()
                        .h(px(rows_of.height(index)))
                        .flex()
                        .flex_col()
                        .justify_center()
                        .overflow_hidden()
                        .child(child)
                        .into_any_element()
                })
                .collect()
        });
        // Where the rows sit, under the list's padding, for `range`.
        let laid = std::rc::Rc::new(std::cell::Cell::new(Bounds::<Pixels>::default()));
        let measure = self.range.is_some().then(|| {
            let laid = laid.clone();
            canvas(move |bounds, _, _| laid.set(bounds), |_, (), _, _| {})
                .absolute()
                .size_full()
        });
        let theme = cx.theme();
        let mut element = div().id(place.id()).flex().flex_col();
        // A list needs a bounded height to scroll in: without one of its
        // own, it takes the space it is given.
        if self.frame.height.or(look.base.height).is_none() {
            element = element.flex_1().min_h_0();
        }
        element
            .look(&look, theme)
            .placed(&self.frame, &look.base, theme)
            .arranged(
                &self.frame,
                &look.base,
                Direction::Column,
                (Space::None, Space::None),
            )
            .overflow_y_scroll()
            .track_scroll(&handle)
            .child(
                div()
                    .relative()
                    .flex_shrink_0()
                    .w_full()
                    .h(px(rows_of.total(self.count)))
                    .children(rows)
                    .children(measure),
            )
            .children(self.range.as_ref().and_then(|range| {
                range::reader(
                    &self.id, range, &handle, laid, rows_of, self.count, painter, place, cx,
                )
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use moonframe_core::ListWindow;

    #[test]
    fn a_window_stops_at_the_last_row() {
        let window = ListWindow { start: 90, len: 40 };
        assert_eq!(window.range(100), 90..100);
        assert_eq!(window.range(50), 50..50);
        let huge = ListWindow {
            start: 0,
            len: usize::MAX,
        };
        assert_eq!(huge.range(5000).len(), ListWindow::MAX);
    }
}
