//! What shows in a tile's place while its picture is not drawn yet: the
//! same spot from a coarser zoom, enlarged, or from the next finer zoom,
//! shrunk. A tile whose file is ready still takes GPUI a moment to decode,
//! and draws nothing meanwhile, so a stand-in stays under each new tile
//! until it has had time to show: what was there is never replaced by a
//! blank.

use {
    super::tiles::{self, Tile},
    gpui_kit::*,
    std::{
        cell::RefCell,
        collections::HashMap,
        path::{Path, PathBuf},
        time::{Duration, Instant},
    },
};

/// Coarser zooms looked at for a stand-in.
const COARSER: u8 = 4;
/// How long a tile keeps a stand-in under it after it first shows: longer
/// than decoding a tile takes.
const SETTLE: Duration = Duration::from_millis(600);
/// Tiles remembered as shown; past this the memory starts over.
const MOST_REMEMBERED: usize = 4096;

thread_local! {
    /// When each tile's file first showed.
    static SHOWN: RefCell<HashMap<PathBuf, Instant>> = RefCell::new(HashMap::new());
}

/// Whether `file` showed long enough ago to be drawn by now.
pub(super) fn settled(file: &Path) -> bool {
    SHOWN.with_borrow_mut(|shown| {
        if shown.len() > MOST_REMEMBERED {
            shown.clear();
        }
        let first = *shown.entry(file.to_owned()).or_insert_with(Instant::now);
        first.elapsed() >= SETTLE
    })
}

/// The best stand-in at hand for `tile`, shown `size` pixels across.
pub(super) fn stand_in(tile: &Tile, size: f64) -> Option<AnyElement> {
    coarser(tile, size).or_else(|| finer(tile, size))
}

/// A coarser tile at hand, enlarged and cropped to `tile`'s place: what
/// zooming in shows until the finer tiles arrive.
fn coarser(tile: &Tile, size: f64) -> Option<AnyElement> {
    (1..=COARSER.min(tile.z)).find_map(|steps| {
        let parent = Tile {
            z: tile.z - steps,
            x: tile.x >> steps,
            y: tile.y >> steps,
            ..tile.clone()
        };
        let file = tiles::ready(&parent)?;
        let across = f64::from(1u32 << steps);
        let offset_x = f64::from(tile.x - (parent.x << steps)) * size;
        let offset_y = f64::from(tile.y - (parent.y << steps)) * size;
        Some(
            img(file)
                .absolute()
                .left(px(-offset_x as f32))
                .top(px(-offset_y as f32))
                .w(px((size * across) as f32))
                .h(px((size * across) as f32))
                .into_any_element(),
        )
    })
}

/// The four tiles of the next finer zoom, shrunk into `tile`'s place:
/// what zooming out shows until the coarser tile arrives. Only when all
/// four are at hand, so no quarter is left blank.
fn finer(tile: &Tile, size: f64) -> Option<AnyElement> {
    let half = size / 2.0;
    let mut quarters = Vec::with_capacity(4);
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let child = Tile {
            z: tile.z.checked_add(1)?,
            x: tile.x * 2 + dx,
            y: tile.y * 2 + dy,
            ..tile.clone()
        };
        let file = tiles::ready(&child)?;
        quarters.push(
            img(file)
                .absolute()
                .left(px((f64::from(dx) * half) as f32))
                .top(px((f64::from(dy) * half) as f32))
                // A pixel over, so the quarters meet without a seam.
                .w(px(half.ceil() as f32 + 1.0))
                .h(px(half.ceil() as f32 + 1.0))
                .into_any_element(),
        );
    }
    Some(
        div()
            .absolute()
            .size_full()
            .children(quarters)
            .into_any_element(),
    )
}
