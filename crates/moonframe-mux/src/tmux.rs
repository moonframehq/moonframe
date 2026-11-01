//! Terminals kept by tmux, on a server of the app's own (`tmux -L
//! <app>`: `moonframe` for the starter, `moonframe-<profile>` for a
//! profile, the app's name for one on the library), one session per
//! terminal named after its key. A server is tmux's own way to keep a group
//! of sessions apart: two apps never see each other's terminals, and `tmux
//! -L <app> ls` lists one app's. They outlive the app, and `tmux -L <app>
//! attach -t <name>` shows one from any other terminal.
//!
//! The server starts without the user's tmux.conf, so their plugins never
//! touch it.
//!
//! The app reaches a session through a control-mode client (`tmux -C`) on
//! pipes rather than drawing tmux's own screen: the pane's raw output comes
//! as `%output` lines for our emulator, keys go back as `send-keys -H`, and
//! the size as `refresh-client -C`. tmux emulates the pane too, so it
//! answers the program's queries itself.

mod control;

use {
    crate::program,
    control::{Expect, Reader},
    moonframe_terminal::{Backend, Open, Opened, Pane, Size},
    std::{
        collections::VecDeque,
        io::{BufReader, Write},
        path::PathBuf,
        process::{Child, ChildStdin, Command, Stdio},
        sync::{Arc, Mutex},
    },
};

/// The tmux server sessions live on until [`Tmux::on_socket`] names the
/// app's.
pub const SOCKET: &str = "moonframe";
/// Scrollback lines copied into a terminal when it attaches.
const HISTORY: u32 = 2000;
/// Bytes per `send-keys` command, to keep command lines short.
const KEYS_PER_COMMAND: usize = 256;

/// tmux, when it is installed.
#[derive(Clone, Debug)]
pub struct Tmux {
    program: PathBuf,
    socket: String,
}

impl Tmux {
    /// The `tmux` on PATH or in the usual places, if it runs.
    pub fn find() -> Option<Self> {
        let program = program::find(&["tmux"], |path| {
            program::first_line(path, &["-V"]).is_some_and(|version| version.starts_with("tmux "))
        })?;
        Some(Self {
            program,
            socket: SOCKET.to_owned(),
        })
    }

    /// The same tmux on another server: the app's, or a test's.
    pub fn on_socket(self, socket: &str) -> Self {
        Self {
            socket: socket.to_owned(),
            ..self
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        // The server reads no tmux.conf: plugins there (tmux-continuum's
        // restore and autosave) would fill it with the user's own sessions
        // and save ours over theirs. A control client draws nothing of
        // tmux's, so its look and keys don't matter here.
        command.args(["-L", &self.socket, "-f", "/dev/null"]);
        command
    }
}

impl Tmux {
    /// The pane of session `name`, when it runs.
    fn pane_of(&self, name: &str, target: &str) -> Option<String> {
        let output = self
            .command()
            .args(["display-message", "-p", "-t", target, "#{pane_id}"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (output.status.success() && !pane_id.is_empty())
            .then_some(pane_id)
            .inspect(|_| tracing::debug!("tmux session `{name}` runs; attaching"))
    }

    /// Starts session `name` detached, and returns its pane.
    fn start(&self, name: &str, open: &Open) -> Result<String, String> {
        let mut start = self.command();
        start.args([
            "new-session",
            "-d",
            "-P",
            "-F",
            "#{pane_id}",
            "-s",
            name,
            "-x",
            &open.size.cols.max(1).to_string(),
            "-y",
            &open.size.rows.max(1).to_string(),
            "-e",
            "COLORTERM=truecolor",
        ]);
        if let Some(cwd) = open.cwd.as_ref().filter(|cwd| cwd.is_dir()) {
            start.arg("-c").arg(cwd);
        } else if let Some(home) = std::env::var_os("HOME") {
            start.arg("-c").arg(home);
        }
        let started = start
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| format!("cannot run tmux: {error}"))?;
        if !started.status.success() {
            return Err(format!(
                "tmux could not start `{name}`: {}",
                String::from_utf8_lossy(&started.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&started.stdout).trim().to_owned())
    }
}

impl Backend for Tmux {
    fn name(&self) -> &'static str {
        "tmux"
    }

    fn open(&self, open: &Open) -> Result<Opened, String> {
        let name = session_name(&open.key);
        let target = format!("={name}:");
        let pane_id = self
            .pane_of(&name, &target)
            .map_or_else(|| self.start(&name, open), Ok)?;
        if !pane_id.starts_with('%') {
            return Err(format!("tmux named no pane for `{name}`"));
        }

        let mut client = self
            .command()
            .args(["-C", "attach-session", "-t", &format!("={name}")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("cannot attach to tmux: {error}"))?;
        let (Some(stdin), Some(stdout)) = (client.stdin.take(), client.stdout.take()) else {
            let _ = client.kill();
            return Err("tmux's control client has no pipes".into());
        };
        let expected = Arc::new(Mutex::new(VecDeque::new()));
        let pane = TmuxPane {
            commands: Mutex::new(Commands {
                stdin,
                expected: expected.clone(),
            }),
            pane: pane_id.clone(),
            client: Mutex::new(client),
        };
        // Its size first, so what is copied next is laid out for it.
        pane.send(&resize_command(open.size), Expect::Nothing);
        pane.send(
            &format!("capture-pane -p -e -S -{HISTORY} -t {pane_id}"),
            Expect::Capture,
        );
        pane.send(
            &format!(
                "display-message -p -t {pane_id} \
                 '#{{cursor_x}} #{{cursor_y}} #{{keypad_cursor_flag}} #{{alternate_on}}'"
            ),
            Expect::Cursor,
        );
        Ok(Opened {
            pane: Box::new(pane),
            output: Box::new(Reader::new(BufReader::new(stdout), pane_id, expected)),
        })
    }
}

/// A session's name for `key`: tmux refuses `.` and `:`, and `=name`
/// targets read better without spaces or quotes. A key of letters, digits,
/// `-`, `_` and `/` keeps the name it always had; any other key would share
/// its name with another (`a.b`, `a b`, `a_b`), so it gets a hash of the
/// whole key after a `~`, which the plain names never have.
pub fn session_name(key: &str) -> String {
    let mut lossy = false;
    let mut name: String = key
        .chars()
        .map(|char| match char {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => char,
            '/' => '+',
            _ => {
                lossy = true;
                '_'
            },
        })
        .collect();
    if lossy {
        name.push_str(&format!("~{:016x}", fnv1a(key.as_bytes())));
    }
    name
}

/// FNV-1a: a hash that stays the same across builds and Rust versions, as a
/// session's name must to be found again at the next launch.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn resize_command(size: Size) -> String {
    format!(
        "refresh-client -C {}x{}",
        size.cols.max(1),
        size.rows.max(1)
    )
}

/// The control client's stdin, and the replies its commands will get, kept
/// together so they stay in the same order.
struct Commands {
    stdin: ChildStdin,
    expected: Arc<Mutex<VecDeque<Expect>>>,
}

struct TmuxPane {
    commands: Mutex<Commands>,
    pane: String,
    client: Mutex<Child>,
}

impl TmuxPane {
    fn send(&self, command: &str, expect: Expect) {
        let Ok(mut commands) = self.commands.lock() else {
            return;
        };
        if let Ok(mut expected) = commands.expected.lock() {
            expected.push_back(expect);
        }
        let line = format!("{command}\n");
        let _ = commands
            .stdin
            .write_all(line.as_bytes())
            .and_then(|()| commands.stdin.flush());
    }
}

impl Drop for TmuxPane {
    /// Detaches: the session and its program keep running.
    fn drop(&mut self) {
        if let Ok(client) = self.client.get_mut() {
            let _ = client.kill();
            let _ = client.wait();
        }
    }
}

impl Pane for TmuxPane {
    fn write(&self, bytes: &[u8]) {
        for chunk in bytes.chunks(KEYS_PER_COMMAND) {
            let mut command = format!("send-keys -H -t {}", self.pane);
            for byte in chunk {
                command.push_str(&format!(" {byte:02x}"));
            }
            self.send(&command, Expect::Nothing);
        }
    }

    fn resize(&self, size: Size) {
        self.send(&resize_command(size), Expect::Nothing);
    }

    fn answers_queries(&self) -> bool {
        true
    }
}
