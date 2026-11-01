//! Gantry's services and their telemetry, shared as `app.telemetry`: SAMPLE
//! data, simulated here. Every 3 seconds (the provider's `interval`) a new
//! sample is taken, standing for 15 minutes of the platform, and the whole
//! state is published. Clock times are UTC: the sandbox has no time zone.
//!
//! Output:
//!   now        the simulated clock, "HH:MM"
//!   times      a heading per sample, oldest first ("TODAY, 06:00")
//!   axis       { index, text }: x-axis labels every 3 hours, "NOW" last
//!   services   { id, name, status, tags, regions, description, version,
//!                deployed, replicas, rps, p95, errors, resources, history,
//!                logs, deploys }
//!     resources  cpu, gpu, memory: { allocated, used, util, label, unit,
//!                limit, limit_unit, step }
//!     history    cpu, gpu, memory: utilization in %, one per sample;
//!                rps and p95 per sample
//!
//! Messages:
//!   limit = { service, resource, by = 1|-1 }  raise or lower an allocation
//!   deploy = id                               roll out a new version

wit_bindgen::generate!({ path: "wit/provider.wit", world: "provider" });

use {
    serde_json::{Map, Value, json},
    std::{
        cell::RefCell,
        collections::VecDeque,
        f64::consts::PI,
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// 24 hours of 15 minutes, both ends included.
const SAMPLES: usize = 97;
/// What one sample stands for, in seconds.
const STEP: i64 = 15 * 60;
/// Log lines kept per service.
const LOGS: usize = 40;
/// Seconds between samples: the provider's `interval`.
const TICK: f64 = 3.0;
const KEYS: [&str; 3] = ["cpu", "gpu", "memory"];
const REGIONS: [&str; 3] = ["iad", "fra", "sin"];

struct Telemetry;

/// What a service asks of one resource, in absolute units: a base, a daily
/// swing and noise. Utilization is that over what is allocated.
struct Resource {
    allocated: i64,
    base: f64,
    swing: f64,
    noise: f64,
    rise: f64,
    unit: &'static str,
    model: &'static str,
    step: i64,
    min: i64,
    max: i64,
    demand: VecDeque<f64>,
}

struct Deploy {
    version: String,
    at: i64,
    who: &'static str,
    note: &'static str,
    status: &'static str,
}

struct Log {
    at: i64,
    level: &'static str,
    text: String,
}

struct Service {
    id: &'static str,
    status: &'static str,
    tags: &'static [&'static str],
    regions: &'static str,
    description: &'static str,
    latest: &'static str,
    version: i64,
    /// Samples since the live version rolled out.
    deployed: i64,
    replicas: i64,
    rps: i64,
    p95: i64,
    errors: f64,
    /// CPU, GPU and memory, in the order of `KEYS`.
    resources: [Resource; 3],
    rps_history: VecDeque<i64>,
    p95_history: VecDeque<i64>,
    /// Samples left until a deploy is live, 0 when none rolls out.
    rolling: u32,
    deploy_log: Vec<Deploy>,
    logs: VecDeque<Log>,
}

/// What the provider keeps between ticks.
struct State {
    services: Vec<Service>,
    /// The clock at the last sample, in seconds since the epoch.
    start: i64,
    /// Samples taken since launch.
    index: i64,
    random: Random,
    /// When the next sample is due, once the first state is published.
    due: Option<f64>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// A small, seedable generator (SplitMix64): the day of history is the same
/// on every launch, the live samples are not.
struct Random(u64);

impl Random {
    /// A float in `[0, 1)`.
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }

    /// An integer in `[0, below)`.
    fn below(&mut self, below: usize) -> usize {
        (self.next() * below as f64) as usize
    }
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64())
}

#[allow(clippy::too_many_arguments)]
fn resource(
    allocated: i64,
    base: f64,
    swing: f64,
    noise: f64,
    unit: &'static str,
    step: i64,
    min: i64,
    max: i64,
) -> Resource {
    Resource {
        allocated,
        base,
        swing,
        noise,
        rise: 0.0,
        unit,
        model: "",
        step,
        min,
        max,
        demand: VecDeque::new(),
    }
}

fn cores(allocated: i64, base: f64, swing: f64, noise: f64) -> Resource {
    resource(allocated, base, swing, noise, "CORES", 1, 2, 32)
}

fn gpus(allocated: i64, base: f64, swing: f64, noise: f64, model: &'static str) -> Resource {
    Resource {
        model,
        ..resource(allocated, base, swing, noise, "GPUS", 1, 1, 8)
    }
}

fn memory(allocated: i64, base: f64, swing: f64, noise: f64) -> Resource {
    resource(allocated, base, swing, noise, "GB", 8, 16, 256)
}

#[allow(clippy::too_many_arguments)]
fn service(
    id: &'static str,
    status: &'static str,
    tags: &'static [&'static str],
    regions: &'static str,
    description: &'static str,
    latest: &'static str,
    [version, deployed, replicas, rps, p95]: [i64; 5],
    errors: f64,
    resources: [Resource; 3],
) -> Service {
    Service {
        id,
        status,
        tags,
        regions,
        description,
        latest,
        version,
        deployed,
        replicas,
        rps,
        p95,
        errors,
        resources,
        rps_history: VecDeque::new(),
        p95_history: VecDeque::new(),
        rolling: 0,
        deploy_log: Vec::new(),
        logs: VecDeque::new(),
    }
}

fn services() -> Vec<Service> {
    vec![
        service(
            "token-relay",
            "active",
            &["API", "EDGE", "AGENT RUNTIME"],
            "iad · fra · sin",
            "Edge relay that fans agent tool calls out to workers, streams tokens back to clients and replays failed runs in three regions.",
            "Stream tokens over QUIC",
            [142, 9, 12, 1840, 412],
            0.18,
            [
                cores(8, 4.4, 0.6, 0.18),
                Resource {
                    rise: 0.9,
                    ..gpus(4, 2.6, 0.45, 0.08, "H100")
                },
                memory(64, 14.0, 1.6, 0.6),
            ],
        ),
        service(
            "vector-index",
            "active",
            &["STORAGE", "SEARCH"],
            "iad · fra",
            "Sharded embedding index behind retrieval: serves nearest-neighbour queries and rebuilds shards as documents land.",
            "Rebuild shards in the background",
            [88, 31, 6, 920, 38],
            0.02,
            [
                cores(16, 9.5, 2.0, 0.4),
                gpus(2, 0.7, 0.3, 0.06, "L40S"),
                memory(128, 92.0, 6.0, 1.5),
            ],
        ),
        service(
            "eval-scheduler",
            "degraded",
            &["BATCH", "AGENT RUNTIME"],
            "iad",
            "Queues evaluation suites against new checkpoints, leases GPUs by the minute and posts scores to the run log.",
            "Lease GPUs by the minute",
            [57, 3, 4, 64, 1290],
            2.4,
            [
                cores(4, 3.3, 0.4, 0.15),
                gpus(8, 7.1, 0.6, 0.2, "H100"),
                memory(96, 58.0, 8.0, 2.0),
            ],
        ),
        service(
            "artifact-store",
            "active",
            &["STORAGE", "API"],
            "iad · fra · sin",
            "Content-addressed store for checkpoints, traces and tool outputs, with signed URLs and a lifecycle sweep.",
            "Signed URLs expire after 15 minutes",
            [211, 64, 9, 2310, 96],
            0.05,
            [
                cores(8, 2.6, 0.5, 0.12),
                gpus(1, 0.05, 0.02, 0.01, "T4"),
                memory(32, 11.0, 2.0, 0.4),
            ],
        ),
        service(
            "webhook-fanout",
            "active",
            &["EDGE", "EVENTS"],
            "fra · sin",
            "Delivers run events to customer webhooks with retries, backoff and a dead-letter queue per endpoint.",
            "Dead-letter queue per endpoint",
            [34, 140, 3, 410, 180],
            0.4,
            [
                cores(2, 0.7, 0.25, 0.06),
                gpus(1, 0.0, 0.0, 0.0, "NONE"),
                memory(16, 5.0, 1.0, 0.3),
            ],
        ),
        service(
            "sandbox-pool",
            "paused",
            &["AGENT RUNTIME", "ISOLATION"],
            "iad",
            "Warm pool of microVM sandboxes where agents run untrusted code; paused while the kernel image is rebuilt.",
            "Kernel 6.9 microVM image",
            [19, 410, 0, 0, 0],
            0.0,
            [
                cores(24, 0.4, 0.1, 0.04),
                gpus(1, 0.0, 0.0, 0.0, "NONE"),
                memory(128, 8.0, 0.5, 0.2),
            ],
        ),
    ]
}

/// A log line from three numbers from 100 to 9099 and a region.
fn log_line(kind: usize, [a, b, c]: [i64; 3], region: &str) -> (&'static str, String) {
    match kind {
        0 => (
            "info",
            format!("stream opened tenant=t_{a:04} run=r_{b:05}"),
        ),
        1 => (
            "info",
            format!(
                "tool call dispatched worker={region}-{} queue_ms={}",
                a % 40,
                b % 90
            ),
        ),
        2 => (
            "info",
            format!("tokens flushed bytes={} region={region}", a * 7),
        ),
        3 => (
            "warn",
            format!(
                "retrying run=r_{b:05} attempt={} reason=upstream_timeout",
                c % 3 + 2
            ),
        ),
        4 => (
            "info",
            format!("health check ok region={region} p50_ms={}", c % 60 + 12),
        ),
        5 => (
            "debug",
            format!("lease renewed gpu={region}-{} ttl={}s", a % 8, 30 + b % 60),
        ),
        _ => (
            "error",
            format!(
                "worker {region}-{} exited code=137 (oom), rescheduling",
                a % 40
            ),
        ),
    }
}
const LOG_KINDS: usize = 7;

/// "HH:MM" of a time, in UTC.
fn hhmm(time: i64) -> String {
    let minutes = time.rem_euclid(86_400) / 60;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn day_of(time: i64) -> i64 {
    time.div_euclid(86_400)
}

fn ago(samples: i64) -> String {
    let minutes = samples * 15;
    if minutes < 1 {
        return "just now".into();
    }
    if minutes < 60 {
        return format!("{minutes}m ago");
    }
    let hours = minutes / 60;
    if hours < 48 {
        return format!("{hours}h ago");
    }
    format!("{}d ago", hours / 24)
}

/// A number as JSON: whole values as integers, the rest as floats.
fn number(value: f64) -> Value {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        json!(value as i64)
    } else {
        json!(value)
    }
}

/// A value rounded to one decimal.
fn tenths(value: f64) -> f64 {
    format!("{value:.1}").parse().unwrap_or(value)
}

fn push<T>(series: &mut VecDeque<T>, value: T, keep: usize) {
    series.push_back(value);
    while series.len() > keep {
        series.pop_front();
    }
}

fn demand_at(resource: &Resource, n: i64, random: &mut Random) -> f64 {
    let phase = 2.0 * PI * n.rem_euclid(96) as f64 / 96.0;
    let rise = resource.rise * resource.base * (n.rem_euclid(192) as f64 / 192.0);
    let value = resource.base
        + resource.swing * phase.sin()
        + rise * 0.25
        + resource.noise * (random.next() * 2.0 - 1.0) * 2.0;
    value.max(0.0)
}

impl State {
    fn new(now: f64) -> Self {
        let mut state = Self {
            services: services(),
            start: (now / STEP as f64).floor() as i64 * STEP,
            index: 0,
            random: Random(7),
            due: None,
        };
        // A day of history to begin with, the same on every launch.
        for service in &mut state.services {
            const WHO: [&str; 3] = ["Ines Varga", "Tomas Reyes", "Priya Nair"];
            const NOTES: [&str; 4] = [
                "Bump runtime to 0.31",
                "Raise stream buffer",
                "Retry budget per tenant",
                "Pin tokenizer build",
            ];
            service.deploy_log.push(Deploy {
                version: format!("v{}", service.version),
                at: -service.deployed,
                who: ["Tomas Reyes", "Priya Nair", "Ines Varga"][service.version as usize % 3],
                note: service.latest,
                status: "live",
            });
            service.deploy_log.extend((1..=4).map(|back: i64| Deploy {
                version: format!("v{}", service.version - back),
                at: -(back * 70 + 9),
                who: WHO[back as usize % 3],
                note: NOTES[back as usize % 4],
                status: "superseded",
            }));
        }
        for n in -(SAMPLES as i64 - 1)..=0 {
            state.sample(n);
        }
        state.random = Random(now.to_bits());
        state
    }

    /// The next sample, 15 minutes on.
    fn advance(&mut self) {
        self.index += 1;
        self.start += STEP;
        self.sample(self.index);
    }

    fn sample(&mut self, n: i64) {
        let random = &mut self.random;
        for service in &mut self.services {
            for resource in &mut service.resources {
                let previous = match resource.demand.back() {
                    Some(&last) => last,
                    None => demand_at(resource, n, random),
                };
                let next = previous * 0.55 + demand_at(resource, n, random) * 0.45;
                push(&mut resource.demand, next, SAMPLES);
            }
            let load = if service.status == "paused" {
                0.0
            } else {
                1.0
            };
            let daily = 0.75 + 0.25 * (2.0 * PI * n.rem_euclid(96) as f64 / 96.0).sin();
            let rps = load * service.rps as f64 * daily * (0.92 + random.next() * 0.16);
            push(&mut service.rps_history, rps as i64, SAMPLES);
            let p95 = load * service.p95 as f64 * (0.85 + random.next() * 0.35);
            push(&mut service.p95_history, p95 as i64, SAMPLES);
            if n > 0 {
                service.deployed += 1;
            }
            if service.rolling > 0 {
                service.rolling -= 1;
                if service.rolling == 0 {
                    service.status = "active";
                    service.version += 1;
                    service.deployed = 0;
                    for deploy in &mut service.deploy_log {
                        deploy.status = "superseded";
                    }
                    service.deploy_log.insert(0, Deploy {
                        version: format!("v{}", service.version),
                        at: n,
                        who: "Ines Varga",
                        note: "Rolled out from the console",
                        status: "live",
                    });
                }
            }
            if load > 0.0 {
                let kind = random.below(LOG_KINDS);
                let numbers = [(); 3].map(|()| random.below(9000) as i64 + 100);
                let (level, text) = log_line(kind, numbers, REGIONS[random.below(REGIONS.len())]);
                push(&mut service.logs, Log { at: n, level, text }, LOGS);
            }
        }
    }

    /// The time of sample `n`.
    fn time_of(&self, n: i64) -> i64 {
        self.start + (n - self.index) * STEP
    }

    fn when_of(&self, n: i64) -> String {
        let time = self.time_of(n);
        let day = if day_of(time) == day_of(self.start) {
            "TODAY"
        } else if day_of(time) == day_of(self.start - 86_400) {
            "YESTERDAY"
        } else {
            "EARLIER"
        };
        format!("{day}, {}", hhmm(time))
    }

    fn out(&self) -> String {
        let last = SAMPLES as i64 - 1;
        let times: Vec<String> = (0..=last)
            .map(|i| self.when_of(self.index - last + i))
            .collect();
        let axis: Vec<Value> = (0..=last)
            .step_by(12)
            .map(|i| {
                let text = if i == last {
                    "NOW".into()
                } else {
                    hhmm(self.start - (last - i) * STEP)
                };
                json!({ "index": i + 1, "text": text })
            })
            .collect();
        let services: Vec<Value> = self
            .services
            .iter()
            .map(|service| self.service_out(service))
            .collect();
        json!({
            "available": true,
            "sample": true,
            "now": hhmm(self.start),
            "times": times,
            "axis": axis,
            "services": services,
        })
        .to_string()
    }

    fn service_out(&self, service: &Service) -> Value {
        let mut resources = Map::new();
        let mut history = Map::new();
        for (key, resource) in KEYS.into_iter().zip(&service.resources) {
            let allocated = resource.allocated;
            let util: Vec<i64> = resource
                .demand
                .iter()
                .map(|demand| {
                    let util = if allocated != 0 {
                        100.0 * demand / allocated as f64
                    } else {
                        0.0
                    };
                    if util > 100.0 {
                        100
                    } else {
                        (util + 0.5) as i64
                    }
                })
                .collect();
            let used = resource
                .demand
                .back()
                .copied()
                .unwrap_or(0.0)
                .min(allocated as f64);
            let (label, limit, limit_unit) = match key {
                "gpu" => (
                    format!("{allocated}\u{d7} {}", resource.model),
                    allocated,
                    resource.unit,
                ),
                "memory" => (format!("{allocated} GB"), allocated * 1024, "MB"),
                _ => (format!("{allocated} CORES"), allocated, resource.unit),
            };
            resources.insert(
                key.into(),
                json!({
                    "allocated": allocated,
                    "used": number(tenths(used)),
                    "util": util.last(),
                    "unit": resource.unit,
                    "step": resource.step,
                    "min": resource.min,
                    "max": resource.max,
                    "label": label,
                    "limit": limit,
                    "limit_unit": limit_unit,
                }),
            );
            history.insert(key.into(), json!(util));
        }
        history.insert("rps".into(), json!(service.rps_history));
        history.insert("p95".into(), json!(service.p95_history));
        let logs: Vec<Value> = service
            .logs
            .iter()
            .rev()
            .map(|log| json!({ "time": hhmm(self.time_of(log.at)), "level": log.level, "text": log.text }))
            .collect();
        let deploys: Vec<Value> = service
            .deploy_log
            .iter()
            .take(5)
            .map(|deploy| {
                json!({
                    "version": deploy.version,
                    "when": ago(self.index - deploy.at),
                    "who": deploy.who,
                    "note": deploy.note,
                    "status": deploy.status,
                })
            })
            .collect();
        json!({
            "id": service.id,
            "name": service.id,
            "status": service.status,
            "tags": service.tags,
            "regions": service.regions,
            "description": service.description,
            "version": format!("v{}", service.version),
            "deployed": ago(service.deployed),
            "replicas": service.replicas,
            "rps": service.rps_history.back(),
            "p95": service.p95_history.back(),
            "errors": number(service.errors),
            "resources": resources,
            "history": history,
            "logs": logs,
            "deploys": deploys,
        })
    }

    fn receive(&mut self, message: &Value) {
        if let Some(limit) = message.get("limit").filter(|limit| limit.is_object()) {
            let Some(service) = self.find(limit.get("service")) else {
                return;
            };
            let Some(at) = KEYS
                .iter()
                .position(|key| limit.get("resource").and_then(Value::as_str) == Some(key))
            else {
                return;
            };
            let resource = &mut service.resources[at];
            let by = match limit.get("by") {
                Some(Value::String(text)) => text.trim().parse().unwrap_or(0.0),
                Some(value) => value.as_f64().unwrap_or(0.0),
                None => 0.0,
            };
            let next = resource.allocated
                + if by < 0.0 {
                    -resource.step
                } else {
                    resource.step
                };
            if (resource.min..=resource.max).contains(&next) {
                resource.allocated = next;
            }
        } else if let Some(deploy) = message
            .get("deploy")
            .filter(|deploy| !deploy.is_object() && !deploy.is_array())
        {
            let Some(service) = self.find(Some(deploy)) else {
                return;
            };
            if service.rolling > 0 {
                return;
            }
            service.status = "deploying";
            service.rolling = 3;
        }
    }

    fn find(&mut self, id: Option<&Value>) -> Option<&mut Service> {
        let id = match id? {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            _ => return None,
        };
        self.services.iter_mut().find(|service| service.id == id)
    }
}

impl Guest for Telemetry {
    fn start(_settings: String) -> Result<(), String> {
        STATE.with_borrow_mut(|state| *state = Some(State::new(now())));
        Ok(())
    }

    fn tick() -> Option<String> {
        STATE.with_borrow_mut(|state| {
            let state = state.as_mut()?;
            if state.due.is_some() {
                state.advance();
            }
            state.due = Some(now() + TICK);
            Some(state.out())
        })
    }

    fn receive(message: String) -> Option<String> {
        // Any message republishes, as the process provider did. The host
        // waits a whole interval again after a message, so samples that fell
        // due meanwhile are taken first.
        STATE.with_borrow_mut(|state| {
            let state = state.as_mut()?;
            while let Some(due) = state.due.filter(|due| now() >= *due) {
                state.advance();
                state.due = Some(due + TICK);
            }
            if let Ok(message) = serde_json::from_str::<Value>(&message) {
                state.receive(&message);
            }
            Some(state.out())
        })
    }
}

export!(Telemetry);
