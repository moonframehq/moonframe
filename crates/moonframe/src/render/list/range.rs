//! A `ui.list` with `range`: told which rows show as the user scrolls, so
//! its mod can ask a provider for the rows around them while `count` stays
//! the whole total. Told only of changes, and at most every [`EVERY`]
//! while they come fast, the last one always arriving.

use {
    super::super::{Cx, Painter, Place, pointer::Sender},
    gpui_kit::*,
    moonframe_core::{Action, Data, with_fields},
    std::{
        cell::RefCell,
        collections::{BTreeMap, HashMap},
        time::{Duration, Instant},
    },
};

/// The least time between two messages of one list.
const EVERY: Duration = Duration::from_millis(100);

thread_local! {
    /// What each list told its mod, by its window and place.
    static KEPT: RefCell<HashMap<String, Kept>> = RefCell::new(HashMap::new());
}

/// The rows that show, one-based; `(0, 0)` when none does.
pub(super) type Shown = (usize, usize);

#[derive(Default)]
struct Kept {
    /// The last range told, and when.
    told: Option<(Shown, Instant)>,
    /// A range held back until [`EVERY`] has passed.
    untold: Option<Shown>,
    /// The window frame it was last drawn in.
    frame: u64,
}

/// What to do with a range the list read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tell {
    Now,
    /// Held back: send the latest after this long.
    After(Duration),
    /// Nothing new, or already held back.
    No,
}

/// The rows of `count`, each `row` high, that show in a box `height` high
/// scrolled `scrolled` down, one-based. Half a pixel either way is layout's
/// rounding, not a row showing.
pub(super) fn shown(scrolled: f32, height: f32, row: f32, count: usize) -> Shown {
    if count == 0 || height <= 0.0 || row <= 0.0 {
        return (0, 0);
    }
    let first = ((scrolled.max(0.0) + 0.5) / row).floor() as usize;
    let end = ((scrolled.max(0.0) + height - 0.5) / row).ceil() as usize;
    let last = end.clamp(first + 1, count);
    (first.min(count - 1) + 1, last)
}

/// Whether the list `key`, drawn in window frame `frame` and showing
/// `rows`, tells its mod `now`.
fn tell(key: &str, frame: u64, rows: Shown, now: Instant) -> Tell {
    KEPT.with_borrow_mut(|kept| {
        let entry = kept.entry(key.to_owned()).or_default();
        // Missing from the frame before: drawn anew, so its mod (whose
        // state may have started over) is told again.
        if frame > 0 && entry.frame + 1 < frame {
            *entry = Kept::default();
        }
        entry.frame = frame;
        if entry.untold.is_some() {
            entry.untold = Some(rows);
            return Tell::No;
        }
        match entry.told {
            Some((told, _)) if told == rows => Tell::No,
            Some((_, at)) if now.saturating_duration_since(at) < EVERY => {
                entry.untold = Some(rows);
                Tell::After(EVERY - now.saturating_duration_since(at))
            },
            _ => {
                entry.told = Some((rows, now));
                Tell::Now
            },
        }
    })
}

/// The range held back from the list `key`, if it is not the one told.
fn late(key: &str, now: Instant) -> Option<Shown> {
    KEPT.with_borrow_mut(|kept| {
        let entry = kept.get_mut(key)?;
        let rows = entry.untold.take()?;
        if entry.told.is_some_and(|(told, _)| told == rows) {
            return None;
        }
        entry.told = Some((rows, now));
        Some(rows)
    })
}

/// `msg` with `first` and `last` added.
pub(super) fn message(msg: &Data, (first, last): Shown) -> Data {
    let number = |row: usize| Data::Integer(row as i64);
    with_fields(
        msg,
        BTreeMap::from([
            ("first".to_owned(), number(first)),
            ("last".to_owned(), number(last)),
        ]),
    )
}

/// A box of no size among the list's children that reads `handle` once
/// the list has laid out and scrolled, and tells `range` what shows.
pub(super) fn reader(
    range: &Action,
    handle: &ScrollHandle,
    row: f32,
    count: usize,
    painter: &Painter<'_, '_>,
    place: &Place<'_>,
    cx: Cx,
) -> Option<AnyElement> {
    let Action::Send { to, msg } = range else {
        return None;
    };
    if painter.preview {
        return None;
    }
    let key = format!("{:?}/{}", painter.window, place.path);
    let frame = painter.frame;
    let sender = Sender {
        app: cx.entity().downgrade(),
        owner: place.owner.to_owned(),
        to: to.clone(),
    };
    let (msg, handle) = (msg.clone(), handle.clone());
    let element = canvas(
        move |_, window, cx| {
            let scrolled = -f32::from(handle.offset().y);
            let height = f32::from(handle.bounds().size.height);
            let rows = shown(scrolled, height, row, count);
            match tell(&key, frame, rows, Instant::now()) {
                Tell::No => {},
                Tell::Now => {
                    let msg = message(&msg, rows);
                    window.defer(cx, move |window, cx| sender.send(msg, window, cx));
                },
                Tell::After(wait) => {
                    window
                        .spawn(cx, async move |cx| {
                            cx.background_executor().timer(wait).await;
                            let _ = cx.update(|window, cx| {
                                if let Some(rows) = late(&key, Instant::now()) {
                                    sender.send(message(&msg, rows), window, cx);
                                }
                            });
                        })
                        .detach();
                },
            }
        },
        |_, (), _, _| {},
    )
    .absolute()
    .size_0();
    Some(element.into_any_element())
}

#[cfg(test)]
mod tests {
    use {
        super::{Data, Tell, late, message, shown, tell},
        std::time::{Duration, Instant},
    };

    #[test]
    fn the_rows_any_part_of_which_shows() {
        // Rows 20 high in a box 100 high: five, and a sixth once scrolled.
        assert_eq!(shown(0.0, 100.0, 20.0, 1000), (1, 5));
        assert_eq!(shown(10.0, 100.0, 20.0, 1000), (1, 6));
        assert_eq!(shown(20.0, 100.0, 20.0, 1000), (2, 6));
        // Rounding is not a row showing.
        assert_eq!(shown(19.7, 100.0, 20.0, 1000), (2, 6));
        // At the end, or with fewer rows than room.
        assert_eq!(shown(19_900.0, 100.0, 20.0, 1000), (996, 1000));
        assert_eq!(shown(0.0, 100.0, 20.0, 3), (1, 3));
        assert_eq!(shown(0.0, 100.0, 20.0, 0), (0, 0));
        assert_eq!(shown(0.0, 0.0, 20.0, 10), (0, 0));
    }

    #[test]
    fn changes_are_told_at_most_every_little_while_and_the_last_arrives() {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let key = "told";
        assert_eq!(tell(key, 1, (1, 5), at(0)), Tell::Now);
        assert_eq!(tell(key, 2, (1, 5), at(10)), Tell::No);
        assert_eq!(
            tell(key, 3, (2, 6), at(30)),
            Tell::After(Duration::from_millis(70))
        );
        // Held back already: the latest replaces it.
        assert_eq!(tell(key, 4, (3, 7), at(50)), Tell::No);
        assert_eq!(late(key, at(100)), Some((3, 7)));
        assert_eq!(late(key, at(100)), None);
        assert_eq!(tell(key, 5, (9, 13), at(300)), Tell::Now);
        // Back where it was told before the wait ended: nothing to say.
        assert_eq!(
            tell(key, 6, (10, 14), at(310)),
            Tell::After(Duration::from_millis(90))
        );
        assert_eq!(tell(key, 7, (9, 13), at(320)), Tell::No);
        assert_eq!(late(key, at(400)), None);
    }

    #[test]
    fn a_list_drawn_again_after_a_gap_tells_its_mod_again() {
        let now = Instant::now();
        let key = "again";
        assert_eq!(tell(key, 1, (1, 5), now), Tell::Now);
        assert_eq!(tell(key, 2, (1, 5), now), Tell::No);
        // Not drawn in frame 3.
        assert_eq!(tell(key, 4, (1, 5), now), Tell::Now);
    }

    #[test]
    fn the_message_is_the_mods_with_the_rows_added() {
        let Data::Map(told) = message(&Data::String("rows".into()), (3, 9)) else {
            panic!("a map")
        };
        assert_eq!(told.get("msg"), Some(&Data::String("rows".into())));
        assert_eq!(told.get("first"), Some(&Data::Integer(3)));
        assert_eq!(told.get("last"), Some(&Data::Integer(9)));
    }
}
