//! Sheprd's chat program (`sheprd-acp`, once `herdr-acp`): finding it, starting a chat in it with no view
//! of its own, finding where that chat listens, and ending it.
//!
//! Herdr learns a chat's endpoint from the pane it runs in; a chat started
//! here runs in none, so its endpoint is the socket its process listens on,
//! named after the endpoint's id in herdr's private endpoint folder.

mod endpoint;

use {
    herdr_chat_protocol::EndpointId,
    std::{
        ffi::OsStr,
        fs::File,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    },
};

/// The program's file names, newest first: Sheprd still bundles it under
/// its old name too, and older installs have only that.
const PROGRAMS: &[&str] = &["sheprd-acp", "herdr-acp"];
/// How long a new chat may take to start listening.
const LISTEN_TIMEOUT: Duration = Duration::from_secs(20);

/// The apps that bundle the program, in the order they are tried.
const BUNDLES: &[&str] = &[
    "Applications/Sheprd.app/Contents/MacOS",
    "Applications/Herdr.app/Contents/MacOS",
];

/// Where an installed app keeps the program: in `/Applications`, then in
/// the user's own `~/Applications`, each app's newest name first.
pub(crate) fn bundled(home: Option<&Path>) -> Vec<PathBuf> {
    let roots = [Some(Path::new("/")), home];
    roots
        .into_iter()
        .flatten()
        .flat_map(|root| {
            BUNDLES.iter().flat_map(move |bundle| {
                PROGRAMS
                    .iter()
                    .map(move |program| root.join(bundle).join(program))
            })
        })
        .collect()
}

/// Where the program was found, and a `program` setting that named no file
/// and so was passed over for the search.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Found {
    pub(crate) program: Option<PathBuf>,
    pub(crate) missing: Option<PathBuf>,
}

/// Where the program is: the setting when it names a file, else the search
/// path, then inside an installed app (`bundled`).
pub(crate) fn find(setting: Option<&Path>, path: &OsStr, bundled: &[PathBuf]) -> Found {
    if let Some(setting) = setting.filter(|setting| setting.is_file()) {
        return Found {
            program: Some(setting.to_path_buf()),
            missing: None,
        };
    }
    let on_path = PROGRAMS
        .iter()
        .find_map(|program| moonframe_providers::program::find(program, path));
    let program = on_path.or_else(|| {
        bundled
            .iter()
            .find(|candidate| candidate.is_file())
            .cloned()
    });
    Found {
        program,
        missing: setting.map(Path::to_path_buf),
    }
}

/// The agents installed here, as the program's `agents` names them.
pub(crate) fn agents(program: &Path, path: &OsStr) -> Vec<String> {
    let output = Command::new(program)
        .arg("agents")
        .env("PATH", path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|name| !name.is_empty() && name.len() <= 32)
            .map(str::to_owned)
            .collect(),
        Ok(_) | Err(_) => Vec::new(),
    }
}

/// What starts one chat.
pub(crate) struct Spawn<'a> {
    pub program: &'a Path,
    pub agent: &'a str,
    pub cwd: &'a Path,
    /// The agent's session to continue; a new conversation instead lets
    /// the user switch to another installed agent (`--choose`), which the
    /// program refuses for a continued one.
    pub load: Option<&'a str>,
    pub path: &'a OsStr,
    /// Where the program's errors go.
    pub log: Option<&'a Path>,
}

/// Starts the chat program with no view, in a process group of its own so
/// it outlives the app (the next launch attaches again) and ends whole.
/// Answers its process id and the endpoint it listens on.
pub(crate) fn spawn(spawn: &Spawn<'_>) -> Result<(u32, EndpointId), String> {
    let folder = herdr_chat_protocol::endpoint_dir()
        .ok_or_else(|| "herdr's endpoint folder is unknown here".to_owned())?;
    let mut command = Command::new(spawn.program);
    command.arg("--headless").arg("--cwd").arg(spawn.cwd);
    match spawn.load {
        Some(session) => command.arg("--load").arg(session),
        None => command.arg("--choose"),
    };
    command
        .arg(spawn.agent)
        .current_dir(spawn.cwd)
        .env("PATH", spawn.path)
        // Not a Herdr pane, even when the app runs in one.
        .env_remove("HERDR_PANE_ID")
        .env_remove("HERDR_SOCKET_PATH")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(match spawn.log.and_then(|log| File::create(log).ok()) {
            Some(file) => Stdio::from(file),
            None => Stdio::null(),
        });
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start {}: {error}", spawn.program.display()))?;
    let pid = child.id();
    let started = Instant::now();
    loop {
        if let Some(endpoint) = endpoint::listening(pid, &folder) {
            // Reap it whenever it ends; it is not waited on otherwise.
            let _ = thread::Builder::new()
                .name("chat-reap".to_owned())
                .spawn(move || child.wait());
            return Ok((pid, endpoint));
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "the chat program ended before it listened ({status})"
            ));
        }
        if started.elapsed() > LISTEN_TIMEOUT {
            end(pid);
            let _ = child.wait();
            return Err(
                "could not find the chat program's listening socket (needs /proc on Linux or lsof)"
                    .to_owned(),
            );
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Whether the chat at `endpoint` may still be running: its socket is there.
pub(crate) fn alive(endpoint: &EndpointId) -> bool {
    herdr_chat_protocol::endpoint_address(endpoint).is_some_and(|address| address.exists())
}

/// Ends the chat program `pid` leads and the agent under it.
#[cfg(unix)]
pub(crate) fn end(pid: u32) {
    use rustix::process::{Pid, Signal, kill_process_group};
    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return;
    };
    if let Err(error) = kill_process_group(pid, Signal::TERM) {
        tracing::debug!("chat: cannot end {pid:?}: {error}");
    }
}

#[cfg(not(unix))]
pub(crate) fn end(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .status();
}
