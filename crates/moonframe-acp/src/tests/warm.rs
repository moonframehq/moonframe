//! An agent started as the dialog opens, which the next run takes.

use super::*;

#[test]
fn a_warm_agent_starts_ahead_and_the_next_run_takes_it() {
    let mut rig = rig_allowing(json!({ "feedback": "all" }));
    rig.send("feedback", json!({ "warm": true }));
    // Not a run: nothing shows, and its handshake goes ahead.
    assert_eq!(rig.acp.last.as_ref().unwrap()["runs"], json!([]));
    let deadline = Instant::now() + Duration::from_secs(10);
    while rig.sent().is_empty() {
        assert!(Instant::now() < deadline, "the agent never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(rig.sent()[0]["method"], "initialize");
    // Wait for its session, so the run sends its prompt at once.
    while !lock(&rig.acp.state)
        .warm
        .as_ref()
        .is_some_and(|run| run.timing.session.is_some())
    {
        assert!(Instant::now() < deadline, "the agent never answered");
        std::thread::sleep(Duration::from_millis(20));
    }
    // Asking again keeps the one there.
    rig.send("feedback", json!({ "warm": true }));
    let conn = lock(&rig.acp.state).warm.as_ref().unwrap().conn;

    rig.send("feedback", json!({ "start": "Make it bold" }));
    assert!(lock(&rig.acp.state).warm.is_none());
    let run = rig.wait_for("done");
    assert_eq!(run["message"], "I changed the title.");
    assert_eq!(run["timing"]["warm"], true);
    assert_eq!(run["timing"]["spawned"], 0);
    assert_eq!(run["timing"]["initialized"], 0);
    assert_eq!(run["timing"]["session"], 0);
    let methods: Vec<Value> = rig
        .sent()
        .iter()
        .map(|line| line["method"].clone())
        .collect();
    assert_eq!(methods[..3], [
        json!("initialize"),
        json!("session/new"),
        json!("session/prompt")
    ]);
    assert_eq!(lock(&rig.acp.state).runs[0].conn, conn);
}

#[test]
fn only_starters_warm_agents() {
    let mut rig = rig();
    rig.send("other", json!({ "warm": true }));
    assert!(lock(&rig.acp.state).warm.is_none());
}

#[test]
fn a_run_elsewhere_leaves_the_warm_agent_waiting() {
    let mut rig = rig_allowing(json!({ "chat": "all" }));
    rig.send("chat", json!({ "warm": true }));
    let elsewhere = tempfile::tempdir().unwrap();
    rig.send(
        "chat",
        json!({ "start": "Hi", "cwd": elsewhere.path().to_str().unwrap() }),
    );
    assert!(lock(&rig.acp.state).warm.is_some());
    assert_eq!(rig.run()["timing"]["warm"], false);
}
