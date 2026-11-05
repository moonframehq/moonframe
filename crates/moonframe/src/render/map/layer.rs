//! A map's tiles: those under the box on the screen's pixel grid, a
//! prefetched ring around them drawn clipped so it decodes early, a
//! low-resolution layer under everything, stand-ins under tiles still
//! decoding, and a fade for each new one.

use {
    super::{
        memory, stand_in,
        tiles::{self, Tile},
        view::Frame,
    },
    gpui_kit::*,
    moonframe_core::{Map, MapProvider},
};

/// The most tiles one map draws: a huge box shows a coarser zoom's worth.
const MOST_TILES: usize = 400;
/// How many zooms coarser the layer under the whole map is: a handful of
/// its tiles cover the box, so they arrive long before the rest and leave
/// no blank while panning to somewhere new.
const BASE: u8 = 3;
/// How long a new tile takes to fade in.
const FADE: std::time::Duration = std::time::Duration::from_millis(200);
/// Tiles fetched beyond each edge of the box, so panning finds them ready.
const RING: i64 = 1;

/// The screen's own pixels, which tile edges snap to: the map box can sit
/// between them, and GPUI rounds each tile's place and size apart, so
/// edges rounded in the box's pixels can leave a one-pixel gap between
/// neighbours while panning.
#[derive(Clone, Copy)]
pub(super) struct Grid {
    /// The box's corner in the window, in points.
    origin: (f64, f64),
    /// Device pixels per point.
    scale: f64,
}

impl Grid {
    pub(super) fn new(origin: Point<Pixels>, scale: f32) -> Self {
        Self {
            origin: (f64::from(origin.x), f64::from(origin.y)),
            scale: f64::from(scale.max(1.0)),
        }
    }

    /// `left` (points from the box's corner) on the screen's pixel grid.
    fn x(self, left: f64) -> f64 {
        ((self.origin.0 + left) * self.scale).round() / self.scale - self.origin.0
    }

    fn y(self, top: f64) -> f64 {
        ((self.origin.1 + top) * self.scale).round() / self.scale - self.origin.1
    }

    /// One device pixel, in points: what each tile reaches over its
    /// neighbour, so no rounding can open a gap between them.
    fn bleed(self) -> f64 {
        1.0 / self.scale
    }
}

/// Asks for the tiles under `frame`: those at its middle go first, as what is
/// asked for last is fetched first.
pub(super) fn ask_for(map: &Map, frame: Frame, scale: f32) {
    let z = tiles::level(map.provider, frame.zoom, scale);
    let count = 1i64 << z;
    let size = 256.0 * (frame.zoom - f64::from(z)).exp2();
    let span = frame.span();
    let (middle_x, middle_y) = (frame.center.0 * span, frame.center.1 * span);
    let (left, top) = (middle_x - frame.size.0 / 2.0, middle_y - frame.size.1 / 2.0);
    let columns = (left / size).floor() as i64..=((left + frame.size.0) / size).floor() as i64;
    let rows = (top / size).floor().max(0.0) as i64
        ..=((top + frame.size.1) / size)
            .floor()
            .min(count as f64 - 1.0) as i64;
    let mut under: Vec<(i64, i64)> = rows
        .flat_map(|row| columns.clone().map(move |column| (column, row)))
        .take(MOST_TILES)
        .collect();
    let away = |&(column, row): &(i64, i64)| {
        let across = (column as f64 + 0.5) * size - middle_x;
        let down = (row as f64 + 0.5) * size - middle_y;
        across * across + down * down
    };
    under.sort_by(|a, b| away(b).total_cmp(&away(a)));
    let retina = scale > 1.5 && tiles::has_retina(map.provider);
    for (column, row) in under {
        let tile = Tile {
            provider: map.provider,
            retina,
            z,
            x: column.rem_euclid(count) as u32,
            y: row as u32,
        };
        tiles::prefetch(&tile, map.key.as_deref());
    }
}

/// The tiles under `frame`, and whether any is still on its way. `ahead` is
/// where a zoom going on reaches the next level: its tiles are fetched before
/// the ring, which is no use to a map that is about to change level.
pub(super) fn tile_layer(
    map: &Map,
    frame: Frame,
    ahead: Option<Frame>,
    scale: f32,
    grid: Grid,
) -> (Div, bool) {
    let z = tiles::level(map.provider, frame.zoom, scale);
    let count = 1i64 << z;
    let size = 256.0 * (frame.zoom - f64::from(z)).exp2();
    let span = frame.span();
    let left = frame.center.0 * span - frame.size.0 / 2.0;
    let top = frame.center.1 * span - frame.size.1 / 2.0;
    let columns = (left / size).floor() as i64..=((left + frame.size.0) / size).floor() as i64;
    let rows = (top / size).floor().max(0.0) as i64
        ..=((top + frame.size.1) / size)
            .floor()
            .min(count as f64 - 1.0) as i64;
    let retina = scale > 1.5 && tiles::has_retina(map.provider);
    let key = map.key.as_deref();
    let at = |column: i64, row: i64| Tile {
        provider: map.provider,
        retina,
        z,
        x: column.rem_euclid(count) as u32,
        y: row as u32,
    };
    // The ring first: requests are served newest first, so what shows
    // (asked for below) comes before it.
    let ring_rows = (*rows.start() - RING).max(0)..=(*rows.end() + RING).min(count - 1);
    let ring_columns = *columns.start() - RING..=*columns.end() + RING;
    let with_ring = ring_rows.clone().count() * ring_columns.clone().count() <= MOST_TILES;
    if with_ring {
        for row in ring_rows.clone() {
            for column in ring_columns.clone() {
                if !rows.contains(&row) || !columns.contains(&column) {
                    tiles::prefetch(&at(column, row), key);
                }
            }
        }
    }
    if let Some(ahead) = ahead {
        ask_for(map, ahead, scale);
    }
    ask_for(map, frame, scale);
    let (mut layer, base) = base_layer(map, frame, z, grid);
    let mut missing = false;
    let mut drawn = 0;
    let (all_rows, all_columns) = if with_ring {
        (ring_rows, ring_columns)
    } else {
        (rows.clone(), columns.clone())
    };
    for row in all_rows {
        for column in all_columns.clone() {
            drawn += 1;
            if drawn > MOST_TILES {
                return (layer, missing);
            }
            // Whole pixels, so neighbours meet without a seam.
            // On the screen's pixels, so neighbours share their edge, and a
            // device pixel over it.
            let x0 = grid.x(column as f64 * size - left);
            let y0 = grid.y(row as f64 * size - top);
            let x1 = grid.x((column + 1) as f64 * size - left) + grid.bleed();
            let y1 = grid.y((row + 1) as f64 * size - top) + grid.bleed();
            let tile = at(column, row);
            let cell = div()
                .absolute()
                .overflow_hidden()
                .left(px(x0 as f32))
                .top(px(y0 as f32))
                .w(px((x1 - x0) as f32))
                .h(px((y1 - y0) as f32));
            if !rows.contains(&row) || !columns.contains(&column) {
                // Just outside the box, clipped away: drawn so GPUI has
                // decoded it before a pan brings it in.
                if let Some(file) = tiles::ready(&tile) {
                    // Counted as shown: in view, it needs no fade.
                    stand_in::settled(&file);
                    layer = layer.child(cell.child(memory::picture(file).size_full()));
                }
                continue;
            }
            if let Some(file) = tiles::tile(&tile, key) {
                // Until it has had time to decode, what stood in for it
                // stays under it, so it never blinks blank; the next
                // redraw after that drops it.
                let fresh = !stand_in::settled(&file);
                let under = fresh.then(|| stand_in::stand_in(&tile, x1 - x0)).flatten();
                let picture = memory::picture(file).size_full();
                // A new tile fades in over what stood in for it.
                let picture = if fresh {
                    let id = format!("{}/{}/{}/{}", tile.provider.name(), tile.z, tile.x, tile.y);
                    picture
                        .with_animation(
                            ElementId::Name(id.into()),
                            Animation::new(FADE).with_easing(ease_out_quint()),
                            |picture, delta| picture.opacity(delta),
                        )
                        .into_any_element()
                } else {
                    picture.into_any_element()
                };
                layer = layer.child(cell.children(under).child(picture));
                continue;
            }
            missing = true;
            // Until it arrives: the same spot from another zoom, else the
            // base layer under everything shows through.
            if let Some(stand_in) = stand_in::stand_in(&tile, x1 - x0) {
                layer = layer.child(cell.child(stand_in));
            }
        }
    }
    // Asked for last, so they arrive first: newest requests go first.
    for tile in &base {
        let _ = tiles::tile(tile, key);
    }
    (layer, missing)
}

/// The tiles [`BASE`] zooms coarser than `z` under the whole box, at
/// standard density (it is enlarged anyway), as the layer the map's own
/// tiles are drawn over; and those tiles, to ask for.
fn base_layer(map: &Map, frame: Frame, z: u8, grid: Grid) -> (Div, Vec<Tile>) {
    let layer = div().absolute().size_full();
    let base = z.saturating_sub(BASE);
    if base == z {
        return (layer, Vec::new());
    }
    let count = 1i64 << base;
    let size = 256.0 * (frame.zoom - f64::from(base)).exp2();
    let span = frame.span();
    let left = frame.center.0 * span - frame.size.0 / 2.0;
    let top = frame.center.1 * span - frame.size.1 / 2.0;
    let columns = (left / size).floor() as i64..=((left + frame.size.0) / size).floor() as i64;
    let rows = (top / size).floor().max(0.0) as i64
        ..=((top + frame.size.1) / size)
            .floor()
            .min(count as f64 - 1.0) as i64;
    let mut tiles = Vec::new();
    let mut images = Vec::new();
    for row in rows {
        for column in columns.clone() {
            let tile = Tile {
                provider: map.provider,
                retina: false,
                z: base,
                x: column.rem_euclid(count) as u32,
                y: row as u32,
            };
            if let Some(file) = tiles::ready(&tile) {
                let x0 = grid.x(column as f64 * size - left);
                let y0 = grid.y(row as f64 * size - top);
                let x1 = grid.x((column + 1) as f64 * size - left) + grid.bleed();
                let y1 = grid.y((row + 1) as f64 * size - top) + grid.bleed();
                images.push(
                    memory::picture(file)
                        .absolute()
                        .left(px(x0 as f32))
                        .top(px(y0 as f32))
                        .w(px((x1 - x0) as f32))
                        .h(px((y1 - y0) as f32)),
                );
            }
            tiles.push(tile);
        }
    }
    (layer.children(images), tiles)
}

/// What shows where nothing has arrived yet: a land color close to the
/// provider's own.
pub(super) fn placeholder(provider: MapProvider) -> Hsla {
    let dark = matches!(
        provider,
        MapProvider::CartoDark | MapProvider::StadiaSmoothDark
    );
    if dark {
        rgb(0x262626).into()
    } else {
        rgb(0xf2efe9).into()
    }
}
