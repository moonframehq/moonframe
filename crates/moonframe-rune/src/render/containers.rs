//! Stacks, panels, splits, regions and lists.

use {
    super::{Painter, Place},
    crate::{
        Ui, Watched,
        style::{framed, looked},
    },
    moonframe_core::{
        Align, Border, Direction, Justify, List, ListWindow, Look, Panel, Part, Region, Resolved,
        Sides, Space, Split, Stack, States, Style, Token,
    },
    rune::{Cursor, Drag, El, Phase, ScrollSpec, Tx, col, el, row, text},
    std::{cell::Cell, collections::HashMap, rc::Rc},
};

/// The smallest a pane gets when a module gives a `max` and no `min`.
const PANE_MIN: f32 = 80.;
/// The room kept at the end of a row region for how many it hides.
const HIDDEN_CHIP: f32 = 28.;
/// Width of the strip a split's edge is dragged by.
const HANDLE: f32 = 6.;
/// A panel's title, as GPUI's `text_xs`.
const PANEL_TITLE: f32 = 12.;
const PANEL_HEADER: f32 = 34.;

fn stacked(direction: Direction) -> El {
    match direction {
        Direction::Row => row(),
        Direction::Column => col(),
    }
}

/// A row of mods, such as a status bar, never spills onto its neighbours, as
/// on GPUI: a member that does not fit wraps onto a line of its own below,
/// which is clipped away, and a first member wider than the row is cut. Each
/// member is as tall as the row, so only the first line shows; the row needs
/// a parent of definite height.
///
/// `grow` here would share the parent evenly whatever the regions hold, so
/// each is as wide as its mods and regions side by side give way in proportion
/// to that, every one keeping what still fits. The parent's `justify` places
/// them: a region has no spare room of its own to align in.
///
/// A member that does not fit wraps below the row, where it is clipped away.
/// Each says where it landed, so the region can count those and say so.
fn row_region(element: El, key: &str, members: impl Iterator<Item = El>) -> El {
    let region = key.to_owned();
    element
        .shrink()
        .h_full()
        .flex_wrap()
        .items_center()
        .clip_rounded(0.)
        .on_layout(move |ui: &mut Ui, rect: [f32; 4]| {
            ui.row_regions.laid_out(&region, rect);
        })
        .children(members.enumerate().map(|(index, member)| {
            let region = key.to_owned();
            row()
                .key(format!("{key}-member{index}"))
                .items_center()
                .h_full()
                .shrink()
                .clip_rounded(0.)
                .on_layout(move |ui: &mut Ui, rect: [f32; 4]| {
                    ui.row_regions.member_laid_out(&region, index, rect);
                })
                .child(member)
        }))
}

/// What a row region knows of where its members landed.
#[derive(Default)]
struct RowRegion {
    /// The bottom of the row, in the window's points.
    bottom: f32,
    /// The top and width of each member, by its place in the row.
    members: HashMap<usize, (f32, f32)>,
}

/// Row regions by their path, as last laid out.
#[derive(Default)]
pub(crate) struct RowRegions(HashMap<String, RowRegion>);

impl RowRegions {
    fn laid_out(&mut self, key: &str, [_, y, _, height]: [f32; 4]) {
        self.0.entry(key.to_owned()).or_default().bottom = y + height;
    }

    fn member_laid_out(&mut self, key: &str, index: usize, [_, y, width, _]: [f32; 4]) {
        let region = self.0.entry(key.to_owned()).or_default();
        region.members.insert(index, (y, width));
    }

    /// How many of its first `count` members wrapped below the row. One with
    /// no width has nothing to show, so it is not missed.
    fn hidden(&self, key: &str, count: usize) -> usize {
        self.0.get(key).map_or(0, |region| {
            region
                .members
                .iter()
                .filter(|(index, (top, width))| {
                    **index < count && *width > 0.5 && *top >= region.bottom - 0.5
                })
                .count()
        })
    }
}

impl<'w> Painter<'w> {
    /// A region with `hidden` members says so at its end, in room of its own,
    /// so the chip covers nothing that still shows.
    fn with_hidden_chip(&self, element: El, hidden: usize, place: &Place<'w>) -> El {
        if hidden == 0 {
            return element;
        }
        let items = if hidden == 1 {
            "item"
        } else {
            "items"
        };
        let chip = row()
            .key(format!("{}-hidden", place.path))
            .pin(None, Some(0.), Some(0.), Some(0.))
            .w(HIDDEN_CHIP)
            .items_center()
            .justify_end()
            .help(format!(
                "{hidden} {items} hidden: widen the window to show them"
            ))
            .child(
                text(format!("+{hidden}"))
                    .font(place.ink.text_style())
                    .color(self.palette.token(Token::MutedForeground)),
            );
        element.pr(HIDDEN_CHIP).child(chip)
    }

    pub(super) fn stack(&self, stack: &'w Stack, place: &Place<'w>) -> El {
        let part = match stack.direction {
            Direction::Row => Part::Row,
            Direction::Column => Part::Column,
        };
        let states = States {
            selected: stack.selected,
            ..States::default()
        };
        let look = self.look(part, &stack.look, states, place);
        let inner = Place {
            parent: stack.direction,
            ink: place.ink.under(&look.base, self.palette),
            ..place.clone()
        };
        let element = stacked(stack.direction).key(place.path.clone());
        let element = match stack.align {
            Align::Stretch => element.items_stretch(),
            Align::Start => element.items_start(),
            Align::Center => element.items_center(),
            Align::End => element.items_end(),
        };
        let element = match stack.justify {
            Justify::Start => element.justify_start(),
            Justify::Center => element.justify_center(),
            Justify::End => element.justify_end(),
            Justify::Between => element.justify_between(),
        };
        let gap = stack
            .frame
            .gap
            .map(Space::pixels)
            .or(look.base.gap)
            .unwrap_or(Space::Normal.pixels());
        let element = element.gap(gap);
        let element = if stack.wrap {
            element.flex_wrap()
        } else {
            element
        };
        let element = element.children(self.nodes(&stack.children, &inner));
        let element = looked(element, &look, stack.frame.border, self.palette);
        let column = stack.direction == Direction::Column;
        let element = framed(element, &stack.frame, column, self.palette);
        let element = if stack.drag {
            element.window_drag()
        } else {
            element
        };
        let element = self.clicked(element, stack.action.as_ref(), place);
        self.extras(element, &stack.extras, place)
    }

    pub(super) fn panel(&self, panel: &'w Panel, place: &Place<'w>) -> El {
        let look = self.look(Part::Panel, &panel.look, States::default(), place);
        let inner = Place {
            parent: Direction::Column,
            ink: place.ink.under(&look.base, self.palette),
            ..place.clone()
        };
        let mut element = col().key(place.path.clone());
        if panel.title.is_some() || !panel.actions.is_empty() {
            let header = self.look(
                Part::PanelHeader,
                &Look::default(),
                States::default(),
                place,
            );
            let mut ink = inner.ink;
            ink.color = self.palette.token(Token::MutedForeground);
            ink.size = PANEL_TITLE;
            ink.weight = 600.;
            let ink = ink.under(&header.base, self.palette);
            let title = panel.title.clone().unwrap_or_default();
            let bar = row()
                .items_center()
                .gap(4.)
                .px(8.)
                .min_h(PANEL_HEADER)
                .child(
                    text(title)
                        .font(ink.text_style())
                        .color(ink.color)
                        .truncate()
                        .grow(1.)
                        .min_w(0.),
                )
                .children(self.nodes(&panel.actions, &inner.child(0)));
            let border = header.base.border.or(Some(Border::Bottom));
            element = element.child(looked(bar, &header, border, self.palette));
        }
        // Padding and gap belong to the body, under the header.
        let mut outer = look.clone();
        outer.base.padding = Sides::default();
        outer.base.gap = None;
        let gap = panel
            .frame
            .gap
            .map(Space::pixels)
            .or(look.base.gap)
            .unwrap_or(Space::Normal.pixels());
        let body = col()
            .grow(1.)
            .min_h(0.)
            .padding(Space::Normal.pixels())
            .gap(gap)
            .children(self.nodes(&panel.children, &inner.child(1)));
        let padding = Resolved {
            base: Style {
                padding: look.base.padding,
                ..Style::default()
            },
            hover: Style::default(),
        };
        let body = looked(body, &padding, None, self.palette);
        let body = match panel.frame.padding {
            Some(padding) => body.padding(padding.pixels()),
            None => body,
        };
        let element = looked(
            element.child(body),
            &outer,
            panel.frame.border,
            self.palette,
        );
        // Rounded corners clip what scrolls under them, as GPUI's panel does.
        let element = match look.base.radius {
            Some(radius) => element.clip_rounded(radius.top_left),
            None => element,
        };
        let mut frame = panel.frame.clone();
        frame.padding = None;
        framed(element, &frame, true, self.palette)
    }

    pub(super) fn split(&self, split: &'w Split, place: &Place<'w>) -> El {
        let across = split.direction == Direction::Row;
        let mut element = stacked(split.direction)
            .key(place.path.clone())
            .grow(1.)
            .min_w(0.)
            .min_h(0.);
        let shown: Vec<_> = split
            .panes
            .iter()
            .enumerate()
            .filter(|(_, pane)| pane.visible)
            .collect();
        for (at, (index, pane)) in shown.iter().enumerate() {
            let inner = place.child(*index);
            let look = self.look(Part::Pane, &pane.look, States::default(), &inner);
            let sized = self.ui.panes.get(&inner.path).copied().or(pane.size);
            let content = Place {
                parent: Direction::Column,
                ink: inner.ink.under(&look.base, self.palette),
                ..inner.clone()
            };
            let body = col().children(self.nodes(&pane.children, &content));
            let body = looked(body, &look, pane.frame.border, self.palette);
            let body = framed(body, &pane.frame, true, self.palette);
            let body = match sized {
                Some(size) if across => body.w(size).h_full(),
                Some(size) => body.h(size).w_full(),
                None => body.grow(1.).min_w(0.).min_h(0.),
            };
            element = element.child(body);
            let Some((next, after)) = shown.get(at + 1) else {
                continue;
            };
            // The edge moves the sized pane beside it: a right-hand sidebar
            // grows as the edge goes left.
            let (target, mine, sign) = if sized.is_some() {
                (*index, *pane, 1.)
            } else {
                (*next, *after, -1.)
            };
            let start = self
                .ui
                .panes
                .get(&place.child(target).path)
                .copied()
                .or(mine.size)
                .unwrap_or(PANE_MIN);
            let (least, most) = (
                mine.min.unwrap_or(PANE_MIN),
                mine.max.unwrap_or(f32::INFINITY),
            );
            let pane_key = place.child(target).path;
            let edge = el().key(format!("{}-edge{at}", place.path)).on_drag(
                move |ui: &mut Ui, drag: &mut Drag, _: &mut Tx| {
                    if matches!(drag.phase, Phase::Down) {
                        drag.data = start;
                    }
                    let moved = if across {
                        drag.dx
                    } else {
                        drag.dy
                    };
                    let size = (drag.data + sign * moved).clamp(least, most);
                    ui.panes.insert(pane_key.clone(), size);
                },
            );
            let line = if split.divider {
                self.palette.token(Token::Border)
            } else {
                rune::Color::TRANSPARENT
            };
            // One point of layout: the strip overlaps the panes either side.
            let overlap = -(HANDLE - 1.) / 2.;
            let edge = if across {
                edge.w(HANDLE)
                    .h_full()
                    .ml(overlap)
                    .mr(overlap)
                    .items_center()
                    .justify_center()
                    .child(el().w(1.).h_full().fill(line))
                    .cursor(Cursor::ResizeColumn)
            } else {
                edge.h(HANDLE)
                    .w_full()
                    .mt(overlap)
                    .mb(overlap)
                    .items_center()
                    .justify_center()
                    .child(el().h(1.).w_full().fill(line))
                    .cursor(Cursor::ResizeRow)
            };
            element = element.child(edge);
        }
        element
    }

    pub(super) fn region(&self, region: &'w Region, place: &Place<'w>) -> El {
        let mods = self.ui.workspace.region(&region.name);
        let mods = mods.into_iter().enumerate();
        let element = stacked(region.direction)
            .key(place.path.clone())
            .gap(region.gap.pixels())
            .min_h(0.);
        let element = match region.justify {
            Justify::Start => element.justify_start(),
            Justify::Center => element.justify_center(),
            Justify::End => element.justify_end(),
            Justify::Between => element.justify_between(),
        };
        match region.direction {
            Direction::Row => {
                let members: Vec<El> = mods
                    .flat_map(|(index, name)| self.members(name, &place.child(index)))
                    .collect();
                let hidden = self.ui.row_regions.hidden(&place.path, members.len());
                let element = row_region(element, &place.path, members.into_iter());
                self.with_hidden_chip(element, hidden, place)
            },
            Direction::Column => element
                .grow(1.)
                .min_w(0.)
                .children(mods.map(|(index, name)| self.slot(name, &place.child(index)))),
        }
    }

    pub(super) fn list(&self, list: &'w List, place: &Place<'w>) -> El {
        let look = self.look(Part::List, &list.look, States::default(), place);
        let key = format!("{}:{}", place.owner, list.id);
        let mut lists = self.ui.lists.borrow_mut();
        let window = ListWindow {
            start: list.start,
            len: list.rows.len(),
        };
        let watched = lists.entry(key).or_insert_with(|| Watched {
            owner: place.owner.to_owned(),
            id: list.id.clone(),
            row_height: list.row_height,
            count: list.count,
            window,
            seen: Rc::new(Cell::new((0., 0.))),
        });
        watched.count = list.count;
        watched.row_height = list.row_height;
        watched.window = window;
        let seen = Rc::clone(&watched.seen);
        drop(lists);
        // Rows sit where they would among all of them: the space above and
        // below stands for those not built.
        let above = list.start as f32 * list.row_height;
        let below =
            list.count.saturating_sub(list.start + list.rows.len()) as f32 * list.row_height;
        let rows = self
            .nodes(&list.rows, place)
            .into_iter()
            .map(|built| built.h(list.row_height));
        let element = col()
            .key(place.path.clone())
            .min_h(0.)
            .child(el().h(above))
            .children(rows.collect::<Vec<_>>())
            .child(el().h(below))
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .on_scroll_offset(move |position| {
                seen.set((position.offset[1], position.viewport[1]));
            });
        let element = looked(element, &look, list.frame.border, self.palette);
        framed(element, &list.frame, true, self.palette)
    }
}
