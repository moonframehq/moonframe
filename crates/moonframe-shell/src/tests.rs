// The helpers driving the provider serve the tests that run commands,
// which need a Unix shell.
#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use {
    super::{ansi::Output, *},
    moonframe_providers::Event,
    std::sync::{Arc, Mutex},
};

fn lines(bytes: &[u8]) -> Vec<String> {
    let mut output = Output::new(100);
    output.feed(bytes);
    output.lines()
}

#[test]
fn colours_become_marks_and_each_line_stands_alone() {
    let got = lines(b"\x1b[33mabc \x1b[1;32mdef\nghi\x1b[0m plain\n");
    assert_eq!(got, [
        "\u{1}yellow;\u{2}abc \u{1}green;b\u{2}def",
        "\u{1}green;b\u{2}ghi\u{1};\u{2} plain",
    ]);
}

#[test]
fn extended_colours_are_hex_and_backgrounds_are_dropped() {
    let got = lines(b"\x1b[38;5;196mred\x1b[48;2;1;2;3m on \x1b[38;2;16;32;48mrgb\x1b[39m");
    assert_eq!(got, ["\u{1}#ff0000;\u{2}red on \u{1}#102030;\u{2}rgb"]);
}

#[test]
fn carriage_returns_overwrite_and_controls_never_forge_marks() {
    assert_eq!(lines(b"10%\r50%\r100%\n"), ["100%"]);
    assert_eq!(lines(b"a\x01red;\x02b\x1b]0;title\x07c\n"), ["ared;bc"]);
    assert_eq!(lines(b"a\tb\n"), ["a       b"]);
}

#[test]
fn a_character_cut_between_reads_is_kept_whole() {
    let mut output = Output::new(10);
    let text = "é".as_bytes();
    output.feed(&text[..1]);
    output.feed(&text[1..]);
    assert_eq!(output.lines(), ["é"]);
}

#[test]
fn only_the_last_lines_are_kept() {
    let mut output = Output::new(2);
    output.feed(b"1\n2\n3\n");
    assert_eq!(output.lines(), ["2", "3"]);
    assert_eq!(output.dropped, 1);
}

#[test]
fn a_tab_at_the_end_of_a_full_line_ends() {
    let mut line = vec![b'x'; 5000];
    line.extend_from_slice(b"\t\tdone\n");
    let got = lines(&line);
    assert_eq!(got[0].chars().count(), 4000);
}

#[test]
fn plain_text_drops_the_marks() {
    assert_eq!(
        ansi::plain("\u{1}red;b\u{2}hi\u{1};\u{2} there"),
        "hi there"
    );
}

type Seen = Arc<Mutex<Vec<Event>>>;

fn host(settings: &str, dir: &Path) -> (Handle, Seen) {
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let events: moonframe_providers::Events = {
        let seen = Arc::clone(&seen);
        Arc::new(move |event| seen.lock().unwrap().push(event))
    };
    let settings =
        moonframe_providers::from_json(&serde_json::from_str(settings).unwrap()).unwrap();
    (Handle::new("shell", dir, settings, events), seen)
}

fn message(text: &str) -> Data {
    moonframe_providers::from_json(&serde_json::from_str(text).unwrap()).unwrap()
}

fn last_runs(seen: &Seen) -> Value {
    let seen = seen.lock().unwrap();
    let data = seen
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::Published { data, .. } => Some(data.clone()),
            _ => None,
        })
        .unwrap();
    to_json(&data)["runs"].clone()
}

fn replies(seen: &Seen) -> Vec<Value> {
    seen.lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            Event::Sent { msg, .. } => Some(to_json(msg)),
            _ => None,
        })
        .collect()
}

#[cfg(unix)]
#[test]
fn a_runner_runs_a_command_and_hears_how_it_ended() {
    let dir = tempfile::tempdir().unwrap();
    let (host, seen) = host(r#"{ "runners": ["chat"] }"#, dir.path());
    let mut shell = Shell::default();
    shell.start(&host).unwrap();
    let cwd = dir.path().display().to_string();
    shell.receive(
        &host,
        "chat",
        message(&format!(
            r#"{{ "run": "printf 'one\\n'; printf '\\033[31mtwo\\033[0m\\n' >&2; exit 3", "id": "x", "cwd": "{cwd}" }}"#
        )),
    );
    let started = replies(&seen);
    assert_eq!(started[0]["ok"], true, "{started:?}");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        shell.tick(&host);
        let runs = last_runs(&seen);
        if runs[0]["status"] != "running" {
            assert_eq!(runs[0]["status"], "failed");
            assert_eq!(runs[0]["code"], 3);
            assert_eq!(runs[0]["text"], "one\ntwo");
            assert_eq!(runs[0]["output"], "one\n\u{1}red;\u{2}two");
            break;
        }
        assert!(Instant::now() < deadline, "it never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
    shell.tick(&host);
    let finished = replies(&seen);
    assert!(
        finished
            .iter()
            .any(|reply| reply["action"] == "finished" && reply["status"] == "failed"),
        "{finished:?}"
    );
}

#[cfg(unix)]
#[test]
fn stop_ends_a_command_and_others_may_neither_run_nor_stop() {
    let dir = tempfile::tempdir().unwrap();
    let (host, seen) = host(r#"{ "runners": ["chat"] }"#, dir.path());
    let mut shell = Shell::default();
    shell.start(&host).unwrap();
    shell.receive(&host, "other", message(r#"{ "run": "echo hi" }"#));
    assert_eq!(replies(&seen)[0]["ok"], false);
    shell.receive(
        &host,
        "chat",
        message(r#"{ "run": "sleep 30", "id": "s" }"#),
    );
    let id = replies(&seen)[1]["run"].as_str().unwrap().to_owned();
    shell.receive(&host, "other", message(&format!(r#"{{ "stop": "{id}" }}"#)));
    std::thread::sleep(Duration::from_millis(200));
    shell.tick(&host);
    assert_eq!(last_runs(&seen)[0]["status"], "running");
    shell.receive(&host, "chat", message(&format!(r#"{{ "stop": "{id}" }}"#)));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        shell.tick(&host);
        if last_runs(&seen)[0]["status"] == "stopped" {
            break;
        }
        assert!(Instant::now() < deadline, "it never stopped");
        std::thread::sleep(Duration::from_millis(20));
    }
}
