//! Click the actual tool detail and deliver the provider reply to its chat instance.
use super::*;

fn detail(node: &Node) -> Option<(&Action, &crate::Text)> {
    let Node::Stack(stack) = node else {
        return None;
    };
    if let Some(action) = &stack.action
        && let Some(Node::Stack(column)) = stack.children.first()
        && let Some(Node::Text(text)) = column.children.first()
        && stack
            .extras
            .tooltip
            .as_deref()
            .is_some_and(|label| label.ends_with("output"))
    {
        return Some((action, text));
    }
    stack.children.iter().find_map(detail)
}

#[test]
fn clicking_preview_requests_original_text_and_clicking_again_collapses() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "main.luau",
        r#"
local chat = use "agent-chat"
return mod { view = function()
    local item = {type="tool", id="call", output_key="e2", title="Search", detail="preview"}
    local group = {type="tools", key="e2", live=true, items={item}}
    return chat {id="test", view={id="c", status="live", view={status="idle", accepts=true, rows={group}}}}
end }
"#,
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    let mut workspace = Workspace::open(
        crate::Roots::bundled(root, vec![], crate::AppLayer {
            path: dir.path().into(),
            keep: vec![],
        }),
        false,
    );
    let instance = "shell/agent-chat#test";
    let inspect = |workspace: &Workspace, expected: &str, lines| {
        let slot = workspace.slot(instance).unwrap();
        let (action, text) = detail(slot.view().unwrap()).unwrap();
        assert_eq!(text.text, expected);
        assert!(text.wrap);
        assert_eq!(text.lines, lines);
        action.clone()
    };
    let expand = inspect(&workspace, "preview", Some(3));
    workspace.dispatch(instance, &expand);
    let requests = workspace.take_outbox();
    let (provider, from, msg) = requests
        .iter()
        .find(|(_, _, msg)| msg.get("tool_output").is_some())
        .unwrap();
    assert_eq!(provider, "chat");
    assert_eq!(from, instance);
    assert_eq!(msg.get("tool_output"), Some(&"e2".into()));
    workspace.send(
        "provider:chat",
        from,
        Data::map([
            ("reply", msg.get("id").unwrap().clone()),
            ("ok", true.into()),
            ("text", "original text".into()),
        ]),
    );
    let collapse = inspect(&workspace, "original text", None);
    workspace.dispatch(instance, &collapse);
    inspect(&workspace, "preview", Some(3));
}

#[test]
fn running_tool_expands_live_detail_and_fetches_only_after_finishing() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "main.luau",
        r#"
local chat = use "agent-chat"
return mod {
    init = function()
        return { status = "running", detail = "step 1" }
    end,
    update = function(state, msg)
        if type(msg) == "table" and msg.step then
            state.status = msg.status or state.status
            state.detail = msg.detail or state.detail
            return state
        end
        return nil
    end,
    view = function(state)
        local item = {
            type = "tool",
            id = "call",
            output_key = "e2",
            status = state.status,
            title = "Run",
            detail = state.detail,
        }
        local group = { type = "tools", key = "e2", live = true, items = { item } }
        return chat {
            id = "test",
            view = { id = "c", status = "live", view = { status = "idle", accepts = true, rows = { group } } },
        }
    end,
}
"#,
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    let mut workspace = Workspace::open(
        crate::Roots::bundled(root, vec![], crate::AppLayer {
            path: dir.path().into(),
            keep: vec![],
        }),
        false,
    );
    let instance = "shell/agent-chat#test";
    let inspect = |workspace: &Workspace, expected: &str, lines| {
        let slot = workspace.slot(instance).unwrap();
        let (action, text) = detail(slot.view().unwrap()).unwrap();
        assert_eq!(text.text, expected);
        assert_eq!(text.lines, lines);
        action.clone()
    };

    // Expanding while running does not fetch fixed pages and tracks live detail updates.
    let expand = inspect(&workspace, "step 1", Some(3));
    workspace.dispatch(instance, &expand);
    assert!(
        workspace
            .take_outbox()
            .iter()
            .all(|(_, _, msg)| msg.get("tool_output").is_none()),
        "running tool should not fetch fixed pages"
    );
    inspect(&workspace, "step 1", None);

    workspace.send(
        "test",
        "shell",
        Data::map([("step", true.into()), ("detail", "step 1\nstep 2".into())]),
    );
    inspect(&workspace, "step 1\nstep 2", None);

    // Once the tool finishes, it shows the final preview and clicking fetches full output.
    workspace.send(
        "test",
        "shell",
        Data::map([
            ("step", true.into()),
            ("status", "done".into()),
            ("detail", "final preview".into()),
        ]),
    );
    let fetch_click = inspect(&workspace, "final preview", Some(3));
    workspace.dispatch(instance, &fetch_click);
    let requests = workspace.take_outbox();
    let (_, from, msg) = requests
        .iter()
        .find(|(_, _, msg)| msg.get("tool_output").is_some())
        .unwrap();
    workspace.send(
        "provider:chat",
        from,
        Data::map([
            ("reply", msg.get("id").unwrap().clone()),
            ("ok", true.into()),
            ("text", "complete output".into()),
            ("more", true.into()),
        ]),
    );
    inspect(&workspace, "complete output", None);
}
