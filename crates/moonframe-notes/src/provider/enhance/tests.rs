#![allow(clippy::unwrap_used)]

use {
    super::{Favicons, fetch::sniff, hosts, html, plan, urls},
    crate::parse::web_links,
    std::collections::HashMap,
};

fn urls_of(source: &str) -> Vec<(String, bool)> {
    web_links(source)
        .into_iter()
        .map(|link| (link.url, link.bare))
        .collect()
}

#[test]
fn finds_web_links_but_not_in_code() {
    let source = "---\nsource: https://front.example\n---\n\
        See [the docs](https://docs.example/a) and https://bare.example/x.\n\
        <https://auto.example> and [https://same.example/](https://same.example)\n\
        `https://inline.code` www.example.com [mail](mailto:a@b.c) ![pic](https://img.example/p.png)\n\
        %% https://comment.example %%\n\
        ```\nhttps://fenced.example\n```\n\
        [ref][r] [also](<https://angle.example/a b>)\n";
    assert_eq!(urls_of(source), vec![
        ("https://docs.example/a".to_string(), false),
        ("https://bare.example/x".to_string(), true),
        ("https://auto.example".to_string(), true),
        ("https://same.example".to_string(), true),
        ("https://angle.example/a b".to_string(), false),
    ]);
}

#[test]
fn bare_links_get_their_titles() {
    let source = "https://a.example/x\n\nRead <https://b.example> and [https://c.example](https://c.example \"t\") \
        but not [mine](https://d.example) or `https://a.example/x`.\n| https://e.example/(a |\n";
    let links = web_links(source);
    let titles: HashMap<String, String> = [
        ("https://a.example/x", "A [page] | *x*"),
        ("https://b.example", "B"),
        ("https://c.example", "C"),
        ("https://d.example", "D"),
    ]
    .into_iter()
    .map(|(url, title)| (url.to_string(), title.to_string()))
    .collect();
    let (text, titled) = urls::rewrite(source, &links, &titles);
    assert_eq!(titled, 3);
    assert_eq!(
        text,
        "[A \\[page\\] \\| \\*x\\*](https://a.example/x)\n\nRead [B](https://b.example) and \
         [C](https://c.example \"t\") but not [mine](https://d.example) or `https://a.example/x`.\n\
         | https://e.example/(a |\n"
    );
    let unbalanced = web_links("https://e.example/a(b\n");
    let titles = HashMap::from([(unbalanced[0].url.clone(), "E".to_string())]);
    assert_eq!(
        urls::rewrite("https://e.example/a(b\n", &unbalanced, &titles).0,
        "[E](<https://e.example/a(b>)\n"
    );
}

#[test]
fn titles_prefer_open_graph_and_decode_entities() {
    let page = "<html><head><title>\n  Plain &amp; simple\n</title>\
        <meta content=\"Rich &#8212; &quot;quoted&quot; &#x27;x&#x27; &unknown;\" property=\"og:title\">\
        </head><body><title>no</title></body></html>";
    assert_eq!(
        html::title(page).as_deref(),
        Some("Rich — \"quoted\" 'x' &unknown;")
    );
    let page = "<!-- <title>comment</title> --><script>var t = '<title>no</title>'</script>\
        <TITLE>Caf&eacute; &lt;1&gt;   two\tlines</TITLE>";
    assert_eq!(
        html::title(page).as_deref(),
        Some("Caf&eacute; <1> two lines")
    );
    assert_eq!(html::title("<title>   </title>"), None);
    assert_eq!(html::decode("a&amp;b &#65;&#x42; &nbsp"), "a&b AB \u{a0}");
}

#[test]
fn icons_prefer_touch_icons_then_png_or_svg() {
    let page = r#"<head>
        <link rel="icon" href="/favicon.ico">
        <link rel="mask-icon" href="/mask.svg">
        <link rel="icon" type="image/png" sizes="32x32" href="/32.png">
        <link rel="apple-touch-icon" sizes="120x120" href="/120.png">
        <link rel=apple-touch-icon sizes=180x180 href=/180.png>
        </head><body><link rel="apple-touch-icon" href="/late.png"></body>"#;
    assert_eq!(html::icon(page).as_deref(), Some("/180.png"));
    let page = r#"<link rel="shortcut icon" href="/x.ico"><link rel="icon" href="icon.svg?v=2">"#;
    assert_eq!(html::icon(page).as_deref(), Some("icon.svg?v=2"));
    let page = r#"<link rel="icon" href="data:image/png;base64,AAAA"><link rel="SHORTCUT ICON" href="/x.ico">"#;
    assert_eq!(html::icon(page).as_deref(), Some("/x.ico"));
    assert_eq!(html::icon("<link rel=stylesheet href=a.css>"), None);
}

#[test]
fn links_resolve_against_the_page_and_stay_on_the_web() {
    let base = "https://a.example/blog/post?x=1#top";
    assert_eq!(
        urls::resolve(base, "/i.png").as_deref(),
        Some("https://a.example/i.png")
    );
    assert_eq!(
        urls::resolve(base, "i.png").as_deref(),
        Some("https://a.example/blog/i.png")
    );
    assert_eq!(
        urls::resolve(base, "//cdn.example/i.png").as_deref(),
        Some("https://cdn.example/i.png")
    );
    assert_eq!(
        urls::resolve("https://a.example", "i.png").as_deref(),
        Some("https://a.example/i.png")
    );
    for bad in ["javascript:alert(1)", "file:///etc/passwd", "data:x", ""] {
        assert_eq!(urls::resolve(base, bad), None, "{bad}");
    }
    assert_eq!(urls::resolve("file:///x", "/i.png"), None);
    assert_eq!(urls::origin(base).as_deref(), Some("https://a.example"));
}

#[test]
fn hosts_are_safe_file_names() {
    assert_eq!(
        urls::host("https://user:pw@WWW.Example.COM.:8080/a").as_deref(),
        Some("www.example.com")
    );
    for bad in [
        "https://../x",
        "https://a..b/",
        "https:///x",
        "https://[::1]/",
        "https://é.example/",
        "https://.hidden/",
        "ftp://a.example/",
    ] {
        assert_eq!(urls::host(bad), None, "{bad}");
    }
    assert_eq!(urls::safe_host("a/b"), None);
    assert_eq!(
        urls::safe_host("a-b.example").as_deref(),
        Some("a-b.example")
    );
}

#[test]
fn pictures_are_known_by_their_bytes() {
    assert_eq!(sniff(b"\x89PNG\r\n\x1a\n...."), Some("png"));
    assert_eq!(sniff(b"GIF89a...."), Some("gif"));
    assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
    assert_eq!(sniff(&[0, 0, 1, 0, 1, 0, 16, 16]), Some("ico"));
    assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
    assert_eq!(
        sniff(b"<?xml version=\"1.0\"?><svg xmlns=\"x\"/>"),
        Some("svg")
    );
    assert_eq!(sniff(b"<!doctype html><html><svg/></html>"), None);
    assert_eq!(sniff(b"Not found"), None);
}

#[test]
fn favicons_are_looked_up_by_host_and_fetched_when_missing() {
    let temp = tempfile::tempdir().unwrap();
    let favicons = Favicons {
        folder: temp.path().to_path_buf(),
    };
    std::fs::write(temp.path().join("a.example.svg"), "<svg/>").unwrap();
    std::fs::write(temp.path().join("empty.example.png"), "").unwrap();
    assert_eq!(
        favicons.cached("a.example"),
        Some(temp.path().join("a.example.svg"))
    );
    assert_eq!(favicons.cached("empty.example"), None);
    assert_eq!(favicons.cached("../a.example"), None);
    assert!(favicons.fresh("a.example"));

    let text =
        "[x](https://a.example/1) https://b.example/2 https://a.example/3 (https://c.example)";
    let found: Vec<String> = hosts(text).collect();
    assert_eq!(found, ["a.example", "b.example", "a.example", "c.example"]);

    let jobs = plan(&web_links(text), &favicons);
    let jobs: Vec<(&str, bool, Vec<&str>)> = jobs
        .iter()
        .map(|job| {
            (
                job.host.as_str(),
                job.icon,
                job.titles.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(jobs, vec![
        ("a.example", false, vec!["https://a.example/3"]),
        ("b.example", true, vec!["https://b.example/2"]),
        ("c.example", true, vec!["https://c.example"]),
    ]);
}
