//! Exercise the actual pattern recipes and browser with the headless host.
use {
    moonframe_core::{AppLayer, Data, Roots, Workspace},
    std::path::{Path, PathBuf},
};

fn workspace() -> Workspace {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Workspace::open(
        Roots::hosted(
            root.join("app"),
            AppLayer {
                path: root.join("patterns/app"),
                keep: vec![],
            },
            Vec::<PathBuf>::new(),
            None,
        ),
        false,
    )
}

fn healthy(workspace: &Workspace) {
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
}

fn send(workspace: &mut Workspace, target: &str, key: &str, value: &str) {
    workspace.send(target, target, Data::map([(key, value.into())]));
    healthy(workspace);
}

fn drawn(workspace: &Workspace, name: &str) -> String {
    format!("{:?}", workspace.slot(name).and_then(|slot| slot.view()))
}

#[test]
fn patterns_search_categories_and_each_detail_render() {
    let mut workspace = workspace();
    healthy(&workspace);
    send(&mut workspace, "catalog", "category", "Navigation");
    let view = drawn(&workspace, "catalog");
    assert!(view.contains("Workspace switcher"));
    assert!(!view.contains("Issue card"));
    send(&mut workspace, "catalog", "category", "All patterns");
    workspace.send(
        "catalog",
        "catalog",
        Data::map([("msg", "query".into()), ("value", "PRIORITY-ICON".into())]),
    );
    healthy(&workspace);
    let view = drawn(&workspace, "catalog");
    assert!(view.contains("Issue card"));
    assert!(!view.contains("Workspace switcher"));
    let names: Vec<_> = workspace
        .components()
        .iter()
        .filter(|component| {
            component
                .path
                .parent()
                .is_some_and(|parent| parent.ends_with("patterns/app/components"))
                && component.name != "pattern-browser"
        })
        .map(|component| component.name.clone())
        .collect();
    assert!(!names.is_empty());
    for name in names {
        send(&mut workspace, "catalog", "select", &name);
        send(&mut workspace, "catalog", "code", "source");
    }
}

#[test]
fn recipes_respond_without_changing_another_preview() {
    let mut workspace = workspace();
    healthy(&workspace);
    let before = drawn(&workspace, "preview-issue-card");
    workspace.send(
        "preview-workspace-switcher",
        "preview-workspace-switcher",
        "toggle".into(),
    );
    healthy(&workspace);
    send(
        &mut workspace,
        "preview-workspace-switcher",
        "workspace",
        "personal",
    );
    assert!(drawn(&workspace, "preview-workspace-switcher").contains("Personal"));
    workspace.send(
        "preview-getting-started",
        "preview-getting-started",
        "connect".into(),
    );
    healthy(&workspace);
    assert!(drawn(&workspace, "preview-getting-started").contains("3 / 4"));
    assert_eq!(drawn(&workspace, "preview-issue-card"), before);
}
