//! Sorrel's research run, shared as `app.research`: SAMPLE DATA from
//! sample.json beside this crate, built in, and a SCRIPTED DEMO of an agent
//! working through its plan, played on a clock (one tick every `interval`,
//! 0.4 s). It is not a model: a real agent would publish its plan, progress
//! and cells the same way.
//!
//! Output: everything in sample.json, plus `run`:
//!   tick, playing, done
//!   step      1 to 4, or 5 once done
//!   found     apps found so far (step 1 adds the sheet's rows one by one)
//!   row, cell the cell being written in step 2 (cell 1 to 4: APY, monthly
//!             fee, minimum deposit, rating), else 0
//!   filled    cells written so far; reviewed: apps reviewed in step 3
//!   elapsed   "2m 14s"; left: "about 2 min left"
//!   used      credits this run used; estimate: what it should use
//!
//! `start_tick` in the `[settings]` of provider.toml starts the run at that
//! tick (for stills: 61 is step 2, row 10, the monthly fee being typed).
//!
//! Messages (any mod): "toggle" pauses or plays (and replays once done),
//! "restart" starts over.

wit_bindgen::generate!({ path: "wit/provider.wit", world: "provider" });

use {
    serde_json::{Value, json},
    std::cell::RefCell,
};

const SAMPLE: &str = include_str!("../sample.json");
/// Cells written per app: APY, monthly fee, minimum deposit, rating.
const CELLS: u64 = 4;
/// Ticks per app in step 2: its cells, and a beat between rows.
const PER_ROW: u64 = CELLS + 1;
/// Demo seconds per tick, for the clock.
const PACE: f64 = 1.84;
const ESTIMATE: u64 = 120;

struct Research;

/// Where each step of the demo starts, in ticks, for a sheet of `apps` rows.
struct Script {
    apps: u64,
    collect: u64,
    review: u64,
    summary: u64,
    done: u64,
    /// The demo plays again from the start.
    replay: u64,
}

impl Script {
    fn new(apps: u64) -> Self {
        let collect = apps + 2;
        let review = collect + PER_ROW * apps;
        let summary = review + apps + 2;
        let done = summary + 12;
        Self {
            apps,
            collect,
            review,
            summary,
            done,
            replay: done + 30,
        }
    }
}

/// What the provider keeps between calls.
struct State {
    data: Value,
    script: Script,
    tick: u64,
    playing: bool,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

impl State {
    fn run(&self) -> Value {
        let Script {
            apps,
            collect,
            review,
            summary,
            done,
            ..
        } = self.script;
        let t = self.tick.min(done);
        let (mut step, mut row, mut cell, mut filled, mut reviewed) = (1, 0, 0, 0, 0);
        let found = t.min(apps);
        if t >= collect {
            step = 2;
            let k = t - collect;
            let index = k / PER_ROW;
            if index < apps {
                row = index + 1;
                let at = k % PER_ROW;
                cell = if at < CELLS {
                    at + 1
                } else {
                    0
                };
                filled = index * CELLS
                    + if cell > 0 {
                        cell - 1
                    } else {
                        CELLS
                    };
            } else {
                filled = apps * CELLS;
            }
        }
        if t >= review {
            step = 3;
            reviewed = (t - review).min(apps);
        }
        if t >= summary {
            step = 4;
        }
        if t >= done {
            step = 5;
        }
        let seconds = 20 + (t as f64 * PACE).floor() as u64;
        let left = (done - t) as f64 * PACE;
        let used = 30
            + found
            + (filled as f64 * 1.15).floor() as u64
            + reviewed / 2
            + if t >= summary {
                (t - summary) / 2
            } else {
                0
            };
        let left = if step == 5 {
            "done".to_string()
        } else if left < 60.0 {
            "under a minute left".to_string()
        } else {
            format!("about {} min left", (left / 60.0).ceil())
        };
        json!({
            "tick": self.tick, "playing": self.playing, "done": step == 5,
            "step": step, "found": found, "row": row, "cell": cell, "filled": filled,
            "reviewed": reviewed, "used": used, "estimate": ESTIMATE,
            "elapsed": format!("{}m {:02}s", seconds / 60, seconds % 60),
            "left": left,
        })
    }

    fn publish(&mut self) -> String {
        self.data["run"] = self.run();
        self.data.to_string()
    }
}

impl Guest for Research {
    fn start(settings: String) -> Result<(), String> {
        let settings: Value = serde_json::from_str(&settings).map_err(|error| error.to_string())?;
        let mut data: Value = serde_json::from_str(SAMPLE).map_err(|error| error.to_string())?;
        data["available"] = Value::Bool(true);
        let apps = data["apps"].as_array().map_or(0, Vec::len) as u64;
        let script = Script::new(apps);
        let tick = match settings.get("start_tick") {
            None => 0,
            Some(value) => value
                .as_u64()
                .filter(|tick| *tick < script.replay)
                .ok_or(format!(
                    "`start_tick` is a tick from 0 to {}",
                    script.replay - 1
                ))?,
        };
        STATE.with_borrow_mut(|state| {
            *state = Some(State {
                data,
                script,
                tick,
                playing: true,
            })
        });
        Ok(())
    }

    fn tick() -> Option<String> {
        STATE.with_borrow_mut(|state| {
            let state = state.as_mut()?;
            if !state.playing {
                return None;
            }
            // The first call publishes where the run starts; each after it
            // plays a tick.
            if state.data.get("run").is_some() {
                state.tick = if state.tick + 1 >= state.script.replay {
                    0
                } else {
                    state.tick + 1
                };
            }
            Some(state.publish())
        })
    }

    fn receive(message: String) -> Option<String> {
        let message: Value = serde_json::from_str(&message).ok()?;
        STATE.with_borrow_mut(|state| {
            let state = state.as_mut()?;
            match message.as_str()? {
                "toggle" if state.tick >= state.script.done => {
                    state.tick = 0;
                    state.playing = true;
                },
                "toggle" => state.playing = !state.playing,
                "restart" => {
                    state.tick = 0;
                    state.playing = true;
                },
                _ => return None,
            }
            Some(state.publish())
        })
    }
}

export!(Research);
