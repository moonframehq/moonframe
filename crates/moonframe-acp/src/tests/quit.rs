//! What is left of an agent once the app quits: nothing, its children
//! included.

use super::*;

/// A stand-in agent that starts a child of its own, as `npx` starts the
/// adapter, then ignores being asked to stop.
const LAUNCHER: &str = r#"
out=$1
reply() { printf '%s\n' "$1"; }
read -r line
reply '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}'
read -r line
reply '{"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1"}}'
read -r line
sleep 300 &
echo $! >"$out.child"
trap '' TERM
while :; do sleep 1; done
"#;

fn alive(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn quitting_stops_an_agent_and_what_it_started() {
    let mut rig = rig_with(LAUNCHER, json!({}));
    rig.send("feedback", json!({ "start": "Hi" }));
    let started = rig.sent.with_extension("jsonl.child");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !started.exists() {
        assert!(
            Instant::now() < deadline,
            "the agent never started its child"
        );
        rig.acp.tick(&rig.host);
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(100));
    let child = std::fs::read_to_string(&started).unwrap();
    let child = child.trim().to_owned();
    assert!(alive(&child));

    let quitting = Instant::now();
    drop(rig.acp);
    assert!(
        quitting.elapsed() < Duration::from_secs(2),
        "{:?}",
        quitting.elapsed()
    );
    std::thread::sleep(Duration::from_millis(100));
    assert!(!alive(&child), "the agent's child outlived the app");
}
