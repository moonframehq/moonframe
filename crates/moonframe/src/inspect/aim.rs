//! Which node a pick targets. The pointer finds the deepest node it can see
//! under it; ↑ then steps out to the node around that one (a cell's row, the
//! row's list, the list's panel) and ↓ steps back in, so a whole row or
//! panel is as easy to pick as the text inside it. A level chosen with ↑ is
//! kept while the pointer moves over nodes of the same kind at the same
//! depth, so moving down a list keeps picking rows rather than cells.

use {
    super::{Inspector, Placed},
    gpui_kit::*,
};

/// Where the pointer is, and how far out of the deepest node ↑ took it.
#[derive(Default)]
pub(crate) struct Aim {
    point: Option<Point<Pixels>>,
    held: Option<Held>,
}

/// A level chosen with ↑: its depth from the outermost node, and what kind
/// of node was there, so the same level is kept only over the same kind.
#[derive(Debug)]
struct Held {
    depth: usize,
    kind: Kind,
}

/// A node's element type (`row`, `list`, `panel`) and the line that built
/// it: rows a list builds share both, whatever classes each one has.
#[derive(Debug, PartialEq, Eq)]
struct Kind {
    element: String,
    line: Option<u32>,
}

impl Kind {
    fn of(placed: &Placed) -> Self {
        let label = &placed.crumb.label;
        let end = label.find(['.', ' ']).unwrap_or(label.len());
        Self {
            element: label[..end].to_owned(),
            line: placed.crumb.line,
        }
    }
}

/// What the picker targets, and what is around it.
#[derive(Clone)]
pub(crate) struct Target {
    pub(crate) placed: Placed,
    /// The node ↑ would step out to.
    pub(crate) around: Option<String>,
    /// How many steps out of the deepest node it is.
    pub(crate) lifted: usize,
    /// The deepest node under the pointer, which ↓ steps back to.
    pub(crate) deepest: Placed,
}

/// The node under `point` that is not clipped away, the deepest first, then
/// each node around it out to the window's.
pub(crate) fn lineage(recorded: &[Placed], point: Point<Pixels>) -> Vec<Placed> {
    recorded
        .iter()
        .rposition(|placed| placed.visible.contains(&point))
        .map(|index| around(recorded, index))
        .unwrap_or_default()
}

/// `recorded[index]` and every recorded node around it, innermost first.
/// A node is recorded before those inside it, so each is looked for only
/// before the last one found. Nodes drawn without a record (a mod's root
/// marker, a pane's wrapper) are passed over.
fn around(recorded: &[Placed], index: usize) -> Vec<Placed> {
    let mut chain = vec![recorded[index].clone()];
    let mut before = &recorded[..index];
    let mut crumb = recorded[index].crumb.parent.clone();
    while let Some(current) = crumb {
        if let Some(path) = &current.path
            && let Some(found) = before.iter().rposition(|placed| &placed.path == path)
        {
            chain.push(before[found].clone());
            before = &before[..found];
        }
        crumb = current.parent.clone();
    }
    chain
}

impl Aim {
    /// The target with the pointer at `point`.
    pub(crate) fn at(&mut self, recorded: &[Placed], point: Point<Pixels>) -> Option<Target> {
        self.point = Some(point);
        let chain = lineage(recorded, point);
        let index = self.index(&chain);
        target(chain, index)
    }

    /// Steps out to the node around the target, or back in toward the
    /// deepest node, with the pointer where it last was (else at `point`).
    pub(crate) fn step(
        &mut self,
        recorded: &[Placed],
        point: Point<Pixels>,
        out: bool,
    ) -> Option<Target> {
        let point = *self.point.get_or_insert(point);
        let chain = lineage(recorded, point);
        let last = chain.len().checked_sub(1)?;
        let current = self.index(&chain);
        let index = if out {
            (current + 1).min(last)
        } else {
            current.saturating_sub(1)
        };
        self.held = (index > 0).then(|| Held {
            depth: last - index,
            kind: Kind::of(&chain[index]),
        });
        target(chain, index)
    }

    /// Where in `chain` the held level is, if a node of its kind is there;
    /// else the deepest node.
    fn index(&self, chain: &[Placed]) -> usize {
        let Some(held) = &self.held else {
            return 0;
        };
        match chain.len().checked_sub(1 + held.depth) {
            Some(index) if Kind::of(&chain[index]) == held.kind => index,
            _ => 0,
        }
    }
}

fn target(chain: Vec<Placed>, index: usize) -> Option<Target> {
    let around = chain
        .get(index + 1)
        .map(|placed| placed.crumb.label.clone());
    let deepest = chain.first()?.clone();
    chain.into_iter().nth(index).map(|placed| Target {
        placed,
        around,
        lifted: index,
        deepest,
    })
}

impl Inspector {
    /// Aims at `point`; true when the target changed.
    pub(crate) fn point(&mut self, point: Point<Pixels>) -> bool {
        let target = self.aim.at(&self.sink.borrow(), point);
        self.retarget(target)
    }

    /// ↑ (`out`) or ↓ while picking; true when the target changed.
    pub(crate) fn step(&mut self, point: Point<Pixels>, out: bool) -> bool {
        let target = self.aim.step(&self.sink.borrow(), point, out);
        self.retarget(target)
    }

    fn retarget(&mut self, target: Option<Target>) -> bool {
        let path = |target: &Option<Target>| {
            target
                .as_ref()
                .map(|target| (target.placed.path.clone(), target.lifted))
        };
        let changed = path(&target) != path(&self.hovered);
        self.hovered = target;
        changed
    }

    /// Stops hovering, and forgets the level ↑ chose.
    pub(crate) fn unhover(&mut self) {
        self.hovered = None;
        self.previewed = None;
        self.aim = Aim::default();
    }

    /// The deepest node a selection of `picked` keeps in its card's trail,
    /// to step back in to: what was under the pointer when it was picked
    /// with ↑, the last selection's when it is one around that, else itself.
    pub(crate) fn deepest(&self, picked: &Placed) -> Placed {
        let hovered = self.hovered.as_ref();
        let hovered = hovered.filter(|target| target.placed.path == picked.path);
        let inside = self.leaf.as_ref().filter(|leaf| {
            leaf.crumb
                .trail()
                .iter()
                .any(|crumb| crumb.path.as_deref() == Some(picked.path.as_str()))
        });
        hovered
            .map(|target| &target.deepest)
            .or(inside)
            .unwrap_or(picked)
            .clone()
    }

    /// The size of the node at `path` in the last whole frame, for a frame
    /// being built; none for one not drawn then.
    pub(crate) fn drawn_size(&self, path: &str) -> Option<Size<Pixels>> {
        self.drawn
            .borrow()
            .iter()
            .find(|placed| placed.path == path)
            .map(|placed| placed.bounds.size)
    }

    /// The node drawn at `path` in the last frame.
    pub(crate) fn placed(&self, path: &str) -> Option<Placed> {
        self.sink
            .borrow()
            .iter()
            .find(|placed| placed.path == path)
            .cloned()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {
        super::{Aim, Target, lineage},
        crate::inspect::{Crumb, Inspector, Placed},
        gpui_kit::{Bounds, Corners, Pixels, Point, point, px, size},
        std::rc::Rc,
    };

    fn crumb(label: &str, path: &str, line: u32, parent: Option<&Placed>) -> Rc<Crumb> {
        Rc::new(Crumb {
            label: label.to_owned(),
            line: Some(line),
            owner: None,
            path: Some(path.to_owned()),
            parent: parent.map(|placed| placed.crumb.clone()),
            does: None,
        })
    }

    fn placed(
        label: &str,
        path: &str,
        line: u32,
        parent: Option<&Placed>,
        (x, y, w, h): (f32, f32, f32, f32),
    ) -> Placed {
        let bounds = Bounds::new(point(px(x), px(y)), size(px(w), px(h)));
        let visible = match parent {
            Some(parent) => bounds.intersect(&parent.visible),
            None => bounds,
        };
        Placed {
            bounds,
            visible,
            corners: Corners::default(),
            path: path.to_owned(),
            crumb: crumb(label, path, line, parent),
        }
    }

    /// A panel holding a list scrolled by 10 rows of 22: row 10 shows at
    /// the list's top, row 9 is scrolled out of sight above it, under the
    /// panel's header. Each row holds a cell holding a text.
    fn mail() -> Vec<Placed> {
        let panel = placed("panel.mail", "mail", 1, None, (0., 0., 900., 600.));
        let header = placed(
            "text.header \"Inbox\"",
            "mail/0",
            2,
            Some(&panel),
            (0., 0., 900., 40.),
        );
        // A mod's root marker sits between the panel and the list, with no
        // record of its own.
        let marker = Rc::new(Crumb {
            label: "mod table".to_owned(),
            line: None,
            owner: Some("table".to_owned()),
            path: None,
            parent: Some(panel.crumb.clone()),
            does: None,
        });
        let mut list = placed(
            "list.mail-list",
            "mail/1",
            3,
            Some(&panel),
            (0., 40., 900., 560.),
        );
        list.crumb = Rc::new(Crumb {
            label: "list.mail-list".to_owned(),
            line: Some(3),
            owner: None,
            path: Some("mail/1".to_owned()),
            parent: Some(marker),
            does: None,
        });
        let mut recorded = vec![panel, header, list.clone()];
        for index in 9..12 {
            let top = 40. + (index as f32 - 10.) * 22.;
            let class = if index == 10 {
                "row.mail-row.selected"
            } else {
                "row.mail-row"
            };
            let path = format!("mail/1/{index}");
            let row = placed(class, &path, 4, Some(&list), (0., top, 900., 22.));
            let cell = placed(
                "column.mail-table-cell",
                &format!("{path}/0"),
                5,
                Some(&row),
                (0., top, 856., 22.),
            );
            let text = placed(
                "text.subject \"Hi\"",
                &format!("{path}/0/0"),
                6,
                Some(&cell),
                (4., top + 3., 100., 16.),
            );
            recorded.extend([row, cell, text]);
        }
        recorded
    }

    fn at(x: f32, y: f32) -> Point<Pixels> {
        point(px(x), px(y))
    }

    fn path(target: Option<Target>) -> String {
        target.map(|target| target.placed.path).unwrap_or_default()
    }

    #[test]
    fn the_pointer_finds_the_deepest_node_it_can_see() {
        let recorded = mail();
        let mut aim = Aim::default();
        assert_eq!(path(aim.at(&recorded, at(10., 50.))), "mail/1/10/0/0");
        assert_eq!(path(aim.at(&recorded, at(500., 50.))), "mail/1/10/0");
        // The scrolled-out row 9 lies under the header but is clipped away.
        assert_eq!(path(aim.at(&recorded, at(10., 30.))), "mail/0");
    }

    #[test]
    fn lineage_walks_out_through_unrecorded_nodes() {
        let recorded = mail();
        let paths: Vec<String> = lineage(&recorded, at(10., 50.))
            .into_iter()
            .map(|placed| placed.path)
            .collect();
        assert_eq!(paths, [
            "mail/1/10/0/0",
            "mail/1/10/0",
            "mail/1/10",
            "mail/1",
            "mail"
        ]);
    }

    #[test]
    fn up_steps_out_to_the_row_list_and_panel_and_down_steps_back() {
        let recorded = mail();
        let mut aim = Aim::default();
        let point = at(10., 50.);
        aim.at(&recorded, point);
        let mut out = Vec::new();
        for _ in 0..5 {
            out.push(path(aim.step(&recorded, point, true)));
        }
        // It stops at the outermost.
        assert_eq!(out, ["mail/1/10/0", "mail/1/10", "mail/1", "mail", "mail"]);
        let target = aim.step(&recorded, point, false).unwrap();
        assert_eq!(target.placed.path, "mail/1");
        assert_eq!(target.lifted, 3);
        assert_eq!(target.around.as_deref(), Some("panel.mail"));
        for _ in 0..5 {
            aim.step(&recorded, point, false);
        }
        assert_eq!(path(aim.at(&recorded, point)), "mail/1/10/0/0");
    }

    #[test]
    fn a_row_chosen_with_up_stays_a_row_down_the_list() {
        let recorded = mail();
        let mut aim = Aim::default();
        aim.at(&recorded, at(10., 50.));
        aim.step(&recorded, at(10., 50.), true);
        assert_eq!(path(aim.step(&recorded, at(10., 50.), true)), "mail/1/10");
        // On the next row's cell padding, not its text: still the row,
        // though its classes differ from the selected row's.
        assert_eq!(path(aim.at(&recorded, at(500., 70.))), "mail/1/11");
        // Over the header, there is no row: the deepest node, until the
        // pointer comes back to a row.
        assert_eq!(path(aim.at(&recorded, at(10., 20.))), "mail/0");
        assert_eq!(path(aim.at(&recorded, at(10., 45.))), "mail/1/10");
    }

    #[test]
    fn a_step_before_the_pointer_moves_aims_where_it_is() {
        let recorded = mail();
        let mut aim = Aim::default();
        assert_eq!(path(aim.step(&recorded, at(10., 50.), true)), "mail/1/10/0");
        assert!(aim.step(&recorded, at(-5., -5.), false).is_some());
        let mut empty = Aim::default();
        assert!(empty.step(&recorded, at(-5., -5.), true).is_none());
    }

    #[test]
    fn a_frame_being_built_still_knows_the_last_frames_sizes() {
        let recorded = mail();
        let mut inspector = Inspector {
            selected: recorded.first().cloned(),
            ..Inspector::default()
        };
        *inspector.sink.borrow_mut() = recorded;
        // A frame asks for the sink twice, the second time finding it empty.
        assert!(inspector.sink().is_some());
        assert!(
            inspector
                .sink()
                .is_some_and(|sink| sink.borrow().is_empty())
        );
        assert_eq!(inspector.drawn_size("mail"), Some(size(px(900.), px(600.))));
        assert_eq!(inspector.drawn_size("gone"), None);
        inspector.selected = None;
    }

    #[test]
    fn the_card_keeps_what_was_under_the_pointer_to_step_back_in() {
        let recorded = mail();
        let mut inspector = Inspector::default();
        *inspector.sink.borrow_mut() = recorded.clone();
        let pointer = at(10., 50.);
        inspector.point(pointer);
        inspector.step(pointer, true);
        assert!(inspector.step(pointer, true));
        // Picked the row with ↑: the card's trail still reaches the text.
        let row = inspector.hovered.clone().unwrap().placed;
        assert_eq!(row.path, "mail/1/10");
        let leaf = inspector.deepest(&row);
        assert_eq!(leaf.path, "mail/1/10/0/0");
        inspector.hovered = None;
        inspector.leaf = Some(leaf);
        // Its list, from the trail, keeps the text; another row does not.
        let list = inspector.placed("mail/1").unwrap();
        assert_eq!(inspector.deepest(&list).path, "mail/1/10/0/0");
        let other = inspector.placed("mail/1/11").unwrap();
        assert_eq!(inspector.deepest(&other).path, "mail/1/11");
    }
}
