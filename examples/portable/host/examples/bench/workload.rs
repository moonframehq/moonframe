//! A provider doing real work, both ways: `bench/providers/digest` asks the
//! host for request logs through the `bench.logs` capability, digests them
//! per route and publishes the summary. Each cycle is one message sent and
//! the summary published, timed at a few batch sizes; a fetch-only cycle
//! tells the cost of moving the lines from that of digesting them.

use {
    super::{Running, ms},
    moonframe::moonframe_core::Data,
    moonframe_providers::{
        Caller, Capabilities, HostCapability, Natives, Portable, ProviderOptions, native::Factory,
        to_json,
    },
    std::{
        collections::BTreeMap,
        path::Path,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
};

/// Lines per batch.
const SIZES: [usize; 3] = [1_000, 10_000, 50_000];
/// Cycles timed per size, after `WARMUP` that are not.
const CYCLES: usize = 30;
const WARMUP: usize = 3;
/// A 50,000-line batch is about 2.5 MB of JSON, past the default 1 MiB a
/// host call may carry; both builds get the same raised limit.
const MAX_BYTES: usize = 16 * 1024 * 1024;

pub(super) fn run(build: &Path) -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../bench")
        .canonicalize()
        .map_err(|e| format!("the bench's providers: {e}"))?;
    let logs = Logs::default();
    println!("Real workload: digest request logs per route, one cycle per message");
    println!("(median of {CYCLES} after {WARMUP} warmup; fetch moves the lines only):");
    println!(
        "  {:>7}  {:>9}  {:>12}  {:>12}  {:>8}",
        "lines", "batch", "wasm", "native", "wasm/nat"
    );
    let mut wasm = Running::start_with(&root, build, "digest", Natives::new(), options(&logs)?);
    let mut native = Running::start_with(&root, build, "digest-native", natives(), options(&logs)?);
    for lines in SIZES {
        for (kind, key) in [("fetch", "fetch"), ("digest", "run")] {
            let message = Data::map([(key, Data::from(lines as i64))]);
            let (wasm_time, wasm_out) = cycles(&mut wasm, &message)?;
            let (native_time, native_out) = cycles(&mut native, &message)?;
            if !same(&wasm_out, &native_out) {
                return Err(format!("the builds disagree at {lines} lines ({kind})"));
            }
            println!(
                "  {lines:>7}  {:>6} KiB  {}  {}  {:>7.2}x  {kind}{}",
                logs.size(lines) / 1024,
                ms(wasm_time),
                ms(native_time),
                wasm_time.as_secs_f64() / native_time.as_secs_f64(),
                if kind == "digest" {
                    format!(", publishes {} KiB", wasm_out.len() / 1024)
                } else {
                    String::new()
                },
            );
        }
    }
    Ok(())
}

/// `message` sent `WARMUP + CYCLES` times; the median time, and the last
/// answer as JSON, for the builds to be compared.
fn cycles(running: &mut Running, message: &Data) -> Result<(Duration, String), String> {
    let mut times = Vec::with_capacity(CYCLES);
    let mut last = String::new();
    for cycle in 0..WARMUP + CYCLES {
        let sent = Instant::now();
        running.send(message.clone())?;
        let answer = to_json(&running.published()?);
        if let Some(problem) = answer.get("problem") {
            return Err(format!("{}: {problem}", running.name));
        }
        if cycle >= WARMUP {
            times.push(sent.elapsed());
        }
        last = answer.to_string();
    }
    times.sort();
    Ok((times[times.len() / 2], last))
}

/// Whether the builds' answers match. Floats may differ in their last
/// bit: a wasm provider's data crosses as JSON text, which the host parses
/// with serde_json's fast float parser, not a round-trip exact one.
fn same(wasm: &str, native: &str) -> bool {
    fn close(a: &serde_json::Value, b: &serde_json::Value) -> bool {
        use serde_json::Value::{Array, Number, Object};
        match (a, b) {
            (Number(x), Number(y)) if x.is_f64() || y.is_f64() => {
                let (x, y) = (
                    x.as_f64().unwrap_or(f64::NAN),
                    y.as_f64().unwrap_or(f64::NAN),
                );
                (x - y).abs() <= f64::EPSILON * 4. * x.abs().max(y.abs()).max(1.)
            },
            (Array(x), Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(x, y)| close(x, y)),
            (Object(x), Object(y)) => {
                x.len() == y.len()
                    && x.iter()
                        .all(|(key, x)| y.get(key).is_some_and(|y| close(x, y)))
            },
            _ => a == b,
        }
    }
    match (serde_json::from_str(wasm), serde_json::from_str(native)) {
        (Ok(a), Ok(b)) => close(&a, &b),
        _ => false,
    }
}

fn options(logs: &Logs) -> Result<ProviderOptions, String> {
    let mut capabilities = Capabilities::default();
    capabilities.limits.max_bytes = MAX_BYTES;
    capabilities.capability("bench.logs", logs.clone())?;
    Ok(ProviderOptions {
        capabilities,
        ..ProviderOptions::default()
    })
}

fn natives() -> Natives {
    let factory: Factory = Arc::new(|| Box::new(Portable::<digest::Digest>::new()));
    Natives::from([("digest-native".to_owned(), factory)])
}

/// The `bench.logs` capability: `batch { lines }` answers that many log
/// lines as a JSON array, the same lines every time, made once per size so
/// the host's own cost is a copy.
#[derive(Clone, Default)]
struct Logs(Arc<Mutex<BTreeMap<usize, Arc<String>>>>);

impl Logs {
    fn batch(&self, lines: usize) -> Arc<String> {
        let mut made = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(made.entry(lines).or_insert_with(|| Arc::new(make(lines))))
    }

    fn size(&self, lines: usize) -> usize {
        self.batch(lines).len()
    }
}

impl HostCapability for Logs {
    fn call(&self, _: &Caller, method: &str, args: &str) -> Result<String, String> {
        if method != "batch" {
            return Err(format!("unknown method `{method}`"));
        }
        let args: serde_json::Value = serde_json::from_str(args).map_err(|e| e.to_string())?;
        let lines = args
            .get("lines")
            .and_then(serde_json::Value::as_u64)
            .ok_or("`lines` is a count")?;
        Ok(self.batch(lines as usize).as_str().to_owned())
    }
}

/// `count` lines of a made-up API's access log, the same for a count.
fn make(count: usize) -> String {
    const ROUTES: [(&str, &str); 12] = [
        ("GET", "/api/users/{}"),
        ("GET", "/api/users/{}/orders"),
        ("GET", "/api/orders/{}"),
        ("POST", "/api/orders"),
        ("GET", "/api/products/{}"),
        ("GET", "/api/products?page={}"),
        ("PUT", "/api/users/{}/cart/items/{}"),
        ("GET", "/api/search?q=item{}"),
        ("POST", "/login"),
        ("GET", "/health"),
        ("DELETE", "/api/sessions/{}"),
        ("GET", "/static/app.{}.js"),
    ];
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let lines: Vec<String> = (0..count)
        .map(|index| {
            let (method, template) = ROUTES[(next() % ROUTES.len() as u64) as usize];
            let path = template.replace("{}", &(next() % 10_000).to_string());
            let status = match next() % 100 {
                0..=1 => 503,
                2..=6 => 404,
                _ => 200,
            };
            let millis = 2
                + next() % 40
                + if next() % 20 == 0 {
                    next() % 900
                } else {
                    0
                };
            let bytes = 200 + next() % 20_000;
            let second = index % 86_400;
            format!(
                "2026-11-17T{:02}:{:02}:{:02}Z {method} {path} {status} {millis}ms {bytes}B",
                second / 3600,
                second / 60 % 60,
                second % 60
            )
        })
        .collect();
    serde_json::to_string(&lines).unwrap_or_default()
}
