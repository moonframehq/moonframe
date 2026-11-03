//! `ui.video`: frames from another process, from a ring in shared memory
//! (`moonframe-frames`), shown by the host at the rate they arrive without
//! going through Luau.

use crate::{Data, Extras, Look};

tokens! {
    /// What a `ui.video`'s pixels are, which the system matches to each
    /// screen, on whichever one the window is. `none` turns that off: the
    /// pixels go to the screen as they are, for a source that already
    /// made them for it.
    VideoColor {
        Srgb = "srgb",
        DisplayP3 = "display-p3",
        Rec709 = "rec709",
        Rec2020 = "rec2020",
        LinearSrgb = "linear-srgb",
        LinearP3 = "linear-p3",
        None = "none",
    }
}

tokens! {
    /// How a `ui.video` scales pixels: `auto` shows them square from 200 %
    /// up, to be told apart, and smooth below.
    VideoFilter {
        Auto = "auto",
        Nearest = "nearest",
        Linear = "linear",
    }
}

/// A rectangle or line the host draws over a `ui.video`, in fractions of
/// the picture: a crop frame, guides. A native view lies above the
/// window's own drawing, so nothing else can be drawn over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoGuide {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Red, green, blue and alpha, 0 to 1.
    pub color: [f32; 4],
}

/// What the host needs to draw a `ui.video`: what changes with `view`.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoView {
    /// Screen pixels per picture pixel, 1 being 100 %; nil fits it.
    pub zoom: Option<f32>,
    /// The point of the picture in the middle, in fractions of it.
    pub center: [f32; 2],
    pub filter: VideoFilter,
    pub color: VideoColor,
    /// Lets values above 1 reach a screen that can show them.
    pub extended: bool,
    /// The code value shown as black.
    pub black: f32,
    /// The code value shown as white; the format's full range when unset.
    pub white: Option<f32>,
    pub gain: f32,
    pub gamma: f32,
    pub guides: Vec<VideoGuide>,
    /// A point to read, in fractions of the `ui.video`'s box, as a
    /// `pointer` message gives it: the host knows where the picture lies.
    pub probe: Option<[f32; 2]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Video {
    pub look: Look,
    pub id: String,
    /// The ring's name.
    pub source: String,
    pub view: VideoView,
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// Sent each second, and as the picture's size changes, with `shown`,
    /// `skipped`, `fps`, `latency`, `live`, `width`, `height`, `format`,
    /// `frame`, `fit` (the zoom that fits the box) and `density` (device
    /// pixels per point) added.
    pub stats: Option<Data>,
    /// Sent as what `probe` reads changes, with `x`, `y`, `source`,
    /// `display` and, on a Bayer frame, `channel` added.
    pub probed: Option<Data>,
    pub extras: Extras,
}
