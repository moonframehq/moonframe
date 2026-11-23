//! The same provider, measured both ways: `app/providers/focus` run as a
//! wasm component and compiled in as a native provider, through the
//! provider runtime the app uses (its threads, channels and checks), so
//! the numbers are what a view waits for.
//!
//! ```sh
//! cargo run --release -p moonframe-portable-example --example bench
//! ```
//!
//! It measures, for each build:
//!
//! - **startup**: from starting the provider to its first data, for wasm
//!   built from source (a cold Cargo build into an empty folder, then warm,
//!   where Cargo finds nothing to do), wasm prebuilt (a `provider.wasm`
//!   alone, as bundled apps ship it), and native;
//! - **round trip**: a mod's message sent, the provider's answer published,
//!   one after another, as median, p95 and p99, and messages per second;
//! - **a real workload** (see `workload.rs`): `bench/providers/digest`
//!   fetching request logs through a host capability, digesting them per
//!   route and publishing the summary, at a few batch sizes.
//!
//! The wasm build needs `rustup target add wasm32-wasip2`.

mod workload;

use {
    moonframe::moonframe_core::{Data, Roots},
    moonframe_providers::{
        Event, Events, Natives, Portable, ProviderOptions, Providers, native::Factory,
    },
    std::{
        collections::BTreeSet,
        fs,
        path::Path,
        sync::{Arc, Mutex, mpsc},
        time::{Duration, Instant},
    },
};

/// Round trips timed, after `WARMUP` that are not.
const TRIPS: usize = 5_000;
const WARMUP: usize = 200;
/// Warm starts timed, for their median.
const STARTS: usize = 5;
/// Longest a provider may take to answer, a cold build included.
const PATIENCE: Duration = Duration::from_secs(600);

fn main() -> Result<(), String> {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../app")
        .canonicalize()
        .map_err(|e| format!("the example's app folder: {e}"))?;
    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    let build = scratch.path().join("build");
    println!("{}\n", machine());

    // Wasm from source: a cold build, then warm ones on the same folder.
    let cold = start(&app, &build, "focus", Natives::new())?;
    let warm = median((0..STARTS).map(|_| start(&app, &build, "focus", Natives::new())))?;
    // Wasm prebuilt: the component alone in a folder of its own.
    let component = build.join("wasm32-wasip2/release/focus.wasm");
    let prebuilt_root = scratch.path().join("prebuilt");
    let folder = prebuilt_root.join("providers/focus");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    fs::copy(
        app.join("providers/focus/provider.toml"),
        folder.join("provider.toml"),
    )
    .map_err(|e| e.to_string())?;
    fs::copy(&component, folder.join("provider.wasm"))
        .map_err(|e| format!("{}: {e}", component.display()))?;
    let size = fs::metadata(folder.join("provider.wasm"))
        .map_err(|e| e.to_string())?
        .len();
    let prebuilt =
        median((0..STARTS).map(|_| start(&prebuilt_root, &build, "focus", Natives::new())))?;
    // Native: the same crate, linked into this binary.
    let native = median((0..STARTS).map(|_| start(&app, &build, "focus-native", natives())))?;

    println!("Startup, to the first data (median of {STARTS}; cold is one build):");
    println!("  wasm, from source, cold build  {}", ms(cold));
    println!("  wasm, from source, warm        {}", ms(warm));
    println!("  wasm, prebuilt provider.wasm   {}", ms(prebuilt));
    println!("  native                         {}", ms(native));
    println!("  component size                 {} KiB\n", size / 1024);

    println!("Round trip, message to published answer ({TRIPS} after {WARMUP} warmup):");
    for (label, root, name, natives) in [
        ("wasm", &prebuilt_root, "focus", Natives::new()),
        ("native", &app, "focus-native", natives()),
    ] {
        let trips = round_trips(root, &build, name, natives)?;
        println!("  {label:<7} {trips}");
    }
    println!();
    workload::run(&build)
}

/// The native build, as the example's host registers it.
fn natives() -> Natives {
    let factory: Factory = Arc::new(|| Box::new(Portable::<focus::Focus>::new()));
    Natives::from([("focus-native".to_owned(), factory)])
}

/// The provider `name` under `root`, started alone, and what it reports.
struct Running {
    providers: Providers,
    events: mpsc::Receiver<Event>,
    name: String,
}

impl Running {
    fn start(root: &Path, build: &Path, name: &str, natives: Natives) -> Self {
        Self::start_with(root, build, name, natives, ProviderOptions::default())
    }

    fn start_with(
        root: &Path,
        build: &Path,
        name: &str,
        natives: Natives,
        options: ProviderOptions,
    ) -> Self {
        let (send, events) = mpsc::channel();
        let send = Mutex::new(send);
        let report: Events = Arc::new(move |event| {
            if let Ok(send) = send.lock() {
                let _ = send.send(event);
            }
        });
        let providers = Providers::start_configured(
            Roots::new([root.to_owned()]),
            build.to_owned(),
            report,
            natives,
            Some(BTreeSet::from([name.to_owned()])),
            options,
        );
        Self {
            providers,
            events,
            name: name.to_owned(),
        }
    }

    /// Waits for the provider's next data; its failure is the bench's.
    fn published(&self) -> Result<Data, String> {
        let until = Instant::now() + PATIENCE;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(left) {
                Ok(Event::Published { provider, data }) if provider == self.name => {
                    return Ok(data);
                },
                Ok(Event::Failed { provider, problem }) if provider == self.name => {
                    return Err(format!("{provider}: {problem}"));
                },
                Ok(_) => {},
                Err(_) => return Err(format!("{}: no data in {PATIENCE:?}", self.name)),
            }
        }
    }

    fn send(&mut self, message: Data) -> Result<(), String> {
        self.providers
            .send(&self.name, "bench", message)
            .map_err(|e| e.to_string())
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.providers.shutdown();
    }
}

/// How long `name` takes from starting to its first data.
fn start(root: &Path, build: &Path, name: &str, natives: Natives) -> Result<Duration, String> {
    let started = Instant::now();
    let running = Running::start(root, build, name, natives);
    running.published()?;
    Ok(started.elapsed())
}

/// Round trips through `name`, timed one by one, then in a burst.
fn round_trips(root: &Path, build: &Path, name: &str, natives: Natives) -> Result<Trips, String> {
    let mut running = Running::start(root, build, name, natives);
    running.published()?;
    let mut times = Vec::with_capacity(TRIPS);
    for trip in 0..WARMUP + TRIPS {
        let sent = Instant::now();
        running.send(Data::from("status"))?;
        running.published()?;
        if trip >= WARMUP {
            times.push(sent.elapsed());
        }
    }
    times.sort();
    let total: Duration = times.iter().sum();
    Ok(Trips {
        median: times[times.len() / 2],
        p95: times[times.len() * 95 / 100],
        p99: times[times.len() * 99 / 100],
        per_second: TRIPS as f64 / total.as_secs_f64(),
    })
}

struct Trips {
    median: Duration,
    p95: Duration,
    p99: Duration,
    per_second: f64,
}

impl std::fmt::Display for Trips {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "median {}  p95 {}  p99 {}  {:.0} messages/s",
            us(self.median),
            us(self.p95),
            us(self.p99),
            self.per_second
        )
    }
}

fn median(times: impl Iterator<Item = Result<Duration, String>>) -> Result<Duration, String> {
    let mut times = times.collect::<Result<Vec<_>, _>>()?;
    times.sort();
    times
        .get(times.len() / 2)
        .copied()
        .ok_or_else(|| "nothing timed".to_owned())
}

fn ms(time: Duration) -> String {
    format!("{:>9.1} ms", time.as_secs_f64() * 1000.)
}

fn us(time: Duration) -> String {
    format!("{:.1} µs", time.as_secs_f64() * 1e6)
}

/// What it ran on, for the numbers to be read against.
fn machine() -> String {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let cpu = run("sysctl", &["-n", "machdep.cpu.brand_string"])
        .or_else(|| {
            fs::read_to_string("/proc/cpuinfo").ok().and_then(|info| {
                info.lines()
                    .find(|line| line.starts_with("model name"))
                    .and_then(|line| line.split(':').nth(1))
                    .map(|name| name.trim().to_owned())
            })
        })
        .unwrap_or_else(|| "unknown CPU".to_owned());
    format!(
        "{cpu}, {} {}, {profile} build",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn run(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
