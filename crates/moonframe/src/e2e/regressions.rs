//! Cases that cross component, parser and window lifetimes.

use super::*;

/// The default inspect shortcut, `secondary-shift-x`, as this platform binds it.
const INSPECT: &str = if cfg!(target_os = "macos") {
    "cmd-shift-x"
} else {
    "ctrl-shift-x"
};

/// A window whose shell mounts the real `mods/feedback`, with the window
/// it opens.
fn with_feedback(cx: &mut TestAppContext) -> Window<'_> {
    let dir = folder(
        r#"
return mod {
    init = function() return { log = "" } end,
    view = function(state)
        return shell(state, ui.column { grow = true,
            ui.row { class = "target", ui.text "Pick me" }, ui.mount "feedback" })
    end,
}"#,
    );
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    std::fs::create_dir(dir.path().join("mods")).unwrap();
    for name in ["feedback", "feedback-window"] {
        std::os::unix::fs::symlink(
            app.join("mods").join(name),
            dir.path().join("mods").join(name),
        )
        .unwrap();
    }
    open_in(cx, dir)
}

impl Window<'_> {
    /// What the feedback window's mod draws, as a string to search: it
    /// opens in a window of its own, which this test window does not draw.
    fn feedback_window(&mut self) -> String {
        let app = self.app.clone();
        self.cx.read_entity(&app, |app, _| {
            format!(
                "{:?}",
                app.workspace
                    .slot("feedback-window")
                    .and_then(|slot| slot.view())
            )
        })
    }

    /// Whether the feedback mod has its window open.
    fn feedback_open(&mut self) -> bool {
        let app = self.app.clone();
        self.cx.read_entity(&app, |app, _| {
            let state = app.workspace.slot("feedback").and_then(|slot| slot.state());
            matches!(state, Some(Ok(moonframe_core::Data::Map(fields))) if fields.get("open") == Some(&moonframe_core::Data::Bool(true)))
        })
    }
}

#[gpui_kit::test]
fn the_inspect_shortcut_opens_the_window_on_a_pick(cx: &mut TestAppContext) {
    let mut window = with_feedback(cx);
    assert!(!window.feedback_window().contains("feedback-chip"));
    window.keys(INSPECT);
    window.click("target");
    assert!(
        !window.feedback_window().contains("feedback-chip"),
        "the card chooses the level first"
    );
    window.keys("enter");
    assert!(window.feedback_open(), "using the pick opens the window");
    let drawn = window.feedback_window();
    assert!(drawn.contains("feedback-chip"), "with the pick attached");
    assert!(
        drawn.contains("Selected in Moonframe"),
        "and its report, for the Copy button"
    );
    assert!(
        !window.shows("feedback-body"),
        "in a window of its own, not over the app"
    );
}

#[gpui_kit::test]
fn the_inspect_shortcut_follows_the_settings_live(cx: &mut TestAppContext) {
    let mut window = with_feedback(cx);
    let app = window.app.clone();
    app.update(window.cx, |app, cx| {
        app.set_inspect_shortcut(Some("cmd-alt-p"), cx)
    });
    window.keys(INSPECT);
    assert!(
        !app.read_with(window.cx, |app, _| app.inspector.picking),
        "the old key"
    );
    window.keys("cmd-alt-p");
    assert!(
        app.read_with(window.cx, |app, _| app.inspector.picking),
        "the new key"
    );
    window.keys("escape");
    app.update(window.cx, |app, cx| app.set_inspect_shortcut(Some(""), cx));
    window.keys("cmd-alt-p");
    assert!(
        !app.read_with(window.cx, |app, _| app.inspector.picking),
        "turned off"
    );
}

#[gpui_kit::test]
fn escaping_the_inspect_shortcut_leaves_the_window_closed(cx: &mut TestAppContext) {
    let mut window = with_feedback(cx);
    window.keys(INSPECT);
    window.keys("escape");
    assert!(!window.feedback_open(), "nothing was picked");
}

#[gpui_kit::test]
fn a_pick_for_a_mod_waits_on_its_card(cx: &mut TestAppContext) {
    let mut window = with_feedback(cx);
    let app = window.app.clone();
    window.keys(INSPECT);
    window.click("target");
    assert!(
        app.read_with(window.cx, |app, _| app.inspector.selected.is_some()),
        "the card shows the pick"
    );
    window.keys("escape");
    assert!(!window.feedback_open(), "cancelled: nothing attached");
    assert!(
        app.read_with(window.cx, |app, _| app.inspector.asker.is_none()),
        "the mod heard there was no pick"
    );
}

#[gpui_kit::test]
fn a_finished_recorder_releases_the_keyboard(cx: &mut TestAppContext) {
    let mut window = open(cx, RECORDER);
    window.click("shortcut-field");
    window.keys("cmd-shift-p");
    window.keys("cmd-k");
    assert_eq!(window.log().matches("keys=").count(), 1, "{}", window.log());
    window.click("shortcut-field");
    window.keys("escape");
    window.keys("cmd-j");
    assert_eq!(window.log().matches("keys=").count(), 1, "{}", window.log());
}

#[gpui_kit::test]
fn a_document_can_open_at_a_requested_heading(cx: &mut TestAppContext) {
    let main = format!(
        r###"{LONG}
return mod {{
    init = function() return {{ log = "" }} end,
    update = function(state, msg) logged(state, "section=" .. msg.heading) end,
    view = function(state)
        return shell(state, ui.markdown {{ text = LONG, grow = true, scroll = true,
            section = "reading", reveal = {{ at = string.find(LONG, "## Section 20", 1, true) - 1 }},
        }})
    end,
}}"###
    );
    let mut window = open(cx, &main);
    assert_eq!(section(&window.log()), 20, "{}", window.log());
}

#[cfg(feature = "math")]
#[gpui_kit::test]
fn inline_math_is_recognized_in_an_appended_paragraph(cx: &mut TestAppContext) {
    let mut window = open(
        cx,
        r#"
return mod {
    init = function() return { log = "", text = "A sufficiently long introduction.\n\n$x" } end,
    update = function(state, msg)
        if msg == "finish" then state.text ..= "$"
        else logged(state, "count=" .. msg.count) end
    end,
    view = function(state)
        return shell(state, ui.column { grow = true,
            ui.row { class = "finish", msg = "finish", ui.text "Finish" },
            ui.markdown { text = state.text, grow = true, find = "$x$", found = "found", stream = true },
        })
    end,
}"#,
    );
    window.click("finish");
    assert!(window.log().contains("count=0"), "{}", window.log());
    assert!(!window.log().contains("count=1"), "{}", window.log());
}

fn chat_source(text: &str) -> String {
    let main = format!(
        r#"local markdown = use "chat-markdown"
return mod {{ view = function() return markdown {{ text = {}, cursor = true }} end }}"#,
        serde_json::to_string(text).unwrap(),
    );
    let dir = folder(&main);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("shell").unwrap();
    assert!(slot.error().is_none(), "{:?}", slot.error());
    let Some(moonframe_core::Node::Markdown(doc)) = slot.view() else {
        panic!("expected chat markdown: {:?}", slot.view());
    };
    doc.text.clone()
}

#[test]
fn the_streaming_cursor_stays_outside_completed_code_fences() {
    for fence in ["```", "~~~~"] {
        for ending in ["", "\n"] {
            let source = chat_source(&format!("{fence}lua\nprint(1)\n{fence}{ending}"));
            let markdown::mdast::Node::Root(root) =
                markdown::to_mdast(&source, &markdown::ParseOptions::gfm()).unwrap()
            else {
                panic!("no document");
            };
            let markdown::mdast::Node::Code(code) = &root.children[0] else {
                panic!("no code block");
            };
            assert_eq!(code.value, "print(1)", "{source}");
            assert!(
                matches!(
                    root.children.last(),
                    Some(markdown::mdast::Node::Paragraph(_))
                ),
                "{source}"
            );
        }
    }
}

#[test]
fn streaming_emphasis_preserves_closed_marks_and_identifiers() {
    for text in [
        "_hello_ world",
        "_hello_",
        "snake_case",
        "__hello__",
        "**hello**",
    ] {
        assert_eq!(chat_source(text), format!("{text} [](cursor:)"));
    }
    assert_eq!(chat_source("_hello"), "_hello_ [](cursor:)");
    assert_eq!(
        chat_source("**hello\nworld"),
        "**hello\nworld** [](cursor:)"
    );
}

const LIGHTBOX: &str = r#"
local lightbox = use "lightbox"
return mod {
    init = function() return { log = "" } end,
    update = function(state, msg) logged(state, "unexpected") end,
    view = function(state)
        return shell(state, lightbox { open = true, dismiss = "close", to = "controller",
            pictures = { { src = "demos/scan.jpg", title = "Scan" } }, change = "viewed" })
    end,
}"#;

#[gpui_kit::test]
fn a_lightbox_fits_when_the_window_is_resized(cx: &mut TestAppContext) {
    let mut window = open(cx, LIGHTBOX);
    for (width, height) in [(800., 600.), (480., 320.), (1200., 900.)] {
        window
            .cx
            .simulate_resize(gpui_kit::size(px(width), px(height)));
        window.settle();
        let bounds = window.bounds("lightbox");
        assert!(
            bounds.origin.x >= px(0.) && bounds.origin.y >= px(0.),
            "{bounds:?}"
        );
        assert!(
            bounds.right() <= px(width) && bounds.bottom() <= px(height),
            "{bounds:?}"
        );
        let bar = window.bounds("lightbox-bar");
        assert!(
            bar.origin.y >= px(0.) && bar.bottom() <= px(height),
            "{bar:?}"
        );
    }
}

#[gpui_kit::test]
fn a_lightbox_sends_zoom_changes_to_its_controller(cx: &mut TestAppContext) {
    let dir = folder(LIGHTBOX);
    // Image resolution only permits files within this app's roots.
    let image = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/components/demos/scan.jpg");
    std::fs::create_dir_all(dir.path().join("demos")).unwrap();
    std::fs::copy(image, dir.path().join("demos/scan.jpg")).unwrap();
    let source = LIGHTBOX.replace(
        "demos/scan.jpg",
        dir.path().join("demos/scan.jpg").to_str().unwrap(),
    );
    std::fs::write(dir.path().join("main.luau"), format!("{PRELUDE}{source}")).unwrap();
    std::fs::create_dir_all(dir.path().join("mods/controller")).unwrap();
    std::fs::write(
        dir.path().join("mods/controller/main.luau"),
        r#"
return mod {
    init = function() return { zoom = 0 } end,
    update = function(state, msg) if msg.msg == "viewed" then state.zoom = msg.zoom end end,
    view = function(state) return ui.text { "zoom=" .. state.zoom } end,
}"#,
    )
    .unwrap();
    let mut window = open_in(cx, dir);
    // The last button is 100%, so this exercises the real viewer's change.
    window.click("image-viewer-button");
    let view = window.cx.read_entity(&window.app, |app, _| {
        format!(
            "{:?}",
            app.workspace
                .slot("controller")
                .and_then(|slot| slot.view())
        )
    });
    assert!(view.contains("zoom=1"), "{view}");
    assert!(!window.log().contains("unexpected"), "{}", window.log());
}

#[gpui_kit::test]
fn a_document_card_opens_when_its_preview_link_is_clicked(cx: &mut TestAppContext) {
    let mut window = open(
        cx,
        r#"
local card = use "document-card"
return mod {
    init = function() return { log = "" } end,
    update = function(state, msg) if msg == "open" then logged(state, "opened") end end,
    view = function(state)
        return shell(state, card { title = "Design", text = "[Design](docs/design.md)", msg = "open" })
    end,
}"#,
    );
    let preview = window.bounds("markdown");
    window.click_at(preview.origin + point(px(12.), px(10.)));
    assert_eq!(window.log(), "|opened");
}
