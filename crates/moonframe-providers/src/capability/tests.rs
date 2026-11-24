use {super::*, std::sync::Mutex};

struct Echo(Arc<Mutex<Vec<Caller>>>);
impl HostCapability for Echo {
    fn call(&self, caller: &Caller, method: &str, args: &str) -> Result<String, String> {
        self.0.lock().unwrap().push(caller.clone());
        match method {
            "large" => Ok(format!("\"{}\"", "x".repeat(100))),
            _ => Ok(args.to_owned()),
        }
    }
}

fn caller() -> Caller {
    Caller {
        provider: "acme:notes".into(),
        folder: "/miniapps/notes/provider".into(),
        root: "/miniapps".into(),
        namespace: Some("acme".into()),
        capabilities: vec!["acme.entities".into()],
    }
}

#[test]
fn declaration_identity_roundtrip_and_size_limits() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut caps = Capabilities::default();
    caps.limits.max_bytes = 32;
    caps.capability("acme.entities", Echo(seen.clone()))
        .unwrap();
    assert!(caps.capability("system", Echo(seen.clone())).is_err());
    assert!(
        caps.capability("moonframe.entities", Echo(seen.clone()))
            .is_err()
    );
    let busy = Arc::default();
    let mut who = caller();
    let json = r#"{"id":42}"#;
    assert_eq!(
        caps.call(&who, &busy, "acme.entities", "echo", json)
            .unwrap(),
        json
    );
    assert_eq!(seen.lock().unwrap().as_slice(), &[who.clone()]);
    assert!(
        caps.call(&who, &busy, "acme.entities", "large", "null")
            .unwrap_err()
            .contains("result exceeds")
    );
    let count = seen.lock().unwrap().len();
    assert!(
        caps.call(&who, &busy, "acme.entities", "echo", &"x".repeat(33))
            .unwrap_err()
            .contains("arguments exceed")
    );
    who.capabilities.clear();
    assert!(
        caps.call(&who, &busy, "acme.entities", "echo", json)
            .unwrap_err()
            .contains("undeclared")
    );
    assert_eq!(seen.lock().unwrap().len(), count);
}

struct Block(Mutex<mpsc::Receiver<()>>);
impl HostCapability for Block {
    fn call(&self, _: &Caller, _: &str, _: &str) -> Result<String, String> {
        self.0.lock().unwrap().recv().unwrap();
        Ok("null".into())
    }
}

#[test]
fn timeout_bounds_wait_and_does_not_spawn_more_blocked_work() {
    let (release, blocked) = mpsc::channel();
    let mut caps = Capabilities::default();
    caps.limits.timeout = Duration::from_millis(20);
    caps.capability("acme.entities", Block(Mutex::new(blocked)))
        .unwrap();
    let busy = Arc::default();
    assert_eq!(
        caps.call(&caller(), &busy, "acme.entities", "wait", "null")
            .unwrap_err(),
        "host-call timed out"
    );
    assert!(
        caps.call(&caller(), &busy, "acme.entities", "wait", "null")
            .unwrap_err()
            .contains("still running")
    );
    release.send(()).unwrap();
}

#[test]
fn unknown_manifest_capability_names_the_capability() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("provider.toml"),
        "kind = 'wasm'\ncapabilities = ['acme.missing']",
    )
    .unwrap();
    std::fs::write(dir.path().join("provider.wasm"), "").unwrap();
    assert!(
        crate::Manifest::load("acme:test", dir.path())
            .unwrap_err()
            .to_string()
            .contains("unknown capability `acme.missing`")
    );
    let mut caps = Capabilities::default();
    caps.capability("acme.missing", Echo(Default::default()))
        .unwrap();
    assert!(crate::Manifest::load_with("acme:test", dir.path(), &caps).is_ok());
}

#[cfg(feature = "tokio")]
#[test]
fn async_calls_use_runtime_and_cancel_on_timeout() {
    struct Wait;
    impl AsyncHostCapability for Wait {
        async fn call(&self, _: &Caller, method: &str, _: &str) -> Result<String, String> {
            if method == "wait" {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
            Ok("42".into())
        }
    }
    let mut caps = Capabilities::default();
    caps.limits.timeout = Duration::from_millis(30);
    caps.capability_async("acme.entities", Wait).unwrap();
    assert_eq!(
        caps.call(&caller(), &Arc::default(), "acme.entities", "now", "null")
            .unwrap(),
        "42"
    );
    assert!(
        caps.call(&caller(), &Arc::default(), "acme.entities", "wait", "null")
            .unwrap_err()
            .contains("timed out")
    );
}
