use super::*;

fn ticked(workspace: &Workspace) -> Vec<(String, bool)> {
    workspace.menus()[0]
        .items
        .iter()
        .filter_map(|entry| match entry {
            crate::MenuEntry::Item(item) => Some((item.label.clone(), item.checked)),
            crate::MenuEntry::Separator => None,
        })
        .collect()
}

#[test]
fn a_menu_item_is_ticked_while_the_shell_state_matches() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "main.luau",
        r#"return mod {
          menus = {
            { title = "View", items = {
              { label = "Library", msg = { layout = "library" }, checked = { layout = "library" } },
              { label = "Journal", msg = { layout = "journal" }, checked = { layout = "journal" } },
              { label = "Always", msg = "x", checked = true },
            } },
          },
          init = function() return { layout = "library" } end,
          update = function(state, msg)
            if type(msg) == "table" then state.layout = msg.layout end
          end,
          view = function() return ui.text "shell" end,
        }"#,
    );
    let mut workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let on = |label: &str, value: bool| (label.to_string(), value);
    assert_eq!(ticked(&workspace), vec![
        on("Library", true),
        on("Journal", false),
        on("Always", true)
    ]);
    assert!(!workspace.take_menus_changed());

    let mut msg = std::collections::BTreeMap::new();
    msg.insert("layout".to_string(), Data::String("journal".into()));
    workspace.send(SHELL, SHELL, Data::Map(msg));
    assert!(workspace.take_menus_changed());
    assert!(!workspace.take_menus_changed());
    assert_eq!(ticked(&workspace), vec![
        on("Library", false),
        on("Journal", true),
        on("Always", true)
    ]);
}

#[test]
fn a_menu_check_must_name_values() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "main.luau",
        r#"return mod {
          menus = { { title = "View", items = { { label = "A", msg = "a", checked = "yes" } } } },
          view = function() return ui.text "shell" end,
        }"#,
    );
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace
            .problems()
            .iter()
            .any(|(_, problem)| problem.to_string().contains("checked")),
        "{:?}",
        workspace.problems()
    );
}
