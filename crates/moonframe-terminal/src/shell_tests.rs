//! Real shells started as a terminal starts them, reporting through the
//! integration's scripts.

use {
    crate::{Backend as _, Engine, Open, Pty, Screen, Session, Signal, Size},
    std::{
        collections::VecDeque,
        process::Command,
        sync::Arc,
        time::{Duration, Instant},
    },
};

/// The whole screen as text.
fn shown(screen: &Screen) -> String {
    (0..screen.rows)
        .map(|row| {
            screen
                .row(row)
                .iter()
                .map(|cell| cell.ch)
                .collect::<String>()
        })
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
fn bash_reports_commands_once_it_is_asked_to() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join(".bash_profile"),
        ". \"$MOONFRAME_SHELL_INTEGRATION/moonframe.bash\"\n",
    )
    .unwrap();
    // Older ones, such as macOS's own 3.2, have no `PS0`, so no start mark.
    let modern = Command::new("bash")
        .args([
            "-c",
            "(( BASH_VERSINFO[0] * 100 + BASH_VERSINFO[1] >= 404 ))",
        ])
        .status()
        .is_ok_and(|status| status.success());
    if !modern {
        eprintln!("no bash of 4.4 or later; skipped");
        return;
    }
    let Some(session) = integrated("bash", home.path()) else {
        eprintln!("bash is not installed; skipped");
        return;
    };
    reports_a_command(&mut Heard::new(&session), "(exit 3)", None);
}
