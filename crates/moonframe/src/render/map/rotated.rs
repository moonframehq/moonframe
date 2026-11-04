//! A turned map's tiles. GPUI draws pictures only upright: its image
//! sprites carry no transform, in the scene or in the shaders, and only
//! its one-color glyphs do. So a turned map samples its decoded tiles
//! itself into one picture of the box, at the window's density, and paints
//! that. The sampling is bilinear, split across a few threads, and takes a
//! few milliseconds for a full window; drawn again only when the map or
//! its tiles change, as pins may animate over it. North up, the map goes
//! back to its tiles as sprites ([`super::layer`]), which cost nothing.

use {
    super::{
        memory::{self, Decoded},
        tiles::{self, Tile},
        view::Frame,
    },
    gpui_kit::*,
    moonframe_core::Map,
    std::{
        cell::RefCell,
        collections::{HashMap, VecDeque},
        path::PathBuf,
        sync::Arc,
    },
};

/// The most tiles one map samples: a huge box goes a zoom coarser.
const MOST_TILES: usize = 400;
/// How many zooms coarser a stand-in for a missing tile may be.
const STAND_IN: u8 = 3;
/// Pictures kept after they stop showing: GPUI may still be drawing the
/// frames that used them, and a new one could land on their atlas space.
const OLD_PICTURES: usize = 2;
/// The most threads one picture is sampled on.
const MOST_THREADS: usize = 8;
/// Pictures with fewer pixels than this are not worth a thread.
const ONE_THREAD: usize = 1 << 16;

/// Which tiles cover the box.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cover {
    /// The zoom of the tiles.
    z: u8,
    /// The first column and row, which may lie outside the world.
    column: i64,
    row: i64,
    columns: usize,
    rows: usize,
}

/// One spot of the grid and what can be shown there, best first: its own
/// tile, then stand-ins, each with how many zooms coarser it is.
struct Spot {
    choices: Vec<(PathBuf, Choice)>,
}

/// Which part of a tile a spot takes: all of it, or a part of a coarser one.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Choice {
    /// How many zooms coarser the tile is than the grid.
    coarser: u8,
    /// Which part across and down, of `1 << coarser` each way.
    part: (u32, u32),
}

/// The tiles under a turned `frame`, as a layer, and whether any is still
/// on its way.
pub(super) fn layer(map: &Map, frame: Frame, scale: f32, key: &str) -> (Div, bool) {
    let source = map.provider.source();
    let round = frame.zoom.round().clamp(0.0, f64::from(source.max_zoom)) as u8;
    let cover = cover(frame, round);
    let count = 1i64 << cover.z;
    let retina = scale > 1.5 && tiles::has_retina(map.provider);
    let api_key = map.key.as_deref();
    let mut missing = false;
    let mut spots = Vec::with_capacity(cover.columns * cover.rows);
    for row in cover.row..cover.row + cover.rows as i64 {
        for column in cover.column..cover.column + cover.columns as i64 {
            if !(0..count).contains(&row) {
                spots.push(Spot {
                    choices: Vec::new(),
                });
                continue;
            }
            let x = column.rem_euclid(count) as u32;
            let tile = Tile {
                provider: map.provider,
                retina,
                z: cover.z,
                x,
                y: row as u32,
            };
            let mut choices = Vec::new();
            match tiles::tile(&tile, api_key) {
                Some(file) => choices.push((file, Choice {
                    coarser: 0,
                    part: (0, 0),
                })),
                None => missing = true,
            }
            for coarser in 1..=STAND_IN.min(cover.z) {
                let parent = Tile {
                    z: cover.z - coarser,
                    x: x >> coarser,
                    y: row as u32 >> coarser,
                    ..tile.clone()
                };
                let mask = (1u32 << coarser) - 1;
                let choice = Choice {
                    coarser,
                    part: (x & mask, row as u32 & mask),
                };
                let standard = Tile {
                    retina: false,
                    ..parent.clone()
                };
                if let Some(file) = tiles::ready(&parent).or_else(|| tiles::ready(&standard)) {
                    choices.push((file, choice));
                }
            }
            spots.push(Spot { choices });
        }
    }
    let key = key.to_owned();
    let picture = canvas(
        |_, _, _| (),
        move |bounds, (), window, cx| paint(&key, frame, cover, &spots, bounds, window, cx),
    )
    .absolute()
    .size_full();
    (div().absolute().size_full().child(picture), missing)
}

/// The tiles at `z` or coarser that cover the box, at most [`MOST_TILES`].
fn cover(frame: Frame, z: u8) -> Cover {
    let (low, high) = frame.world_box();
    let mut z = z;
    loop {
        let count = (1i64 << z) as f64;
        let column = (low.0 * count).floor() as i64;
        let row = (low.1 * count).floor() as i64;
        let columns = ((high.0 * count).floor() as i64 - column + 1) as usize;
        let rows = ((high.1 * count).floor() as i64 - row + 1) as usize;
        if columns * rows <= MOST_TILES || z == 0 {
            return Cover {
                z,
                column,
                row,
                columns,
                rows,
            };
        }
        z -= 1;
    }
}

/// What a picture shows: the same again needs no sampling.
#[derive(PartialEq)]
struct Shown {
    frame: Frame,
    cover: Cover,
    size: (u32, u32),
    tiles: Vec<Option<(ImageId, Choice)>>,
}

/// One map's picture, and those that may still be drawing.
struct Painted {
    window: WindowId,
    shown: Shown,
    picture: Arc<RenderImage>,
    old: VecDeque<Arc<RenderImage>>,
}

thread_local! {
    static PAINTED: RefCell<HashMap<String, Painted>> = RefCell::new(HashMap::new());
}

/// Lets go of the pictures of a closed window.
pub(super) fn forget(window: WindowId) {
    PAINTED.with_borrow_mut(|painted| painted.retain(|_, kept| kept.window != window));
}

/// Lets go of the map `key`'s picture, back to north.
pub(super) fn release(key: &str, window: &mut Window, cx: &mut App) {
    let Some(kept) = PAINTED.with_borrow_mut(|painted| painted.remove(key)) else {
        return;
    };
    for picture in kept.old.into_iter().chain([kept.picture]) {
        cx.drop_image(picture, Some(window));
    }
}

fn paint(
    key: &str,
    frame: Frame,
    cover: Cover,
    spots: &[Spot],
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let scale = window.scale_factor();
    let width = (f32::from(bounds.size.width) * scale).ceil() as u32;
    let height = (f32::from(bounds.size.height) * scale).ceil() as u32;
    if width == 0 || height == 0 {
        return;
    }
    let mut waiting = false;
    let mut images = Vec::with_capacity(spots.len());
    for spot in spots {
        let mut found = None;
        for (file, choice) in &spot.choices {
            match memory::decoded(file, window, cx) {
                Some(Decoded::Ready(image)) => {
                    found = Some((image, *choice));
                    break;
                },
                Some(Decoded::Waiting) => waiting = true,
                None => {},
            }
        }
        images.push(found);
    }
    // A decoded tile tells nobody: look again next frame.
    if waiting {
        window.request_animation_frame();
    }
    let shown = Shown {
        frame,
        cover,
        size: (width, height),
        tiles: images
            .iter()
            .map(|found| found.as_ref().map(|(image, choice)| (image.id, *choice)))
            .collect(),
    };
    let unchanged =
        PAINTED.with_borrow(|painted| painted.get(key).is_some_and(|kept| kept.shown == shown));
    let mut gone = Vec::new();
    if !unchanged {
        let grid = Grid {
            cover,
            cells: images
                .iter()
                .map(|found| {
                    found
                        .as_ref()
                        .and_then(|(image, choice)| Cell::of(image, *choice))
                })
                .collect(),
        };
        if let Some(picture) = picture(&grid, frame, (width, height)) {
            let id = window.window_handle().window_id();
            PAINTED.with_borrow_mut(|painted| match painted.get_mut(key) {
                Some(kept) => {
                    let old = std::mem::replace(&mut kept.picture, picture);
                    kept.old.push_back(old);
                    kept.shown = shown;
                    while kept.old.len() > OLD_PICTURES {
                        gone.extend(kept.old.pop_front());
                    }
                },
                None => {
                    painted.insert(key.to_owned(), Painted {
                        window: id,
                        shown,
                        picture,
                        old: VecDeque::new(),
                    });
                },
            });
        }
    } else {
        // Nothing new: what no frame uses any more can go, one at a time.
        PAINTED.with_borrow_mut(|painted| {
            if let Some(kept) = painted.get_mut(key) {
                gone.extend(kept.old.pop_front());
            }
        });
    }
    for picture in gone {
        cx.drop_image(picture, Some(window));
    }
    let picture = PAINTED.with_borrow(|painted| painted.get(key).map(|kept| kept.picture.clone()));
    if let Some(picture) = picture {
        let _ = window.paint_image(bounds, bounds, Corners::default(), picture, 0, false);
    }
}

/// A tile's pixels, as sampled: which texels of it fill one spot.
struct Cell<'a> {
    /// BGRA, as GPUI decodes it.
    bytes: &'a [u8],
    width: u32,
    /// The corner of the part, in texels.
    left: u32,
    top: u32,
    /// Texels across one spot: a coarser tile's part is smaller.
    across: u32,
    down: u32,
}

impl<'a> Cell<'a> {
    fn of(image: &'a RenderImage, choice: Choice) -> Option<Self> {
        let size = image.size(0);
        let (width, height) = (
            u32::try_from(size.width.0).ok()?,
            u32::try_from(size.height.0).ok()?,
        );
        let (across, down) = (width >> choice.coarser, height >> choice.coarser);
        if across == 0 || down == 0 {
            return None;
        }
        Some(Self {
            bytes: image.as_bytes(0)?,
            width,
            left: choice.part.0 * across,
            top: choice.part.1 * down,
            across,
            down,
        })
    }

    /// The color `x` and `y` across the spot, each 0 to 1 in 32-bit
    /// fractions: between its four nearest texels, clamped at its edges,
    /// where the neighbouring tile's texel is all but the same.
    #[inline]
    fn sample(&self, x: u32, y: u32) -> u32 {
        let (left, fx) = texel(x, self.across);
        let (top, fy) = texel(y, self.down);
        let right = (left + 1).min(self.across - 1);
        let bottom = (top + 1).min(self.down - 1);
        let at = |column: u32, row: u32| {
            let index = ((self.top + row) * self.width + self.left + column) as usize * 4;
            self.bytes
                .get(index..index + 4)
                .and_then(|bytes| bytes.try_into().ok())
                .map_or(0, u32::from_le_bytes)
        };
        let above = mix(at(left, top), at(right, top), fx);
        let below = mix(at(left, bottom), at(right, bottom), fx);
        mix(above, below, fy)
    }
}

/// The texel at or before `fraction` of `size` texels, measured between
/// texel centers, and how far past it `fraction` lies, 0 to 256.
#[inline]
fn texel(fraction: u32, size: u32) -> (u32, u32) {
    // 24.8 fixed point, half a texel back to the first center.
    let at = ((u64::from(fraction) * u64::from(size)) >> 24) as i64 - 128;
    if at < 0 {
        (0, 0)
    } else {
        ((at >> 8) as u32, (at & 255) as u32)
    }
}

/// `a` to `b`, `amount` of the way out of 256, all four channels at once.
#[inline]
fn mix(a: u32, b: u32, amount: u32) -> u32 {
    let keep = 256 - amount;
    let even = (a & 0x00ff_00ff) * keep + (b & 0x00ff_00ff) * amount;
    let odd = ((a >> 8) & 0x00ff_00ff) * keep + ((b >> 8) & 0x00ff_00ff) * amount;
    ((even >> 8) & 0x00ff_00ff) | (odd & 0xff00_ff00)
}

/// The tiles covering a box, row by row.
struct Grid<'a> {
    cover: Cover,
    cells: Vec<Option<Cell<'a>>>,
}

/// Where sampling starts and how it steps, in spots of the grid, as 32.32
/// fixed point: enough that a whole row adds up no visible error.
#[derive(Clone, Copy, Debug)]
struct Walk {
    start: (i64, i64),
    /// One pixel right, and one pixel down.
    across: (i64, i64),
    down: (i64, i64),
}

impl Walk {
    /// How `frame` crosses `cover` in a picture of `size` pixels.
    fn of(frame: Frame, cover: Cover, size: (u32, u32)) -> Self {
        let count = (1i64 << cover.z) as f64;
        let fixed = |value: f64| (value * 4_294_967_296.0) as i64;
        let density = (
            frame.size.0 / f64::from(size.0),
            frame.size.1 / f64::from(size.1),
        );
        let spot = |(x, y): (f64, f64)| {
            (
                fixed(x * count - cover.column as f64),
                fixed(y * count - cover.row as f64),
            )
        };
        // Pixel centers, each a step of the box from the corner's.
        let start = spot(frame.world_at(density.0 / 2.0, density.1 / 2.0));
        let step = |left: f64, top: f64| {
            let (x, y) = frame.unturn((left, top));
            let per_spot = count / frame.span();
            (fixed(x * per_spot), fixed(y * per_spot))
        };
        Self {
            start,
            across: step(density.0, 0.0),
            down: step(0.0, density.1),
        }
    }
}

/// Fills `rows`, starting at picture row `first`, `width` pixels each.
fn sample(grid: &Grid<'_>, walk: Walk, rows: &mut [u8], first: u32, width: u32) {
    let (columns, tall) = (grid.cover.columns as i64, grid.cover.rows as i64);
    for (offset, row) in rows.chunks_exact_mut(width as usize * 4).enumerate() {
        let index = i64::from(first) + offset as i64;
        let mut x = walk.start.0 + walk.down.0 * index;
        let mut y = walk.start.1 + walk.down.1 * index;
        for pixel in row.as_chunks_mut::<4>().0 {
            let (column, line) = (x >> 32, y >> 32);
            if (0..columns).contains(&column)
                && (0..tall).contains(&line)
                && let Some(cell) = &grid.cells[(line * columns + column) as usize]
            {
                *pixel = cell.sample(x as u32, y as u32).to_le_bytes();
            }
            x += walk.across.0;
            y += walk.across.1;
        }
    }
}

/// The box's picture, in pixels of `size`; where no tile shows it is clear,
/// over the map's placeholder.
fn pixels(grid: &Grid<'_>, frame: Frame, size: (u32, u32)) -> Vec<u8> {
    let (width, height) = size;
    let walk = Walk::of(frame, grid.cover, size);
    let mut bytes = vec![0u8; width as usize * height as usize * 4];
    let pixels = width as usize * height as usize;
    let threads = if pixels < ONE_THREAD {
        1
    } else {
        std::thread::available_parallelism().map_or(1, |count| count.get().min(MOST_THREADS))
    };
    let per_thread = (height as usize).div_ceil(threads);
    std::thread::scope(|scope| {
        for (index, rows) in bytes
            .chunks_mut(per_thread * width as usize * 4)
            .enumerate()
        {
            let first = (index * per_thread) as u32;
            scope.spawn(move || sample(grid, walk, rows, first, width));
        }
    });
    bytes
}

fn picture(grid: &Grid<'_>, frame: Frame, size: (u32, u32)) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(size.0, size.1, pixels(grid, frame, size))?;
    Some(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}

#[cfg(test)]
mod tests {
    use {
        super::{Cell, Cover, Grid, Walk, cover, mix, pixels, texel},
        crate::render::map::view::Frame,
    };

    const WHOLE: u32 = u32::MAX;

    /// A tile of one color, in BGRA.
    fn plain(color: [u8; 4], side: u32) -> Vec<u8> {
        color.repeat((side * side) as usize)
    }

    fn cell(bytes: &[u8], side: u32) -> Cell<'_> {
        Cell {
            bytes,
            width: side,
            left: 0,
            top: 0,
            across: side,
            down: side,
        }
    }

    fn pixel(bytes: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * width + x) * 4) as usize;
        [
            bytes[index],
            bytes[index + 1],
            bytes[index + 2],
            bytes[index + 3],
        ]
    }

    const RED: [u8; 4] = [0, 0, 255, 255];
    const BLUE: [u8; 4] = [255, 0, 0, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];

    #[test]
    fn mixing_goes_from_one_color_to_the_other() {
        let (a, b) = (0x10_20_30_40, 0x90_a0_b0_c0);
        assert_eq!(mix(a, b, 0), a);
        assert_eq!(mix(a, b, 128), 0x50_60_70_80);
    }

    #[test]
    fn a_texel_center_is_exact_and_edges_clamp() {
        // Texel 1 of 4 is centered at 3/8.
        assert_eq!(texel(3 << 29, 4), (1, 0));
        assert_eq!(texel(0, 4), (0, 0));
        assert_eq!(texel(WHOLE, 4).0, 3);
    }

    #[test]
    fn a_coarser_tile_gives_the_part_under_the_spot() {
        // A 2x2 tile, one color a texel: the bottom-right quarter is white.
        let mut bytes = Vec::new();
        for color in [RED, BLUE, GREEN, WHITE] {
            bytes.extend_from_slice(&color);
        }
        let quarter = Cell {
            bytes: &bytes,
            width: 2,
            left: 1,
            top: 1,
            across: 1,
            down: 1,
        };
        assert_eq!(quarter.sample(1 << 31, 1 << 31).to_le_bytes(), WHITE);
    }

    /// Four one-color tiles meeting at the middle of a 512-pixel box at
    /// zoom 1: the whole world, red and blue over green and white.
    fn world(bearing: f64) -> (Frame, Cover) {
        let frame = Frame {
            center: (0.5, 0.5),
            zoom: 1.0,
            size: (512.0, 512.0),
            bearing,
        };
        let cover = Cover {
            z: 1,
            column: 0,
            row: 0,
            columns: 2,
            rows: 2,
        };
        (frame, cover)
    }

    fn drawn(bearing: f64) -> Vec<u8> {
        let (frame, cover) = world(bearing);
        let tiles: Vec<Vec<u8>> = [RED, BLUE, GREEN, WHITE]
            .iter()
            .map(|color| plain(*color, 8))
            .collect();
        let grid = Grid {
            cover,
            cells: tiles.iter().map(|tile| Some(cell(tile, 8))).collect(),
        };
        pixels(&grid, frame, (512, 512))
    }

    #[test]
    fn north_up_the_picture_matches_the_tiles() {
        let bytes = drawn(0.0);
        assert_eq!(pixel(&bytes, 512, 100, 100), RED);
        assert_eq!(pixel(&bytes, 512, 400, 100), BLUE);
        assert_eq!(pixel(&bytes, 512, 100, 400), GREEN);
        assert_eq!(pixel(&bytes, 512, 400, 400), WHITE);
    }

    #[test]
    fn east_up_turns_the_tiles_a_quarter_to_the_left() {
        let bytes = drawn(90.0);
        // What lay east comes to the top, what lay north to the left.
        assert_eq!(pixel(&bytes, 512, 100, 100), BLUE);
        assert_eq!(pixel(&bytes, 512, 400, 100), WHITE);
        assert_eq!(pixel(&bytes, 512, 100, 400), RED);
        assert_eq!(pixel(&bytes, 512, 400, 400), GREEN);
    }

    #[test]
    fn the_picture_agrees_with_where_a_frame_puts_places() {
        for bearing in [17.0, 123.0, 301.5] {
            let (frame, _) = world(bearing);
            let bytes = drawn(bearing);
            // Well inside the red tile's north-west of the world.
            let (left, top) = frame.at(50.0, -90.0);
            assert_eq!(
                pixel(&bytes, 512, left as u32, top as u32),
                RED,
                "{bearing}"
            );
            let (left, top) = frame.at(-50.0, 90.0);
            assert_eq!(
                pixel(&bytes, 512, left as u32, top as u32),
                WHITE,
                "{bearing}"
            );
        }
    }

    #[test]
    fn a_denser_picture_shows_the_same_map() {
        let (frame, cover) = world(90.0);
        let tile = plain(RED, 8);
        let grid = Grid {
            cover,
            cells: vec![Some(cell(&tile, 8)), None, None, None],
        };
        let bytes = pixels(&grid, frame, (1024, 1024));
        // Red is the north-west: bottom left once east is up.
        assert_eq!(pixel(&bytes, 1024, 200, 800), RED);
        // Where no tile shows the picture is clear.
        assert_eq!(pixel(&bytes, 1024, 800, 800), [0, 0, 0, 0]);
    }

    #[test]
    fn every_corner_of_a_turned_box_falls_on_the_cover() {
        let frame = Frame {
            center: (0.31, 0.47),
            zoom: 12.3,
            size: (1200.0, 800.0),
            bearing: 33.0,
        };
        let cover = cover(frame, 12);
        assert!(cover.columns * cover.rows <= super::MOST_TILES);
        let size = (1200, 800);
        let walk = Walk::of(frame, cover, size);
        for (x, y) in [(0i64, 0i64), (1199, 0), (0, 799), (1199, 799)] {
            let column = (walk.start.0 + walk.across.0 * x + walk.down.0 * y) >> 32;
            let row = (walk.start.1 + walk.across.1 * x + walk.down.1 * y) >> 32;
            assert!((0..cover.columns as i64).contains(&column), "{column}");
            assert!((0..cover.rows as i64).contains(&row), "{row}");
        }
    }

    #[test]
    fn a_huge_box_goes_a_zoom_coarser_rather_than_past_the_limit() {
        let frame = Frame {
            center: (0.5, 0.5),
            zoom: 14.0,
            size: (9000.0, 6000.0),
            bearing: 45.0,
        };
        let cover = cover(frame, 14);
        assert!(cover.z < 14 && cover.columns * cover.rows <= super::MOST_TILES);
    }
}
