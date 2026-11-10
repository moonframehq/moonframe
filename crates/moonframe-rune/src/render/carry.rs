//! A row's or column's `carry` and `accept`: dragging things inside the app.
//! A row with `carry` dims while a box of its size follows the pointer; the
//! innermost row with `accept` under the pointer hears it move over, leave,
//! and drop, as on GPUI. Targets are found where the last frame drew them.

use {
    super::{Painter, Place},
    crate::Ui,
    moonframe_core::{CarryPhase, Data, Pointer, Token, Travel},
    rune::{Drag, El, Phase, Tx, el},
};

/// How faint a row is while it is carried.
const CARRIED: f32 = 0.4;
/// Moves over a target shorter than this, in points, are not sent.
const STILL: f32 = 2.;
/// The box's corners.
const RADIUS: f32 = 8.;

/// A row that takes what is carried, where the last frame drew it.
pub(crate) struct Target {
    path: String,
    owner: String,
    pointer: Pointer,
    rect: [f32; 4],
}

/// What a drag carries, and where it is.
pub(crate) struct Carrying {
    data: Data,
    from: String,
    path: String,
    /// The carried row's size, and where in it the pointer took it.
    size: [f32; 2],
    grip: [f32; 2],
    /// The pointer, in window points.
    at: [f32; 2],
    /// The target it is over, and where it last told it.
    over: Option<(String, [f32; 2])>,
}

/// Where `at` is in `rect`, as a message reports it.
fn within([x, y, width, height]: [f32; 4], [px, py]: [f32; 2]) -> Travel {
    let fraction = |at: f32, size: f32| {
        if size > 0. {
            at / size
        } else {
            0.
        }
    };
    Travel {
        x: fraction(px - x, width),
        y: fraction(py - y, height),
        dx: 0.,
        dy: 0.,
        width,
        height,
    }
}

impl Ui {
    /// The innermost target under `at`: the smallest one holding it.
    fn target_at(&self, at: [f32; 2]) -> Option<usize> {
        let targets = self.carry_targets.borrow();
        targets
            .iter()
            .enumerate()
            .filter(|(_, target)| {
                let [x, y, width, height] = target.rect;
                (x..x + width).contains(&at[0]) && (y..y + height).contains(&at[1])
            })
            .min_by(|(_, a), (_, b)| (a.rect[2] * a.rect[3]).total_cmp(&(b.rect[2] * b.rect[3])))
            .map(|(index, _)| index)
    }

    /// Tells the target `index` of what is carried, as `phase`.
    fn tell_target(
        &mut self,
        index: usize,
        phase: CarryPhase,
        data: &Data,
        from: &str,
        at: [f32; 2],
    ) {
        let said = {
            let targets = self.carry_targets.borrow();
            targets.get(index).and_then(|target| {
                let to = target
                    .pointer
                    .to
                    .clone()
                    .unwrap_or_else(|| target.owner.clone());
                let msg =
                    target
                        .pointer
                        .accept_message(phase, data, from, &within(target.rect, at))?;
                Some((target.owner.clone(), to, msg))
            })
        };
        if let Some((owner, to, msg)) = said {
            self.send(&owner, &to, msg);
        }
    }

    fn target_path(&self, index: usize) -> Option<String> {
        self.carry_targets
            .borrow()
            .get(index)
            .map(|target| target.path.clone())
    }

    fn target_index(&self, path: &str) -> Option<usize> {
        self.carry_targets
            .borrow()
            .iter()
            .position(|target| target.path == path)
    }

    /// The carried thing moved to `at`: over a new target, off an old one.
    fn carry_to(&mut self, at: [f32; 2]) {
        let Some(mut carrying) = self.carrying.get_mut().take() else {
            return;
        };
        carrying.at = at;
        let now = self.target_at(at);
        let now_path = now.and_then(|index| self.target_path(index));
        let was = carrying.over.as_ref().map(|(path, _)| path.clone());
        if was != now_path
            && let Some(old) = was.as_deref().and_then(|path| self.target_index(path))
        {
            self.tell_target(old, CarryPhase::Leave, &carrying.data, &carrying.from, at);
        }
        let moved = carrying.over.as_ref().is_none_or(|(path, last)| {
            Some(path) != now_path.as_ref()
                || (last[0] - at[0]).abs() >= STILL
                || (last[1] - at[1]).abs() >= STILL
        });
        if let (Some(index), true) = (now, moved) {
            self.tell_target(index, CarryPhase::Over, &carrying.data, &carrying.from, at);
        }
        carrying.over = match (now_path, moved) {
            (Some(path), true) => Some((path, at)),
            (Some(path), false) => carrying.over.filter(|(over, _)| *over == path),
            (None, _) => None,
        };
        *self.carrying.get_mut() = Some(carrying);
    }

    /// The button came up: a drop on the target under it, unless cancelled.
    fn put_down(&mut self, cancelled: bool) {
        let Some(carrying) = self.carrying.get_mut().take() else {
            return;
        };
        let target = carrying
            .over
            .as_ref()
            .and_then(|(path, _)| self.target_index(path));
        if let Some(index) = target {
            let phase = if cancelled {
                CarryPhase::Leave
            } else {
                CarryPhase::Drop
            };
            self.tell_target(index, phase, &carrying.data, &carrying.from, carrying.at);
        }
    }
}

impl<'w> Painter<'w> {
    /// `element` as something carried, and as a place that takes it.
    pub(super) fn carried(&self, element: El, pointer: &Pointer, place: &Place<'w>) -> El {
        let rect = rune::memo::window_layout(&place.path);
        if pointer.accept.is_some()
            && let Some(rect) = rect
        {
            self.ui.carry_targets.borrow_mut().push(Target {
                path: place.path.clone(),
                owner: place.owner.to_owned(),
                pointer: pointer.clone(),
                rect,
            });
        }
        let Some(data) = pointer.carry.clone() else {
            return element;
        };
        let carried = self.ui.carrying.borrow();
        let lifted = carried
            .as_ref()
            .filter(|carrying| carrying.path == place.path);
        if let Some(carrying) = lifted {
            let [x, y] = carrying.at;
            let [gx, gy] = carrying.grip;
            let [width, height] = carrying.size;
            self.floating.borrow_mut().push(
                el().key("carried")
                    .pin(Some(x - gx), Some(y - gy), None, None)
                    .size(width, height)
                    .radius(RADIUS)
                    .fill(self.palette.token(Token::Card).alpha(0.9))
                    .border(1., self.palette.token(Token::Accent))
                    .shadow(8., 24., rune::Color::BLACK.alpha(0.2))
                    .inert(),
            );
        }
        let element = if lifted.is_some() {
            element.opacity(CARRIED)
        } else {
            element
        };
        drop(carried);
        let (from, path) = (place.owner.to_owned(), place.path.clone());
        element.on_drag(move |ui: &mut Ui, drag: &mut Drag, _: &mut Tx| {
            let origin = rect.map_or([0., 0.], |[x, y, ..]| [x, y]);
            let at = [origin[0] + drag.x, origin[1] + drag.y];
            match drag.phase {
                Phase::Down => {
                    *ui.carrying.get_mut() = Some(Carrying {
                        data: data.clone(),
                        from: from.clone(),
                        path: path.clone(),
                        size: [drag.w, drag.h],
                        grip: [drag.x, drag.y],
                        at,
                        over: None,
                    });
                },
                Phase::Move => ui.carry_to(at),
                Phase::Up => {
                    ui.carry_to(at);
                    ui.put_down(drag.cancelled());
                },
            }
        })
    }
}
