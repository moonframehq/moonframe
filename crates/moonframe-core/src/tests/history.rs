use {super::*, crate::Paths};

const GRAPH: &str = include_str!("../../../../app/components/commit-graph.luau");

/// Draws the commits `build` makes with `commit-graph`.
fn graph(build: &str) -> (tempfile::TempDir, Workspace) {
    let source = format!(
        r#"
local commit_graph = use "commit-graph"
return mod {{ view = function()
    local commits = {{}}
    {build}
    return commit_graph {{ commits = commits }}
end }}
"#
    );
    let dir = app(&[("graph", &source)]);
    write(dir.path(), "components/commit-graph.luau", GRAPH);
    let workspace = Workspace::open(dir.path(), false);
    (dir, workspace)
}

/// Every `ui.paths` under `node`, in drawing order.
fn paths_in<'a>(node: &'a Node, found: &mut Vec<&'a Paths>) {
    match node {
        Node::Paths(paths) => found.push(paths),
        Node::Stack(stack) => {
            for child in &stack.children {
                paths_in(child, found);
            }
        },
        _ => {},
    }
}

fn drawn(workspace: &Workspace) -> Vec<&Paths> {
    let slot = workspace.slot("graph").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    let mut found = Vec::new();
    paths_in(slot.view().unwrap(), &mut found);
    found
}

#[test]
fn the_most_commits_git_gives_fit_the_host() {
    // A line and a dot each, then two lines and a dot for every merge.
    for (name, parents, lines) in [
        ("linear", "{ sha(row + 1) }", 2 * 500 - 1),
        ("merges", "{ sha(row + 1), sha(row + 2) }", 3 * 500 - 3),
    ] {
        let (_dir, workspace) = graph(&format!(
            r#"
    local function sha(row) return if row <= 500 then `c{{row}}` else nil end
    for row = 1, 500 do
        table.insert(commits, {{ sha = sha(row), parents = {parents} }})
    end
"#
        ));
        let layers = drawn(&workspace);
        assert!(layers.len() > 1, "{name}: drawn in layers");
        assert!(
            layers.iter().all(|layer| layer.paths.len() <= 500),
            "{name}"
        );
        let total: usize = layers.iter().map(|layer| layer.paths.len()).sum();
        assert_eq!(total, lines, "{name}: nothing is lost between layers");
    }
}

#[test]
fn a_fork_takes_a_lane_of_its_own_until_it_joins() {
    let (_dir, workspace) = graph(
        r#"
    commits = {
        { sha = "merge", parents = { "left", "right" } },
        { sha = "left", parents = { "base" } },
        { sha = "right", parents = { "base" } },
        { sha = "base", parents = {} },
    }
"#,
    );
    let layers = drawn(&workspace);
    let [layer] = layers.as_slice() else {
        panic!("one layer holds so few lines");
    };
    // Each dot starts at its right edge: the middle of its lane plus its
    // radius, in lanes 14 wide.
    let dots = layer.paths[layer.paths.len() - 4..].iter();
    let lanes: Vec<f32> = dots
        .map(|dot| (dot.points[0][0] - 2.5 - 7.0) / 14.0)
        .collect();
    assert_eq!(lanes, [0.0, 0.0, 1.0, 0.0]);
}

/// Publishes what it is sent on `tabs/<msg.bar>`, as `tabbed` does.
const BAR: &str = r#"
return mod {
  update = function(state, msg)
    return state, { { to = "topic:tabs/" .. msg.bar, msg = { title = msg.title, tab = { kind = msg.kind } } } }
  end,
  view = function() return ui.text "bar" end,
}
"#;

fn select(workspace: &mut Workspace, bar: &str, kind: &str, title: &str) {
    let msg = Data::Map(
        [
            ("bar".to_owned(), Data::from(bar)),
            ("kind".to_owned(), Data::from(kind)),
            ("title".to_owned(), Data::from(title)),
        ]
        .into(),
    );
    workspace.send("bar", "bar", msg);
}

/// What was asked of git since the last look: `log` of each message.
fn asked(workspace: &mut Workspace) -> Vec<Data> {
    workspace
        .take_outbox()
        .into_iter()
        .filter(|(to, ..)| to == "git")
        .map(|(_, _, msg)| match msg {
            Data::Map(mut fields) => fields.remove("log").unwrap_or_default(),
            other => other,
        })
        .collect()
}

#[test]
fn history_reads_the_log_only_while_a_bar_shows_it() {
    let dir = app(&[
        ("bar", BAR),
        (
            "history",
            include_str!("../../../../app/mods/history/main.luau"),
        ),
    ]);
    write(dir.path(), "components/commit-graph.luau", GRAPH);
    write(
        dir.path(),
        "components/empty-state.luau",
        include_str!("../../../../app/components/empty-state.luau"),
    );
    let mut workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("history").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    assert_eq!(asked(&mut workspace), [], "no tab, no log");

    select(&mut workspace, "main", "component:terminal", "zsh");
    assert_eq!(asked(&mut workspace), [], "another tab asks for nothing");

    select(&mut workspace, "main", "mod:history", "History");
    assert_eq!(asked(&mut workspace), [Data::Integer(200)]);

    select(&mut workspace, "main", "mod:history", "Renamed");
    assert_eq!(asked(&mut workspace), [], "a new title changes no answer");

    // Still shown in the second bar, so the first going changes nothing.
    select(&mut workspace, "bottom", "mod:history", "History");
    select(&mut workspace, "main", "component:terminal", "zsh");
    assert_eq!(asked(&mut workspace), []);

    select(&mut workspace, "bottom", "component:terminal", "zsh");
    assert_eq!(asked(&mut workspace), [Data::Bool(false)]);
}

/// No bar speaks for a copy in a panel of its own, as Hangar's: it asks.
#[test]
fn history_in_a_panel_of_its_own_asks_for_the_log() {
    let panel = r#"
return mod { view = function()
    return ui.mount { "history", id = "graph", props = { always = true, compact = true } }
end }
"#;
    let dir = app(&[
        ("panel", panel),
        (
            "history",
            include_str!("../../../../app/mods/history/main.luau"),
        ),
    ]);
    write(dir.path(), "components/commit-graph.luau", GRAPH);
    write(
        dir.path(),
        "components/empty-state.luau",
        include_str!("../../../../app/components/empty-state.luau"),
    );
    let mut workspace = Workspace::open(dir.path(), false);
    assert_eq!(asked(&mut workspace), [Data::Integer(200)]);
}
