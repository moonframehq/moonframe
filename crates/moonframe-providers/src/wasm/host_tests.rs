//! Exercise the actual component import, not only registry dispatch.
use {
    crate::{Caller, Capabilities, Event, HostCapability, ProviderOptions, Providers},
    moonframe_core::{
        Data,
        entries::{Entries, RootPolicy},
    },
    std::{
        fs,
        sync::{Arc, Mutex, mpsc},
        time::Duration,
    },
};

// On success publish the host's JSON; on Err publish the number 1. The
// memory is instantiated before lowering host-call (which writes its result).
const GUEST: &str = r#"(component
  (import "moonframe:provider/host" (instance $host
    (export "host-call" (func (param "capability" string) (param "method" string)
      (param "args" string) (result (result string (error string)))))))
  (alias export $host "host-call" (func $call))
  (core module $mem
    (memory (export "memory") 1)
    (global $heap (mut i32) (i32.const 4096))
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      (local $old i32) (local.set $old (global.get $heap))
      (global.set $heap (i32.add (global.get $heap) (local.get 3))) (local.get $old)))
  (core instance $mem (instantiate $mem))
  (alias core export $mem "memory" (core memory $memory))
  (alias core export $mem "realloc" (core func $realloc))
  (core func $call (canon lower (func $call) (memory $memory) (realloc $realloc)))
  (core module $guest
    (import "mem" "memory" (memory 1))
    (import "host" "call" (func $call (param i32 i32 i32 i32 i32 i32 i32)))
    (data (i32.const 100) "mshub.entities")
    (data (i32.const 120) "echo")
    (data (i32.const 128) "{\22id\22:42}")
    (data (i32.const 150) "1")
    (func (export "start") (param i32 i32) (result i32) (i32.const 0))
    (func (export "tick") (result i32)
      (call $call (i32.const 100) (i32.const 14) (i32.const 120) (i32.const 4)
        (i32.const 128) (i32.const 9) (i32.const 16))
      (i32.store (i32.const 32) (i32.const 1))
      (if (i32.load (i32.const 16))
        (then (i32.store (i32.const 36) (i32.const 150)) (i32.store (i32.const 40) (i32.const 1)))
        (else (i32.store (i32.const 36) (i32.load (i32.const 20))) (i32.store (i32.const 40) (i32.load (i32.const 24)))))
      (i32.const 32))
    (func (export "receive") (param i32 i32) (result i32) (i32.const 48)))
  (core instance $guest (instantiate $guest
    (with "mem" (instance $mem))
    (with "host" (instance (export "call" (func $call))))))
  (func (export "start") (param "settings" string) (result (result (error string)))
    (canon lift (core func $guest "start") (memory $memory) (realloc $realloc)))
  (func (export "tick") (result (option string))
    (canon lift (core func $guest "tick") (memory $memory) (realloc $realloc)))
  (func (export "receive") (param "message" string) (result (option string))
    (canon lift (core func $guest "receive") (memory $memory) (realloc $realloc))))"#;

struct Echo {
    seen: Arc<Mutex<Vec<Caller>>>,
    delay: Duration,
    large: bool,
}
impl HostCapability for Echo {
    fn call(&self, who: &Caller, method: &str, args: &str) -> Result<String, String> {
        assert_eq!(method, "echo");
        self.seen.lock().unwrap().push(who.clone());
        std::thread::sleep(self.delay);
        Ok(if self.large {
            format!("\"{}\"", "x".repeat(100))
        } else {
            args.to_owned()
        })
    }
}

#[test]
fn wasm_host_call_checks_declarations_and_supplies_loader_identity() {
    for mode in ["roundtrip", "undeclared", "args", "result", "timeout"] {
        let declared = mode != "undeclared";
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("notes/provider");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("provider.wasm"), GUEST).unwrap();
        fs::write(
            dir.join("provider.toml"),
            format!(
                "kind = 'wasm'\ninterval = 60\ncapabilities = {}",
                if declared {
                    "['mshub.entities']"
                } else {
                    "[]"
                }
            ),
        )
        .unwrap();
        let entries = Entries::default();
        entries
            .add_root("mshub", root.path(), RootPolicy::default())
            .unwrap();
        entries
            .add_entry("mshub", "notes", dir.parent().unwrap())
            .unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut capabilities = Capabilities::default();
        if mode == "args" {
            capabilities.limits.max_bytes = 8;
        }
        if mode == "result" {
            capabilities.limits.max_bytes = 32;
        }
        if mode == "timeout" {
            capabilities.limits.timeout = Duration::from_millis(10);
        }
        capabilities
            .capability("mshub.entities", Echo {
                seen: seen.clone(),
                delay: if mode == "timeout" {
                    Duration::from_millis(80)
                } else {
                    Duration::ZERO
                },
                large: mode == "result",
            })
            .unwrap();
        let (send, receive) = mpsc::channel();
        let mut providers = Providers::start_configured(
            entries.apply(root.path().into()),
            root.path().join("build"),
            Arc::new(move |e| {
                let _ = send.send(e);
            }),
            Default::default(),
            None,
            ProviderOptions {
                capabilities,
                ..Default::default()
            },
        );
        let event = receive.recv_timeout(Duration::from_secs(20)).unwrap();
        match event {
            Event::Published { provider, data } => {
                assert_eq!(provider, "mshub:notes");
                assert_eq!(
                    data,
                    if mode == "roundtrip" {
                        crate::from_json(&serde_json::json!({"id":42})).unwrap()
                    } else {
                        Data::Integer(1)
                    }
                );
            },
            other => panic!("{other:?}"),
        }
        if declared && mode != "args" {
            assert_eq!(seen.lock().unwrap().as_slice(), &[Caller {
                provider: "mshub:notes".into(),
                folder: dir.clone(),
                root: root.path().to_owned(),
                namespace: Some("mshub".into()),
                capabilities: vec!["mshub.entities".into()]
            }]);
        } else {
            assert!(seen.lock().unwrap().is_empty());
        }
        providers.shutdown();
    }
}
