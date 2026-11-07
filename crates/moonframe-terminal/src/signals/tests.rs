use super::*;

fn scan(bytes: &[u8]) -> Vec<Signal> {
    let mut signals = Vec::new();
    Scanner::default().feed(bytes, &mut signals);
    signals
}

/// `signals` without the time a command took, which each run differs in.
fn untimed(signals: Vec<Signal>) -> Vec<Signal> {
    signals
        .into_iter()
        .map(|signal| match signal {
            Signal::Finished {
                command, status, ..
            } => Signal::Finished {
                command,
                status,
                took: Duration::ZERO,
            },
            other => other,
        })
        .collect()
}

#[test]
fn a_command_runs_from_prompt_to_prompt() {
    let output = b"\x1b]133;A\x1b\\$ \x1b]133;C;cmdline_url=cargo%20test\x07hello\r\n\
        \x1b]133;D;101\x1b\\\x1b]133;A\x1b\\";
    let command = Some("cargo test".to_owned());
    assert_eq!(untimed(scan(output)), [
        Signal::Prompt,
        Signal::Started {
            command: command.clone()
        },
        Signal::Finished {
            command,
            status: Some(101),
            took: Duration::ZERO
        },
        Signal::Prompt,
    ]);
}

#[test]
fn sequences_are_found_across_any_chunks() {
    let output = b"text\x1b]2;a title\x1b\\more\x1b]7;file:///tmp\x07";
    let whole = scan(output);
    assert_eq!(whole.len(), 2);
    for cut in 0..output.len() {
        let mut scanner = Scanner::default();
        let mut signals = Vec::new();
        scanner.feed(&output[..cut], &mut signals);
        scanner.feed(&output[cut..], &mut signals);
        assert_eq!(signals, whole, "cut at {cut}");
    }
}

#[test]
fn an_end_without_a_start_is_nothing() {
    assert_eq!(scan(b"\x1b]133;D;0\x1b\\"), []);
}

#[test]
fn a_redrawn_prompt_and_doubled_marks_say_nothing_new() {
    // fish marks commands itself, and our integration does as well.
    let output = b"\x1b]133;A\x07\x1b]133;A\x07\x1b]133;C;cmdline_url=ls\x07\
        \x1b]133;C;cmdline_url=ls\x07\x1b]133;D;0\x07\x1b]133;D;0\x07";
    let kinds = scan(output)
        .into_iter()
        .map(|signal| std::mem::discriminant(&signal))
        .collect::<Vec<_>>();
    assert_eq!(kinds.len(), 3);
}

#[test]
fn a_prompt_ends_a_command_that_never_said_so() {
    let signals = untimed(scan(b"\x1b]133;C\x07\x1b]133;A\x07"));
    assert_eq!(signals[1], Signal::Finished {
        command: None,
        status: None,
        took: Duration::ZERO
    });
    assert_eq!(signals[2], Signal::Prompt);
}

#[test]
fn folders_are_decoded_and_name_their_machine() {
    assert_eq!(scan(b"\x1b]7;file://mac.local/Users/me/my%20code\x07"), [
        Signal::Folder {
            path: "/Users/me/my code".into(),
            host: Some("mac.local".into())
        }
    ]);
    assert_eq!(scan(b"\x1b]7;file:///tmp\x07"), [Signal::Folder {
        path: "/tmp".into(),
        host: None
    }]);
    assert_eq!(scan(b"\x1b]7;https://example.com/\x07"), []);
}

#[test]
fn notices_come_from_programs_but_progress_does_not() {
    assert_eq!(scan(b"\x1b]9;Claude needs your input\x07"), [
        Signal::Notice {
            title: None,
            body: "Claude needs your input".into()
        }
    ]);
    assert_eq!(scan(b"\x1b]777;notify;Build;done; all green\x07"), [
        Signal::Notice {
            title: Some("Build".into()),
            body: "done; all green".into()
        }
    ]);
    assert_eq!(scan(b"\x1b]9;4;1;50\x07"), []);
}

#[test]
fn control_characters_never_reach_a_mod() {
    assert_eq!(scan(b"\x1b]2;a\x08b\x7fc\x07"), [Signal::Title(
        "abc".into()
    )]);
    let line = b"\x1b]133;C;cmdline_url=a%0Ab%1B\x07";
    assert_eq!(scan(line), [Signal::Started {
        command: Some("ab".into())
    }]);
}

#[test]
fn broken_sequences_do_not_swallow_what_follows() {
    // Cancelled, then cut short by another escape.
    assert_eq!(
        scan(b"\x1b]2;gone\x18\x1b]2;also gone\x1b[0m\x1b]2;kept\x07"),
        [Signal::Title("kept".into())]
    );
    // Malformed text around it, and escapes that are not sequences.
    assert_eq!(
        scan(b"\x1b\x1b]2;kept\x07\x1b[31mred\x1b]133;Z\x07\x1b]133\x07"),
        [Signal::Title("kept".into())]
    );
    assert_eq!(scan(b"\x1b]7;file://no-path\x07\x1b]7\x07\x1b]"), []);
    assert_eq!(decode("100%"), "100%");
    assert_eq!(decode("%zz%4"), "%zz%4");
}

#[test]
fn an_endless_sequence_is_skipped_and_costs_no_more_memory() {
    let mut scanner = Scanner::default();
    let mut signals = Vec::new();
    scanner.feed(b"\x1b]2;", &mut signals);
    for _ in 0..64 {
        scanner.feed(&[b'a'; 1024], &mut signals);
    }
    assert!(scanner.sequence.len() <= MAX_SEQUENCE);
    scanner.feed(b"\x07\x1b]2;next\x07", &mut signals);
    assert_eq!(signals, [Signal::Title("next".into())]);
}

#[test]
fn long_text_is_cut_on_a_character() {
    let mut output = b"\x1b]2;".to_vec();
    output.extend("é".repeat(MAX_TEXT + 10).as_bytes());
    output.push(BEL);
    let [Signal::Title(title)] = &scan(&output)[..] else {
        panic!("no title");
    };
    assert_eq!(title.chars().count(), MAX_TEXT);
}
