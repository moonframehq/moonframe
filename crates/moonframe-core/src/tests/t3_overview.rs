//! Drive the example through the shell's real view and message paths.
use super::*;

fn shows(workspace: &Workspace, label: &str) -> bool {
    fn contains(node: &Data, label: &str) -> bool {
        let Data::Map(fields) = node else {
            return false;
        };
        fields.get("text") == Some(&Data::from(label))
            || matches!(fields.get("children"), Some(Data::List(nodes)) if nodes.iter().any(|child| contains(child, label)))
    }
    let slot = workspace.slot(SHELL).unwrap();
    assert!(slot.error().is_none(), "{:?}", slot.error());
    assert!(slot.broken().is_empty(), "{:?}", slot.broken());
    contains(&slot.view().unwrap().snapshot().tree, label)
}

#[test]
fn t3_overview_filters_folds_and_expands_live_sessions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile = tempfile::tempdir().unwrap();
    let roots = crate::Roots::profile(
        root.join("app"),
        [],
        Some(crate::AppLayer {
            path: root.join("examples/t3-overview"),
            keep: Vec::new(),
        }),
        None,
        profile.path().to_owned(),
    );
    let mut workspace = Workspace::open_with(roots, false, None);
    assert!(shows(&workspace, "Connecting to T3 Code…"));
    let sample = mlua::Lua::new();
    let snapshot = sample
        .load(
            r##"return { available = true, total = 2, threads = {
                {id="a", title="Review theme", group="waiting", status="running", project="Desktop",
                 provider="codex", updatedAt="today", worktree="/work/a", issues={}, pullRequests={
                    {label="#42", title="desktop", url="https://example.com/pull/42"}}},
                {id="b", title="Fix scroll", group="running", status="starting", project="Mobile",
                 updatedAt="today", issues={}, pullRequests={}},
            }}"##,
        )
        .eval()
        .unwrap();
    workspace.set_host("t3", Data::read(&snapshot).unwrap());
    assert!(shows(&workspace, "Fix scroll"));
    for (key, value, label, visible) in [
        ("tab", "Pull requests", "Fix scroll", false),
        ("thread", "a", "codex · today · /work/a", true),
        ("fold", "waiting", "Review theme", false),
        ("fold", "waiting", "Review theme", true),
        ("tab", "Issues", "No matching sessions", true),
        ("tab", "Worktrees", "Review theme", true),
    ] {
        workspace.send(SHELL, SHELL, Data::map([(key, Data::from(value))]));
        assert_eq!(shows(&workspace, label), visible, "{key} = {value}");
    }
}
