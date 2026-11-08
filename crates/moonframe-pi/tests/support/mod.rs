//! Runs a provider as the app does, in folders of its own: send it messages,
//! wait on what it publishes.
#![allow(dead_code, reason = "each test binary uses part of this")]
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test support")]

pub mod mock_llm;

use {
    moonframe_core::Data,
    moonframe_pi::Config,
    moonframe_providers::{Event, Handle, Native, from_json, program, to_json},
    serde_json::{Value, json},
    std::{
        env, fs,
        path::{Path, PathBuf},
        process::Command,
        sync::{Arc, Condvar, Mutex},
        time::{Duration, Instant},
    },
};

pub const WAIT: Duration = Duration::from_secs(25);

#[derive(Default)]
struct Seen {
    value: Value,
    sizes: Vec<usize>,
    counts: usize,
    failed: Vec<String>,
}

/// A provider running under test.
pub struct App {
    provider: Option<Box<dyn Native>>,
    host: Handle,
    seen: Arc<(Mutex<Seen>, Condvar)>,
}

impl App {
    /// Starts `provider` under the name `name`.
    pub fn start(name: &str, mut provider: Box<dyn Native>) -> Self {
        let seen: Arc<(Mutex<Seen>, Condvar)> = Arc::default();
        let heard = Arc::clone(&seen);
        let events = Arc::new(move |event| {
            let (seen, changed) = &*heard;
            let mut seen = seen.lock().unwrap();
            match event {
                Event::Published { data, .. } => {
                    seen.value = to_json(&data);
                    let size = seen.value.to_string().len();
                    seen.sizes.push(size);
                    seen.counts = seen.counts.max(count(&seen.value));
                },
                Event::Failed { problem, .. } => seen.failed.push(problem.to_string()),
                _ => {},
            }
            changed.notify_all();
        });
        // What it receives comes from `test`, a writer, as an app's
        // provider.toml names the mods that may change a checkout.
        let settings = from_json(&json!({ "writers": ["test"] })).unwrap_or(Data::Nil);
        let host = Handle::new(name, Path::new("."), settings, events);
        provider.start(&host).unwrap();
        Self {
            provider: Some(provider),
            host,
            seen,
        }
    }

    /// Sends what a mod would: through `Data`, so an empty table is `{}`.
    pub fn send(&mut self, message: &Value) {
        let data = from_json(message).unwrap();
        let provider = self.provider.as_mut().unwrap();
        provider.receive(&self.host, "test", data);
    }

    /// What is published now.
    pub fn value(&self) -> Value {
        self.seen.0.lock().unwrap().value.clone()
    }

    /// The largest line and the most values published.
    pub fn largest(&self) -> (usize, usize) {
        let seen = self.seen.0.lock().unwrap();
        (seen.sizes.iter().copied().max().unwrap_or(0), seen.counts)
    }

    /// Everything published so far, as a count.
    pub fn publishes(&self) -> usize {
        self.seen.0.lock().unwrap().sizes.len()
    }

    /// Waits for `test` to find something in what is published.
    pub fn until<T>(&self, label: &str, test: impl Fn(&Value) -> Option<T>) -> T {
        self.until_within(label, WAIT, test)
    }

    /// [`App::until`], giving up after `timeout`.
    pub fn until_within<T>(
        &self,
        label: &str,
        timeout: Duration,
        test: impl Fn(&Value) -> Option<T>,
    ) -> T {
        let started = Instant::now();
        let (seen, changed) = &*self.seen;
        let mut seen = seen.lock().unwrap();
        loop {
            if !seen.value.is_null()
                && let Some(found) = test(&seen.value)
            {
                return found;
            }
            let left = timeout.saturating_sub(started.elapsed());
            if left.is_zero() {
                let text = seen.value.to_string();
                let start: String = text.chars().take(8000).collect();
                panic!("timed out on {label}: {start}; failed: {:?}", seen.failed);
            }
            seen = changed.wait_timeout(seen, left).unwrap().0;
        }
    }

    /// Waits for `test` to hold of what is published.
    pub fn when(&self, label: &str, test: impl Fn(&Value) -> bool) {
        self.until(label, |value| test(value).then_some(()));
    }

    /// Stops the provider, as the app does.
    pub fn stop(&mut self) {
        self.provider.take();
    }
}

/// How many values the host would count.
pub fn count(value: &Value) -> usize {
    match value {
        Value::Array(items) => 1 + items.iter().map(count).sum::<usize>(),
        Value::Object(fields) => 1 + fields.values().map(count).sum::<usize>(),
        _ => 1,
    }
}

/// The items of the open thread of kind `kind`.
pub fn items(value: &Value, kind: &str) -> Vec<Value> {
    let list = value["thread"]["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    list.into_iter().filter(|item| item["k"] == kind).collect()
}

/// Whether the open thread has an item of `kind` passing `test`.
pub fn has_item(value: &Value, kind: &str, test: impl Fn(&Value) -> bool) -> bool {
    items(value, kind).iter().any(test)
}

pub fn git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A small repository to work on: `<root>/tidy-cart`.
pub fn fixture_project(root: &Path) -> PathBuf {
    let project = root.join("tidy-cart");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    fs::write(
        project.join("src/price.js"),
        "export function formatPrice(cents) {\n  const dollars = Math.floor(cents / 100);\n  const rest = cents % 100;\n  return `$${dollars}.${rest}`;\n}\n",
    )
    .unwrap();
    fs::write(project.join("price.test.js"), "// The tests.\n").unwrap();
    git(&project, &["init", "-q", "-b", "main"]);
    // A machine's own identity and signing are no part of these tests.
    git(&project, &["config", "user.name", "t"]);
    git(&project, &["config", "user.email", "t@example.com"]);
    git(&project, &["config", "commit.gpgsign", "false"]);
    git(&project, &["add", "-A"]);
    git(&project, &["commit", "-qm", "Start"]);
    project
}

/// pi's folder under `root`, with the mock model on `port`.
pub fn mock_agent_dir(root: &Path, port: u16) -> PathBuf {
    let dir = root.join("agent");
    fs::create_dir_all(&dir).unwrap();
    let models = json!({
        "providers": {
            "mock": {
                "baseUrl": format!("http://127.0.0.1:{port}/v1"),
                "api": "openai-completions",
                "apiKey": "mock",
                "models": [{
                    "id": "mock-1",
                    "name": "Mock One",
                    "reasoning": true,
                    "input": ["text", "image"],
                    "contextWindow": 200_000,
                    "maxTokens": 8000,
                    "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0 },
                }],
            },
        },
    });
    fs::write(dir.join("models.json"), models.to_string()).unwrap();
    dir
}

/// The real pi, not through a shim, which needs the real mise folders.
pub fn real_pi() -> PathBuf {
    if let Some(path) = env::var_os("PILOT_PI") {
        return path.into();
    }
    let dirs = program::default_search_path();
    let plain: Vec<PathBuf> = env::split_paths(&dirs)
        .filter(|dir| !dir.components().any(|part| part.as_os_str() == "shims"))
        .collect();
    program::find("pi", &env::join_paths(plain).unwrap()).expect("these tests need pi")
}

/// Keeps everything under `root`, with `pi` as the one to run. pi gets a
/// home of its own, so nothing here can reach the user's sessions.
pub fn config(root: &Path, agent: &Path, pi: PathBuf) -> Config {
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    Config {
        agent_dir: agent.to_owned(),
        data_dir: root.join("data"),
        home: home.clone(),
        pi: Some(pi),
        login_shell: false,
        env: vec![
            ("HOME".to_owned(), home.to_string_lossy().into_owned()),
            (
                "PI_CODING_AGENT_DIR".to_owned(),
                agent.to_string_lossy().into_owned(),
            ),
            ("PI_OFFLINE".to_owned(), "1".to_owned()),
        ],
    }
}

/// A provider on the real pi and the mock model, in `root`.
pub struct Rig {
    pub app: App,
    pub agent: PathBuf,
    pub project: PathBuf,
    pub root: PathBuf,
}

pub fn start_rig(root: &Path) -> Rig {
    // Links resolved, as the paths pi and the provider report are.
    let root = fs::canonicalize(root).unwrap();
    let port = mock_llm::serve(0).unwrap();
    let agent = mock_agent_dir(&root, port);
    let project = fixture_project(&root);
    let pi = moonframe_pi::pi_with(config(&root, &agent, real_pi()));
    let app = App::start("pi", pi);
    app.when("pi and its models", |value| {
        value["runtime"]["status"] == "ready"
            && value["models"]
                .as_array()
                .is_some_and(|list| !list.is_empty())
    });
    Rig {
        app,
        agent,
        project,
        root,
    }
}

/// A one-pixel picture.
pub const DOT_PNG: [u8; 70] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0x60, 0xf8, 0x5f,
    0x0f, 0x00, 0x02, 0x87, 0x01, 0x80, 0xeb, 0x47, 0xba, 0x92, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];
