//! Tandem's help desk, shared as `app.desk`: SAMPLE DATA from sample.json
//! beside this crate, built in, kept in memory (each launch starts over),
//! and changed by the app's messages. Publishes whenever it changes. Point
//! it at your help desk's API to make it real.
//!
//! Output (`app.desk`):
//!   assistant, workspace { team, company }, me { name, email }
//!   week      { total, share, first_reply, split = { { id, label, count } } }
//!   tickets   as in sample.json: { id, kind, priority, age, minutes, title,
//!             requester { name, team }, source, status, status_label, mine,
//!             created, device, summary, sla, watchers, check?, earlier?, thread }
//!   counts    { all, open, mine, waiting }
//!
//! Messages (one key each):
//!   watch = id                 a ticket is on screen: a check step still
//!                              running finishes a few seconds later
//!   act = id                   run the check's suggestion (its primary button)
//!   reply = { id, text, internal }  add a reply, or an internal note, to the thread
//!   new = true                 open a blank ticket at the top of the inbox

wit_bindgen::generate!({ path: "wit/provider.wit", world: "provider" });

use {
    serde_json::{Value, json},
    std::{
        cell::RefCell,
        time::{Duration, Instant},
    },
};

const SAMPLE: &str = include_str!("../sample.json");
const FIRST_NUMBER: u64 = 1240;
const REPLY_LIMIT: usize = 2000;

struct Desk;

/// Something that finishes later: a running step of a ticket's check, either
/// one the check was already running or the suggestion `act` started.
enum Finish {
    Step { ticket: String, step: usize },
    Act { ticket: String, step: usize },
}

struct Timer {
    at: Instant,
    finish: Finish,
}

/// What the provider keeps between calls.
struct State {
    desk: Value,
    next_number: u64,
    timers: Vec<Timer>,
    /// The JSON published last, so only changes are published.
    last: String,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// True unless null, false, 0, "" or "0".
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(number)) => number.as_f64() != Some(0.0),
        Some(Value::String(text)) => !text.is_empty() && text != "0",
        Some(_) => true,
    }
}

/// A plain value as text: a string or a number.
fn scalar(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

impl State {
    fn new() -> Result<Self, String> {
        let desk = serde_json::from_str(SAMPLE).map_err(|error| error.to_string())?;
        Ok(Self {
            desk,
            next_number: FIRST_NUMBER,
            timers: Vec::new(),
            last: String::new(),
        })
    }

    fn tickets(&mut self) -> &mut Vec<Value> {
        if !self.desk["tickets"].is_array() {
            self.desk["tickets"] = json!([]);
        }
        self.desk["tickets"]
            .as_array_mut()
            .expect("tickets is an array")
    }

    fn find(&mut self, id: Option<&Value>) -> Option<&mut Value> {
        let id = scalar(id)?;
        self.tickets()
            .iter_mut()
            .find(|ticket| scalar(ticket.get("id")).as_deref() == Some(&id))
    }

    fn later(&mut self, seconds: u64, finish: Finish) {
        let at = Instant::now() + Duration::from_secs(seconds);
        self.timers.push(Timer { at, finish });
    }

    /// Finish each running step of a ticket's check, one after the other.
    fn watch(&mut self, id: &str) {
        let Some(check) = self
            .find(Some(&json!(id)))
            .and_then(|ticket| ticket.get_mut("check"))
        else {
            return;
        };
        if !truthy(Some(check)) {
            return;
        }
        let watched = check.get("watched").and_then(Value::as_u64);
        check["watched"] = json!(watched.unwrap_or(0) + 1);
        if watched.is_some_and(|count| count > 0) {
            return;
        }
        let running: Vec<usize> = check["steps"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter(|(_, step)| step["state"] == "running")
            .map(|(index, _)| index)
            .collect();
        for (turn, step) in (1..).zip(running) {
            self.later(12 * turn, Finish::Step {
                ticket: id.to_string(),
                step,
            });
        }
    }

    /// Run the suggestion: a new step that runs, then the assistant says so.
    fn act(&mut self, id: &str) {
        let Some(ticket) = self.find(Some(&json!(id))) else {
            return;
        };
        let Some(check) = ticket.get_mut("check").filter(|check| truthy(Some(check))) else {
            return;
        };
        if truthy(check.get("acting")) || truthy(check.get("acted")) {
            return;
        }
        check["acting"] = Value::Bool(true);
        let label = format!("{}…", check["primary"].as_str().unwrap_or_default());
        if !check["steps"].is_array() {
            check["steps"] = json!([]);
        }
        let steps = check["steps"].as_array_mut().expect("steps is an array");
        steps.push(json!({ "label": label, "state": "running" }));
        let step = steps.len() - 1;
        ticket["status"] = json!("progress");
        ticket["status_label"] = json!("In progress");
        self.later(4, Finish::Act {
            ticket: id.to_string(),
            step,
        });
    }

    fn finish(&mut self, finish: Finish) {
        match finish {
            Finish::Step { ticket, step } => {
                let Some(step) = self
                    .find(Some(&json!(ticket)))
                    .and_then(|ticket| ticket["check"]["steps"].get_mut(step))
                else {
                    return;
                };
                step["state"] = json!("done");
                if truthy(step.get("done_label")) {
                    step["label"] = step["done_label"].clone();
                }
            },
            Finish::Act { ticket, step } => {
                let Some(ticket) = self.find(Some(&json!(ticket))) else {
                    return;
                };
                let name = ticket["requester"]["name"].as_str().unwrap_or_default();
                let name = name.split(' ').next().unwrap_or_default().to_string();
                let check = &mut ticket["check"];
                let primary = check["primary"].as_str().unwrap_or_default().to_string();
                if let Some(step) = check["steps"].get_mut(step) {
                    step["state"] = json!("done");
                    step["label"] = json!(format!("{primary}: done"));
                }
                if let Some(check) = check.as_object_mut() {
                    check.remove("acting");
                }
                check["acted"] = Value::Bool(true);
                let text = format!(
                    "All done, {name}. I ran \"{}\" for you through {}. Let me know if things still feel off and I'll bring in the IT team.",
                    check["suggestion"].as_str().unwrap_or_default(),
                    check["via"].as_str().unwrap_or_default(),
                );
                say_in(ticket, text);
            },
        }
    }

    fn receive(&mut self, message: &Value) {
        let Some(message) = message.as_object() else {
            return;
        };
        if let Some(id) = message
            .get("watch")
            .filter(|_| self.find(message.get("watch")).is_some())
        {
            self.watch(&scalar(Some(id)).unwrap_or_default());
        } else if let Some(id) = message
            .get("act")
            .filter(|_| self.find(message.get("act")).is_some())
        {
            self.act(&scalar(Some(id)).unwrap_or_default());
        } else if let Some(reply) = message
            .get("reply")
            .filter(|reply| reply.is_object() && self.find(reply.get("id")).is_some())
        {
            let Some(text) = scalar(reply.get("text")) else {
                return;
            };
            if text.chars().all(char::is_whitespace) {
                return;
            }
            let entry = json!({
                "from": if truthy(reply.get("internal")) { "note" } else { "agent" },
                "name": self.desk["me"]["name"],
                "age": "just now",
                "text": text.chars().take(REPLY_LIMIT).collect::<String>(),
            });
            if let Some(ticket) = self.find(reply.get("id")) {
                push_thread(ticket, entry);
            }
        } else if truthy(message.get("new")) {
            let id = format!("INC-{}", self.next_number);
            self.next_number += 1;
            let me = self.desk["me"]["name"].clone();
            let ticket = json!({
                "id": id, "kind": "Incident", "priority": "Low", "age": "now", "minutes": 0,
                "title": "New ticket", "requester": { "name": me, "team": "IT" },
                "source": "web", "status": "open", "status_label": "Open", "mine": true,
                "created": "just now", "device": { "name": "Not set", "detail": "" },
                "summary": "A blank ticket opened from the inbox. Describe the problem in a reply.",
                "sla": [
                    { "left": "1h 00m", "label": "First response", "due": "in 1 hour", "used": 0, "urgent": false },
                    { "left": "8h 00m", "label": "Resolution", "due": "in 8 hours", "used": 0, "urgent": false },
                ],
                "watchers": [me], "thread": [],
            });
            self.tickets().insert(0, ticket);
        }
    }

    /// Runs what is due, then the desk to publish if it changed.
    fn update(&mut self) -> Option<String> {
        let now = Instant::now();
        let (due, waiting) = std::mem::take(&mut self.timers)
            .into_iter()
            .partition(|timer| timer.at <= now);
        self.timers = waiting;
        for timer in due {
            self.finish(timer.finish);
        }
        let text = self.out().to_string();
        if text == self.last {
            return None;
        }
        self.last.clone_from(&text);
        Some(text)
    }

    fn out(&mut self) -> Value {
        let (mut all, mut open, mut mine, mut waiting) = (0, 0, 0, 0);
        for ticket in self.tickets().iter() {
            let status = scalar(ticket.get("status")).unwrap_or_default();
            all += 1;
            if status != "resolved" {
                open += 1;
                if truthy(ticket.get("mine")) {
                    mine += 1;
                }
            }
            if status == "waiting" {
                waiting += 1;
            }
        }
        let mut out = self.desk.clone();
        out["available"] = Value::Bool(true);
        out["counts"] = json!({ "all": all, "open": open, "mine": mine, "waiting": waiting });
        out
    }
}

fn push_thread(ticket: &mut Value, entry: Value) {
    if !ticket["thread"].is_array() {
        ticket["thread"] = json!([]);
    }
    if let Some(thread) = ticket["thread"].as_array_mut() {
        thread.push(entry);
    }
}

fn say_in(ticket: &mut Value, text: String) {
    push_thread(
        ticket,
        json!({ "from": "assistant", "age": "just now", "text": text }),
    );
}

impl Guest for Desk {
    fn start(_settings: String) -> Result<(), String> {
        let state = State::new()?;
        STATE.with_borrow_mut(|slot| *slot = Some(state));
        Ok(())
    }

    fn tick() -> Option<String> {
        STATE.with_borrow_mut(|state| state.as_mut()?.update())
    }

    fn receive(message: String) -> Option<String> {
        let message: Value = serde_json::from_str(&message).ok()?;
        STATE.with_borrow_mut(|state| {
            let state = state.as_mut()?;
            state.receive(&message);
            state.update()
        })
    }
}

export!(Desk);
