use {super::*, crate::RevisionTimeline};

fn timeline(props: &str) -> Result<RevisionTimeline, String> {
    let source =
        format!("return mod {{ view = function() return ui.revision_timeline {{ {props} }} end }}");
    match view_of(&source)? {
        Node::RevisionTimeline(timeline) => Ok(timeline),
        other => Err(format!("expected a revision timeline, got {other:?}")),
    }
}

#[test]
fn revision_timeline_keeps_stable_ids_and_selection_payload() {
    let ruler = timeline(
        r#"id = 'history', entries = {
        { id = 'a', day = 'Today', label = '09:00' },
        { id = 'b', day = 'Today', label = '10:00' }
    }, selected = 'b', select = { action = 'restore-preview' }, grow = true"#,
    )
    .unwrap();
    assert_eq!(ruler.selected.as_deref(), Some("b"));
    let message = ruler.selection_message(1).unwrap();
    assert_eq!(message.get("revision").and_then(Data::as_str), Some("b"));
    assert_eq!(message.get("day").and_then(Data::as_str), Some("Today"));
    assert_eq!(message.get("index").and_then(Data::as_i64), Some(2));
    assert_eq!(
        message.get("action").and_then(Data::as_str),
        Some("restore-preview")
    );
    assert!(ruler.selection_message(2).is_none());
    // Empty histories and a temporarily missing selected id are valid.
    assert!(timeline("selected = 'gone'").unwrap().entries.is_empty());
}

#[test]
fn revision_timeline_rejects_ambiguous_and_invalid_records() {
    for props in [
        "entries = { { id = 'a', day = 'Today', label = 'One' }, { id = 'a', day = 'Today', label = 'Two' } }",
        "entries = { { id = '', day = 'Today', label = 'One' } }",
        "entries = { { id = 'a', day = '', label = 'One' } }",
        "entries = { { id = 'a', day = 'Today' } }",
        "width = -1",
        "disabled = 1",
        "magnification = 0",
        "radius = 0",
        "spacing = 0",
        "spread = -1",
        "tick_width = 0",
        "tick_height = 0",
        "side = 'top'",
        "labels = 'sometimes'",
    ] {
        assert!(timeline(props).is_err(), "accepted {props}");
    }
}

#[test]
fn revision_timeline_configures_an_undated_chat_rail() {
    let ruler = timeline(
        r#"entries = { { id = 'turn-1', label = 'Hello', summary = 'A longer preview' } },
        side = 'left', labels = 'hover', dates = false, preview = true,
        tick_width = 6, tick_height = 1, spacing = 12, magnification = 3, radius = 2, spread = 0"#,
    )
    .unwrap();
    assert_eq!(ruler.side, crate::TimelineSide::Left);
    assert_eq!(ruler.labels, crate::TimelineLabels::Hover);
    assert!(!ruler.dates && ruler.preview);
    assert_eq!(ruler.entries[0].day, None);
    assert_eq!(
        ruler.entries[0].summary.as_deref(),
        Some("A longer preview")
    );
    assert_eq!(ruler.metrics, crate::TimelineMetrics {
        tick_width: 6.0,
        tick_height: 1.0,
        spacing: 12.0,
        magnification: 3.0,
        radius: 2.0,
        spread: 0.0,
    });
}
