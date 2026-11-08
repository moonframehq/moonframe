#![allow(clippy::unwrap_used)]

use {super::*, serde_json::json};

fn object(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

#[test]
fn scalars_show_as_python_showed_them() {
    assert_eq!(as_text(&json!(null)), "");
    assert_eq!(as_text(&json!(true)), "1");
    assert_eq!(as_text(&json!(false)), "0");
    assert_eq!(as_text(&json!(3.0)), "3");
    assert_eq!(as_text(&json!(-0.0)), "0");
    assert_eq!(as_text(&json!(2.5)), "2.5");
    assert_eq!(as_text(&json!(12)), "12");
    assert_eq!(as_text(&json!("text")), "text");
}

#[test]
fn truthy_is_false_only_for_nothing_false_zero_and_empty() {
    for falsy in [
        json!(null),
        json!(false),
        json!(0),
        json!(0.0),
        json!(""),
        json!("0"),
    ] {
        assert!(!truthy(Some(&falsy)), "{falsy}");
    }
    assert!(!truthy(None));
    for yes in [json!(true), json!(1), json!("no"), json!([]), json!({})] {
        assert!(truthy(Some(&yes)), "{yes}");
    }
}

#[test]
fn clip_cuts_by_characters_and_keeps_short_numbers() {
    assert_eq!(clip(&json!("héllo"), 5), json!("héllo"));
    assert_eq!(clip(&json!("héllo!"), 5), json!("héll…"));
    assert_eq!(clip(&json!(4.0), 5), json!(4));
    assert_eq!(clip(&json!(4.5), 5), json!(4.5));
    assert_eq!(clip(&json!(123_456), 5), json!("1234…"));
    assert_eq!(clip(&json!(true), 5), json!("1"));
}

#[test]
fn one_line_collapses_unicode_whitespace_but_not_separators() {
    assert_eq!(
        one_line(&json!(" \u{3000}\n Fix\t\tthe\u{2028}bug ")),
        json!("Fix the bug ")
    );
    // \x1c to \x1f are not whitespace here, as they were not in Python.
    assert_eq!(one_line(&json!("a\u{1c}b")), json!("a\u{1c}b"));
    let long = "word ".repeat(30);
    let line = one_line(&json!(long));
    assert_eq!(line.as_str().unwrap().chars().count(), 90);
    assert!(line.as_str().unwrap().ends_with('…'));
    assert_eq!(one_line(&json!(7)), json!(7));
}

#[test]
fn typed_takes_what_a_person_typed() {
    let user =
        |content: Value| object(json!({ "type": "user", "message": { "content": content } }));
    assert_eq!(typed(&user(json!("Fix it"))), Some(json!("Fix it")));
    assert_eq!(typed(&user(json!(42))), Some(json!(42)));
    assert_eq!(
        typed(&user(
            json!([{ "type": "image" }, { "type": "text", "text": "Look" }])
        )),
        Some(json!("Look"))
    );
    assert_eq!(
        typed(&user(json!([{ "type": "tool_result", "content": "x" }]))),
        None
    );
    assert_eq!(typed(&user(json!("  <command-name>/clear"))), None);
    assert_eq!(typed(&user(json!("Caveat: the messages below"))), None);
    assert_eq!(typed(&user(json!(" \n "))), None);
    assert_eq!(typed(&user(json!(true))), None);
    assert_eq!(typed(&user(json!({ "text": "x" }))), None);
    assert_eq!(
        typed(&user(json!([{ "type": "text", "text": null }]))),
        None
    );
    let mut meta = user(json!("hidden"));
    meta.insert("isMeta".to_owned(), json!(true));
    assert_eq!(typed(&meta), None);
    let mut side = user(json!("side"));
    side.insert("isSidechain".to_owned(), json!("yes"));
    assert_eq!(typed(&side), None);
}

#[test]
fn basenames_ignore_trailing_slashes() {
    assert_eq!(basename("/a/b/"), "b");
    assert_eq!(basename("b"), "b");
    assert_eq!(basename("///"), "/");
    assert_eq!(basename(""), "");
}

#[test]
fn projects_are_repositories_with_worktrees_under_them() {
    assert_eq!(project_of(Some("/Users/me/code/app"), "x"), "app");
    assert_eq!(project_of(Some("/Users/me/code/app/"), "x"), "app");
    assert_eq!(
        project_of(
            Some("/Users/me/.herdr/worktrees/moonframe/clear-cloud/sub"),
            "x"
        ),
        "moonframe"
    );
    // Without a folder after the repository's, it is not a herdr worktree.
    assert_eq!(
        project_of(Some("/Users/me/.herdr/worktrees/moonframe"), "x"),
        "moonframe"
    );
    assert_eq!(
        project_of(Some("/Users/me/code/app/.claude/worktrees/agent-1/"), "x"),
        "app"
    );
    assert_eq!(
        project_of(
            Some("/Users/me/code/app/.claude/worktrees/agent-1/src"),
            "x"
        ),
        "src"
    );
    assert_eq!(project_of(None, "-Users-me-code-cinder"), "cinder");
    assert_eq!(project_of(Some(""), "-Users-me-code-"), "unknown");
    assert_eq!(project_of(None, "-tmp-0"), "unknown");
    assert_eq!(project_of(None, "plain"), "plain");
}

#[test]
fn ages_are_short() {
    assert_eq!(age(100, 100), "now");
    assert_eq!(age(200, 100), "now");
    assert_eq!(age(0, 59), "now");
    assert_eq!(age(0, 60), "1m");
    assert_eq!(age(0, 3600), "1h");
    assert_eq!(age(0, 86400 * 2), "2d");
    assert_eq!(age(0, 86400 * 65), "2mo");
}

#[test]
fn byte_searches_find_first_and_last() {
    assert_eq!(find(b"abcabc", b"bc", 0), Some(1));
    assert_eq!(find(b"abcabc", b"bc", 2), Some(4));
    assert_eq!(find(b"abc", b"x", 0), None);
    assert_eq!(rfind(b"abcabc", b"ab"), Some(3));
    assert_eq!(rfind(b"a", b"ab"), None);
    assert_eq!(line_at(b"one\ntwo\nthree", 4), b"two");
    assert_eq!(line_at(b"one\ntwo", 4), b"two");
}

#[test]
fn quoted_strings_are_unescaped_and_need_their_quote() {
    let line = "{\"cwd\":\"/a/\\\"b\\\"\\u00e9\",\"gitBranch\":\"main\"}".as_bytes();
    assert_eq!(quoted(line, br#""cwd":""#).as_deref(), Some("/a/\"b\"é"));
    assert_eq!(quoted(line, br#""gitBranch":""#).as_deref(), Some("main"));
    // The first is cut off; the next one is taken.
    assert_eq!(
        quoted(b"\"cwd\":\"/a\\\n\"cwd\":\"/b\"", br#""cwd":""#).as_deref(),
        Some("/b")
    );
    assert_eq!(quoted(br#""cwd":"/a"#, br#""cwd":""#), None);
    assert_eq!(escaped(br#"ab\"cd"#, 0, 3), (4, false));
    assert_eq!(escaped(br#"ab"cd"#, 0, 300), (2, true));
}

#[test]
fn strip_cwd_takes_the_folder_off_paths_in_it() {
    assert_eq!(strip_cwd("/a/b/c.rs", "/a/b"), "c.rs");
    assert_eq!(strip_cwd("/a/bc.rs", "/a/b"), "/a/bc.rs");
    assert_eq!(strip_cwd("/a/b/c.rs", ""), "/a/b/c.rs");
}
