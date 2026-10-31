//! System load for the status bar: the host's samples, plus a CPU history
//! for sparklines, published as `app.system` once a second.

wit_bindgen::generate!({ path: "../../../wit/provider.wit", world: "provider" });

use {
    moonframe::provider::host,
    serde_json::{Value, json},
    std::{cell::RefCell, collections::VecDeque},
};

const DEFAULT_HISTORY: usize = 16;

struct System;

/// What the provider keeps between ticks.
struct State {
    history: VecDeque<f64>,
    length: usize,
}

thread_local! {
    static STATE: RefCell<State> = const {
        RefCell::new(State { history: VecDeque::new(), length: DEFAULT_HISTORY })
    };
}

impl Guest for System {
    fn start(settings: String) -> Result<(), String> {
        let settings: Value = serde_json::from_str(&settings).map_err(|error| error.to_string())?;
        let length = match settings.get("history") {
            None => DEFAULT_HISTORY,
            Some(value) => value
                .as_u64()
                .filter(|length| (1..=120).contains(length))
                .ok_or("`history` is a number of samples from 1 to 120")? as usize,
        };
        STATE.with_borrow_mut(|state| state.length = length);
        Ok(())
    }

    fn tick() -> Option<String> {
        let sample = match host::system_load() {
            Ok(sample) => sample,
            Err(problem) => {
                host::log(&problem);
                return None;
            },
        };
        let mut load: Value = serde_json::from_str(&sample).ok()?;
        let cpu = load.get("cpu")?.as_f64()?;
        let history = STATE.with_borrow_mut(|state| {
            if state.history.len() == state.length {
                state.history.pop_front();
            }
            state.history.push_back(cpu);
            json!(state.history)
        });
        load["cpu_history"] = history;
        Some(load.to_string())
    }

    fn receive(message: String) -> Option<String> {
        // `{ to = "provider:system", msg = "clear" }` empties the sparkline.
        if serde_json::from_str::<Value>(&message).ok()? == json!("clear") {
            STATE.with_borrow_mut(|state| state.history.clear());
        }
        None
    }
}

export!(System);
