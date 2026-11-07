//! `ui.video`: the newest frame of a ring in shared memory, as 8-bit pixels.
//! GPUI's renderer shows these 16-bit and colour managed in a Metal view of
//! its own; here a thread reduces each frame for Rune's `Pixels`, which keeps
//! up the picture but not its range.

use {
    moonframe_frames::{Format, FrameInfo, Reader},
    rune::Pixels,
    std::{
        cell::RefCell,
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        time::Duration,
    },
};

/// How often the ring is looked at: a frame at 120 Hz.
const LOOK: Duration = Duration::from_millis(8);
/// How long to wait before opening a ring that isn't there yet.
const WAIT: Duration = Duration::from_millis(250);

/// A ring being shown.
struct Stream {
    pixels: Pixels,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by tests; the thread writes it")
    )]
    shown: Arc<AtomicU64>,
}

#[derive(Default)]
pub(crate) struct Videos {
    streams: RefCell<HashMap<String, Stream>>,
}

/// The high byte of a 16-bit little-endian sample at `at`.
fn high(row: &[u8], at: usize) -> u8 {
    row.get(at + 1).copied().unwrap_or(0)
}

/// A half float at `at` as 0 to 255.
fn half(row: &[u8], at: usize) -> u8 {
    let bits = u16::from_le_bytes([
        row.get(at).copied().unwrap_or(0),
        row.get(at + 1).copied().unwrap_or(0),
    ]);
    let exponent = i32::from((bits >> 10) & 0x1f);
    let fraction = f32::from(bits & 0x3ff) / 1024.;
    let value = match exponent {
        0 => fraction * 2f32.powi(-14),
        31 => 1.,
        _ => (1. + fraction) * 2f32.powi(exponent - 15),
    };
    let value = if bits & 0x8000 == 0 {
        value
    } else {
        0.
    };
    (value.clamp(0., 1.) * 255.).round() as u8
}

/// `bytes`, a frame `info` describes, as straight RGBA.
pub(crate) fn rgba(info: &FrameInfo, bytes: &[u8]) -> Vec<u8> {
    let (width, height) = (info.width as usize, info.height as usize);
    let mut out = Vec::with_capacity(width * height * 4);
    for row in bytes.chunks(info.stride as usize).take(height) {
        for x in 0..width {
            let pixel: [u8; 4] = match info.format {
                Format::Rgba8 => {
                    let at = x * 4;
                    [0, 1, 2, 3].map(|channel| row.get(at + channel).copied().unwrap_or(0))
                },
                Format::Rgba16 => [0, 2, 4, 6].map(|channel| high(row, x * 8 + channel)),
                Format::Rgba16Float => [0, 2, 4, 6].map(|channel| half(row, x * 8 + channel)),
                // A Bayer mosaic shows as its samples, undemosaiced.
                Format::Mono16 | Format::Bayer16 => {
                    let value = high(row, x * 2);
                    [value, value, value, 255]
                },
            };
            out.extend_from_slice(&pixel);
        }
    }
    out
}

fn stream(source: &str, pixels: &Pixels, shown: &AtomicU64) {
    loop {
        let Ok(reader) = Reader::open(source) else {
            std::thread::sleep(WAIT);
            continue;
        };
        let mut last = 0;
        loop {
            let newest = reader.latest();
            if newest != last {
                let frame = reader.read(newest, |info, start, length| {
                    // SAFETY: `read` hands the slot's rows, `length` bytes from
                    // `start`, for this call; its seal is checked after.
                    #[allow(unsafe_code)]
                    let bytes = unsafe { std::slice::from_raw_parts(start, length) };
                    (*info, rgba(info, bytes))
                });
                if let Some((info, picture)) = frame {
                    pixels.set(info.width, info.height, picture);
                    shown.store(info.number, Ordering::Release);
                    last = newest;
                }
            }
            std::thread::sleep(LOOK);
        }
    }
}

impl Videos {
    /// The number of the newest frame of `source` shown so far.
    #[cfg(test)]
    fn shown(&self, source: &str) -> u64 {
        self.streams
            .borrow()
            .get(source)
            .map_or(0, |stream| stream.shown.load(Ordering::Acquire))
    }

    /// The pixels of the ring `source`, read from now on by a thread of its own.
    pub(crate) fn pixels(&self, source: &str) -> Pixels {
        let mut streams = self.streams.borrow_mut();
        if let Some(found) = streams.get(source) {
            return found.pixels.clone();
        }
        let pixels = Pixels::new();
        let shown = Arc::new(AtomicU64::new(0));
        let (name, reading, count) = (source.to_owned(), pixels.clone(), Arc::clone(&shown));
        let started = std::thread::Builder::new()
            .name("rune-video".into())
            .spawn(move || stream(&name, &reading, &count));
        if let Err(problem) = started {
            tracing::warn!("cannot read {source}: {problem}");
        }
        streams.insert(source.to_owned(), Stream {
            pixels: pixels.clone(),
            shown,
        });
        pixels
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, moonframe_frames::Pattern};

    fn info(format: Format, stride: u32) -> FrameInfo {
        FrameInfo {
            number: 1,
            width: 2,
            height: 1,
            format,
            stride,
            time: 0,
            pattern: Pattern::from_raw(0),
        }
    }

    #[test]
    fn sixteen_bits_keep_their_high_byte_and_strides_are_skipped() {
        let row = [
            0, 200, 0, 100, 0, 50, 0, 255, 0, 10, 0, 20, 0, 30, 0, 40, 9, 9,
        ];
        let picture = rgba(&info(Format::Rgba16, 18), &row);
        assert_eq!(picture, [200, 100, 50, 255, 10, 20, 30, 40]);
    }

    #[test]
    fn a_frame_written_to_a_ring_reaches_the_pixels() {
        let name = format!("rune-test-{}", std::process::id());
        let mut ring = moonframe_frames::Writer::create(&name, 2, 2 * 2 * 4).unwrap();
        let videos = Videos::default();
        let pixels = videos.pixels(&name);
        let number = ring
            .write(2, 2, Format::Rgba8, Pattern::from_raw(0), 0, &[9; 16])
            .unwrap();
        for _ in 0..200 {
            if videos.shown(&name) == number {
                assert_eq!(pixels.size(), (2, 2));
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the frame never arrived");
    }

    #[test]
    fn half_floats_read_as_their_values() {
        // 1.0 and 0.5.
        let row = [0, 0x3c, 0, 0x38];
        assert_eq!((half(&row, 0), half(&row, 2)), (255, 128));
    }

    #[test]
    fn mono_is_grey_and_opaque() {
        let picture = rgba(&info(Format::Mono16, 4), &[0, 7, 0, 9]);
        assert_eq!(picture, [7, 7, 7, 255, 9, 9, 9, 255]);
    }
}
