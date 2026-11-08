//! `moonframe pane new`: a terminal tab in the running window, opened from
//! the command line, with a coding agent started in it if asked:
//!
//! ```sh
//! moonframe pane new --agent claude "add a clock to the status bar"
//! ```
//!
//! The command leaves a request in the app's state folder and the window
//! picks it up, so it works the same on every platform and needs no
//! socket. When no window takes it, the command opens the app, which takes
//! it as it starts. The tab is the main tab bar's (`tabs/main/open`), a
//! terminal in the user's folder unless `--cwd` says otherwise, and the
//! command line is typed into its shell by the app, never by Luau: a mod
//! picks where a terminal starts, never what runs in it.

use {
    serde::{Deserialize, Serialize},
    std::{
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
        process::{Command, ExitCode, Stdio},
        time::{Duration, SystemTime, UNIX_EPOCH},
    },
};

/// The words `pane` takes.
pub(crate) const USAGE: &str = "\
Usage: moonframe pane new [--agent NAME] [--cwd DIR] [--title TEXT] [PROMPT...]

Opens a terminal tab in the running window (opening the app if it isn't),
in your folder (~/.config/moonframe, or the profile's) unless --cwd says
otherwise. With --agent, the app types the agent's command into it, with
the prompt, so you watch the agent work while the window changes:

  moonframe pane new --agent claude \"add a clock to the status bar\"
  moonframe pane new --agent codex --cwd ~/code/my-app
  moonframe --profile some-app pane new --agent claude

  --agent NAME  the agent's command: claude, codex, gemini, opencode, ...
                (one word; it runs in your login shell, as you)
  --cwd DIR     the folder the terminal starts in
  --title TEXT  the tab's title (default: the agent's name)";

/// How long a request waits for a window before the command opens the app.
const RUNNING: Duration = Duration::from_millis(1500);
/// How long a request waits for the app the command opened.
const STARTING: Duration = Duration::from_secs(20);
/// A request older than this is dropped unread: the window that was meant
/// to show it never came, and a tab opening much later would surprise.
const MAX_AGE: Duration = Duration::from_secs(60);
/// How often the window looks for requests.
pub(crate) const POLL: Duration = Duration::from_millis(400);

/// A tab to open, as the command leaves it for the window.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Request {
    /// The agent's command, one word.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agent: Option<String>,
    /// What to ask it, if anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) prompt: Option<String>,
    /// An absolute folder; the window's own user folder when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cwd: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    /// Seconds since the epoch, when the command made it.
    #[serde(default)]
    pub(crate) created: u64,
}

impl Request {
    /// The tab's title.
    pub(crate) fn title(&self) -> Option<String> {
        self.title.clone().or_else(|| self.agent.clone())
    }

    /// What the app types into a new terminal running `shell`: the agent
    /// with the prompt quoted for that shell, and Return. `None` for a
    /// plain shell. A prompt for a shell it cannot quote for is left out
    /// (see [`Request::prompt_lost`]): the agent starts without it.
    pub(crate) fn typed(&self, shell: Shell) -> Option<Vec<u8>> {
        let agent = self.agent.as_deref().filter(|agent| valid_agent(agent))?;
        let mut line = agent.to_owned();
        if let Some(prompt) = self.prompt()
            && let Some(quoted) = shell.quote(&prompt)
        {
            line.push(' ');
            line.push_str(&quoted);
        }
        line.push('\r');
        Some(line.into_bytes())
    }

    /// Whether `shell` drops the prompt: one Moonframe cannot quote for.
    pub(crate) fn prompt_lost(&self, shell: Shell) -> bool {
        self.agent.is_some() && self.prompt().is_some() && shell == Shell::Unknown
    }

    fn prompt(&self) -> Option<String> {
        self.prompt.as_deref().map(clean).filter(|p| !p.is_empty())
    }

    fn expired(&self, now: u64) -> bool {
        now.saturating_sub(self.created) > MAX_AGE.as_secs()
    }
}

/// An agent's command is one plain word, so nothing in it reaches the
/// shell as syntax.
fn valid_agent(agent: &str) -> bool {
    !agent.is_empty()
        && agent.len() <= 64
        && !agent.starts_with('-')
        && agent
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+'))
}

/// The prompt without control characters: no escape sequences reach the
/// terminal, and no line break submits half of it.
fn clean(prompt: &str) -> String {
    prompt
        .chars()
        .map(|c| {
            if c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

/// The shell a new terminal runs, which decides how the prompt is quoted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shell {
    /// sh, bash, zsh, dash, ksh and the like.
    Posix,
    Fish,
    /// PowerShell, Windows' or the cross-platform `pwsh`.
    Pwsh,
    /// Windows' `cmd.exe`.
    Cmd,
    /// One whose quoting Moonframe does not know (nushell, xonsh, ...).
    Unknown,
}

impl Shell {
    /// The one the terminals start: `$SHELL`, or `COMSPEC` on Windows, as
    /// the backends pick it.
    pub(crate) fn current() -> Self {
        let var = if cfg!(windows) {
            "COMSPEC"
        } else {
            "SHELL"
        };
        Self::named(
            std::env::var_os(var)
                .as_deref()
                .and_then(|path| path.to_str()),
        )
    }

    /// The shell at `path`, by its file name.
    fn named(path: Option<&str>) -> Self {
        let Some(path) = path else {
            return if cfg!(windows) {
                Self::Cmd
            } else {
                Self::Unknown
            };
        };
        let name = path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(path)
            .to_ascii_lowercase();
        match name.strip_suffix(".exe").unwrap_or(&name) {
            "sh" | "bash" | "zsh" | "dash" | "ksh" | "mksh" | "oksh" | "loksh" | "ash" | "yash" => {
                Self::Posix
            },
            "fish" => Self::Fish,
            "pwsh" | "powershell" => Self::Pwsh,
            "cmd" => Self::Cmd,
            _ => Self::Unknown,
        }
    }

    /// `text` as one argument, taken literally: nothing in it expands or
    /// ends the command. `None` for a shell whose rules are unknown.
    fn quote(self, text: &str) -> Option<String> {
        Some(match self {
            // One form both read alike, as tmux's or herdr-core's shell may
            // not be $SHELL: single quotes, closed around a quote (`'\''`)
            // and around a backslash (`'\\'`), which fish would otherwise
            // read as an escape inside them.
            Self::Posix | Self::Fish => {
                let mut quoted = String::from("'");
                for c in text.chars() {
                    match c {
                        '\'' => quoted.push_str(r"'\''"),
                        '\\' => quoted.push_str(r"'\\'"),
                        c => quoted.push(c),
                    }
                }
                quoted.push('\'');
                quoted
            },
            // PowerShell also ends a single-quoted string at the curly
            // single quotes; each is doubled to stay literal.
            Self::Pwsh => {
                let mut quoted = String::from("'");
                for c in text.chars() {
                    if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
                        quoted.push(c);
                    }
                    quoted.push(c);
                }
                quoted.push('\'');
                quoted
            },
            // Inside double quotes cmd takes & | < > ^ literally, but not a
            // double quote, which ends them, or %NAME%, which expands: the
            // first becomes a single quote and the second a full-width %.
            Self::Cmd => format!("\"{}\"", text.replace('"', "'").replace('%', "\u{FF05}")),
            Self::Unknown => return None,
        })
    }
}

/// The folder requests wait in, beside the app's state: one for each user
/// folder (`--user`) and profile, so a request reaches only a window of
/// that folder and profile.
pub(crate) fn inbox(user: &Path, profile: Option<&str>) -> PathBuf {
    let user = user.canonicalize().unwrap_or_else(|_| user.to_owned());
    let folder = format!("{:016x}", fnv(user.to_string_lossy().as_bytes()));
    crate::paths::state(profile).with_file_name(match profile {
        Some(profile) => format!("panes-{folder}-{profile}"),
        None => format!("panes-{folder}"),
    })
}

/// FNV-1a: the same for the same folder in every build, unlike std's hasher.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Leaves `request` in `inbox`, whole or not at all, and returns its path.
fn leave(inbox: &Path, request: &Request) -> std::io::Result<PathBuf> {
    fs::create_dir_all(inbox)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let name = format!("{stamp}-{}", std::process::id());
    let path = inbox.join(format!("{name}.json"));
    let partial = inbox.join(format!(".{name}.partial"));
    let json = serde_json::to_vec_pretty(request).map_err(std::io::Error::other)?;
    fs::write(&partial, json)?;
    fs::rename(&partial, &path)?;
    Ok(path)
}

/// The requests waiting in `inbox`, oldest first, each removed as it is
/// read; expired and unreadable ones are dropped. Each is claimed first by
/// renaming it, which only one window can do, so two windows of the same
/// folder never both open it.
pub(crate) fn take(inbox: &Path) -> Vec<Request> {
    let Ok(entries) = fs::read_dir(inbox) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    let now = now();
    paths
        .into_iter()
        .filter_map(|path| {
            let claimed = path.with_extension(format!("claimed-{}", std::process::id()));
            fs::rename(&path, &claimed).ok()?;
            let text = fs::read_to_string(&claimed).ok();
            // Taken, read or not: a broken one must not come back each poll.
            let _ = fs::remove_file(&claimed);
            let request: Request = match serde_json::from_str(text.as_deref()?) {
                Ok(request) => request,
                Err(error) => {
                    tracing::warn!("pane request {}: {error}", path.display());
                    return None;
                },
            };
            if request.expired(now) {
                tracing::info!("pane request {}: too old, dropped", path.display());
                return None;
            }
            Some(request)
        })
        .collect()
}

/// `pane WORDS`, for the window of the user folder `user` and `profile`,
/// called `title`.
pub(crate) fn run(user: &Path, profile: Option<&str>, title: &str, words: &[String]) -> ExitCode {
    let app = crate::paths::name();
    let request = match parse(words) {
        Ok(Some(request)) => request,
        Ok(None) => {
            println!("{}", crate::cli::branded(USAGE));
            return ExitCode::SUCCESS;
        },
        Err(message) => {
            eprintln!("{app} pane: {message}\n\n{}", crate::cli::branded(USAGE));
            return ExitCode::from(2);
        },
    };
    if !cfg!(feature = "terminal") {
        eprintln!(
            "{app} pane: terminal support was left out of this build; enable `alacritty` or `ghostty`"
        );
        return ExitCode::FAILURE;
    }
    // The window creates it as it starts; made now, it names the same
    // inbox before and after.
    if let Err(error) = fs::create_dir_all(user) {
        eprintln!("{app} pane: cannot create {}: {error}", user.display());
        return ExitCode::FAILURE;
    }
    let inbox = inbox(user, profile);
    let path = match leave(&inbox, &request) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{app} pane: cannot write to {}: {error}", inbox.display());
            return ExitCode::FAILURE;
        },
    };
    let what = request
        .agent
        .as_deref()
        .map_or_else(|| "a terminal".to_owned(), str::to_owned);
    if taken(&path, RUNNING) {
        println!("Opened {what} in a new tab of {title}.");
        return ExitCode::SUCCESS;
    }
    // No window took it: open one, which takes it as it starts.
    if let Err(error) = launch() {
        let _ = fs::remove_file(&path);
        eprintln!("{app} pane: no window is open, and opening one failed: {error}");
        return ExitCode::FAILURE;
    }
    if taken(&path, STARTING) {
        println!("Opened {title} with {what} in a new tab.");
        ExitCode::SUCCESS
    } else {
        let _ = fs::remove_file(&path);
        eprintln!("{app} pane: the window did not open; see its log");
        ExitCode::FAILURE
    }
}

/// Whether a window took the request at `path` within `wait`.
fn taken(path: &Path, wait: Duration) -> bool {
    let until = std::time::Instant::now() + wait;
    while std::time::Instant::now() < until {
        if !path.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    !path.exists()
}

/// Opens the app as this command was run (`--profile`, `--user` and the
/// like, the words before `pane`), on its own: closing the terminal it
/// was started from does not close it.
fn launch() -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let before = args
        .iter()
        .position(|arg| arg == "pane" || arg == "panes")
        .unwrap_or(args.len());
    let mut command = Command::new(exe);
    command
        .args(&args[..before])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        command.creation_flags(DETACHED_PROCESS);
    }
    command.spawn().map(drop)
}

/// The request `words` ask for; `None` for help.
fn parse(words: &[String]) -> Result<Option<Request>, String> {
    let mut words = words.iter();
    match words.next().map(String::as_str) {
        Some("new") => {},
        None | Some("help" | "-h" | "--help") => return Ok(None),
        Some(other) => return Err(format!("unknown command `{other}`; try `new`")),
    }
    let mut request = Request {
        created: now(),
        ..Request::default()
    };
    let mut prompt = Vec::new();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--agent" => {
                let agent = words.next().ok_or("--agent needs a name")?;
                if !valid_agent(agent) {
                    return Err(format!(
                        "`{agent}` is not an agent's command: one word of a-z, 0-9, - _ . +"
                    ));
                }
                request.agent = Some(agent.clone());
            },
            "--cwd" => {
                let dir = words.next().ok_or("--cwd needs a folder")?;
                let dir = expand(dir);
                let dir = dir
                    .canonicalize()
                    .map_err(|error| format!("--cwd {}: {error}", dir.display()))?;
                if !dir.is_dir() {
                    return Err(format!("--cwd {}: not a folder", dir.display()));
                }
                request.cwd = Some(dir);
            },
            "--title" => request.title = Some(words.next().ok_or("--title needs a text")?.clone()),
            "-h" | "--help" => return Ok(None),
            "--" => prompt.extend(words.by_ref().cloned()),
            flag if flag.starts_with("--") => return Err(format!("unknown option: {flag}")),
            _ => prompt.push(word.clone()),
        }
    }
    if !prompt.is_empty() {
        if request.agent.is_none() {
            return Err("a prompt needs --agent, to say who it is for".to_owned());
        }
        request.prompt = Some(prompt.join(" "));
    }
    Ok(Some(request))
}

/// `~/...` under the home folder; anything else as it is.
fn expand(dir: &str) -> PathBuf {
    match (dir.strip_prefix("~"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest.trim_start_matches('/')),
        _ => PathBuf::from(dir),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|word| (*word).to_owned()).collect()
    }

    #[cfg(not(feature = "terminal"))]
    #[test]
    fn terminal_free_commands_fail_before_creating_the_user_folder() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("user");
        assert_eq!(
            run(
                &user,
                None,
                "Moonframe",
                &words(&["new", "--agent", "claude"])
            ),
            ExitCode::FAILURE
        );
        assert!(!user.exists());
        assert_eq!(
            run(&user, None, "Moonframe", &words(&["help"])),
            ExitCode::SUCCESS
        );
        assert!(!user.exists());
    }

    #[test]
    fn parses_an_agent_and_its_prompt() {
        let request = parse(&words(&["new", "--agent", "claude", "add a clock", "now"]))
            .unwrap()
            .unwrap();
        assert_eq!(request.agent.as_deref(), Some("claude"));
        assert_eq!(request.prompt.as_deref(), Some("add a clock now"));
        assert_eq!(request.title().as_deref(), Some("claude"));
    }

    #[test]
    fn refuses_what_is_not_one_word() {
        assert!(parse(&words(&["new", "--agent", "claude; rm -rf ~"])).is_err());
        assert!(parse(&words(&["new", "--agent", "--yolo"])).is_err());
        assert!(parse(&words(&["new", "a prompt for no one"])).is_err());
        assert!(parse(&words(&["open"])).is_err());
        assert_eq!(parse(&words(&[])), Ok(None));
    }

    #[test]
    fn types_the_prompt_as_one_quoted_argument() {
        let request = Request {
            agent: Some("claude".into()),
            prompt: Some("it's $HOME\n`and` \x1b[2Jthis".into()),
            ..Request::default()
        };
        let typed = |shell| String::from_utf8(request.typed(shell).unwrap()).unwrap();
        assert_eq!(
            typed(Shell::Posix),
            "claude 'it'\\''s $HOME `and`  [2Jthis'\r"
        );
        assert_eq!(typed(Shell::Fish), typed(Shell::Posix));
        assert_eq!(typed(Shell::Pwsh), "claude 'it''s $HOME `and`  [2Jthis'\r");
        assert_eq!(typed(Shell::Cmd), "claude \"it's $HOME `and`  [2Jthis\"\r");
        // A shell it cannot quote for starts the agent without the prompt.
        assert_eq!(typed(Shell::Unknown), "claude\r");
        assert!(request.prompt_lost(Shell::Unknown));
        assert!(!request.prompt_lost(Shell::Posix));
        assert_eq!(Request::default().typed(Shell::Posix), None);
    }

    #[test]
    fn nothing_in_a_prompt_runs_in_cmd_or_powershell() {
        let request = Request {
            agent: Some("claude".into()),
            prompt: Some(r#"a "b" & del %TEMP% | c ‘d’"#.into()),
            ..Request::default()
        };
        let typed = |shell| String::from_utf8(request.typed(shell).unwrap()).unwrap();
        assert_eq!(
            typed(Shell::Cmd),
            "claude \"a 'b' & del \u{FF05}TEMP\u{FF05} | c ‘d’\"\r"
        );
        assert_eq!(
            typed(Shell::Pwsh),
            "claude 'a \"b\" & del %TEMP% | c ‘‘d’’'\r"
        );
        let fish = Request {
            prompt: Some(r"back\slash\".into()),
            ..request
        };
        // Read as `back\slash\` by sh and fish alike: no backslash is ever
        // inside the quotes, so none escapes the closing one.
        assert_eq!(
            String::from_utf8(fish.typed(Shell::Fish).unwrap()).unwrap(),
            r"claude 'back'\\'slash'\\''".to_owned() + "\r"
        );
    }

    #[test]
    fn knows_shells_by_name() {
        assert_eq!(Shell::named(Some("/bin/zsh")), Shell::Posix);
        assert_eq!(Shell::named(Some("/opt/homebrew/bin/fish")), Shell::Fish);
        assert_eq!(
            Shell::named(Some(r"C:\Windows\System32\cmd.exe")),
            Shell::Cmd
        );
        assert_eq!(
            Shell::named(Some(r"C:\Program Files\PowerShell\7\pwsh.exe")),
            Shell::Pwsh
        );
        assert_eq!(Shell::named(Some("/usr/bin/nu")), Shell::Unknown);
    }

    #[test]
    fn each_user_folder_and_profile_has_its_own_inbox() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        assert_eq!(inbox(a.path(), None), inbox(a.path(), None));
        assert_ne!(inbox(a.path(), None), inbox(b.path(), None));
        assert_ne!(inbox(a.path(), None), inbox(a.path(), Some("work")));
    }

    #[test]
    fn a_claimed_request_is_not_taken_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = leave(
            dir.path(),
            &Request {
                created: now(),
                ..Request::default()
            },
        )
        .unwrap();
        // Another window claimed it first.
        fs::rename(&path, path.with_extension("claimed-1")).unwrap();
        assert!(take(dir.path()).is_empty());
    }

    #[test]
    fn a_request_goes_through_the_inbox_once() {
        let dir = tempfile::tempdir().unwrap();
        let request = Request {
            agent: Some("codex".into()),
            created: now(),
            ..Request::default()
        };
        let path = leave(dir.path(), &request).unwrap();
        assert!(path.exists());
        assert_eq!(take(dir.path()), vec![request]);
        assert!(!path.exists());
        assert!(take(dir.path()).is_empty());
    }

    #[test]
    fn an_old_or_broken_request_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let old = Request {
            created: now() - 3600,
            ..Request::default()
        };
        leave(dir.path(), &old).unwrap();
        fs::write(dir.path().join("broken.json"), "{").unwrap();
        assert!(take(dir.path()).is_empty());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
