use {
    crate::{
        Backend as _, Color, Emulator, Engine, Key, Open, Pty, Screen, Session, Signal, Size,
        encode, paste,
    },
    std::{
        collections::VecDeque,
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
        let opened = Pty
            .open(&Open {
                key: "test/main".into(),
                cwd: None,
                size: Size::default(),
                env: Vec::new(),
            })
            .unwrap();
        let session = Session::start(opened, engine, Size::default(), Arc::new(|| {})).unwrap();
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
        // Final output must be drained even when closing the PTY is what
        // unblocks its reader (ConPTY on Windows).
        session.write(if cfg!(windows) {
            b"echo final-%COMSPEC:~0,0%42 & exit\r"
        } else {
            b"echo final-$((40 + 2)); exit\r"
        });
        let deadline = Instant::now() + Duration::from_secs(15);
        while !session.exited() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            session.exited(),
            "{engine:?}: the shell exit was not confirmed"
        );
        let screen = session.screen();
        assert!(!screen.disconnected);
        assert!(
            (0..screen.rows).any(|row| text(&screen, row).contains("final-42")),
            "{engine:?}: final output was lost: {screen:?}"
        );
    }
}

#[test]
fn output_ending_only_reports_an_exit_when_the_backend_confirms_it() {
    struct EndedPane(bool);
    impl crate::Pane for EndedPane {
        fn write(&self, _: &[u8]) {}

        fn resize(&self, _: Size) {}

        fn exited(&self) -> bool {
            self.0
        }
    }

    for engine in [Engine::Alacritty, Engine::Ghostty]
        .into_iter()
        .filter(|engine| engine.available())
    {
        for confirmed in [false, true] {
            let (wake, woken) = std::sync::mpsc::channel();
            let opened = crate::Opened {
                pane: Box::new(EndedPane(confirmed)),
                output: Box::new(std::iter::once(b"final output".to_vec())),
            };
            let session = Session::start(
                opened,
                engine,
                Size::default(),
                Arc::new(move || {
                    let _ = wake.send(());
                }),
            )
            .unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            let screen = loop {
                let screen = session.screen();
                if screen.exited || screen.disconnected {
                    break screen;
                }
                woken
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .unwrap();
            };
            assert_eq!(session.exited(), confirmed, "{engine:?}");
            assert_eq!(screen.disconnected, !confirmed, "{engine:?}");
            assert_eq!(text(&screen, 0), "final output", "{engine:?}");
        }
    }
}

#[test]
fn a_shell_says_which_folder_it_is_in_until_its_session_goes() {
    let Some(engine) = [Engine::Alacritty, Engine::Ghostty]
        .into_iter()
        .find(|engine| engine.available())
    else {
        return;
    };
    let dir = std::env::temp_dir().canonicalize().unwrap();
    let opened = Pty
        .open(&Open {
            key: "test/folder".into(),
            cwd: Some(dir.clone()),
            size: Size::default(),
            env: Vec::new(),
        })
        .unwrap();
    let session = Session::start(opened, engine, Size::default(), Arc::new(|| {})).unwrap();
    let folder = session.folder();
    assert!(folder.same(&session.folder()));
    // Windows names the folder in a form of its own (`\\?\`); elsewhere
    // it is the one the shell started in.
    if !cfg!(windows) {
        let cwd = folder.get().map(|cwd| cwd.canonicalize().unwrap_or(cwd));
        assert_eq!(cwd, Some(dir));
    }
    drop(session);
    assert_eq!(folder.get(), None);
}

/// The whole screen as text.
fn shown(screen: &Screen) -> String {
    (0..screen.rows)
        .map(|row| text(screen, row))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A shell of this machine, started as a terminal would be, with the
/// integration's environment and a home of its own: the user's files stay
/// out of it. `None` when the machine does not have that shell.
fn integrated(name: &str, home: &std::path::Path) -> Option<Session> {
    let shell = std::env::var_os("PATH")
        .iter()
        .flat_map(std::env::split_paths)
        .map(|folder| folder.join(name))
        .find(|path| path.is_file())?
        .to_string_lossy()
        .into_owned();
    let folder = home.join("cache");
    crate::install_integration(&folder).unwrap();
    let mut env = crate::integration::environment(&shell, &folder, |_| None);
    env.push(("SHELL".into(), shell));
    env.push(("HOME".into(), home.to_string_lossy().into_owned()));
    env.push((
        "XDG_CONFIG_HOME".into(),
        home.to_string_lossy().into_owned(),
    ));
    let open = Open {
        key: "test/shell".into(),
        cwd: Some(home.to_owned()),
        size: Size::default(),
        env,
    };
    let opened = Pty.open(&open).unwrap();
    let engine = [Engine::Alacritty, Engine::Ghostty]
        .into_iter()
        .find(|engine| engine.available())
        .unwrap();
    Some(Session::start(opened, engine, Size::default(), Arc::new(|| {})).unwrap())
}

/// What a session reported, heard in order however it arrives.
struct Heard<'a> {
    session: &'a Session,
    waiting: VecDeque<Signal>,
}

impl<'a> Heard<'a> {
    fn new(session: &'a Session) -> Self {
        Self {
            session,
            waiting: VecDeque::new(),
        }
    }

    /// Every signal up to and including the first `until` accepts; panics
    /// with what came when none does in time.
    fn until(&mut self, until: impl Fn(&Signal) -> bool) -> Vec<Signal> {
        let mut signals = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            self.waiting.extend(self.session.take_signals());
            while let Some(signal) = self.waiting.pop_front() {
                let found = until(&signal);
                signals.push(signal);
                if found {
                    return signals;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let screen = shown(&self.session.screen());
        panic!("not reported; got {signals:?}\n{screen}");
    }
}

/// Whether `wanted` shows on the screen in time.
fn wait_for(session: &Session, wanted: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if shown(&session.screen()).contains(wanted) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn is_prompt(signal: &Signal) -> bool {
    *signal == Signal::Prompt
}

/// A failing command, its folder and the words of its line, from a real
/// shell; `line` must end with status 3.
fn reports_a_command(heard: &mut Heard<'_>, line: &str, command: Option<&str>) {
    let start = heard.until(is_prompt);
    assert!(
        start
            .iter()
            .any(|signal| matches!(signal, Signal::Folder { path, .. } if !path.is_empty())),
        "no folder: {start:?}"
    );
    heard.session.write(format!("{line}\r").as_bytes());
    let run = heard.until(|signal| matches!(signal, Signal::Finished { .. }));
    let command = command.map(str::to_owned);
    assert!(
        run.contains(&Signal::Started {
            command: command.clone()
        }),
        "no start: {run:?}"
    );
    let Some(Signal::Finished {
        command: finished,
        status,
        ..
    }) = run.last()
    else {
        panic!("no end: {run:?}");
    };
    assert_eq!((finished, *status), (&command, Some(3)));
}

#[test]
#[cfg(unix)]
fn zsh_reports_commands_and_leaves_its_files_alone() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join(".zshrc"),
        "PROMPT='> '\nexport FROM_RC=$((40 + 2))\n",
    )
    .unwrap();
    let Some(session) = integrated("zsh", home.path()) else {
        eprintln!("zsh is not installed; skipped");
        return;
    };
    let line = "(echo 'é %'; exit 3)";
    let mut heard = Heard::new(&session);
    reports_a_command(&mut heard, line, Some(line));
    heard.until(is_prompt);
    session.write(b"echo \"rc=$FROM_RC dot=[${ZDOTDIR-none}]\"\r");
    assert!(
        wait_for(&session, "rc=42 dot=[none]"),
        "the user's files did not load as usual:\n{}",
        shown(&session.screen())
    );
}

#[test]
#[cfg(unix)]
fn fish_reports_commands() {
    let home = tempfile::tempdir().unwrap();
    let Some(session) = integrated("fish", home.path()) else {
        eprintln!("fish is not installed; skipped");
        return;
    };
    let line = "sh -c 'exit 3'";
    let mut heard = Heard::new(&session);
    reports_a_command(&mut heard, line, Some(line));
    heard.until(is_prompt);
    session.write(
        b"set -q MOONFRAME_FISH_DATA; or string match -q '*cache/fish*' -- \"$XDG_DATA_DIRS\"; \
          or echo clean-(math 40 + 2)\r",
    );
    assert!(
        wait_for(&session, "clean-42"),
        "fish kept our folder:\n{}",
        shown(&session.screen())
    );
}

#[test]
#[cfg(unix)]
fn bash_reports_commands_once_it_is_asked_to() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join(".bash_profile"),
        ". \"$MOONFRAME_SHELL_INTEGRATION/moonframe.bash\"\n",
    )
    .unwrap();
    let Some(session) = integrated("bash", home.path()) else {
        eprintln!("bash is not installed; skipped");
        return;
    };
    reports_a_command(&mut Heard::new(&session), "(exit 3)", None);
}
