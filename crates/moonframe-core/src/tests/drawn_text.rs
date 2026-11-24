use {
    super::*,
    crate::{Request, TextOf},
};

const PAGE: &str = r##"return mod { region = "main", view = function()
    return ui.column {
        ui.row { ui.text "Inbox", ui.badge { "3" } },
        ui.markdown { text = "# Notes\n\nA *first* line.\n\nA second." },
        ui.popover { open = false, content = ui.text "hidden menu", ui.button { "Menu" } },
        ui.hsplit { ui.pane { "left pane" }, ui.pane { visible = false, "hidden pane" } },
        ui.mount "footer",
        ui.input { id = "draft", value = "typing" },
    }
end }"##;

const FOOTER: &str = r#"return mod { view = function() return ui.text "Footer" end }"#;

#[test]
fn a_mod_reads_as_the_text_it_shows() {
    let dir = app(&[("page", PAGE), ("footer", FOOTER)]);
    let workspace = Workspace::open(dir.path(), false);
    let text = workspace.drawn_text(&TextOf::Mod("page".to_owned()));
    assert_eq!(
        text.as_deref(),
        Some("Inbox 3\n# Notes\nA *first* line.\nA second.\nMenu\nleft pane\nFooter")
    );
    assert_eq!(
        workspace.drawn_text(&TextOf::Region("main".to_owned())),
        text
    );
    assert_eq!(
        workspace.drawn_text(&TextOf::Mod("missing".to_owned())),
        None
    );
}

#[test]
fn asking_for_text_is_a_request_the_app_answers() {
    let asker = r#"return mod {
        init = function() return {} end,
        update = function(state, msg)
            return state, { { to = "app:text", msg = { mod = "page", reply = "page" } } }
        end,
        view = function() return ui.text "asker" end,
    }"#;
    let dir = app(&[("asker", asker), ("page", PAGE), ("footer", FOOTER)]);
    let mut workspace = Workspace::open(dir.path(), false);
    workspace.send("asker", "asker", Data::from("go"));
    let requests = workspace.take_requests();
    assert!(
        requests.iter().any(|(from, request)| from == "asker"
            && matches!(request, Request::Text(asked) if asked.of == Some(TextOf::Mod("page".to_owned())))),
        "{requests:?}"
    );
}
