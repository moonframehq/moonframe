use super::*;

#[test]
fn browsers_take_web_addresses_and_commands_name_them() {
    use crate::{Browser, BrowserCommand};
    let source = r#"return mod { view = function()
        return ui.column {
            ui.browser "https://example.com",
            ui.browser { id = "docs", url = "file:///tmp/a.html", zoom = 2, navigate = { go = true } },
            ui.button { "Back", command = "browser-back", browser = "docs" },
            ui.button { "Open", command = "browser-open", browser = "docs", to = "other" },
        }
    end }"#;
    let dir = app(&[("web", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("web").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    let Some(Node::Stack(column)) = slot.view() else {
        panic!("expected a column");
    };
    let [
        Node::Browser(first),
        Node::Browser(docs),
        Node::Button(back),
        Node::Button(open),
    ] = &column.children[..]
    else {
        panic!("unexpected children {:?}", column.children);
    };
    assert_eq!(
        (first.id.as_str(), first.url.as_deref()),
        ("browser", Some("https://example.com"))
    );
    let Browser { zoom, navigate, .. } = docs;
    assert_eq!((*zoom, navigate.is_some()), (2.0, true));
    assert_eq!(
        back.action,
        Some(Action::Browser {
            to: None,
            id: "docs".into(),
            command: BrowserCommand::Back
        })
    );
    assert_eq!(
        open.action,
        Some(Action::Browser {
            to: Some("other".into()),
            id: "docs".into(),
            command: BrowserCommand::Open
        })
    );

    for (view, expected) in [
        (
            r#"ui.browser "javascript:alert(1)""#,
            "not an http, https or file address",
        ),
        (
            r#"ui.browser "vscode://x""#,
            "not an http, https or file address",
        ),
        (
            r#"ui.browser { url = "https://a b" }"#,
            "not an http, https or file address",
        ),
        (
            r#"ui.button { "x", command = "browser-back" }"#,
            "needs `browser",
        ),
        (
            r#"ui.button { "x", msg = "m", browser = "a" }"#,
            "`browser` goes with a browser command",
        ),
        (
            r#"ui.button { "x", command = "quit", browser = "a" }"#,
            "`browser` goes with a browser command",
        ),
        (
            r#"ui.button { "x", command = "browser-go", browser = "a" }"#,
            "browser-back",
        ),
    ] {
        let source = format!("return mod {{ view = function() return {view} end }}");
        let dir = app(&[("bad", &source)]);
        let workspace = Workspace::open(dir.path(), false);
        let error = workspace
            .slot("bad")
            .unwrap()
            .error()
            .map(ToString::to_string);
        assert!(
            error
                .as_deref()
                .is_some_and(|error| error.contains(expected)),
            "{view}: {error:?}"
        );
    }
}

#[test]
fn images_name_a_file_and_refuse_urls() {
    use {crate::Fit, std::path::PathBuf};
    let source = r#"return mod { view = function()
        return ui.row {
          ui.image "logo.png",
          ui.image { src = "~/Pictures/me.png", width = 32, height = 32, fit = "cover", class = "avatar" },
          ui.image { src = "/tmp/abs.png" },
        }
    end }"#;
    let dir = app(&[("pics", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("pics").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    let Some(Node::Stack(row)) = slot.view() else {
        panic!("no row");
    };
    let images: Vec<_> = row
        .children
        .iter()
        .map(|child| match child {
            Node::Image(image) => image.clone(),
            other => panic!("expected an image, got {other:?}"),
        })
        .collect();
    assert_eq!(images[0].fit, Fit::Contain);
    assert_eq!(
        (images[1].width, images[1].height),
        (Some(32.0), Some(32.0))
    );
    assert_eq!(images[1].fit, Fit::Cover);
    assert_eq!(images[1].look.class, ["avatar"]);
    // Each resolves against the mod's folder or home, inside what is allowed.
    let root = tempfile::tempdir().unwrap();
    let real = root.path().canonicalize().unwrap();
    write(&real, "app/mods/pics/logo.png", "png");
    write(&real, "home/Pictures/me.png", "png");
    let folder = real.join("app/mods/pics");
    let home = real.join("home");
    let resolved: Vec<Option<PathBuf>> = images
        .iter()
        .map(|image| image.resolve(&folder, Some(&home), std::slice::from_ref(&real)))
        .collect();
    assert_eq!(
        resolved,
        [
            Some(real.join("app/mods/pics/logo.png")),
            Some(real.join("home/Pictures/me.png")),
            None,
        ]
    );

    for (view, expected) in [
        (
            r#"ui.image "https://example.com/a.png""#,
            "images are files, not https:// URLs",
        ),
        (r#"ui.image { src = "" }"#, "an empty path"),
        (r#"ui.image {}"#, "image.src: missing"),
        (
            r#"ui.image { src = "a.png", fit = "stretch" }"#,
            "unknown `stretch`",
        ),
        (
            r#"ui.image { src = "a.png", alt = "x" }"#,
            "unknown key `alt`",
        ),
    ] {
        let source = format!("return mod {{ view = function() return {view} end }}");
        let dir = app(&[("bad", &source)]);
        let workspace = Workspace::open(dir.path(), false);
        let error = workspace.slot("bad").unwrap().error().unwrap().to_string();
        assert!(error.contains(expected), "{view}: {error}");
    }
}

#[test]
fn images_come_only_from_the_app_folders_and_the_cache() {
    let app = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(app.path(), "mods/pic/avatar.png", "png");
    write(outside.path(), "secret.png", "png");
    let image = |src: &str| crate::Image {
        look: crate::Look::default(),
        src: src.to_owned(),
        width: None,
        height: None,
        fit: crate::Fit::Contain,
        zoom: None,
        center: None,
        measure: None,
        to: None,
        overlay: Vec::new(),
        extras: crate::Extras::default(),
    };
    let folder = app.path().join("mods/pic");
    let allowed = [app.path().to_owned()];
    let shown = |src: &str| image(src).resolve(&folder, None, &allowed).is_some();
    assert!(shown("avatar.png"));
    assert!(!shown("missing.png"));
    // Out of the app's folders, by a path or by `..`: an empty box.
    let secret = outside.path().join("secret.png");
    assert!(!shown(secret.to_str().unwrap()));
    let named = |path: &Path| {
        path.components()
            .filter(|part| matches!(part, std::path::Component::Normal(_)))
            .collect::<PathBuf>()
    };
    let up = "../".repeat(named(&folder).components().count());
    let relative = format!("{up}{}", named(&secret).display());
    assert!(!shown(&relative));
    // A folder is not a picture.
    assert!(!shown("."));
}

#[test]
fn buttons_take_an_icon_and_text_takes_lines_and_wrap() {
    let source = r#"return mod { view = function()
  return ui.row {
    ui.button { icon = "plus", msg = "add" },
    ui.text { "a long body", lines = 3 },
    ui.text { "fn main() {}", wrap = false },
  }
end }"#;
    let Node::Stack(row) = view_of(source).unwrap() else {
        panic!("expected a row");
    };
    let Node::Button(button) = &row.children[0] else {
        panic!("expected a button");
    };
    assert_eq!(button.label, "");
    assert_eq!(
        button.icon.as_ref().map(crate::IconName::name),
        Some("plus")
    );
    let (Node::Text(body), Node::Text(code)) = (&row.children[1], &row.children[2]) else {
        panic!("expected texts");
    };
    assert_eq!((body.lines, body.wrap), (Some(3), true));
    assert!(!code.wrap);
    let missing = r#"return mod { view = function() return ui.button { msg = "x" } end }"#;
    assert!(view_of(missing).unwrap_err().contains("label"));
}

#[test]
fn only_web_addresses_count_as_remote() {
    use crate::{is_file_url, is_remote_url, is_web_url};
    assert!(is_remote_url("https://example.com/a"));
    assert!(is_remote_url("HTTP://example.com"));
    // A file is a page a browser may show, but never one for the system
    // browser, which would open whatever program it is.
    assert!(is_web_url("file:///Users/me/notes.html"));
    assert!(!is_remote_url("file:///Applications/Calculator.app"));
    assert!(is_file_url("FILE:///tmp/a.html"));
    assert!(!is_file_url("https://example.com"));
    assert!(!is_remote_url("javascript:alert(1)"));
}

#[test]
fn typed_addresses_go_where_a_browser_would() {
    use crate::typed_address;
    let go = |text: &str| typed_address(text);
    assert_eq!(
        go("  https://a.test/x  ").as_deref(),
        Some("https://a.test/x")
    );
    assert_eq!(go("example.com").as_deref(), Some("https://example.com"));
    assert_eq!(
        go("docs.rs/serde?x=1#top").as_deref(),
        Some("https://docs.rs/serde?x=1#top")
    );
    assert_eq!(
        go("localhost:3000/a").as_deref(),
        Some("http://localhost:3000/a")
    );
    assert_eq!(
        go("127.0.0.1:8080").as_deref(),
        Some("http://127.0.0.1:8080")
    );
    assert_eq!(
        go("file:///tmp/a.html").as_deref(),
        Some("file:///tmp/a.html")
    );
    // Words, and anything that is not a host, are a search.
    assert_eq!(
        go("rust borrow checker").as_deref(),
        Some("https://duckduckgo.com/?q=rust+borrow+checker")
    );
    assert_eq!(
        go("serde").as_deref(),
        Some("https://duckduckgo.com/?q=serde")
    );
    assert_eq!(
        go("a&b=c é").as_deref(),
        Some("https://duckduckgo.com/?q=a%26b%3Dc+%C3%A9")
    );
    assert_eq!(
        go(".hidden").as_deref(),
        Some("https://duckduckgo.com/?q=.hidden")
    );
    // An address with a scheme the browser can't show goes nowhere.
    assert_eq!(
        go("javascript:alert(1)").as_deref(),
        Some("https://duckduckgo.com/?q=javascript%3Aalert%281%29")
    );
    assert_eq!(go("vscode://x"), None);
    assert_eq!(go("   "), None);
}

/// The one element `view` returns, or the problem reading it.
fn input_view(view: &str) -> Result<Node, String> {
    let source = format!("return mod {{ view = function() return {view} end }}");
    let dir = app(&[("field", &source)]);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("field").unwrap();
    match slot.error() {
        Some(error) => Err(error.to_string()),
        None => Ok(slot.view().unwrap().clone()),
    }
}

#[test]
fn inputs_read_their_text_messages_and_keys() {
    use crate::{Input, KeyChord};
    let Ok(Node::Input(input)) = input_view(
        r#"ui.input {
          id = "search", value = "abc", placeholder = "Search…",
          change = { query = true }, submit = "go", to = "results",
          keys = { up = "previous", ["shift-tab"] = { back = 1 }, ["secondary-k"] = "clear" },
          focus = true, mono = true, width = 240, class = "search",
        }"#,
    ) else {
        panic!("no input");
    };
    assert_eq!(input.id.as_deref(), Some("search"));
    assert_eq!(input.value.as_deref(), Some("abc"));
    assert_eq!(input.placeholder.as_deref(), Some("Search…"));
    assert_eq!(input.to.as_deref(), Some("results"));
    assert_eq!(input.submit, Some(Data::from("go")));
    assert!(input.focus && input.mono && !input.multiline && !input.disabled);
    assert_eq!((input.width, input.grow), (Some(240.0), false));
    assert_eq!(input.look.class, ["search"]);
    // Sorted by name, so the order is the same on every render.
    let keys: Vec<_> = input.keys.iter().map(|(chord, _)| chord.clone()).collect();
    assert_eq!(
        keys,
        [
            KeyChord::parse("secondary-k").unwrap(),
            KeyChord::parse("shift-tab").unwrap(),
            KeyChord::parse("up").unwrap(),
        ]
    );
    assert!(keys[0].secondary && keys[0].key == "k");
    assert!(keys[1].shift && keys[1].key == "tab" && !keys[1].ctrl);

    // A table message gets `value`; anything else is wrapped.
    let change = input.change.as_ref().unwrap();
    assert_eq!(
        Input::message(change, "ab"),
        Data::Map(
            [
                ("query".to_owned(), Data::Bool(true)),
                ("value".to_owned(), Data::from("ab")),
            ]
            .into()
        )
    );
    assert_eq!(
        Input::message(&Data::from("go"), "ab"),
        Data::Map(
            [
                ("msg".to_owned(), Data::from("go")),
                ("value".to_owned(), Data::from("ab")),
            ]
            .into()
        )
    );
    // A list can't take a named key, so it is wrapped too.
    let list = Data::List(vec![Data::Integer(1)]);
    assert_eq!(
        Input::message(&list, ""),
        Data::Map(
            [
                ("msg".to_owned(), list.clone()),
                ("value".to_owned(), Data::from("")),
            ]
            .into()
        )
    );

    let Ok(Node::Input(lines)) = input_view("ui.input { multiline = true, rows = 6, grow = true }")
    else {
        panic!("no multiline input");
    };
    assert!(lines.multiline && lines.grow);
    assert_eq!((lines.rows, lines.id.as_deref()), (6, None));
    let Ok(Node::Input(plain)) = input_view("ui.input {}") else {
        panic!("no plain input");
    };
    assert_eq!(plain.rows, Input::DEFAULT_ROWS);
    assert!(plain.keys.is_empty() && plain.change.is_none() && plain.browser.is_none());
    let Ok(Node::Input(bar)) = input_view("ui.input { browser = 'docs', to = 'pages' }") else {
        panic!("no address bar");
    };
    assert_eq!(bar.browser.as_deref(), Some("docs"));
}

#[test]
fn markdown_reads_what_a_mod_asks_of_a_document() {
    use crate::{Markdown, Reveal};
    let Ok(Node::Markdown(doc)) = input_view(
        r##"ui.markdown { "# Notes", lines = 4, images = true, image = "open",
            outline = "headings", section = "reading", reveal = { at = 12, seq = 3 },
            find = "note", find_index = 2, found = "found", stream = true, to = "reader" }"##,
    ) else {
        panic!("no markdown");
    };
    assert_eq!(doc.lines, Some(4));
    assert!(doc.images && doc.stream);
    assert_eq!(doc.reveal, Some(Reveal { at: 12, seq: 3 }));
    assert_eq!(doc.find.as_deref(), Some("note"));
    assert_eq!(doc.find_index, Some(2));
    assert_eq!(doc.to.as_deref(), Some("reader"));
    assert_eq!(
        Markdown::note(&Data::from("found"), [("count", Data::Integer(3))]),
        Data::Map(
            [
                ("msg".to_owned(), Data::from("found")),
                ("count".to_owned(), Data::Integer(3)),
            ]
            .into()
        ),
    );
}

#[test]
fn markdown_refuses_what_makes_no_sense() {
    for (source, expected) in [
        ("ui.markdown { 'a', lines = 0 }", "whole number of lines"),
        ("ui.markdown { 'a', lines = 1.5 }", "whole number of lines"),
        (
            "ui.markdown { 'a', lines = 2, scroll = true }",
            "not for `scroll`",
        ),
        ("ui.markdown { 'a', find_index = 0 }", "from 1"),
        ("ui.markdown { 'a', reveal = {} }", "needs `at`"),
    ] {
        let Err(error) = input_view(source) else {
            panic!("{source} should fail");
        };
        assert!(error.to_string().contains(expected), "{source}: {error}");
    }
}

#[test]
fn an_input_captures_keys_and_reports_what_is_selected() {
    use crate::Input;
    let Ok(Node::Input(input)) = input_view(r#"ui.input { capture = "chord", to = "keys" }"#)
    else {
        panic!("no input");
    };
    assert_eq!(input.capture, Some(Data::from("chord")));
    let Data::Map(entries) = Input::selection_message(&Data::from("moved"), "text", 3, 2) else {
        panic!("not a map");
    };
    assert_eq!(entries.get("selected"), Some(&Data::Integer(2)));
    assert_eq!(entries.get("caret"), Some(&Data::Integer(3)));
}

#[test]
fn text_takes_a_language() {
    let Ok(Node::Text(text)) = input_view(r#"ui.text { "let x", syntax = "rust" }"#) else {
        panic!("no text");
    };
    assert_eq!(text.syntax.as_deref(), Some("rust"));
}

#[test]
fn markdown_reads_a_document_and_its_link() {
    use crate::Markdown;
    let Ok(Node::Markdown(doc)) = input_view(
        r##"ui.markdown { "# Notes\n\nSome *text*.", link = { open = true }, to = "editor",
            grow = true, scroll = true, max_width = 720, class = "doc" }"##,
    ) else {
        panic!("no markdown");
    };
    assert_eq!(doc.text, "# Notes\n\nSome *text*.");
    assert_eq!(doc.to.as_deref(), Some("editor"));
    assert!(doc.grow && doc.scroll);
    assert_eq!(
        (doc.max_width, doc.look.class.as_slice()),
        (Some(720.0), ["doc".to_owned()].as_slice())
    );
    assert_eq!(
        Markdown::message(doc.link.as_ref().unwrap(), "https://luau.org"),
        Data::Map(
            [
                ("open".to_owned(), Data::Bool(true)),
                ("url".to_owned(), Data::from("https://luau.org")),
            ]
            .into()
        )
    );
    assert_eq!(
        Markdown::message(&Data::from("link"), "#top"),
        Data::Map(
            [
                ("msg".to_owned(), Data::from("link")),
                ("url".to_owned(), Data::from("#top")),
            ]
            .into()
        )
    );

    // A document is longer than a label may be, and a text area holds one.
    let Ok(Node::Markdown(long)) = input_view("ui.markdown(string.rep('x', 100000))") else {
        panic!("no long markdown");
    };
    assert_eq!(long.text.len(), 100_000);
    let Ok(Node::Input(area)) =
        input_view("ui.input { multiline = true, value = string.rep('x', 100000) }")
    else {
        panic!("no long text area");
    };
    assert_eq!(area.value.map(|value| value.len()), Some(100_000));

    for (view, expected) in [
        ("ui.markdown { to = 'x' }", "`to` goes with `link`"),
        ("ui.markdown { 'a', lable = 'x' }", "unknown key `lable`"),
        ("ui.markdown { ui.text 'a' }", "expected a string"),
        ("ui.input { value = string.rep('x', 20000) }", "longer than"),
        (
            "ui.markdown(string.rep('x', 1048577))",
            "longer than 1048576 bytes",
        ),
    ] {
        let error = input_view(view).unwrap_err();
        assert!(error.contains(expected), "{view}: {error}");
    }
}

#[test]
fn bad_inputs_are_refused() {
    for (view, expected) in [
        ("ui.input { lable = 'x' }", "unknown key `lable`"),
        (
            "ui.input { rows = 3 }",
            "`rows` goes with `multiline = true`",
        ),
        (
            "ui.input { multiline = true, rows = 0 }",
            "a whole number from 1",
        ),
        (
            "ui.input { multiline = true, rows = 2.5 }",
            "a whole number from 1",
        ),
        ("ui.input { id = 'a b' }", "an id is 1 to 64"),
        ("ui.input { browser = 'a/b' }", "an id is 1 to 64"),
        ("ui.input { to = 'x' }", "`to` goes with `change`"),
        ("ui.input { keys = 'up' }", "expected a table"),
        ("ui.input { keys = { 'up' } }", "keys are key names"),
        (
            "ui.input { keys = { arrowup = 'x' } }",
            "unknown key `arrowup`",
        ),
        (
            "ui.input { keys = { ['ctl-n'] = 'x' } }",
            "unknown modifier `ctl`",
        ),
        (
            "ui.input { keys = { ['ctrl-ctrl-n'] = 'x' } }",
            "`ctrl` twice",
        ),
        ("ui.input { keys = { N = 'x' } }", "unknown key `N`"),
        (
            "ui.input { keys = { ['ctrl-shift-n'] = 'a', ['shift-ctrl-n'] = 'b' } }",
            "the same key as",
        ),
        (
            "ui.input { keys = { up = function() end } }",
            "cannot leave its module",
        ),
        (
            "ui.input { value = true }",
            "input.value: expected a string",
        ),
        ("ui.input { prose = true }", "`prose` goes with `multiline"),
        (
            "ui.input { dim = 'sentence' }",
            "`dim` goes with `multiline",
        ),
        (
            "ui.input { typewriter = true }",
            "`typewriter` goes with `multiline",
        ),
        (
            "ui.input { multiline = true, dim = 'line' }",
            "`line` is no dim",
        ),
    ] {
        let error = input_view(view).err();
        assert!(
            error
                .as_deref()
                .is_some_and(|error| error.contains(expected)),
            "{view}: {error:?}"
        );
    }
}

#[test]
fn writing_fields_read_their_modes() {
    use crate::Dim;
    let Ok(Node::Input(input)) = input_view(
        "ui.input { multiline = true, prose = true, dim = 'paragraph', typewriter = true }",
    ) else {
        panic!("no writing field");
    };
    assert!(input.prose && input.typewriter && input.writing());
    assert_eq!(input.dim, Some(Dim::Paragraph));
    let Ok(Node::Input(focus)) = input_view("ui.input { multiline = true, dim = 'sentence' }")
    else {
        panic!("no focus field");
    };
    assert_eq!(focus.dim, Some(Dim::Sentence));
    assert!(!focus.prose && focus.writing());
    let Ok(Node::Input(plain)) = input_view("ui.input { multiline = true }") else {
        panic!("no text area");
    };
    assert!(!plain.writing());
}
