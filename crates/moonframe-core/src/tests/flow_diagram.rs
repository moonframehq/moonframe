use {super::*, crate::Stack};

const COMPONENT: &str = include_str!("../../../../app/components/flow-diagram.luau");

/// Three nodes in an L: `web` and `api` in one row, `db` under `api`.
const NODES: &str = r#"nodes = {
    { id = "web", label = "Web", col = 0, row = 0 },
    { id = "api", label = "API", col = 1, row = 0 },
    { id = "db", label = "Database", col = 1, row = 1 },
}"#;

/// What the diagram drew with `props` beside [`NODES`]: the children of its box.
fn diagram(props: &str) -> Vec<Node> {
    let source = format!(
        "local flow_diagram = use \"flow-diagram\"\nreturn mod {{ view = function()\n  return \
         flow_diagram {{ {NODES}, {props} }}\nend }}\n"
    );
    let dir = app(&[("flow", &source)]);
    write(dir.path(), "components/flow-diagram.luau", COMPONENT);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("flow").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    let Some(Node::Stack(root)) = slot.view() else {
        panic!("expected the diagram's box");
    };
    root.children.clone()
}

fn keys(children: &[Node]) -> Vec<&str> {
    children
        .iter()
        .filter_map(|child| match child {
            Node::Stack(stack) => stack.key.as_deref(),
            _ => None,
        })
        .collect()
}

/// The card of node `id`: inside the middle row of its keyed cell.
fn card<'a>(children: &'a [Node], id: &str) -> &'a Stack {
    let cell = children.iter().find_map(|child| match child {
        Node::Stack(stack) if stack.key.as_deref() == Some(id) => Some(stack),
        _ => None,
    });
    let row = match cell.map(|cell| &cell.children[1]) {
        Some(Node::Stack(row)) => row,
        _ => panic!("no cell row for {id}"),
    };
    match row.children.first() {
        Some(Node::Stack(card)) => card,
        _ => panic!("no card for {id}"),
    }
}

/// Every text under `node`, in order.
fn texts(node: &Node) -> Vec<String> {
    match node {
        Node::Text(text) => vec![text.text.clone()],
        Node::Stack(stack) => stack.children.iter().flat_map(texts).collect(),
        _ => Vec::new(),
    }
}

/// Where each token's box is centred, as fractions of the box (the dot
/// itself, on lane 0).
fn tokens(children: &[Node]) -> Vec<(f32, f32)> {
    children
        .iter()
        .filter_map(|child| match child {
            Node::Stack(stack) => {
                let token = stack.children.iter().any(|inner| {
                    matches!(inner, Node::Stack(dot)
                        if dot.look.class.iter().any(|class| class == "flow-diagram-token"))
                });
                let area = stack.frame.area.as_ref()?;
                if !token {
                    return None;
                }
                Some((
                    area.left? + area.width? / 2.0,
                    area.top? + area.height? / 2.0,
                ))
            },
            _ => None,
        })
        .collect()
}

fn has_class(node: &Node, class: &str) -> bool {
    let look = match node {
        Node::Stack(stack) => &stack.look,
        Node::Icon(icon) => &icon.look,
        _ => return false,
    };
    look.class.iter().any(|each| each == class)
}

/// The layers that hold a link's line, and those that hold its head.
fn link_layers<'a>(children: &'a [Node], inner: &str) -> Vec<&'a Stack> {
    children
        .iter()
        .filter_map(|child| match child {
            Node::Stack(stack) if stack.children.iter().any(|each| has_class(each, inner)) => {
                Some(stack)
            },
            _ => None,
        })
        .collect()
}

fn sent<'a>(card: &'a Stack, key: &str) -> Option<&'a str> {
    match &card.action {
        Some(Action::Send { msg, .. }) => msg.get(key).and_then(Data::as_str),
        _ => None,
    }
}

#[test]
fn flow_diagram_draws_level_and_upright_links_and_leaves_out_a_slanted_one() {
    let still = diagram("");
    let linked = diagram(
        r#"links = { { from = "web", to = "api" }, { from = "api", to = "db" }, { from = "web", to = "db" } }"#,
    );
    // Each node is a keyed cell; the two links it can draw come before them,
    // each as a line and a head.
    assert_eq!(keys(&still), ["web", "api", "db"]);
    assert_eq!(keys(&linked), ["web", "api", "db"]);
    assert_eq!(linked.len() - still.len(), 4);
}

#[test]
fn flow_diagram_tokens_show_only_with_a_time() {
    let flows = r#"links = { { from = "web", to = "api" }, { from = "api", to = "db" } },
        flows = { { path = { "web", "api", "db" }, tokens = { "warning", "danger" } } }"#;
    let still = diagram(flows);
    let moving = diagram(&format!("{flows}, time = 1.5"));
    // A flow with no tokens draws none, and breaks nothing.
    let bare = diagram(&format!(
        "{flows}, time = 1.5, flows = {{ {{ path = {{ \"web\", \"api\" }} }} }}"
    ));
    assert_eq!(bare.len(), still.len());
    assert_eq!(moving.len() - still.len(), 2);
}

#[test]
fn flow_diagram_a_click_on_a_node_sends_its_id() {
    let plain = diagram(r#"selected = "api""#);
    assert_eq!(sent(card(&plain, "web"), "select"), Some("web"));
    assert!(card(&plain, "api").selected);
    assert!(!card(&plain, "web").selected);
    let named = diagram(r#"event = "node""#);
    assert_eq!(sent(card(&named, "db"), "node"), Some("db"));
}

#[test]
fn flow_diagram_draws_every_link_word_over_the_cards() {
    let drawn = diagram(
        r#"links = { { from = "web", to = "api", label = "HTTPS" }, { from = "api", to = "db", label = "SQL" } }"#,
    );
    // Both words come after the last card, so no card covers them, and the
    // word of the upright link is drawn too.
    let last_cell = drawn
        .iter()
        .rposition(|child| matches!(child, Node::Stack(stack) if stack.key.is_some()))
        .unwrap();
    let after: Vec<String> = drawn[last_cell + 1..].iter().flat_map(texts).collect();
    assert_eq!(after, ["HTTPS", "SQL"]);
}

#[test]
fn flow_diagram_token_speed_is_the_same_in_any_grid() {
    // One token from `web` to `api`, neighbours: at 0.5 cells a second it
    // is half way after one second, however many columns the grid has.
    let flow = r#"links = { { from = "web", to = "api" } },
        flows = { { path = { "web", "api" }, tokens = { "warning" } } }, time = 1, speed = 0.5"#;
    for columns in [2.0_f32, 6.0] {
        let drawn = diagram(&format!("{flow}, columns = {columns}"));
        let [(x, _)] = tokens(&drawn)[..] else {
            panic!("expected one token");
        };
        let (web, api) = (0.5 / columns, 1.5 / columns);
        let along = (x - web) / (api - web);
        assert!((along - 0.5).abs() < 0.01, "{columns} columns: {along}");
    }
}

#[test]
fn flow_diagram_a_card_never_takes_more_than_its_cell() {
    let drawn = diagram("");
    let web = card(&drawn, "web");
    // No fixed width: it grows to its size at most, and narrows in a narrow cell.
    assert_eq!(web.frame.width, None);
    assert_eq!(web.frame.max_width, Some(136.0));
    assert!(web.frame.grow);
    let small = diagram(r#"size = "small""#);
    assert_eq!(card(&small, "web").frame.max_width, Some(126.0));
}

#[test]
fn flow_diagram_tokens_skip_a_slanted_step() {
    let slanted = diagram(
        r#"links = { { from = "web", to = "db" } },
        flows = { { path = { "web", "db" }, tokens = { "warning" } } }, time = 1"#,
    );
    assert!(
        tokens(&slanted).is_empty(),
        "no token on a link that is left out"
    );
    // `web` to `db` is slanted and left out; `db` to `api` is upright, in
    // column 1 of 2: every token stays on it.
    let mixed = diagram(
        r#"links = { { from = "web", to = "db" }, { from = "api", to = "db" } },
        flows = { { path = { "web", "db", "api" }, tokens = { "warning", "danger", "success" } } }, time = 0.3"#,
    );
    let drawn = tokens(&mixed);
    assert_eq!(drawn.len(), 3);
    for (x, _) in drawn {
        assert!(
            (x - 0.75).abs() < 0.001,
            "a token left the upright link: {x}"
        );
    }
}

#[test]
fn flow_diagram_a_link_line_runs_from_centre_to_centre() {
    let drawn = diagram(r#"links = { { from = "web", to = "api" }, { from = "api", to = "db" } }"#);
    // Under the cards from one centre to the other, with nothing beside it:
    // whatever gap two narrow cards leave, the line shows in it.
    let lines = link_layers(&drawn, "flow-diagram-line");
    assert_eq!(lines.len(), 2);
    for line in &lines {
        assert_eq!(line.children.len(), 1, "the line shares its layer");
    }
    let area = |stack: &Stack| stack.frame.area.unwrap();
    let (level, upright) = (area(lines[0]), area(lines[1]));
    assert_eq!((level.left, level.width), (Some(0.25), Some(0.5)));
    assert_eq!((upright.top, upright.height), (Some(0.25), Some(0.5)));
}

#[test]
fn flow_diagram_a_link_head_meets_the_card_it_points_at() {
    let drawn = diagram(r#"links = { { from = "api", to = "web" }, { from = "api", to = "db" } }"#);
    let heads = link_layers(&drawn, "flow-diagram-head");
    assert_eq!(heads.len(), 2);
    // Next to the card it points at, half the card away from its centre,
    // less the 4 pixels a small chevron leaves past its tip.
    let edge = |stack: &Stack, at: usize| match &stack.children[at] {
        Node::Stack(gap) if !gap.frame.shrink => (gap.frame.width, gap.frame.height),
        _ => panic!("no fixed gap at {at}"),
    };
    // `api` to `web` points left: the gap is first. `api` to `db` points down: last.
    assert_eq!(edge(heads[0], 0), (Some(136.0 / 2.0 - 4.0), None));
    assert!(has_class(&heads[0].children[1], "flow-diagram-head"));
    let last = heads[1].children.len() - 1;
    assert_eq!(edge(heads[1], last), (None, Some(60.0 / 2.0 - 4.0)));
    assert!(has_class(&heads[1].children[last - 1], "flow-diagram-head"));
}

#[test]
fn flow_diagram_a_card_sits_on_an_opaque_plate() {
    let drawn = diagram(r#"selected = "api""#);
    // The plate takes the click and the size; the tinted face is inside it,
    // so a picked card shows the plate, not the lines and tokens under it.
    for (id, picked) in [("api", true), ("web", false)] {
        let plate = card(&drawn, id);
        assert!(
            plate
                .look
                .class
                .iter()
                .any(|class| class == "flow-diagram-plate")
        );
        let [Node::Stack(face)] = &plate.children[..] else {
            panic!("expected one face on the plate of {id}");
        };
        assert!(
            face.look
                .class
                .iter()
                .any(|class| class == "flow-diagram-node")
        );
        assert_eq!((plate.selected, face.selected), (picked, picked));
    }
}

#[test]
fn flow_diagram_tokens_travel_only_drawn_links() {
    // `web` to `api` has a link drawn the other way round: tokens travel it.
    // `api` to `db` has none, and `typo` names no node: neither is a step,
    // and no step jumps from `api` over the unknown node to `db`.
    let drawn = diagram(
        r#"links = { { from = "api", to = "web" } },
        flows = { { path = { "web", "api", "typo", "db" }, tokens = { "warning", "danger" } } }, time = 0.4"#,
    );
    let found = tokens(&drawn);
    assert_eq!(found.len(), 2);
    for (x, y) in found {
        assert!((y - 0.25).abs() < 0.001, "a token left the level link: {y}");
        assert!((0.25..=0.75).contains(&x), "a token left the link: {x}");
    }
    let unlinked =
        diagram(r#"flows = { { path = { "web", "api" }, tokens = { "warning" } } }, time = 0.4"#);
    assert!(tokens(&unlinked).is_empty(), "no token without a link");
}

#[test]
fn flow_diagram_lanes_sit_a_fixed_number_of_pixels_apart() {
    let flows = |lane: i32| {
        format!(
            r#"links = {{ {{ from = "web", to = "api" }} }},
            flows = {{ {{ path = {{ "web", "api" }}, tokens = {{ "warning" }}, lane = {lane} }} }}, time = 0.4"#
        )
    };
    // A spacer of twice the gap on one side of the dot, in pixels, in a
    // column: down for lane 1, up for lane -1.
    let spacer = |drawn: &[Node]| -> (usize, Option<f32>) {
        let layer = drawn
            .iter()
            .find_map(|child| match child {
                Node::Stack(stack)
                    if stack
                        .children
                        .iter()
                        .any(|each| has_class(each, "flow-diagram-token")) =>
                {
                    Some(stack)
                },
                _ => None,
            })
            .expect("a token");
        let at = layer
            .children
            .iter()
            .position(|each| !has_class(each, "flow-diagram-token"))
            .expect("a spacer");
        match &layer.children[at] {
            Node::Stack(gap) => (at, gap.frame.height),
            _ => panic!("the spacer is a row"),
        }
    };
    assert_eq!(spacer(&diagram(&flows(1))), (0, Some(14.0)));
    assert_eq!(spacer(&diagram(&flows(-1))), (1, Some(14.0)));
}

#[test]
fn flow_diagram_a_negative_cell_moves_the_grid_over() {
    let drawn = diagram(
        r#"nodes = {
        { id = "web", label = "Web", col = -1, row = 0 },
        { id = "api", label = "API", col = 0, row = 0 },
    }"#,
    );
    let left = |id: &str| {
        drawn
            .iter()
            .find_map(|child| match child {
                Node::Stack(stack) if stack.key.as_deref() == Some(id) => stack.frame.area,
                _ => None,
            })
            .and_then(|area| area.left)
    };
    assert_eq!(keys(&drawn), ["web", "api"]);
    assert_eq!((left("web"), left("api")), (Some(0.0), Some(0.5)));
}
