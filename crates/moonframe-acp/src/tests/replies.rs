//! A run's whole reply in order, and who gave it, as a coding-agent app
//! shows them.

use super::*;

/// A stand-in agent that says who it is and which model it uses, then
/// talks, reads a file and talks again.
const TALKER: &str = r#"
out=$1
reply() { printf '%s\n' "$1"; }
read -r line; printf '%s\n' "$line" >>"$out"
reply '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{},"agentInfo":{"name":"fake-agent","title":"Fake Agent","version":"1.2"}}}'
read -r line; printf '%s\n' "$line" >>"$out"
reply '{"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1","configOptions":[{"id":"model","name":"Model","category":"model","type":"select","currentValue":"m2","options":[{"value":"m1","name":"Small"},{"value":"m2","name":"Large"}]}]}}'
read -r line; printf '%s\n' "$line" >>"$out"
reply '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Let me "}}}}'
reply '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"look."}}}}'
reply '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"tool_call","toolCallId":"t1","title":"Read README.md","kind":"read","status":"in_progress"}}}'
reply '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"tool_call_update","toolCallId":"t1","status":"completed"}}}'
reply '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Two files."}}}}'
reply '{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}'
sleep 30
"#;

#[test]
fn a_run_shows_its_whole_reply_and_who_gave_it() {
    let mut rig = rig_with(TALKER, json!({}));
    rig.send("feedback", json!({ "start": "What is here?" }));
    let run = rig.wait_for("done");
    // The last message alone, as before, and everything in order.
    assert_eq!(run["message"], "Two files.");
    assert_eq!(
        run["parts"][0],
        json!({ "kind": "text", "text": "Let me look." })
    );
    assert_eq!(run["parts"][1]["kind"], "tools");
    assert_eq!(run["parts"][1]["tools"][0]["title"], "Read README.md");
    assert_eq!(run["parts"][1]["tools"][0]["status"], "completed");
    assert_eq!(
        run["parts"][2],
        json!({ "kind": "text", "text": "Two files." })
    );
    assert_eq!(run["agent_info"]["title"], "Fake Agent");
    assert_eq!(run["agent_info"]["version"], "1.2");
    // The model by its name, not its id.
    assert_eq!(run["model"], "Large");

    // All of it comes back after a restart.
    rig.acp.tick(&rig.host);
    drop(std::mem::take(&mut rig.acp));
    let mut again = Acp::default();
    again.start(&rig.host).unwrap();
    let back = &again.last.as_ref().unwrap()["runs"][0];
    assert_eq!(back["parts"], run["parts"]);
    assert_eq!(back["model"], "Large");
    assert_eq!(back["agent_info"], run["agent_info"]);
}
