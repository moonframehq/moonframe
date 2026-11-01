//! Vigil's workflow operations, shared as `app.vigil`: SAMPLE DATA. The
//! figures for each time range come from sample.json beside this crate,
//! built in; the live feed is simulated here, once a second (the provider's
//! `interval`): a run starts every few seconds, running ones finish (now and
//! then with an error), and finished runs count towards the success figures.
//! Point it at your orchestrator's API to make it real.
//!
//! Messages (from any mod): `{ range = "1H" | "4H" | "24H" | "72H" | "7D" |
//! "30D" }` switches the figures; `{ rerun = <run id> }` starts that run's
//! workflow again.

wit_bindgen::generate!({ path: "wit/provider.wit", world: "provider" });

use {
    serde_json::{Value, json},
    std::{
        cell::RefCell,
        collections::HashMap,
        time::{SystemTime, UNIX_EPOCH},
    },
};

const SAMPLE: &str = include_str!("../sample.json");
/// Runs kept in the live feed.
const FEED: usize = 9;
/// Workflows shown in the upcoming schedule.
const UPCOMING: usize = 6;
/// Seconds between steps of the simulation: the provider's `interval`.
const TICK: f64 = 1.0;

struct Vigil;

struct Workflow {
    name: String,
    every: Value,
    period: i64,
    error: String,
    fails: f64,
}

struct Run {
    id: String,
    workflow: String,
    started: f64,
    duration: f64,
    target: f64,
    status: String,
    error: String,
    retried: bool,
}

/// What the provider keeps between ticks.
struct State {
    sample: Value,
    workflows: Vec<Workflow>,
    range: String,
    /// The figures of the chosen range, copied so live runs can add to them.
    figures: Value,
    /// The live feed, newest first.
    runs: Vec<Run>,
    serial: u64,
    next_start: f64,
    tick: u64,
    random: Random,
    /// When the next second of the simulation is due, once the first state
    /// is published.
    due: Option<f64>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// A small, seedable generator (SplitMix64), seeded the same on every
/// launch, as the simulation always was.
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
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64())
}

fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// A number as JSON: whole values as integers, the rest as floats.
fn number(value: f64) -> Value {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        json!(value as i64)
    } else {
        json!(value)
    }
}

/// A value rounded to three decimals.
fn thousandths(value: f64) -> f64 {
    format!("{value:.3}").parse().unwrap_or(value)
}

fn bump(value: &mut Value) {
    *value = json!(value.as_i64().unwrap_or(0) + 1);
}

/// 1444 as "1 444" (a thin space), as the design groups thousands; below a
/// thousand the number stays a number.
fn grouped(value: &Value) -> Value {
    let Some(n) = value.as_i64() else {
        return value.clone();
    };
    if n.abs() < 1000 {
        return json!(n);
    }
    let digits = n.unsigned_abs().to_string();
    let mut out = String::from(if n < 0 {
        "-"
    } else {
        ""
    });
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at) % 3 == 0 {
            out.push('\u{2009}');
        }
        out.push(digit);
    }
    json!(out)
}

fn ago(seconds: f64) -> String {
    let seconds = seconds as i64;
    if seconds < 60 {
        return format!("{seconds}s ago");
    }
    let (m, s) = (seconds / 60, seconds % 60);
    if m < 60 {
        return format!("{m}m {s}s ago");
    }
    format!("{}h {}m ago", m / 60, m % 60)
}

fn until(seconds: i64) -> String {
    if seconds < 60 {
        return format!("in {seconds}s");
    }
    let (m, s) = (seconds / 60, seconds % 60);
    if m < 60 {
        return format!("in {m}m {s}s");
    }
    format!("in {}h {}m", m / 60, m % 60)
}

fn last_point(series: &Value) -> f64 {
    series["points"]
        .as_array()
        .and_then(|points| points.last())
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

impl State {
    fn new(now: f64) -> Result<Self, String> {
        let sample: Value =
            serde_json::from_str(SAMPLE).map_err(|error| format!("sample.json: {error}"))?;
        let workflows = sample["workflows"]
            .as_array()
            .ok_or("sample.json has no workflows")?
            .iter()
            .map(|flow| Workflow {
                name: text(&flow["name"]),
                every: flow["every"].clone(),
                period: flow["period"]
                    .as_i64()
                    .filter(|period| *period > 0)
                    .unwrap_or(1),
                error: text(&flow["error"]),
                fails: flow["fails"].as_f64().unwrap_or(0.0),
            })
            .collect();
        let mut state = Self {
            sample,
            workflows,
            range: String::new(),
            figures: Value::Null,
            runs: Vec::new(),
            serial: 0,
            next_start: now + 6.0,
            tick: 0,
            random: Random(42),
            due: None,
        };
        state.choose("1H");
        let history = state.sample["history"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for past in history {
            let workflow = text(&past["workflow"]);
            let status = text(&past["status"]);
            let duration = past["duration"].as_f64().unwrap_or(0.0);
            let error = match state.workflow(&workflow) {
                Some(flow) if status == "failed" => flow.error.clone(),
                _ => String::new(),
            };
            state.serial += 1;
            state.runs.push(Run {
                id: format!("r{}", state.serial),
                workflow,
                started: now - past["age"].as_f64().unwrap_or(0.0),
                duration,
                target: past["target"].as_f64().unwrap_or(duration),
                status,
                error,
                retried: false,
            });
        }
        Ok(state)
    }

    fn workflow(&self, name: &str) -> Option<&Workflow> {
        self.workflows.iter().find(|flow| flow.name == name)
    }

    fn choose(&mut self, name: &str) {
        if let Some(figures) = self.sample["ranges"].get(name) {
            self.figures = figures.clone();
            self.range = name.into();
        }
    }

    fn start_run(&mut self, workflow: String, at: f64) {
        self.serial += 1;
        let target = 4.0 + self.random.next() * 55.0;
        self.runs.insert(0, Run {
            id: format!("r{}", self.serial),
            workflow,
            started: at,
            duration: 0.0,
            target,
            status: "running".into(),
            error: String::new(),
            retried: false,
        });
        self.runs.truncate(FEED);
    }

    fn finish(&mut self, at: usize) {
        let (fails, error) = self
            .workflow(&self.runs[at].workflow)
            .map_or((0.0, String::new()), |flow| {
                (flow.fails, flow.error.clone())
            });
        let failed = self.random.next() < fails;
        let run = &mut self.runs[at];
        run.status = if failed {
            "failed"
        } else {
            "success"
        }
        .into();
        run.duration = run.target;
        run.error = if failed {
            error
        } else {
            String::new()
        };
        bump(
            &mut self.figures["success"][if failed {
                "errors"
            } else {
                "success"
            }],
        );
        let name = run.workflow.clone();
        if let Some(rows) = self.figures["errored"].as_array_mut() {
            for row in rows
                .iter_mut()
                .filter(|row| row["name"].as_str() == Some(&name))
            {
                bump(&mut row["runs"]);
                if failed {
                    bump(&mut row["failed"]);
                    bump(&mut row["count"]);
                }
            }
        }
    }

    /// A small swing of a series' latest point around the sample's own
    /// value, so the charts breathe without drifting away.
    fn wobble(&mut self, series: &str, step: f64) {
        let base = last_point(&self.sample["ranges"][&self.range][series]);
        let swing = (self.random.next() * 2.0 - 1.0) * step;
        if let Some(last) = self.figures[series]["points"]
            .as_array_mut()
            .and_then(|points| points.last_mut())
        {
            *last = number(thousandths(base + swing));
        }
    }

    /// One second of the simulation.
    fn step(&mut self) {
        self.tick += 1;
        let at = now();
        for index in 0..self.runs.len() {
            let run = &self.runs[index];
            if run.status == "running" && at - run.started >= run.target {
                self.finish(index);
            }
        }
        if at >= self.next_start && !self.workflows.is_empty() {
            let pick = (self.random.next() * self.workflows.len() as f64) as usize;
            self.start_run(self.workflows[pick].name.clone(), at);
            self.next_start = at + 5.0 + self.random.next() * 6.0;
        }
        if self.tick.is_multiple_of(3) {
            self.wobble("latency", 0.08);
            self.wobble("sla", 0.2);
            self.wobble("lag", 8.0);
        }
    }

    fn receive(&mut self, message: &Value) {
        if !message.is_object() {
            return;
        }
        if let Some(name) = message["range"].as_str() {
            self.choose(name);
        }
        let id = match &message["rerun"] {
            Value::String(id) => id.clone(),
            Value::Number(id) => id.to_string(),
            _ => return,
        };
        if let Some(run) = self
            .runs
            .iter_mut()
            .find(|run| run.id == id && !run.retried)
        {
            run.retried = true;
            let workflow = run.workflow.clone();
            self.start_run(workflow, now());
        }
    }

    fn publish(&self) -> String {
        let at = now();
        let figures = &self.figures;
        let (sla, latency, lag) = (&figures["sla"], &figures["latency"], &figures["lag"]);
        let ok = figures["success"]["success"].as_f64().unwrap_or(0.0);
        let bad = figures["success"]["errors"].as_f64().unwrap_or(0.0);
        let rate = if ok + bad > 0.0 {
            100.0 * ok / (ok + bad)
        } else {
            100.0
        };
        let empty = Vec::new();
        let errored = figures["errored"].as_array().unwrap_or(&empty);
        let most = errored
            .iter()
            .filter_map(|row| row["count"].as_f64())
            .fold(0.0, f64::max);

        let feed: Vec<Value> = self
            .runs
            .iter()
            .map(|run| {
                let running = run.status == "running";
                json!({
                    "id": run.id,
                    "started": ago(at - run.started),
                    "workflow": run.workflow,
                    "duration": format!("{:.1}s", if running { at - run.started } else { run.duration }),
                    "status": run.status,
                    "error": run.error,
                    "retried": run.retried,
                })
            })
            .collect();

        // The latest finished run of each workflow.
        let mut last: HashMap<&str, &str> = HashMap::new();
        for run in self.runs.iter().filter(|run| run.status != "running") {
            last.entry(&run.workflow).or_insert(&run.status);
        }
        let mut upcoming: Vec<(i64, Value)> = self
            .workflows
            .iter()
            .map(|flow| {
                let offset = flow.name.chars().count() as i64 * 37;
                let left = flow.period - (at as i64 + offset) % flow.period;
                (
                    left,
                    json!({
                        "id": flow.name,
                        "workflow": flow.name,
                        "every": flow.every,
                        "next": until(left),
                        "seconds": left,
                        "last": last.get(flow.name.as_str()).copied().unwrap_or("success"),
                    }),
                )
            })
            .collect();
        upcoming.sort_by_key(|(left, _)| *left);
        let upcoming: Vec<Value> = upcoming
            .into_iter()
            .take(UPCOMING)
            .map(|(_, row)| row)
            .collect();

        // The bar reads against a 97% floor, so a fraction of a percent still
        // shows: 98.7% fills a little over half.
        let share = ((rate - 97.0) / 3.0).clamp(0.05, 0.95);
        let active = figures["workflows"]["active"].as_i64().unwrap_or(0);
        let paused = figures["workflows"]["paused"].as_i64().unwrap_or(0);
        json!({
            "available": true,
            "note": "Sample data, simulated live.",
            "range": self.range,
            "tick": self.tick,
            "sla": {
                "value": format!("{}%", sla["value"].as_f64().unwrap_or(0.0) as i64),
                "latency": sla["latency"],
                "freshness": sla["freshness"],
                "points": sla["points"],
            },
            "latency": {
                "value": format!("{:.1}s", last_point(latency)),
                "p99": latency["p99"],
                "p95": latency["p95"],
                "points": latency["points"],
            },
            "lag": {
                "value": format!("{}s", text(&lag["value"])),
                "p99": lag["p99"],
                "p95": lag["p95"],
                "points": lag["points"],
            },
            "workflows": { "total": active + paused, "active": active, "paused": paused },
            "success": {
                "rate": format!("{rate:.1}%"),
                "success": grouped(&figures["success"]["success"]),
                "errors": grouped(&figures["success"]["errors"]),
                "share": number(thousandths(share)),
            },
            "errored": errored
                .iter()
                .map(|row| {
                    let count = row["count"].as_f64().unwrap_or(0.0);
                    json!({
                        "name": row["name"],
                        "kind": row["kind"],
                        "runs": grouped(&row["runs"]),
                        "failed": grouped(&row["failed"]),
                        "count": grouped(&row["count"]),
                        "share": if most > 0.0 { number(thousandths(count / most)) } else { json!(0) },
                    })
                })
                .collect::<Vec<_>>(),
            "feed": feed,
            "upcoming": upcoming,
        })
        .to_string()
    }
}

impl Guest for Vigil {
    fn start(_settings: String) -> Result<(), String> {
        let state = State::new(now())?;
        STATE.with_borrow_mut(|slot| *slot = Some(state));
        Ok(())
    }

    fn tick() -> Option<String> {
        STATE.with_borrow_mut(|state| {
            let state = state.as_mut()?;
            if state.due.is_some() {
                state.step();
            }
            state.due = Some(now() + TICK);
            Some(state.publish())
        })
    }

    fn receive(message: String) -> Option<String> {
        // Any message republishes, as the process provider did. The host
        // waits a whole interval again after a message, so steps that fell
        // due meanwhile are taken first.
        STATE.with_borrow_mut(|state| {
            let state = state.as_mut()?;
            while let Some(due) = state.due.filter(|due| now() >= *due) {
                state.step();
                state.due = Some(due + TICK);
            }
            if let Ok(message) = serde_json::from_str::<Value>(&message) {
                state.receive(&message);
            }
            Some(state.publish())
        })
    }
}

export!(Vigil);
