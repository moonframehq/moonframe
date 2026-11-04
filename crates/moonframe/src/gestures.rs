//! Trackpad gestures GPUI does not deliver: two fingers turning, a
//! two-finger double tap and a swipe. GPUI has scrolls, pinches
//! (`on_pinch`) and pressure (`on_mouse_pressure`) of its own, but its
//! input enum is closed, so these come round it: a local event monitor
//! (macOS only, see `macos`) hands each gesture to [`deliver`], which finds
//! what is under it among the [`Gestures`] areas the last frame painted.
//!
//! An area is a child of the element it listens for, filling it:
//!
//! ```ignore
//! div().relative().child(gestures().on_rotate(|event, window, cx| {
//!     turn(event.degrees, window, cx);
//! }))
//! ```
//!
//! It takes nothing from what is around it: clicks, scrolls and pinches go
//! on as before, and an occluding element above it hides it. Nested areas
//! wanting the same gesture go by size: the smallest over the place has it.
//! Elsewhere than macOS an area simply hears nothing.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(test, target_os = "macos"))]
mod swipe;

pub use moonframe_core::Swipe as SwipeDirection;
use {
    gpui_kit::*,
    std::{cell::RefCell, collections::HashMap, rc::Rc},
};

/// Two fingers turning over an area.
#[derive(Clone, Debug)]
pub struct RotateEvent {
    /// Where the pointer is, in the window.
    pub position: Point<Pixels>,
    /// Degrees turned since the last event, clockwise.
    pub degrees: f32,
    pub modifiers: Modifiers,
    pub phase: TouchPhase,
}

/// A two-finger double tap over an area, with Smart zoom turned on in the
/// system's trackpad settings.
#[derive(Clone, Debug)]
pub struct SmartZoomEvent {
    /// Where the pointer is, in the window.
    pub position: Point<Pixels>,
    pub modifiers: Modifiers,
}

/// A finished swipe over an area: three fingers where the system's
/// trackpad settings leave those to apps, or two across, as a scroll that
/// stays clearly sideways. Being a scroll too, that one also scrolls.
#[derive(Clone, Debug)]
pub struct SwipeEvent {
    /// Where the pointer is, in the window.
    pub position: Point<Pixels>,
    pub direction: SwipeDirection,
    pub modifiers: Modifiers,
}

type Listener<Event> = Rc<dyn Fn(&Event, &mut Window, &mut App)>;

/// An area hearing gestures, placed as a child of what it listens for.
#[derive(Clone, Default)]
pub struct Gestures {
    rotate: Option<Listener<RotateEvent>>,
    smart_zoom: Option<Listener<SmartZoomEvent>>,
    swipe: Option<Listener<SwipeEvent>>,
}

/// An area hearing no gesture yet.
pub fn gestures() -> Gestures {
    Gestures::default()
}

impl Gestures {
    /// Runs `listener` as two fingers turn over the area.
    #[must_use]
    pub fn on_rotate(
        mut self,
        listener: impl Fn(&RotateEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.rotate = Some(Rc::new(listener));
        self
    }

    /// Runs `listener` on a two-finger double tap over the area.
    #[must_use]
    pub fn on_smart_zoom(
        mut self,
        listener: impl Fn(&SmartZoomEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.smart_zoom = Some(Rc::new(listener));
        self
    }

    /// Runs `listener` when a swipe over the area ends.
    #[must_use]
    pub fn on_swipe(
        mut self,
        listener: impl Fn(&SwipeEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.swipe = Some(Rc::new(listener));
        self
    }
}

impl IntoElement for Gestures {
    type Element = Canvas<Hitbox>;

    fn into_element(self) -> Self::Element {
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, cx| {
                #[cfg(target_os = "macos")]
                macos::install(cx);
                #[cfg(not(target_os = "macos"))]
                let _ = cx;
                let window = window.window_handle().window_id();
                AREAS.with_borrow_mut(|areas| {
                    areas.entry(window).or_default().push(Area {
                        hitbox,
                        listeners: self,
                    });
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}

/// An area as the last frame painted it.
struct Area {
    hitbox: Hitbox,
    listeners: Gestures,
}

thread_local! {
    /// What each window's last frame painted.
    static AREAS: RefCell<HashMap<WindowId, Vec<Area>>> = RefCell::new(HashMap::new());
}

/// A new frame of `window` is being drawn: its areas paint themselves
/// again, and those it no longer has are gone.
pub(crate) fn begin_frame(window: WindowId) {
    AREAS.with_borrow_mut(|areas| areas.remove(&window));
}

/// A gesture as the platform reports it.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Clone, Copy, Debug)]
enum Gesture {
    Rotate { degrees: f32, phase: TouchPhase },
    SmartZoom,
    Swipe(SwipeDirection),
}

/// Hands `gesture`, at `position` in `window`, to the smallest area over
/// that place listening for it, if any is.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn deliver(
    gesture: Gesture,
    position: Point<Pixels>,
    modifiers: Modifiers,
    window: &mut Window,
    cx: &mut App,
) {
    let wants = |area: &Area| match gesture {
        Gesture::Rotate { .. } => area.listeners.rotate.is_some(),
        Gesture::SmartZoom => area.listeners.smart_zoom.is_some(),
        Gesture::Swipe(_) => area.listeners.swipe.is_some(),
    };
    // Out of the table first: a listener may draw, and so paint areas.
    let listeners = AREAS.with_borrow(|areas| {
        let areas = areas.get(&window.window_handle().window_id())?;
        let hit = areas
            .iter()
            .filter(|area| wants(area) && area.hitbox.is_hovered_at(position, window))
            .map(|area| (area.hitbox.bounds.size, &area.listeners));
        smallest(hit).cloned()
    });
    let Some(listeners) = listeners else {
        return;
    };
    match gesture {
        Gesture::Rotate { degrees, phase } => {
            if let Some(listener) = listeners.rotate {
                let event = RotateEvent {
                    position,
                    degrees,
                    modifiers,
                    phase,
                };
                listener(&event, window, cx);
            }
        },
        Gesture::SmartZoom => {
            if let Some(listener) = listeners.smart_zoom {
                listener(
                    &SmartZoomEvent {
                        position,
                        modifiers,
                    },
                    window,
                    cx,
                );
            }
        },
        Gesture::Swipe(direction) => {
            if let Some(listener) = listeners.swipe {
                let event = SwipeEvent {
                    position,
                    direction,
                    modifiers,
                };
                listener(&event, window, cx);
            }
        },
    }
}

/// What has the least area; the last of equals, drawn over the others.
fn smallest<T>(sized: impl Iterator<Item = (Size<Pixels>, T)>) -> Option<T> {
    let area = |size: &Size<Pixels>| f32::from(size.width) * f32::from(size.height);
    sized
        .fold(None, |best: Option<(Size<Pixels>, T)>, next| match best {
            Some(best) if area(&best.0) < area(&next.0) => Some(best),
            _ => Some(next),
        })
        .map(|(_, found)| found)
}

#[cfg(test)]
mod tests {
    use {
        super::smallest,
        gpui_kit::{Pixels, Size, px, size},
    };

    #[test]
    fn the_smallest_area_has_a_gesture_and_the_last_of_equals() {
        let box_of = |side: f32| size(px(side), px(side));
        let areas = [(box_of(100.), "outer"), (box_of(10.), "inner")];
        assert_eq!(smallest(areas.into_iter()), Some("inner"));
        let reversed = [(box_of(10.), "inner"), (box_of(100.), "outer")];
        assert_eq!(smallest(reversed.into_iter()), Some("inner"));
        let equal = [(box_of(10.), "under"), (box_of(10.), "over")];
        assert_eq!(smallest(equal.into_iter()), Some("over"));
        assert_eq!(smallest(std::iter::empty::<(Size<Pixels>, ())>()), None);
    }
}
