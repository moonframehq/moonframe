use {
    crate::{Color, Emulator, Engine, Key, Screen, Session, Size, Spawn, encode, paste},
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
};

fn key(name: &'static str, text: Option<&'static str>) -> Key<'static> {
    Key {
        name,
        text,
        ..Key::default()
    }
}

#[test]
fn keys_send_what_xterm_sends() {
    assert_eq!(encode(key("a", Some("a")), false), Some(b"a".to_vec()));
    assert_eq!(encode(key("enter", None), false), Some(b"\r".to_vec()));
    assert_eq!(encode(key("up", None), false), Some(b"\x1b[A".to_vec()));
    assert_eq!(encode(key("up", None), true), Some(b"\x1bOA".to_vec()));
    let ctrl_c = Key {
        ctrl: true,
        ..key("c", Some("c"))
    };
    assert_eq!(encode(ctrl_c, false), Some(vec![3]));
    let alt_b = Key {
        alt: true,
        ..key("b", Some("b"))
    };
    assert_eq!(encode(alt_b, false), Some(b"\x1bb".to_vec()));
    let back_tab = Key {
        shift: true,
        ..key("tab", None)
    };
    assert_eq!(encode(back_tab, false), Some(b"\x1b[Z".to_vec()));
    assert_eq!(encode(key("f13", None), false), None);
}

#[test]
fn pastes_are_bracketed_when_asked_and_cannot_close_it_early() {
    assert_eq!(paste("a\nb", false), b"a\rb");
    assert_eq!(paste("ls\x1b[201~rm", true), b"\x1b[200~ls[201~rm\x1b[201~");
    // Removing the end marker once would rebuild it from the pieces.
    assert_eq!(
        paste("\x1b[20\x1b[201~1~", true),
        b"\x1b[200~[20[201~1~\x1b[201~"
    );
    // C1 controls (8-bit CSI) go too; tabs and newlines stay.
    assert_eq!(
        paste("a\u{9b}201~\tb\n", true),
        b"\x1b[200~a201~\tb\n\x1b[201~"
    );
}

/// What each engine built here makes of the same output.
fn emulators() -> Vec<(Engine, Box<dyn Emulator>)> {
    let size = Size {
        cols: 20,
        rows: 4,
        ..Size::default()
    };
    [Engine::Alacritty, Engine::Ghostty]
        .into_iter()
        .filter_map(|engine| engine.emulator(size).map(|emulator| (engine, emulator)))
        .collect()
}

fn text(screen: &Screen, row: u16) -> String {
    screen
        .row(row)
        .iter()
        .map(|cell| cell.ch)
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[test]
fn both_engines_read_text_colors_and_the_cursor() {
    let emulators = emulators();
    assert!(!emulators.is_empty(), "no engine built");
    for (engine, mut emulator) in emulators {
        emulator.feed(b"hello\r\n\x1b[31mred\x1b[0m \x1b[1mbold");
        let screen = emulator.screen();
        assert_eq!((screen.cols, screen.rows), (20, 4), "{engine:?}");
        assert_eq!(text(&screen, 0), "hello", "{engine:?}");
        assert_eq!(text(&screen, 1), "red bold", "{engine:?}");
        let red = screen.row(1)[0];
        assert!(
            matches!(red.fg, Color::Rgb(r, g, b) if r > g.saturating_add(60) && r > b.saturating_add(60)),
            "{engine:?}: {:?}",
            red.fg
        );
        assert_eq!(screen.row(0)[0].fg, Color::Default, "{engine:?}");
        assert!(screen.row(1)[4].bold, "{engine:?}");
        assert_eq!(screen.cursor, Some((8, 1)), "{engine:?}");

        // Device status report: the cursor position, sent back.
        emulator.feed(b"\x1b[6n");
        assert_eq!(emulator.replies(), b"\x1b[2;9R", "{engine:?}");

        emulator.feed(b"\x1b[?1h\x1b[?2004h");
        let screen = emulator.screen();
        assert!(screen.app_cursor && screen.bracketed_paste, "{engine:?}");

        emulator.resize(Size {
            cols: 10,
            rows: 3,
            ..Size::default()
        });
        let screen = emulator.screen();
        assert_eq!((screen.cols, screen.rows), (10, 3), "{engine:?}");
        assert_eq!(screen.cells.len(), 30, "{engine:?}");
    }
}

#[test]
fn both_engines_paint_the_theme_palette_under_the_shell_own() {
    let mut theme = [(0, 0, 0); 16];
    theme[1] = (0x12, 0x34, 0x56);
    theme[2] = (0x65, 0x43, 0x21);
    for (engine, mut emulator) in emulators() {
        emulator.set_palette(Some(theme));
        emulator.feed(b"\x1b[31mA\x1b[42mB\x1b[0m");
        let row = emulator.screen().row(0).to_vec();
        assert_eq!(row[0].fg, Color::Rgb(0x12, 0x34, 0x56), "{engine:?}");
        assert_eq!(row[1].bg, Color::Rgb(0x65, 0x43, 0x21), "{engine:?}");

        // A color the shell sets wins over the theme's.
        emulator.feed(b"\x1b]4;1;rgb:ff/00/00\x07\r\x1b[31mA");
        assert_eq!(
            emulator.screen().row(0)[0].fg,
            Color::Rgb(0xff, 0, 0),
            "{engine:?}"
        );

        // Back to xterm's for the rest.
        emulator.set_palette(None);
        emulator.feed(b"\r\x1b[32mC");
        assert_ne!(
            emulator.screen().row(0)[0].fg,
            Color::Rgb(0x65, 0x43, 0x21),
            "{engine:?}"
        );
    }
}

#[test]
fn a_session_runs_a_shell() {
    for engine in [Engine::Alacritty, Engine::Ghostty]
        .into_iter()
        .filter(|engine| engine.available())
    {
        let session = Session::spawn(
            Spawn {
                engine,
                cwd: None,
                size: Size::default(),
            },
            Arc::new(|| {}),
        )
        .unwrap();
        // What the shell prints, never what it echoes as typed: POSIX
        // arithmetic, or on Windows cmd.exe an empty slice of a variable.
        let command: &[u8] = if cfg!(windows) {
            b"echo moonframe-%COMSPEC:~0,0%42\r"
        } else {
            b"echo moonframe-$((40 + 2))\r"
        };
        session.write(command);
        let deadline = Instant::now() + Duration::from_secs(15);
        let found = loop {
            let screen = session.screen();
            if (0..screen.rows).any(|row| text(&screen, row).contains("moonframe-42")) {
                break true;
            }
            if Instant::now() > deadline {
                break false;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(found, "{engine:?}: {:?}", session.screen());
        session.write(b"exit\r");
    }
}
