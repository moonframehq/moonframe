use {super::*, crate::limits::count_values, serde_json::json};

const MIB: usize = 1024 * 1024;

fn host_takes(line: &str, value: &Value) -> bool {
    line.len() <= MIB && count_values(value) <= 10_000
}

fn last_id(value: &Value) -> Value {
    let items = as_list(&value["thread"]["items"]);
    items.last().map_or(Value::Null, |item| item["id"].clone())
}

#[test]
fn the_newest_stays_when_a_conversation_is_too_long() {
    let text = |id: &str| json!({ "k": "text", "id": id, "text": "\"\n".repeat(200_000) });
    let many = json!({ "thread": {
        "earlier": 0, "items": [text("a"), text("b"), text("c"), text("end")],
    }});
    let (line, value) = encode(many);
    assert!(host_takes(&line, &value));
    assert_eq!(last_id(&value), "end");
    assert!(value["thread"]["earlier"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn one_huge_item_is_cut() {
    let huge = json!({ "thread": { "earlier": 0, "items": [
        { "k": "tool", "id": "t", "output": "é".repeat(2_000_000) },
    ]}});
    let (line, value) = encode(huge);
    assert!(host_takes(&line, &value));
    let output = value["thread"]["items"][0]["output"]
        .as_str()
        .unwrap_or_default();
    assert!(output.contains("cut, too long"));
}

#[test]
fn many_small_items_fit_too() {
    let items: Vec<Value> = (0..4000)
        .map(|id| json!({ "k": "note", "id": id, "text": "x" }))
        .collect();
    let (line, value) = encode(json!({ "thread": { "earlier": 0, "items": items } }));
    assert!(host_takes(&line, &value));
}

#[test]
fn every_key_stays_with_no_thread_open() {
    let skills: Vec<Value> = (0..30_000)
        .map(
            |id| json!({ "id": id, "name": format!("skill {id}"), "description": "d".repeat(200) }),
        )
        .collect();
    let crowded = json!({
        "now": 1, "skills": skills, "thread": null, "workspaces": [], "prefs": { "group": "time" },
    });
    let (line, value) = encode(crowded);
    assert!(host_takes(&line, &value));
    let mut keys: Vec<&String> = value
        .as_object()
        .map(|fields| fields.keys().collect())
        .unwrap_or_default();
    keys.sort();
    assert_eq!(keys, ["now", "prefs", "skills", "thread", "workspaces"]);
}

#[test]
fn whatever_part_of_the_thread_is_long_is_cut() {
    let dialog = json!({ "thread": {
        "earlier": 0, "items": [], "dialog": { "prefill": "x".repeat(3_000_000) }, "widgets": [],
    }});
    let (line, value) = encode(dialog);
    assert!(host_takes(&line, &value));
}

#[test]
fn a_short_snapshot_passes_as_it_is() {
    let small = json!({ "thread": { "earlier": 0, "items": [{ "id": "a" }] } });
    let (_, value) = encode(small.clone());
    assert_eq!(value, small);
}

#[test]
fn visible_items_show_the_end_and_count_the_rest() {
    let items: Vec<Map<String, Value>> = (0..10)
        .map(|id| json!({ "id": id, "text": "x", "args": {} }))
        .filter_map(|item| item.as_object().cloned())
        .collect();
    let (earlier, shown) = visible_items(items, 3);
    assert_eq!(earlier, 7);
    assert_eq!(shown.len(), 3);
    assert_eq!(shown[2]["id"], 9);
    assert!(shown[0].get("args").is_none(), "arguments stay out");
}

#[test]
fn a_version_changes_with_the_item() {
    let item = json!({ "text": "ab", "status": "done" });
    let more = json!({ "text": "abc", "status": "done" });
    assert_ne!(item_version(&item), item_version(&more));
    assert_eq!(item_version(&item), "2.done..0....");
}
