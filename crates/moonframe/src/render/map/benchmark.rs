//! A benchmark of the first map in the window, for `map-benchmark` builds run
//! with `MOONFRAME_MAP_BENCHMARK` set: it drives the map through real input
//! events, on screen, prints what each run cost, and quits.
//!
//! `MOONFRAME_MAP_BENCHMARK_SINCE`, in milliseconds since the Unix epoch, is
//! when the app was launched, for how long the map took to appear.

use {
    gpui_kit::*,
    std::{
        cell::{Cell, RefCell},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    },
};

/// How long the map is left to fill in before the runs start.
const SETTLE: Duration = Duration::from_secs(4);
/// How long each run lasts.
const RUN: Duration = Duration::from_secs(4);
/// How often an input event goes out, at most.
const EVENT: Duration = Duration::from_millis(8);

/// What one run saw.
#[derive(Default)]
struct Run {
    /// Milliseconds between frames.
    gaps: Vec<f64>,
    /// Milliseconds each input event took to handle.
    inputs: Vec<f64>,
    /// Frames with a tile that had nothing to show, and how many tiles.
    holed: u32,
    holes: u32,
    /// Times the map changed tile level.
    levels: u32,
    last: Option<Instant>,
    level: Option<u8>,
}

thread_local! {
    static STARTED: Cell<bool> = const { Cell::new(false) };
    static RUNNING: RefCell<Option<Run>> = const { RefCell::new(None) };
    /// Tiles with nothing to show in the frame being drawn.
    static HOLES: Cell<u32> = const { Cell::new(0) };
    static DRAWN: Cell<Option<f64>> = const { Cell::new(None) };
    static FILLED: Cell<Option<f64>> = const { Cell::new(None) };
}

/// Counts a tile's place in the frame being drawn, and whether something
/// shows there.
pub(super) fn cell(covered: bool) {
    if !covered {
        HOLES.set(HOLES.get() + 1);
    }
}

/// Counts a frame of the map, drawn at tile `level`.
pub(super) fn frame(level: u8, missing: bool) {
    let since = since_launch();
    if DRAWN.get().is_none() {
        DRAWN.set(since);
    }
    if !missing && FILLED.get().is_none() {
        FILLED.set(since);
    }
    let holes = HOLES.replace(0);
    RUNNING.with_borrow_mut(|run| {
        let Some(run) = run else {
            return;
        };
        let now = Instant::now();
        if let Some(last) = run.last.replace(now) {
            run.gaps.push((now - last).as_secs_f64() * 1e3);
        }
        if run.level.replace(level).is_some_and(|was| was != level) {
            run.levels += 1;
        }
        if holes > 0 {
            run.holed += 1;
            run.holes += holes;
        }
    });
}

fn since_launch() -> Option<f64> {
    let launched: f64 = std::env::var("MOONFRAME_MAP_BENCHMARK_SINCE")
        .ok()?
        .parse()
        .ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    Some(now.as_secs_f64() * 1e3 - launched)
}

/// Starts the runs, once, if this launch asked for them.
pub(super) fn start<T: 'static>(cx: &Context<T>) {
    if std::env::var_os("MOONFRAME_MAP_BENCHMARK").is_none() || STARTED.replace(true) {
        return;
    }
    cx.spawn(async move |_, cx| {
        let cx: &mut AsyncApp = cx;
        run_all(cx).await;
        std::process::exit(0);
    })
    .detach();
}

/// A place in the map's half of the window.
fn over_map(window: &Window) -> Point<Pixels> {
    let size = window.viewport_size();
    point(size.width * 0.65, size.height * 0.5)
}

/// Sends what `event` makes of how far through the run it is, every
/// [`EVENT`], for a [`RUN`].
async fn drive(
    cx: &mut AsyncApp,
    handle: AnyWindowHandle,
    event: impl Fn(f64, Point<Pixels>) -> Option<PlatformInput>,
) {
    let executor = cx.background_executor().clone();
    let started = Instant::now();
    while started.elapsed() < RUN {
        let through = started.elapsed().as_secs_f64();
        let _ = cx.update_window(handle, |_, window, cx| {
            if let Some(input) = event(through, over_map(window)) {
                let sent = Instant::now();
                window.dispatch_event(input, cx);
                let took = sent.elapsed().as_secs_f64() * 1e3;
                RUNNING.with_borrow_mut(|run| {
                    if let Some(run) = run {
                        run.inputs.push(took);
                    }
                });
            }
        });
        executor.timer(EVENT).await;
    }
}

fn scroll(position: Point<Pixels>, pixels: f32) -> PlatformInput {
    PlatformInput::Scroll(ScrollEvent {
        position,
        delta: ScrollDelta::Pixels(point(px(0.), px(pixels))),
        modifiers: Modifiers::default(),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    })
}

fn pinch(position: Point<Pixels>, delta: f32, phase: TouchPhase) -> PlatformInput {
    PlatformInput::Pinch(PinchEvent {
        position,
        delta,
        modifiers: Modifiers::default(),
        phase,
    })
}

/// `-1` or `1`, turning over every `seconds`.
fn way(through: f64, seconds: f64) -> f32 {
    if ((through / seconds) as u32).is_multiple_of(2) {
        -1.0
    } else {
        1.0
    }
}

async fn run_all(cx: &mut AsyncApp) {
    let executor = cx.background_executor().clone();
    executor.timer(SETTLE).await;
    let Some(handle) = cx.update(|cx| cx.windows().first().copied()) else {
        return;
    };
    let cpu_at_start = cpu_seconds();
    print_launch();
    let mut lines = Vec::new();

    begin();
    drive(cx, handle, |through, at| {
        Some(scroll(at, way(through, 1.0) * 3.0))
    })
    .await;
    lines.push(finish("Zoom in and out", None));
    begin();
    drive(cx, handle, |through, at| {
        Some(scroll(at, way(through, 0.4) * 12.0))
    })
    .await;
    lines.push(finish("Fast zoom in and out", None));
    begin();
    drive(cx, handle, |through, at| {
        let phase = if through < 0.02 {
            TouchPhase::Started
        } else {
            TouchPhase::Moved
        };
        Some(pinch(at, -way(through, 1.0) * 0.03, phase))
    })
    .await;
    let _ = cx.update_window(handle, |_, window, cx| {
        window.dispatch_event(pinch(over_map(window), 0.0, TouchPhase::Ended), cx);
    });
    lines.push(finish("Pinch", None));
    executor.timer(Duration::from_secs(1)).await;

    let _ = cx.update_window(handle, |_, window, cx| {
        let position = over_map(window);
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
    });
    begin();
    drive(cx, handle, |through, at| {
        let across = (through * 3.0).sin() as f32 * 240.0;
        Some(PlatformInput::MouseMove(MouseMoveEvent {
            position: point(at.x + px(across), at.y),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::default(),
        }))
    })
    .await;
    let _ = cx.update_window(handle, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: over_map(window),
                modifiers: Modifiers::default(),
                click_count: 1,
            }),
            cx,
        );
    });
    lines.push(finish("Drag", None));
    executor.timer(Duration::from_secs(1)).await;

    begin();
    let cpu = cpu_seconds();
    executor.timer(RUN).await;
    let idle = cpu
        .zip(cpu_seconds())
        .map(|(from, to)| (to - from) / RUN.as_secs_f64());
    lines.push(finish("Idle", idle));

    print_table(&lines, cpu_at_start);
}

fn begin() {
    RUNNING.set(Some(Run::default()));
}

/// The row of the run that just ended.
fn finish(name: &str, cpu: Option<f64>) -> Vec<String> {
    let Some(mut run) = RUNNING.take() else {
        return Vec::new();
    };
    run.gaps.sort_by(f64::total_cmp);
    let count = run.gaps.len();
    let at = |share: f64| {
        run.gaps
            .get(((count as f64 * share) as usize).min(count.saturating_sub(1)))
            .copied()
            .unwrap_or_default()
    };
    let seconds = run.gaps.iter().sum::<f64>() / 1e3;
    let input = run.inputs.iter().sum::<f64>() / run.inputs.len().max(1) as f64;
    vec![
        name.to_owned(),
        format!("{:.0}", count as f64 / seconds.max(f64::EPSILON)),
        format!("{:.1}", at(0.5)),
        format!("{:.1}", at(0.99)),
        format!("{:.1}", at(1.0)),
        if run.inputs.is_empty() {
            "-".to_owned()
        } else {
            format!("{input:.2}")
        },
        run.levels.to_string(),
        format!("{} ({})", run.holed, run.holes),
        cpu.map_or_else(|| "-".to_owned(), |cpu| format!("{:.0}%", cpu * 100.0)),
    ]
}

/// The CPU time this process has used, as `ps` says.
fn cpu_seconds() -> Option<f64> {
    let output = std::process::Command::new("ps")
        .args(["-o", "cputime=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    text.trim().split(':').try_fold(0.0, |total, part| {
        Some(total * 60.0 + part.parse::<f64>().ok()?)
    })
}

#[allow(clippy::print_stdout, reason = "the report is what this is for")]
fn print_launch() {
    let shown = |at: Option<f64>| at.map_or_else(|| "-".to_owned(), |at| format!("{at:.0} ms"));
    println!();
    println!("Launch to first map frame  {}", shown(DRAWN.get()));
    println!("Launch to all tiles shown  {}", shown(FILLED.get()));
}

#[allow(clippy::print_stdout, reason = "the report is what this is for")]
fn print_table(lines: &[Vec<String>], cpu_at_start: Option<f64>) {
    let head = [
        "", "fps", "p50 ms", "p99 ms", "max ms", "input ms", "levels", "holes", "cpu",
    ];
    let row = |cells: &[&str]| {
        let name = cells.first().copied().unwrap_or_default();
        let rest: Vec<String> = cells
            .iter()
            .skip(1)
            .map(|cell| format!("{cell:>9}"))
            .collect();
        println!("{name:<22}{}", rest.join(""));
    };
    println!();
    row(&head);
    for line in lines {
        row(&line.iter().map(String::as_str).collect::<Vec<_>>());
    }
    println!();
    println!("p50, p99, max: time between frames. input: time to handle one event.");
    println!("levels: tile level changes. holes: frames (tiles) with nothing to show.");
    if let Some(cpu) = cpu_at_start {
        println!("CPU used up to the first run: {cpu:.1} s.");
    }
}
