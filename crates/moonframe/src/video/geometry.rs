//! Where a frame goes in a `ui.video`, and what a probe reads there.

use {
    moonframe_core::{Data, VideoFilter, VideoGuide, VideoView},
    moonframe_frames::{Format, FrameInfo, Tone},
    std::collections::BTreeMap,
};

/// From this zoom up pixels show square under `auto`.
const SQUARE_FROM: f32 = 2.0;
/// A guide's line, in drawable pixels.
const LINE: f32 = 2.0;

/// A picture placed in a drawable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Placement {
    /// Drawable pixels per picture pixel.
    pub(super) scale: f32,
    /// The picture's top left, in drawable pixels.
    pub(super) offset: [f32; 2],
}

/// `picture` in a drawable of `size`: fitted and centred, or at `zoom`
/// with `center` in the middle.
pub(super) fn place(size: [f32; 2], picture: [f32; 2], view: &VideoView) -> Placement {
    let fitted = (size[0] / picture[0]).min(size[1] / picture[1]);
    let scale = view.zoom.unwrap_or(fitted);
    let center = if view.zoom.is_some() {
        view.center
    } else {
        [0.5, 0.5]
    };
    Placement {
        scale,
        offset: [
            size[0] / 2. - center[0] * picture[0] * scale,
            size[1] / 2. - center[1] * picture[1] * scale,
        ],
    }
}

/// What lies at `box_point`, fractions of a drawable of `size`, in
/// fractions of the picture; off it, outside 0 to 1.
pub(super) fn picture_point(
    box_point: [f32; 2],
    size: [f32; 2],
    picture: [f32; 2],
    placement: Placement,
) -> [f32; 2] {
    let along = |axis: usize| {
        (box_point[axis] * size[axis] - placement.offset[axis]) / placement.scale / picture[axis]
    };
    [along(0), along(1)]
}

/// Whether pixels show square at `scale`.
pub(super) fn nearest(filter: VideoFilter, scale: f32) -> bool {
    match filter {
        VideoFilter::Nearest => true,
        VideoFilter::Linear => false,
        VideoFilter::Auto => scale >= SQUARE_FROM,
    }
}

/// The largest code value of `format`, which unset `white` means.
pub(super) fn full_range(format: Format) -> f32 {
    match format {
        Format::Rgba8 => 255.,
        Format::Rgba16 | Format::Mono16 | Format::Bayer16 => 65535.,
        Format::Rgba16Float => 1.,
    }
}

/// The transform `view` asks for, in `format`'s code values.
pub(super) fn tone(view: &VideoView, format: Format) -> Tone {
    Tone {
        black: view.black,
        white: view.white.unwrap_or_else(|| full_range(format)),
        gain: view.gain,
        gamma: view.gamma,
    }
}

/// A guide's lines as triangles, each vertex `[x, y, r, g, b, a]` with
/// `x` and `y` in clip space.
pub(super) fn guide_triangles(
    guide: &VideoGuide,
    size: [f32; 2],
    picture: [f32; 2],
    placement: Placement,
) -> Vec<[f32; 6]> {
    let at = |x: f32, y: f32| {
        [
            placement.offset[0] + x * picture[0] * placement.scale,
            placement.offset[1] + y * picture[1] * placement.scale,
        ]
    };
    let [left, top] = at(guide.x, guide.y);
    let [right, bottom] = at(guide.x + guide.width, guide.y + guide.height);
    let mut bars = vec![[left, top, right.max(left + LINE), top + LINE]];
    if guide.height != 0. {
        bars.push([left, top, left + LINE, bottom]);
    }
    if guide.width != 0. && guide.height != 0. {
        bars.push([right - LINE, top, right, bottom]);
        bars.push([left, bottom - LINE, right, bottom]);
    }
    if guide.height != 0. && guide.width == 0. {
        bars.remove(0);
    }
    let clip = |x: f32, y: f32| [x / size[0] * 2. - 1., 1. - y / size[1] * 2.];
    let [red, green, blue, alpha] = guide.color;
    bars.into_iter()
        .flat_map(|[x0, y0, x1, y1]| {
            [
                clip(x0, y0),
                clip(x1, y0),
                clip(x0, y1),
                clip(x1, y0),
                clip(x1, y1),
                clip(x0, y1),
            ]
        })
        .map(|[x, y]| [x, y, red, green, blue, alpha])
        .collect()
}

/// A 16-bit float as an `f32`.
pub(super) fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 {
        1.
    } else {
        -1.
    };
    let exponent = i32::from((bits >> 10) & 0x1f);
    let fraction = f32::from(bits & 0x3ff) / 1024.;
    sign * match exponent {
        0 => fraction * 2f32.powi(-14),
        31 if fraction == 0. => f32::INFINITY,
        31 => f32::NAN,
        _ => (1. + fraction) * 2f32.powi(exponent - 15),
    }
}

/// What `pixels` hold at `x`, `y`, a fraction of the picture each: the
/// value there as the source has it, and as the transform shows it.
/// `None` off the picture.
pub(super) fn probe(
    info: &FrameInfo,
    pixels: &[u8],
    [x, y]: [f32; 2],
    view: &VideoView,
) -> Option<BTreeMap<String, Data>> {
    if !(0.0..1.0).contains(&x) || !(0.0..1.0).contains(&y) {
        return None;
    }
    let column = (x * info.width as f32) as u32;
    let row = (y * info.height as f32) as u32;
    let start =
        row as usize * info.stride as usize + column as usize * info.format.bytes_per_pixel();
    let raw = pixels.get(start..start + info.format.bytes_per_pixel())?;
    let word = |at: usize| u16::from_le_bytes([raw[at], raw[at + 1]]);
    let source: Vec<f32> = match info.format {
        Format::Rgba8 => raw[..3].iter().map(|value| f32::from(*value)).collect(),
        Format::Rgba16 => (0..3).map(|at| f32::from(word(at * 2))).collect(),
        Format::Rgba16Float => (0..3).map(|at| half(word(at * 2))).collect(),
        Format::Mono16 | Format::Bayer16 => vec![f32::from(word(0))],
    };
    let tone = tone(view, info.format);
    let list = |values: &[f32]| {
        Data::List(
            values
                .iter()
                .map(|value| Data::Number(f64::from(*value)))
                .collect(),
        )
    };
    let shown: Vec<f32> = source.iter().map(|value| tone.apply(*value)).collect();
    let mut fields = BTreeMap::from([
        ("x".to_owned(), Data::Integer(column.into())),
        ("y".to_owned(), Data::Integer(row.into())),
        ("source".to_owned(), list(&source)),
        ("display".to_owned(), list(&shown)),
    ]);
    if info.format == Format::Bayer16 {
        let channel = ["red", "green", "blue"][info.pattern.channel(column, row)];
        fields.insert("channel".to_owned(), Data::from(channel));
    }
    Some(fields)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, moonframe_core::VideoColor, moonframe_frames::Pattern};

    fn view() -> VideoView {
        VideoView {
            zoom: None,
            center: [0.5, 0.5],
            filter: VideoFilter::Auto,
            color: VideoColor::Srgb,
            extended: false,
            black: 0.,
            white: None,
            gain: 1.,
            gamma: 1.,
            guides: Vec::new(),
            probe: None,
        }
    }

    fn info(format: Format) -> FrameInfo {
        FrameInfo {
            number: 1,
            width: 2,
            height: 2,
            format,
            stride: 2 * format.bytes_per_pixel() as u32,
            time: 0,
            pattern: Pattern::Rggb,
        }
    }

    #[test]
    fn a_picture_fits_its_view_centred() {
        let placed = place([200., 200.], [100., 50.], &view());
        assert_eq!(placed.scale, 2.);
        assert_eq!(placed.offset, [0., 50.]);
    }

    #[test]
    fn a_zoomed_picture_puts_its_center_in_the_middle() {
        let placed = place([100., 100.], [1000., 1000.], &VideoView {
            zoom: Some(1.),
            center: [0.25, 0.75],
            ..view()
        });
        assert_eq!(placed.scale, 1.);
        assert_eq!(placed.offset, [50. - 250., 50. - 750.]);
    }

    #[test]
    fn a_point_in_the_box_finds_its_place_in_the_picture() {
        // Fitted with bars above and below.
        let size = [200., 200.];
        let picture = [100., 50.];
        let placed = place(size, picture, &view());
        assert_eq!(picture_point([0.5, 0.5], size, picture, placed), [0.5, 0.5]);
        assert_eq!(picture_point([0., 0.25], size, picture, placed), [0., 0.]);
        assert!(picture_point([0.5, 0.1], size, picture, placed)[1] < 0.);
    }

    #[test]
    fn pixels_show_square_once_they_are_large() {
        assert!(!nearest(VideoFilter::Auto, 1.));
        assert!(nearest(VideoFilter::Auto, 4.));
        assert!(nearest(VideoFilter::Nearest, 0.5));
        assert!(!nearest(VideoFilter::Linear, 8.));
    }

    #[test]
    fn a_probe_tells_the_source_from_what_shows() {
        let view = VideoView {
            white: Some(4095.),
            gamma: 2.,
            ..view()
        };
        let mut pixels = [0u8; 16];
        pixels[8..10].copy_from_slice(&1024u16.to_le_bytes());
        let read = probe(&info(Format::Mono16), &pixels[..8], [0.6, 0.6], &view).unwrap();
        let Data::List(source) = &read["source"] else {
            panic!("no list");
        };
        assert_eq!(source[0], Data::Number(0.));
        let frame = FrameInfo {
            stride: 4,
            ..info(Format::Mono16)
        };
        let read = probe(&frame, &[0, 0, 0, 0, 0, 0, 0, 4], [0.6, 0.6], &view).unwrap();
        assert_eq!(read["source"], Data::List(vec![Data::Number(1024.)]));
        let Data::List(shown) = &read["display"] else {
            panic!("no list");
        };
        let Data::Number(shown) = shown[0] else {
            panic!("no number");
        };
        assert!((shown - (1024f64 / 4095.).sqrt()).abs() < 1e-3);
    }

    #[test]
    fn a_bayer_probe_says_which_colour_the_site_is() {
        let read = probe(&info(Format::Bayer16), &[0; 8], [0.9, 0.9], &view()).unwrap();
        assert_eq!(read["channel"], Data::from("blue"));
        assert!(probe(&info(Format::Bayer16), &[0; 8], [1.5, 0.], &view()).is_none());
    }

    #[test]
    fn half_floats_are_read() {
        assert_eq!(half(0x3c00), 1.);
        assert_eq!(half(0xc000), -2.);
        assert_eq!(half(0x3800), 0.5);
        assert_eq!(half(0), 0.);
    }

    #[test]
    fn a_rectangle_is_four_bars_and_a_line_one() {
        let placed = place([100., 100.], [100., 100.], &view());
        let guide = VideoGuide {
            x: 0.1,
            y: 0.1,
            width: 0.5,
            height: 0.5,
            color: [1.; 4],
        };
        assert_eq!(
            guide_triangles(&guide, [100., 100.], [100., 100.], placed).len(),
            24
        );
        for line in [
            VideoGuide {
                height: 0.,
                ..guide
            },
            VideoGuide { width: 0., ..guide },
        ] {
            assert_eq!(
                guide_triangles(&line, [100., 100.], [100., 100.], placed).len(),
                6
            );
        }
    }
}
