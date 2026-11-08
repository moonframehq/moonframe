//! A `ui.video`'s thread: takes the newest frame from its ring and hands
//! it to the screen, on its own, so no frame passes through the window's
//! drawing or Luau. Frames it was too slow for are skipped, which costs
//! the engine nothing.

use {
    super::{geometry, render::Renderer},
    futures::channel::mpsc::UnboundedSender,
    moonframe_core::{Data, Video, VideoView, with_fields},
    moonframe_frames::{FrameInfo, Reader},
    std::{
        collections::BTreeMap,
        sync::{
            Arc, Mutex, PoisonError,
            atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        },
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    },
};

/// How often it looks for a new frame: under 1/4 of a frame at 50 i/s.
const POLL: Duration = Duration::from_millis(2);
const PAUSED_POLL: Duration = Duration::from_millis(50);
/// How often stats go out, and how long without a frame means not live.
const STATS: Duration = Duration::from_secs(1);
/// How often it looks for the ring, when there is none or it has gone
/// quiet (an engine that restarted makes a new one).
const REOPEN: Duration = Duration::from_millis(500);
/// The most often a probe reads a moving picture.
const PROBE: Duration = Duration::from_millis(100);

/// What the window tells the thread.
pub(super) struct Shared {
    wanted: Mutex<Wanted>,
    /// Bumped when `wanted` or the drawable changes, to draw again.
    generation: AtomicU64,
    paused: AtomicBool,
    stopped: AtomicBool,
    /// Device pixels per point, as an `f32`'s bits.
    density: AtomicU32,
}

struct Wanted {
    source: String,
    view: VideoView,
    stats: Option<Data>,
    probed: Option<Data>,
}

impl Shared {
    pub(super) fn new(video: &Video) -> Arc<Self> {
        Arc::new(Self {
            wanted: Mutex::new(Wanted {
                source: video.source.clone(),
                view: video.view.clone(),
                stats: video.stats.clone(),
                probed: video.probed.clone(),
            }),
            generation: AtomicU64::new(1),
            paused: AtomicBool::new(true),
            stopped: AtomicBool::new(false),
            density: AtomicU32::new(1f32.to_bits()),
        })
    }

    /// Takes what the mod's view says now.
    pub(super) fn update(&self, video: &Video) {
        let mut wanted = self.wanted.lock().unwrap_or_else(PoisonError::into_inner);
        let changed = wanted.view != video.view || wanted.source != video.source;
        wanted.source.clone_from(&video.source);
        wanted.view.clone_from(&video.view);
        wanted.stats.clone_from(&video.stats);
        wanted.probed.clone_from(&video.probed);
        if changed {
            self.touch();
        }
    }

    pub(super) fn set_density(&self, density: f32) {
        self.density.store(density.to_bits(), Ordering::Relaxed);
    }

    /// Asks for the frame to be drawn again.
    pub(super) fn touch(&self) {
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub(super) fn pause(&self, paused: bool) {
        if self.paused.swap(paused, Ordering::Relaxed) && !paused {
            self.touch();
        }
    }

    pub(super) fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

/// Counted between two reports.
#[derive(Default)]
struct Counts {
    shown: u64,
    first: u64,
    newest: u64,
    /// Sum of each frame's age when shown, in milliseconds.
    age: f64,
}

/// Runs a stream until told to stop.
pub(super) fn run(
    shared: &Shared,
    mut renderer: Renderer,
    owner: &str,
    messages: &UnboundedSender<(String, Data)>,
) {
    let mut reader: Option<Reader> = None;
    let mut source = String::new();
    let mut last_number = 0;
    let mut drawn_generation = 0;
    let mut frame: Option<FrameInfo> = None;
    let mut counts = Counts::default();
    let (mut reported, mut looked, mut new_at) = (Instant::now(), None::<Instant>, None::<Instant>);
    let mut probed_at = None::<Instant>;
    let mut told_size = None;
    let mut told_probe = None;
    while !shared.stopped.load(Ordering::Relaxed) {
        if shared.paused.load(Ordering::Relaxed) {
            thread::sleep(PAUSED_POLL);
            continue;
        }
        let generation = shared.generation.load(Ordering::Acquire);
        let (view, wanted_source, stats, probed) = {
            let wanted = shared.wanted.lock().unwrap_or_else(PoisonError::into_inner);
            (
                wanted.view.clone(),
                wanted.source.clone(),
                wanted.stats.clone(),
                wanted.probed.clone(),
            )
        };
        if wanted_source != source {
            source = wanted_source;
            (reader, last_number, frame) = (None, 0, None);
        }
        let quiet = new_at.is_none_or(|at| at.elapsed() > REOPEN);
        if quiet && looked.is_none_or(|at: Instant| at.elapsed() > REOPEN) {
            looked = Some(Instant::now());
            match Reader::open(&source) {
                Ok(ring) => {
                    // A new run of the engine counts from the start again.
                    if ring.latest() < last_number {
                        last_number = 0;
                    }
                    reader = Some(ring);
                },
                Err(error) => tracing::trace!("video {source}: {error}"),
            }
        }
        let mut fresh = false;
        if let Some(ring) = &reader {
            let latest = ring.latest();
            if latest > last_number {
                let uploaded = ring.read(latest, |info, pixels, bytes| {
                    renderer.upload(info, pixels);
                    (*info, bytes)
                });
                if let Some((info, _)) = uploaded {
                    renderer.commit();
                    if counts.first == 0 {
                        counts.first = latest;
                    }
                    counts.newest = latest;
                    frame = Some(info);
                    fresh = true;
                    new_at = Some(Instant::now());
                }
                last_number = latest;
            }
        }
        if let Some(info) = &frame
            && (fresh || generation != drawn_generation)
        {
            renderer.draw(info, &view);
            drawn_generation = generation;
            if fresh {
                counts.shown += 1;
                counts.age += age(info.time);
            }
        } else {
            thread::sleep(POLL);
        }
        if let (Some(ring), Some(info), Some(point), Some(msg)) =
            (&reader, &frame, view.probe, &probed)
            && probed_at.is_none_or(|at: Instant| at.elapsed() > PROBE)
        {
            probed_at = Some(Instant::now());
            let size = renderer.size();
            let picture = [info.width as f32, info.height as f32];
            let placement = geometry::place(size, picture, &view);
            let point = geometry::picture_point(point, size, picture, placement);
            let read = ring
                .read(info.number, |info, pixels, bytes| {
                    // SAFETY: `read` hands out `bytes` readable bytes; a
                    // value torn by the writer is dropped just below.
                    #[allow(unsafe_code, reason = "reading the slot; see SAFETY")]
                    let pixels = unsafe { std::slice::from_raw_parts(pixels, bytes) };
                    geometry::probe(info, pixels, point, &view)
                })
                .flatten();
            if let Some(read) = read
                && told_probe.as_ref() != Some(&read)
            {
                told_probe = Some(read.clone());
                let _ = messages.unbounded_send((owner.to_owned(), with_fields(msg, read)));
            }
        }
        let size = frame.as_ref().map(|info| (info.width, info.height));
        if reported.elapsed() >= STATS || size != told_size {
            tracing::debug!(
                source,
                shown = counts.shown,
                frame = last_number,
                age = counts.age / counts.shown.max(1) as f64,
                size = ?renderer.size(),
                "video"
            );
            if let Some(msg) = &stats {
                let live = new_at.is_some_and(|at| at.elapsed() < STATS);
                let mut fields = report(&counts, frame.as_ref(), live, reported.elapsed());
                if let Some(info) = &frame {
                    let picture = [info.width as f32, info.height as f32];
                    let fit = geometry::place(
                        renderer.size(),
                        picture,
                        &VideoView {
                            zoom: None,
                            ..view.clone()
                        },
                    )
                    .scale;
                    fields.insert("fit".to_owned(), Data::Number(f64::from(fit)));
                    fields.insert(
                        "density".to_owned(),
                        Data::Number(f64::from(f32::from_bits(
                            shared.density.load(Ordering::Relaxed),
                        ))),
                    );
                }
                let _ = messages.unbounded_send((owner.to_owned(), with_fields(msg, fields)));
            }
            told_size = size;
            reported = Instant::now();
            counts = Counts::default();
        }
    }
}

/// How old a frame captured at `time` is now, in milliseconds.
fn age(time: u64) -> f64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos() as u64);
    now.saturating_sub(time) as f64 / 1e6
}

/// A stats message's fields.
fn report(
    counts: &Counts,
    frame: Option<&FrameInfo>,
    live: bool,
    took: Duration,
) -> BTreeMap<String, Data> {
    let span = counts.newest.saturating_sub(counts.first) + u64::from(counts.first > 0);
    let integer = |value: u64| Data::Integer(i64::try_from(value).unwrap_or(i64::MAX));
    let mut fields = BTreeMap::from([
        ("shown".to_owned(), integer(counts.shown)),
        (
            "skipped".to_owned(),
            integer(span.saturating_sub(counts.shown)),
        ),
        (
            "fps".to_owned(),
            Data::Number(counts.shown as f64 / took.as_secs_f64().max(0.001)),
        ),
        ("live".to_owned(), Data::Bool(live)),
    ]);
    if counts.shown > 0 {
        fields.insert(
            "latency".to_owned(),
            Data::Number(counts.age / counts.shown as f64),
        );
    }
    if let Some(frame) = frame {
        fields.insert("width".to_owned(), integer(frame.width.into()));
        fields.insert("height".to_owned(), integer(frame.height.into()));
        fields.insert("format".to_owned(), Data::from(frame.format.name()));
        fields.insert("frame".to_owned(), integer(frame.number));
    }
    fields
}
