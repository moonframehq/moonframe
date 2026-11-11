use super::*;

const SPEAKER: &str = r#"
return mod {
  view = function()
    return ui.panel {
      ui.text(`{ui.locale()}|{ui.t("greeting", { name = "Ada", count = 2 })}|{ui.t("missing")}|{ui.number(1234.5)}|{ui.date(1794312000, { style = "long" })}`),
    }
  end,
}
"#;

const ENGLISH: &str = "greeting = { $count ->\n    [one] Hello { $name }\n   *[other] Hello { $name }, { $count } times\n}\n";
const FRENCH: &str = "greeting = Bonjour { $name }\n";

fn speaker() -> tempfile::TempDir {
    let dir = app(&[("speaker", SPEAKER)]);
    write(dir.path(), "locales/en-US/main.ftl", ENGLISH);
    write(dir.path(), "locales/fr/main.ftl", FRENCH);
    dir
}

#[test]
fn a_view_writes_its_words_in_the_language_chosen() {
    let dir = speaker();
    let mut workspace = Workspace::open(dir.path(), false);
    workspace.set_languages(&["en-US"]);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(
        panel_text(&workspace, "speaker"),
        "en-US|Hello Ada, 2 times|missing|1,234.5|November 10, 2026"
    );

    workspace.set_languages(&["fr-FR", "en-US"]);
    assert_eq!(workspace.locale(), "fr");
    assert_eq!(
        panel_text(&workspace, "speaker"),
        "fr|Bonjour Ada|missing|1\u{202f}234,5|10 novembre 2026"
    );
}

#[test]
fn a_broken_message_file_is_a_problem() {
    let dir = speaker();
    write(dir.path(), "locales/fr/broken.ftl", "oops = {\n");
    let mut workspace = Workspace::open(dir.path(), false);
    workspace.set_languages(&["fr"]);
    let problems = workspace.problems();
    assert!(
        problems
            .iter()
            .any(|(name, error)| *name == "locales" && error.to_string().contains("broken.ftl")),
        "{problems:?}"
    );
}

#[test]
fn bad_options_say_what_is_wrong() {
    let error = view_of(
        r#"return mod { view = function() return ui.text(ui.number(1, { decimals = 99 })) end }"#,
    )
    .unwrap_err();
    assert!(error.contains("expected 0 to 20"), "{error}");
    let error = view_of(
        r#"return mod { view = function() return ui.text(ui.date(0, { date = false })) end }"#,
    )
    .unwrap_err();
    assert!(error.contains("shows nothing"), "{error}");
    let error =
        view_of(r#"return mod { view = function() return ui.text(ui.t("x", { n = {} })) end }"#)
            .unwrap_err();
    assert!(error.contains("`n` is a table"), "{error}");
}
