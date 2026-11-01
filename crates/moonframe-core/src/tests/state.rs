use super::*;

#[test]
fn mod_state_outlives_the_workspace() {
    let dir = app(&[("counter", COUNTER)]);
    let memory = Memory::default();
    let mut workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory.clone())));
    workspace.send("counter", "counter", Data::from("add"));
    workspace.send("counter", "counter", Data::from("add"));
    let saved = memory.0.borrow().0.get("mod:counter").cloned();
    assert_eq!(
        saved,
        Some(Data::Map([("count".to_owned(), Data::Integer(2))].into()))
    );
    // A change saves once; an unchanged state is not saved again.
    let saves = memory.0.borrow().1;
    workspace.reload();
    assert_eq!(memory.0.borrow().1, saves);

    // A new workspace, as at the next launch, starts where the last one left off.
    drop(workspace);
    let workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory)));
    assert_eq!(panel_text(&workspace, "counter"), "count 2");
}

/// A mod that counts, shows the label it was mounted with, and starts from
/// its props' `start`.
const TALLY: &str = r#"
return mod {
  init = function(previous, props)
    return previous or { count = props and props.start or 0 }
  end,
  update = function(state, msg)
    if msg == "add" then state.count += 1 end
  end,
  view = function(state, app, props)
    return ui.text(`{props and props.label or "?"} {state.count}`)
  end,
}
"#;

#[test]
fn a_mod_mounted_twice_keeps_two_states() {
    let dir = app(&[("tally", TALLY)]);
    write(
        dir.path(),
        "main.luau",
        r#"return mod { view = function()
            return ui.column {
              ui.mount { "tally", id = "a", props = { label = "A" } },
              ui.mount { "tally", id = "b", props = { label = "B", start = 10 } },
            }
        end }"#,
    );
    let mut workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(instance_text(&workspace, "tally#a"), "A 0");
    assert_eq!(instance_text(&workspace, "tally#b"), "B 10");
    // A click inside an instance goes to that instance only.
    workspace.send("tally#a", "tally#a", Data::from("add"));
    assert_eq!(instance_text(&workspace, "tally#a"), "A 1");
    assert_eq!(instance_text(&workspace, "tally#b"), "B 10");
}

const CLICKER: &str = r#"
return component {
  title = "Clicker",
  example = { name = "demo" },
  init = function(previous, props) return previous or { clicks = 0 } end,
  update = function(state, msg, props)
    if msg == "click" then
      state.clicks += 1
      return state, { { to = "parent", msg = { clicked = props.name, clicks = state.clicks } } }
    elseif msg == "reset" then
      state.clicks = 0
    end
  end,
  build = function(props, state)
    return ui.button { `{props.name} {state.clicks}`, msg = "click" }
  end,
}
"#;

const USES_CLICKER: &str = r#"
local clicker = use "clicker"
return mod {
  init = function(previous) return previous or { heard = "" } end,
  update = function(state, msg)
    if type(msg) == "table" and msg.clicked then
      state.heard = `{msg.clicked} {msg.clicks}`
      if msg.clicks == 2 then
        return state, { { to = "clicker#one", msg = "reset" } }
      end
    end
  end,
  view = function(state)
    return ui.column {
      clicker { id = "one", name = "one" },
      clicker { id = "two", name = "two" },
      ui.text(state.heard),
    }
  end,
}
"#;

fn button_label(workspace: &Workspace, key: &str) -> String {
    match workspace.slot(key).and_then(crate::Slot::view) {
        Some(Node::Button(button)) => button.label.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

#[test]
fn a_component_with_state_keeps_its_own_and_talks_to_its_parent() {
    let dir = app(&[("host", USES_CLICKER)]);
    write(dir.path(), "components/clicker.luau", CLICKER);
    let mut workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    // Each is the drawing mod's own, under its key.
    assert_eq!(button_label(&workspace, "host/clicker#one"), "one 0");
    assert_eq!(button_label(&workspace, "host/clicker#two"), "two 0");

    workspace.send("host/clicker#one", "host/clicker#one", Data::from("click"));
    assert_eq!(button_label(&workspace, "host/clicker#one"), "one 1");
    assert_eq!(button_label(&workspace, "host/clicker#two"), "two 0");
    // `to = "parent"` reached the mod that drew it.
    let Some(Node::Stack(column)) = workspace.slot("host").and_then(crate::Slot::view) else {
        panic!("no column");
    };
    assert!(matches!(&column.children[2], Node::Text(text) if text.text == "one 1"));

    // The parent reaches its instance by `clicker#one`, and it resets.
    workspace.send("host/clicker#one", "host/clicker#one", Data::from("click"));
    assert_eq!(button_label(&workspace, "host/clicker#one"), "one 0");
}

#[test]
fn an_instance_resumes_its_saved_state_when_drawn_again() {
    let dir = app(&[("tally", TALLY)]);
    // The shell draws the instance only while `shown`.
    write(
        dir.path(),
        "main.luau",
        r#"return mod {
          init = function(previous) return previous or { shown = true } end,
          update = function(state, msg) if msg == "flip" then state.shown = not state.shown end end,
          view = function(state)
            return ui.column { state.shown and ui.mount { "tally", id = "kept" } }
          end,
        }"#,
    );
    let memory = Memory::default();
    let mut workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory.clone())));
    workspace.send("tally#kept", "tally#kept", Data::from("add"));
    workspace.send("tally#kept", "tally#kept", Data::from("add"));
    assert!(memory.0.borrow().0.contains_key("mod:tally#kept"));

    // Hidden: the instance is dropped, its state kept.
    workspace.send(SHELL, SHELL, Data::from("flip"));
    assert!(workspace.slot("tally#kept").is_none());
    workspace.send(SHELL, SHELL, Data::from("flip"));
    assert_eq!(instance_text(&workspace, "tally#kept"), "? 2");

    // And across a restart.
    drop(workspace);
    let workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory)));
    assert_eq!(instance_text(&workspace, "tally#kept"), "? 2");
}

#[test]
fn a_stateless_component_cannot_be_mounted_with_an_id() {
    let dir = app(&[]);
    write(
        dir.path(),
        "main.luau",
        r#"return mod { view = function()
            return ui.column { ui.mount { "nope", id = "x" } }
        end }"#,
    );
    let workspace = Workspace::open(dir.path(), false);
    let problems: Vec<String> = workspace
        .problems()
        .iter()
        .map(|(name, error)| format!("{name}: {error}"))
        .collect();
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("nope#x") && problem.contains("no mod named `nope`")),
        "{problems:?}"
    );
}

#[test]
fn a_previews_instances_stay_inside_the_preview() {
    // Sends everywhere it should not, when told to.
    let leaky = r#"return mod {
      update = function(state, msg)
        if msg == "go" then
          return state, {
            { to = "topic:leak", msg = true },
            { to = "tally#kept", msg = "add" },
            { to = "provider:files", msg = { open = "/" } },
          }
        end
      end,
      view = function() return ui.text "leaky" end,
    }"#;
    let dir = app(&[("tally", TALLY), ("leaky", leaky)]);
    write(
        dir.path(),
        "main.luau",
        r#"return mod { view = function() return ui.column { ui.mount { "tally", id = "kept" } } end }"#,
    );
    // A component whose preview mounts the same ids as the real app.
    write(
        dir.path(),
        "components/mounter.luau",
        r#"return component {
          example = { label = "x" },
          build = function(props)
            return ui.column { ui.mount { "tally", id = "kept" }, ui.mount { "leaky", id = "one" } }
          end,
        }"#,
    );
    let memory = Memory::default();
    let mut workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory.clone())));
    workspace.send("tally#kept", "tally#kept", Data::from("add"));
    assert_eq!(instance_text(&workspace, "tally#kept"), "? 1");
    // The preview's copy is its own, not the real instance.
    assert_eq!(instance_text(&workspace, "demo:mounter/tally#kept"), "? 0");

    workspace.send(
        "demo:mounter/leaky#one",
        "demo:mounter/leaky#one",
        Data::from("go"),
    );
    assert_eq!(
        instance_text(&workspace, "tally#kept"),
        "? 1",
        "the real instance heard it"
    );
    assert!(
        !memory.0.borrow().0.contains_key("topic:leak"),
        "the topic was published"
    );
    assert!(workspace.take_outbox().is_empty(), "a provider was sent to");
    assert!(
        !memory.0.borrow().0.keys().any(|key| key.contains("demo:")),
        "a preview's state was saved"
    );
}

#[test]
fn state_that_cannot_be_kept_says_so() {
    let dir = app(&[(
        "keeps",
        r#"return mod {
          init = function() return { callback = function() end } end,
          view = function() return ui.text "keeps" end,
        }"#,
    )]);
    let mut workspace = Workspace::open(dir.path(), false);
    workspace.reload();
    let error = workspace
        .slot("keeps")
        .unwrap()
        .error()
        .unwrap()
        .to_string();
    assert!(error.contains("not kept"), "{error}");
}
