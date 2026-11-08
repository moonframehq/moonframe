//! The sessions found, grouped by repository, and the one opened.

use {
    crate::{
        meta::{Meta, meta_of},
        sample,
        text::{age, as_text, basename, project_of, truthy},
        transcript,
    },
    serde_json::{Value, json},
    std::{
        collections::{HashMap, HashSet},
        fs,
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// Sessions listed.
pub(crate) const MOST: usize = 60;

#[derive(Clone, Debug)]
pub(crate) struct Session {
    pub(crate) id: String,
    pub(crate) title: Value,
    pub(crate) branch: String,
    pub(crate) age: String,
    pub(crate) pr: bool,
    pub(crate) gone: bool,
    /// Its transcript; sample sessions have none.
    pub(crate) path: Option<String>,
}

impl Session {
    fn json(&self) -> Value {
        json!({
            "id": self.id,
            "title": self.title,
            "branch": self.branch,
            "age": self.age,
            "pr": self.pr,
            "gone": self.gone,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Group {
    pub(crate) name: String,
    /// The folder of its latest session, "" when unknown.
    pub(crate) path: String,
    pub(crate) sessions: Vec<Session>,
}

/// A transcript's file as last looked at, and what it said.
struct Cached {
    mtime: i64,
    size: u64,
    meta: Option<Meta>,
}

struct Found {
    path: String,
    dir: String,
    mtime: i64,
    size: u64,
}

pub(crate) struct Sessions {
    /// `~/.claude/projects`, with forward slashes.
    root: String,
    sample: bool,
    cache: HashMap<String, Cached>,
    opened: Option<String>,
    total: Option<usize>,
    groups: Vec<Group>,
    /// The transcript last read, by session and mtime.
    shown: Option<(String, Option<Value>)>,
    stopped: Arc<AtomicBool>,
}

/// Seconds since 1970.
pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}

/// A file's modification time, in whole seconds.
fn mtime(info: &fs::Metadata) -> i64 {
    info.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}

/// Whether `id` may name a session: what the CLI's ids and the samples are.
fn valid_id(id: &str) -> bool {
    (1..=80).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

impl Sessions {
    pub(crate) fn new(root: String, sample: bool, stopped: Arc<AtomicBool>) -> Self {
        Self {
            root,
            sample,
            cache: HashMap::new(),
            opened: None,
            total: None,
            groups: Vec::new(),
            shown: None,
            stopped,
        }
    }

    fn stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    /// Every transcript under the root, newest first.
    fn found(&self) -> Option<Vec<Found>> {
        let folders = fs::read_dir(&self.root).ok()?;
        let mut files = Vec::new();
        for folder in folders.flatten() {
            let Some(dir) = folder.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if dir.starts_with('.') {
                continue;
            }
            let Ok(names) = fs::read_dir(format!("{}/{dir}", self.root)) else {
                continue;
            };
            for name in names.flatten() {
                let Some(name) = name.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                if !name.ends_with(".jsonl") {
                    continue;
                }
                let path = format!("{}/{dir}/{name}", self.root);
                let Ok(info) = fs::metadata(&path) else {
                    continue;
                };
                files.push(Found {
                    path,
                    dir: dir.clone(),
                    mtime: mtime(&info),
                    size: info.len(),
                });
            }
        }
        files.sort_by(|a, b| b.mtime.cmp(&a.mtime).then_with(|| a.path.cmp(&b.path)));
        Some(files)
    }

    /// Looks again at the sessions on disk.
    pub(crate) fn scan(&mut self) {
        if self.sample {
            self.groups = sample::groups();
            self.total = Some(sample::TOTAL);
            return;
        }
        let Some(files) = self.found() else {
            return;
        };
        let now = now();
        let mut groups: Vec<Group> = Vec::new();
        let mut listed = 0;
        for file in &files {
            if listed >= MOST || self.stopped() {
                break;
            }
            let fresh = self
                .cache
                .get(&file.path)
                .is_some_and(|hit| hit.mtime == file.mtime && hit.size == file.size);
            if !fresh {
                let meta = meta_of(Path::new(&file.path));
                self.cache.insert(file.path.clone(), Cached {
                    mtime: file.mtime,
                    size: file.size,
                    meta,
                });
            }
            let Some(meta) = self.cache.get(&file.path).and_then(|hit| hit.meta.as_ref()) else {
                continue;
            };
            let name = project_of(meta.cwd.as_deref(), &file.dir);
            let at = match groups.iter().position(|group| group.name == name) {
                Some(at) => at,
                None => {
                    groups.push(Group {
                        name,
                        path: meta.cwd.clone().unwrap_or_default(),
                        sessions: Vec::new(),
                    });
                    groups.len() - 1
                },
            };
            let base = basename(&file.path);
            let id = base.strip_suffix(".jsonl").filter(|id| !id.is_empty());
            groups[at].sessions.push(Session {
                id: id.unwrap_or(base).to_owned(),
                title: meta.title.clone(),
                branch: meta.branch.clone().unwrap_or_default(),
                age: age(file.mtime, now),
                pr: meta.pr,
                gone: meta
                    .cwd
                    .as_ref()
                    .is_some_and(|cwd| !Path::new(cwd).is_dir()),
                path: Some(file.path.clone()),
            });
            listed += 1;
        }
        if self.stopped() {
            return;
        }
        // Only what is still there is kept.
        let there: HashSet<&str> = files.iter().map(|file| file.path.as_str()).collect();
        self.cache.retain(|path, _| there.contains(path.as_str()));
        self.total = Some(files.len());
        self.groups = groups;
    }

    pub(crate) fn find_session(&self, wanted: &str) -> Option<(&Session, &Group)> {
        self.groups.iter().find_map(|group| {
            let session = group.sessions.iter().find(|session| session.id == wanted)?;
            Some((session, group))
        })
    }

    /// A message from a mod: `{ open = id }`, `{ close = true }`,
    /// `{ refresh = true }`.
    pub(crate) fn receive(&mut self, message: &Value) {
        let Some(message) = message.as_object() else {
            return;
        };
        let wanted = match message.get("open") {
            Some(open @ (Value::Number(_) | Value::String(_))) => Some(as_text(open)),
            _ => None,
        };
        if let Some(wanted) = wanted.filter(|wanted| valid_id(wanted)) {
            self.opened = Some(wanted);
        } else if truthy(message.get("close")) {
            self.opened = None;
        }
        if truthy(message.get("refresh")) {
            self.scan();
        }
    }

    /// What `app.history` shares. The opened transcript is read again only
    /// when its file changed.
    pub(crate) fn view(&mut self) -> Value {
        let groups: Vec<Value> = self
            .groups
            .iter()
            .map(|group| {
                json!({
                    "name": group.name,
                    "path": group.path,
                    "sessions": group.sessions.iter().map(Session::json).collect::<Vec<_>>(),
                })
            })
            .collect();
        let mut data = json!({
            "available": true,
            "source": if self.sample { "sample" } else { "cli" },
            "total": self.total.unwrap_or(0),
            "groups": groups,
        });
        if let Some(opened) = self.opened.clone() {
            let key = self
                .find_session(&opened)
                .map_or_else(String::new, |(session, _)| {
                    let mtime = session
                        .path
                        .as_ref()
                        .and_then(|path| fs::metadata(path).ok())
                        .map_or(0, |info| mtime(&info));
                    format!("{opened}:{mtime}")
                });
            if self.shown.as_ref().is_none_or(|(shown, _)| *shown != key) {
                let read = self.transcript(&opened);
                self.shown = Some((key, read));
            }
            if let Some((_, Some(transcript))) = &self.shown {
                data["transcript"] = transcript.clone();
            }
        }
        data
    }

    fn transcript(&self, wanted: &str) -> Option<Value> {
        let (session, group) = self.find_session(wanted)?;
        if self.sample {
            return Some(sample::transcript(session, group));
        }
        transcript::read(session, group, &self.stopped)
    }
}
