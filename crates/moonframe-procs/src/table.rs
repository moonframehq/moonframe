//! What the monitor shows: the CPU readings so far, what the app asked for
//! (a query, a sort, open groups), and the processes grouped by name.

use {
    crate::read::{Proc, Reading},
    serde_json::{Map, Value, json},
    std::collections::{BTreeMap, BTreeSet, VecDeque},
};

/// Groups listed.
const MAX_GROUPS: usize = 80;
/// Members listed in an open group.
const MAX_CHILDREN: usize = 12;
/// CPU readings kept for `history`.
const HISTORY: usize = 60;

/// A number as JSON, a whole one without a fraction, as procs.py wrote it.
pub(crate) fn number(value: f64) -> Value {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 9.0e15 {
        json!(value as i64)
    } else {
        json!(value)
    }
}

/// To one decimal, rounding halves up, as procs.py did.
fn round1(value: f64) -> Value {
    number((value * 10.0 + 0.5).trunc() / 10.0)
}

/// A JSON scalar as text, whole numbers without a fraction.
fn as_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(found) => match found.as_f64() {
            Some(float) if float.is_finite() && float.fract() == 0.0 && float.abs() < 9.0e15 => {
                (float as i64).to_string()
            },
            _ => found.to_string(),
        },
        other => other.to_string(),
    }
}

/// A string or a number (not a bool): what a query or a name may be.
fn scalar(value: Option<&Value>) -> Option<&Value> {
    value.filter(|value| value.is_string() || value.is_number())
}

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Cpu,
    Memory,
}

impl Sort {
    fn name(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
        }
    }

    fn key(self, cpu: f64, mem: i64) -> f64 {
        match self {
            Self::Cpu => cpu,
            Self::Memory => mem as f64,
        }
    }
}

/// Processes of one name.
struct Group<'a> {
    name: &'a str,
    cpu: f64,
    mem: i64,
    members: Vec<&'a Proc>,
}

pub(crate) struct Monitor {
    /// As asked: a string, or a number kept as one.
    query: Value,
    sort: Sort,
    open: BTreeSet<String>,
    history: VecDeque<i64>,
    cpu: i64,
    user: i64,
    system: i64,
    load: [f64; 3],
}

impl Default for Monitor {
    fn default() -> Self {
        Self {
            query: Value::String(String::new()),
            sort: Sort::Cpu,
            open: BTreeSet::new(),
            history: VecDeque::new(),
            cpu: 0,
            user: 0,
            system: 0,
            load: [0.0; 3],
        }
    }
}

impl Monitor {
    /// A message from a mod: any of `query`, `sort` and `expand`.
    pub(crate) fn receive(&mut self, message: &Value) {
        let Some(message) = message.as_object() else {
            return;
        };
        if let Some(query) = scalar(message.get("query")) {
            self.query = match query.as_f64() {
                Some(float) if query.is_number() => number(float),
                _ => query.clone(),
            };
        }
        match message.get("sort").and_then(Value::as_str) {
            Some("cpu") => self.sort = Sort::Cpu,
            Some("memory") => self.sort = Sort::Memory,
            _ => {},
        }
        if let Some(name) = scalar(message.get("expand")) {
            let name = as_text(name);
            if !self.open.remove(&name) {
                self.open.insert(name);
            }
        }
    }

    /// A new `iostat` reading.
    pub(crate) fn read(&mut self, reading: Reading) {
        let busy = 100 - reading.idle;
        self.cpu = busy;
        self.user = reading.user;
        self.system = reading.system;
        self.load = reading.load;
        self.history.push_back(busy);
        while self.history.len() > HISTORY {
            self.history.pop_front();
        }
    }

    /// The groups to list, and how many processes match the query.
    fn groups(&self, procs: &[Proc]) -> (Vec<Value>, usize) {
        let query = as_text(&self.query).to_lowercase();
        let mut by: BTreeMap<&str, Group> = BTreeMap::new();
        let mut total = 0;
        for proc in procs {
            if !query.is_empty()
                && !proc.name.to_lowercase().contains(&query)
                && !proc.pid.to_string().contains(&query)
            {
                continue;
            }
            total += 1;
            let group = by.entry(&proc.name).or_insert_with(|| Group {
                name: &proc.name,
                cpu: 0.0,
                mem: 0,
                members: Vec::new(),
            });
            group.cpu += proc.cpu;
            group.mem = group.mem.saturating_add(proc.mem);
            group.members.push(proc);
        }
        let sort = self.sort;
        let mut ordered: Vec<Group> = by.into_values().collect();
        ordered.sort_by(|a, b| {
            sort.key(b.cpu, b.mem)
                .total_cmp(&sort.key(a.cpu, a.mem))
                .then_with(|| a.name.cmp(b.name))
        });
        ordered.truncate(MAX_GROUPS);
        let rows = ordered
            .into_iter()
            .map(|mut group| {
                // Stable: equals stay in `ps`'s order.
                group
                    .members
                    .sort_by(|a, b| sort.key(b.cpu, b.mem).total_cmp(&sort.key(a.cpu, a.mem)));
                let count = group.members.len();
                let open = count > 1 && self.open.contains(group.name);
                let mut row = Map::new();
                row.insert("name".into(), json!(group.name));
                row.insert("pid".into(), json!(group.members.first().map(|p| p.pid)));
                row.insert("count".into(), json!(count));
                row.insert("cpu".into(), round1(group.cpu));
                row.insert("mem".into(), json!(group.mem));
                row.insert("open".into(), json!(open));
                if open {
                    let children = group.members.iter().take(MAX_CHILDREN).map(|p| {
                        json!({ "pid": p.pid, "name": p.name, "cpu": round1(p.cpu), "mem": p.mem })
                    });
                    row.insert("children".into(), children.collect());
                }
                Value::Object(row)
            })
            .collect();
        (rows, total)
    }

    /// `app.procs`, with these processes and this `memory`.
    pub(crate) fn snapshot(&self, procs: &[Proc], memory: Value) -> Value {
        let (groups, total) = self.groups(procs);
        json!({
            "available": true,
            "cpu": self.cpu,
            "user": self.user,
            "system": self.system,
            "load": self.load.iter().map(|load| number(*load)).collect::<Vec<_>>(),
            "history": self.history,
            "memory": memory,
            "groups": groups,
            "total": total,
            "query": self.query,
            "sort": self.sort.name(),
        })
    }
}
