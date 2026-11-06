//! Cases that cross component, parser and window lifetimes.

use super::*;

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
