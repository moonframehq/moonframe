//! A split's dividers take the keyboard and a double-click: a click on one
//! gives it focus, the arrow keys along the split then move it 16 px (64
//! with Shift), Home and End take the pane before it to its least and most,
//! and a double-click puts the panes back to their starting `size`.
//!
//! GPUI's resize handle draws what its renderer returns inside its band, so
//! the renderer here wraps the look in a box that takes focus and the
//! double-click, and notes where it was laid out: a renderer is not told
//! which divider it draws, so a key finds that by where the box sits among
//! the panes' sizes.

use {
    gpui_kit::{
        base::{ResizablePanelGroup, ResizableState, ResizeHandleRenderer},
        *,
    },
    std::{
        cell::{Cell, RefCell},
        collections::{HashMap, HashSet},
        rc::Rc,
    },
};

/// How far an arrow moves a divider, and with Shift.
const STEP: f32 = 16.;
const SHIFT_STEP: f32 = 64.;

thread_local! {
    /// Each split's sizes, by the key its group keeps them under.
    static STATES: RefCell<HashMap<String, Entity<ResizableState>>> = RefCell::new(HashMap::new());
    /// Each split's dividers, by the same key.
    static SPLITS: RefCell<HashMap<String, Split>> = RefCell::new(HashMap::new());
    /// The splits drawn since their window's last frame ended, by key.
    static DRAWN: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// What a split tells its mod once its panes are resized: by a drag, a
/// key or a double-click.
pub(super) type Resized = Rc<dyn Fn(&Entity<ResizableState>, &mut Window, &mut App)>;

#[derive(Default)]
struct Split {
    /// The window it is drawn in.
    window: Option<WindowId>,
    /// Its `resize`, if it has one.
    resized: Option<Resized>,
    vertical: bool,
    /// The paint priority of the layer it is drawn in: 0 for the window,
    /// an overlay's inside one, whose backdrop keeps keys from those below.
    layer: usize,
    /// The group's box, where its first pane starts.
    group: Rc<Cell<Bounds<Pixels>>>,
    /// Each pane's starting `size`.
    sizes: Vec<Option<f32>>,
    /// Each divider drawn, in the order the group draws them.
    dividers: Vec<Divider>,
}

struct Divider {
    focus: FocusHandle,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

/// How a split is drawn, for its dividers.
pub(super) struct Drawn {
    /// Its group's key.
    pub(super) key: String,
    /// The window it is in, whose own sizes it keeps.
    pub(super) window: Option<WindowId>,
    /// The paint priority of the layer it is drawn in.
    pub(super) layer: usize,
    pub(super) vertical: bool,
    /// Its panes' starting sizes.
    pub(super) sizes: Vec<Option<f32>>,
    /// Its `resize`, if it has one.
    pub(super) resized: Option<Resized>,
}

/// A frame of `window` is drawn: the splits it no longer drew go, with
/// their sizes, dividers and focus.
pub(crate) fn frame_drawn(window: WindowId) {
    let drawn = DRAWN.with_borrow_mut(|drawn| {
        let mine: HashSet<String> = SPLITS.with_borrow(|splits| {
            drawn
                .iter()
                .filter(|key| {
                    splits
                        .get(*key)
                        .is_some_and(|split| split.window == Some(window))
                })
                .cloned()
                .collect()
        });
        drawn.retain(|key| !mine.contains(key));
        mine
    });
    forget(|split, key| split.window != Some(window) || drawn.contains(key));
}

/// Windows other than `open` closed: their splits go.
pub(crate) fn closed(open: &[WindowId]) {
    forget(|split, _| split.window.is_none_or(|window| open.contains(&window)));
}

/// How many splits are kept, for tests.
#[cfg(test)]
pub(crate) fn kept() -> usize {
    SPLITS.with_borrow(HashMap::len)
}

/// Keeps only the splits `keep` says, and their sizes.
fn forget(keep: impl Fn(&Split, &str) -> bool) {
    SPLITS.with_borrow_mut(|splits| splits.retain(|key, split| keep(split, key)));
    let kept = |key: &String| SPLITS.with_borrow(|splits| splits.contains_key(key));
    STATES.with_borrow_mut(|states| states.retain(|key, _| kept(key)));
}

/// `group`, drawn as `drawn` says, with dividers drawn as `look` draws them
/// that take keys and a double-click, and a box of no size that notes
/// where the group is, to go beside it.
pub(super) fn wire(
    group: ResizablePanelGroup,
    drawn: Drawn,
    look: ResizeHandleRenderer,
) -> (ResizablePanelGroup, impl IntoElement) {
    // Kept per window: two windows showing one split each size their own.
    let key = format!("{:?}/{}", drawn.window, drawn.key);
    let key = key.as_str();
    DRAWN.with_borrow_mut(|keys| keys.insert(key.to_owned()));
    let resized = drawn.resized.clone();
    let at = SPLITS.with_borrow_mut(|splits| {
        let split = splits.entry(key.to_owned()).or_default();
        split.window = drawn.window;
        split.resized = drawn.resized;
        split.vertical = drawn.vertical;
        split.layer = drawn.layer;
        split.sizes = drawn.sizes;
        split.group.clone()
    });
    let measure = canvas(move |laid, _, _| at.set(laid), |_, (), _, _| {})
        .absolute()
        .size_full();
    // A split keyed anew (a pane started or stopped moving) starts afresh,
    // as its group does.
    let split = key.rsplit_once('#').map_or(key, |(split, _)| split);
    let same = |other: &String| {
        other == key
            || other
                .rsplit_once('#')
                .map_or(other.as_str(), |(split, _)| split)
                != split
    };
    STATES.with_borrow_mut(|states| states.retain(|other, _| same(other)));
    SPLITS.with_borrow_mut(|splits| splits.retain(|other, _| same(other)));
    // Sizes kept here, so keys can change them. The first frame draws
    // before there is one, with the group's own.
    let group = match STATES.with_borrow(|states| states.get(key).cloned()) {
        Some(state) => group.with_state(&state),
        None => group,
    };
    let group = match resized {
        Some(resized) => group.on_resize(move |state, window, cx| resized(state, window, cx)),
        None => group,
    };
    let key = key.to_owned();
    let drawn = Rc::new(Cell::new(0));
    let group = group.with_handle_appearance(Rc::new(move |handle, window, cx| {
        let inner = look(handle, window, cx).unwrap_or_else(|| div().into_any_element());
        if STATES.with_borrow(|states| !states.contains_key(&key)) {
            let state = cx.new(|_| ResizableState::default());
            STATES.with_borrow_mut(|states| states.insert(key.clone(), state));
            window.refresh();
        }
        let index = drawn.get();
        drawn.set(index + 1);
        let (focus, bounds) = SPLITS.with_borrow_mut(|splits| {
            let split = splits.entry(key.clone()).or_default();
            if split.dividers.len() <= index {
                split.dividers.push(Divider {
                    focus: cx.focus_handle(),
                    bounds: Rc::default(),
                });
            }
            let divider = &split.dividers[index];
            (divider.focus.clone(), divider.bounds.clone())
        });
        let (reset, clicked) = (key.clone(), bounds.clone());
        Some(
            div()
                .id(("moonframe-divider", index))
                .size_full()
                .track_focus(&focus)
                .on_click(move |event, window, cx| {
                    if event.click_count() == 2 {
                        restore(&reset, clicked.get(), window, cx);
                    }
                })
                .child(inner)
                .child(
                    canvas(move |laid, _, _| bounds.set(laid), |_, (), _, _| {})
                        .absolute()
                        .size_full(),
                )
                .into_any_element(),
        )
    }));
    (group, measure)
}

/// The split and the bounds of the divider with focus in `window`, in a
/// layer of paint priority `above` or higher.
fn focused(window: &Window, above: usize) -> Option<(String, Bounds<Pixels>)> {
    SPLITS.with_borrow(|splits| {
        splits.iter().find_map(|(key, split)| {
            if split.layer < above {
                return None;
            }
            let divider = split
                .dividers
                .iter()
                .find(|divider| divider.focus.is_focused(window))?;
            Some((key.clone(), divider.bounds.get()))
        })
    })
}

/// Which divider of a group whose panes are `sizes` long sits at `along`
/// (from the group's start): the one ending the pane before it, from 0.
fn index_at(sizes: &[Pixels], along: Pixels) -> Option<usize> {
    let mut end = Pixels::ZERO;
    let mut best: Option<(usize, Pixels)> = None;
    for (index, size) in sizes.iter().enumerate().take(sizes.len().saturating_sub(1)) {
        end += *size;
        let off = (end - along).abs();
        if best.is_none_or(|(_, least)| off < least) {
            best = Some((index, off));
        }
    }
    best.map(|(index, _)| index)
}

/// The state of the split `key`, whether it splits top to bottom, and
/// which of its dividers is the one at `bounds`.
fn locate(
    key: &str,
    bounds: Bounds<Pixels>,
    cx: &App,
) -> Option<(Entity<ResizableState>, bool, usize)> {
    let state = STATES.with_borrow(|states| states.get(key).cloned())?;
    let (vertical, group) = SPLITS.with_borrow(|splits| {
        splits
            .get(key)
            .map(|split| (split.vertical, split.group.get()))
    })?;
    let along = if vertical {
        bounds.center().y - group.top()
    } else {
        bounds.center().x - group.left()
    };
    let index = index_at(state.read(cx).sizes(), along)?;
    Some((state, vertical, index))
}

/// Moves the focused divider for a key: arrows along the split, Home and
/// End; only one in a layer of paint priority `above` or higher, as an
/// overlay's backdrop keeps keys from the window behind it. Whether a
/// divider had focus and took the key.
pub(crate) fn keystroke(
    keystroke: &Keystroke,
    above: usize,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some((key, bounds)) = focused(window, above) else {
        return false;
    };
    let modifiers = keystroke.modifiers;
    if modifiers.control || modifiers.alt || modifiers.platform || modifiers.function {
        return false;
    }
    let step = px(if modifiers.shift {
        SHIFT_STEP
    } else {
        STEP
    });
    let Some((state, vertical, index)) = locate(&key, bounds, cx) else {
        return false;
    };
    let size = state.read(cx).sizes()[index];
    let to = match (keystroke.key.as_str(), vertical) {
        ("left", false) | ("up", true) => size - step,
        ("right", false) | ("down", true) => size + step,
        ("home", _) => Pixels::ZERO,
        ("end", _) => state.read(cx).container_size(),
        _ => return false,
    };
    state.update(cx, |state, cx| state.resize_panel(index, to, window, cx));
    tell_resized(&key, &state, window, cx);
    true
}

/// Tells the split `key`'s mod its panes' sizes, as a drag does once it
/// ends.
fn tell_resized(key: &str, state: &Entity<ResizableState>, window: &mut Window, cx: &mut App) {
    if let Some(resized) = SPLITS.with_borrow(|splits| splits.get(key)?.resized.clone()) {
        // Once the key is done with: the app hears keys while updating.
        let state = state.clone();
        window.defer(cx, move |window, cx| resized(&state, window, cx));
    }
}

/// Puts the panes beside the divider at `bounds` of the split `key` back to
/// their starting size: the pane before it, else the one after, else halves.
fn restore(key: &str, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let Some((state, _, index)) = locate(key, bounds, cx) else {
        return;
    };
    let starting = SPLITS.with_borrow(|splits| {
        let sizes = &splits.get(key)?.sizes;
        Some((
            sizes.get(index).copied().flatten(),
            sizes.get(index + 1).copied().flatten(),
        ))
    });
    let current = state.read(cx).sizes().clone();
    let (pane, size) = match starting {
        Some((Some(before), _)) => (index, px(before)),
        Some((None, Some(after))) => (index + 1, px(after)),
        _ => (index, (current[index] + current[index + 1]) / 2.),
    };
    state.update(cx, |state, cx| state.resize_panel(pane, size, window, cx));
    tell_resized(key, &state, window, cx);
}

#[cfg(test)]
mod tests {
    use {
        super::index_at,
        gpui_kit::{Pixels, px},
    };

    #[test]
    fn a_divider_is_found_where_the_pane_before_it_ends() {
        let sizes: Vec<Pixels> = [200., 300., 100.].into_iter().map(px).collect();
        assert_eq!(index_at(&sizes, px(201.)), Some(0));
        assert_eq!(index_at(&sizes, px(499.)), Some(1));
        assert_eq!(index_at(&sizes[..1], px(0.)), None);
    }
}
