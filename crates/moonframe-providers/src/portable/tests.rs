use {
    super::*,
    crate::{Caller, Capabilities, Event, HostCapability, ProviderOptions, Providers},
    serde_json::{Value, json},
    std::{
        fs,
        sync::{Arc, mpsc},
        time::Duration,
    },
};

/// Counts its ticks; a mod's `"reset"` starts it again, and what it counts
/// survives a restart through `save` and `restore`.
struct Counter {
    count: u64,
    step: u64,
}

impl Provider for Counter {
    fn start(settings: Value) -> Result<Self, String> {
        let step = settings.get("step").and_then(Value::as_u64).unwrap_or(1);
        moonframe_provider::log("counting");
        Ok(Self { count: 0, step })
    }

    fn tick(&mut self) -> Option<Value> {
        self.count += self.step;
        Some(json!({ "count": self.count }))
    }

    fn receive(&mut self, message: Value) -> Option<Value> {
        if message == "reset" {
            self.count = 0;
        }
        Some(json!({ "count": self.count }))
    }

    fn save(&mut self) -> Option<String> {
        Some(self.count.to_string())
    }

    fn restore(&mut self, state: &str) -> Result<(), String> {
        self.count = state.parse().map_err(|_| "not a count")?;
        Ok(())
    }
}

fn events() -> (crate::Events, mpsc::Receiver<Event>) {
    let (sender, receiver) = mpsc::channel();
    let sender = Mutex::new(sender);
    let events: crate::Events = Arc::new(move |event| {
        let _ = sender.lock().unwrap().send(event);
    });
    (events, receiver)
}

/// The next thing published, past how the provider's status stands.
fn published(received: &mpsc::Receiver<Event>) -> Value {
    match received.recv_timeout(Duration::from_secs(5)).unwrap() {
        Event::Published { data, .. } => json::to_json(&data),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn it_is_called_as_the_wasm_host_calls_it_and_hands_on_its_state() {
    let dir = tempfile::tempdir().unwrap();
    let (events, received) = events();
    let settings = json::from_json(&json!({ "step": 2 })).unwrap();
    let host = Handle::new("counter", dir.path(), settings, events);

    let mut first = Portable::<Counter>::new();
    first.start(&host).unwrap();
    assert_eq!(
        published(&received),
        json!({ "count": 2 }),
        "the first tick"
    );
    first.tick(&host);
    assert_eq!(published(&received), json!({ "count": 4 }));
    first.receive(&host, "status", Data::from("reset"));
    assert_eq!(published(&received), json!({ "count": 0 }));
    first.tick(&host);
    assert_eq!(published(&received), json!({ "count": 2 }));
    drop(first);

    let mut next = Portable::<Counter>::new();
    next.start(&host).unwrap();
    assert_eq!(
        published(&received),
        json!({ "count": 4 }),
        "restored, then ticked"
    );
}

#[test]
fn a_handle_without_a_manifest_grants_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (events, _received) = events();
    let host = Handle::new("bare", dir.path(), Data::Nil, events);
    let answer = entered(&host, || {
        moonframe_provider::host_call::<_, Value>("example.echo", "echo", &1)
    });
    assert!(answer.unwrap_err().contains("undeclared"));
    assert!(entered(&host, moonframe_provider::system_load).is_err());
}

/// Echoes `{ method, args, provider }` back.
struct Echo;

impl HostCapability for Echo {
    fn call(&self, caller: &Caller, method: &str, args: &str) -> Result<String, String> {
        let args: Value = serde_json::from_str(args).map_err(|e| e.to_string())?;
        Ok(json!({ "method": method, "args": args, "provider": caller.provider }).to_string())
    }
}

/// Asks the host on every tick, as `examples/capability-provider` does.
struct Asking;

impl Provider for Asking {
    fn start(_: Value) -> Result<Self, String> {
        Ok(Self)
    }

    fn tick(&mut self) -> Option<Value> {
        Some(
            moonframe_provider::host_call("example.echo", "echo", &json!({ "hello": 1 }))
                .unwrap_or_else(|problem: String| json!({ "problem": problem })),
        )
    }
}

#[test]
fn a_native_folder_runs_it_with_the_capabilities_it_declares() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("providers/asking");
    fs::create_dir_all(&folder).unwrap();
    fs::write(
        folder.join("provider.toml"),
        "kind = 'native'\nstart = 'always'\ninterval = 60\ncapabilities = ['example.echo']\n",
    )
    .unwrap();
    let mut capabilities = Capabilities::default();
    capabilities.capability("example.echo", Echo).unwrap();
    let factory: crate::native::Factory = Arc::new(|| Box::new(Portable::<Asking>::new()));
    let (events, received) = events();
    let _providers = Providers::start_configured(
        dir.path().into(),
        dir.path().join("build"),
        events,
        crate::Natives::from([("asking".to_owned(), factory)]),
        None,
        ProviderOptions {
            capabilities,
            ..Default::default()
        },
    );
    assert_eq!(
        published(&received),
        json!({ "method": "echo", "args": { "hello": 1 }, "provider": "asking" })
    );
}
