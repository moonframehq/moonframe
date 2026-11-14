use super::*;

/// Publishes what it is sent on `topic:<msg.topic>`.
const PUBLISHER: &str = r#"
return mod {
  update = function(state, msg)
    return state, { { to = "topic:" .. msg.topic, msg = msg.value } }
  end,
  view = function() return ui.text "publisher" end,
}
"#;

/// Shows the last thing it heard on the topics it subscribes to.
fn listener(subscribe: &str) -> String {
    format!(
        r#"return mod {{
          subscribe = {{ {subscribe} }},
          init = function(previous) return previous or {{ heard = "nothing" }} end,
          update = function(state, msg)
            if type(msg) == "table" and msg.topic then
              state.heard = msg.topic .. "=" .. tostring(type(msg.value) == "table" and msg.value.branch or msg.value)
            end
          end,
          view = function(state, app, props) return ui.text(state.heard) end,
        }}"#
    )
}

fn publish(workspace: &mut Workspace, topic: &str, value: Data) {
    let msg = Data::Map(
        [
            ("topic".to_owned(), Data::from(topic)),
            ("value".to_owned(), value),
        ]
        .into(),
    );
    workspace.send("publisher", "publisher", msg);
}

#[test]
fn a_topic_pushes_to_its_subscribers_and_is_saved() {
    let exact = listener(r#""tabs/main""#);
    let prefix = listener(r#""tabs/*""#);
    let other = listener(r#""theme""#);
    let dir = app(&[
        ("publisher", PUBLISHER),
        ("exact", &exact),
        ("prefix", &prefix),
        ("other", &other),
    ]);
    let memory = Memory::default();
    let mut workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory.clone())));
    publish(&mut workspace, "tabs/main", Data::from("3 tabs"));
    let text = |workspace: &Workspace, name: &str| match workspace
        .slot(name)
        .and_then(crate::Slot::view)
    {
        Some(Node::Text(text)) => text.text.clone(),
        other => panic!("{name}: {other:?}"),
    };
    assert_eq!(text(&workspace, "exact"), "tabs/main=3 tabs");
    assert_eq!(text(&workspace, "prefix"), "tabs/main=3 tabs");
    assert_eq!(text(&workspace, "other"), "nothing");
    assert_eq!(
        memory.0.borrow().0.get("topic:tabs/main"),
        Some(&Data::from("3 tabs"))
    );

    // Provider data is pushed on `provider/<name>`.
    let git = listener(r#""provider/git""#);
    write(dir.path(), "mods/git-watch/main.luau", &git);
    workspace.reload();
    workspace.set_host(
        "git",
        Data::Map([("branch".to_owned(), Data::from("main"))].into()),
    );
    assert_eq!(text(&workspace, "git-watch"), "provider/git=main");

    // A bad name is the sender's problem, and a provider's topic is not a mod's to publish.
    publish(&mut workspace, "has space", Data::Bool(true));
    let error = workspace
        .slot("publisher")
        .unwrap()
        .error()
        .unwrap()
        .to_string();
    assert!(error.contains("a topic is 1 to"), "{error}");
    publish(&mut workspace, "provider/git", Data::Bool(true));
    assert!(workspace.slot("publisher").unwrap().error().is_some());

    // At the next launch the topic is there, and a subscriber hears it as it loads.
    drop(workspace);
    let fresh = Workspace::open_with(dir.path(), false, Some(Box::new(memory)));
    assert_eq!(text(&fresh, "exact"), "tabs/main=3 tabs");
}

#[test]
fn the_apps_own_topics_reach_only_their_mod_and_no_mod_forges_them() {
    let picker = listener(r#""app/inspect""#);
    let dir = app(&[
        ("publisher", PUBLISHER),
        ("picker", &picker),
        ("bystander", &picker),
    ]);
    let mut workspace = Workspace::open_with(dir.path(), false, None);
    let heard = |workspace: &Workspace, name: &str| match workspace
        .slot(name)
        .and_then(crate::Slot::view)
    {
        Some(Node::Text(text)) => text.text.clone(),
        other => panic!("{other:?}"),
    };
    let pick = |what: &str| Data::Map([("branch".to_owned(), Data::from(what))].into());
    workspace.app_topic("inspect", "picker", pick("picked"));
    assert_eq!(heard(&workspace, "picker"), "app/inspect=picked");
    assert_eq!(heard(&workspace, "bystander"), "nothing");

    // Sent straight to a mod, looking like topic news, it is refused.
    let forged = Data::Map(
        [
            ("topic".to_owned(), Data::from("app/inspect")),
            ("value".to_owned(), pick("forged")),
        ]
        .into(),
    );
    workspace.send("publisher", "picker", forged);
    assert_eq!(heard(&workspace, "picker"), "app/inspect=picked");
    assert!(workspace.slot("publisher").unwrap().error().is_some());

    publish(&mut workspace, "app/inspect", Data::Bool(true));
    let error = workspace
        .slot("publisher")
        .unwrap()
        .error()
        .unwrap()
        .to_string();
    // Refused either way: published, or handed to the publisher as news.
    assert!(error.contains("app/"), "{error}");
}

#[test]
fn the_apps_news_reaches_every_module_that_hears_it_and_is_not_kept() {
    let hears = listener(r#""app/url""#);
    let prefix = listener(r#""app/*""#);
    let deaf = listener(r#""tabs/main""#);
    let dir = app(&[("hears", &hears), ("prefix", &prefix), ("deaf", &deaf)]);
    let memory = Memory::default();
    let mut workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory.clone())));
    let heard = |workspace: &Workspace, name: &str| match workspace
        .slot(name)
        .and_then(crate::Slot::view)
    {
        Some(Node::Text(text)) => text.text.clone(),
        other => panic!("{other:?}"),
    };
    workspace.app_news(
        "url",
        Data::Map([("branch".to_owned(), Data::from("notes:/new"))].into()),
    );
    assert_eq!(heard(&workspace, "hears"), "app/url=notes:/new");
    assert_eq!(heard(&workspace, "prefix"), "app/url=notes:/new");
    assert_eq!(heard(&workspace, "deaf"), "nothing");
    assert!(workspace.topic("app/url").is_none());
    assert!(!memory.0.borrow().0.contains_key("topic:app/url"));
}

#[test]
fn an_instance_mounted_late_catches_up_on_its_topics() {
    let watcher = listener(r#""tabs/main""#);
    let dir = app(&[("publisher", PUBLISHER), ("watcher", &watcher)]);
    write(
        dir.path(),
        "main.luau",
        r#"return mod {
          init = function(previous) return previous or { shown = false } end,
          update = function(state, msg) if msg == "show" then state.shown = true end end,
          view = function(state)
            return ui.column { state.shown and ui.mount { "watcher", id = "late" } }
          end,
        }"#,
    );
    let mut workspace = Workspace::open(dir.path(), false);
    publish(&mut workspace, "tabs/main", Data::from("early"));
    workspace.send(SHELL, SHELL, Data::from("show"));
    assert_eq!(instance_text(&workspace, "watcher#late"), "tabs/main=early");
}

#[test]
fn providers_are_wanted_by_what_views_read_and_mods_hear() {
    let reader = r#"return mod { region = "left", view = function(state, app)
        return ui.text(app.git and app.git.branch or "none") end }"#;
    let dir = app(&[
        ("reader", reader),
        (
            "hears",
            &listener(r#""provider/files", "provider/gh*", "tabs/main""#),
        ),
    ]);
    let workspace = Workspace::open(dir.path(), false);
    let wanted = workspace.wanted_providers().unwrap();
    for name in ["git", "files", "gh*"] {
        assert!(wanted.contains(name), "{name} is not wanted: {wanted:?}");
    }
    assert!(!wanted.contains("tabs/main"));
    assert!(!wanted.contains("system"));

    // A view that goes through every key wants them all.
    let all = r#"return mod { region = "left", view = function(state, app)
        local count = 0
        for _ in app do count += 1 end
        return ui.text(tostring(count)) end }"#;
    let dir = app(&[("all", all)]);
    assert_eq!(Workspace::open(dir.path(), false).wanted_providers(), None);
    let dir = app(&[("hears", &listener(r#""provider/*""#))]);
    assert_eq!(Workspace::open(dir.path(), false).wanted_providers(), None);
}
