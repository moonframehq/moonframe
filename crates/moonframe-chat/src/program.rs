//! The `herdr-acp` program: finding it, starting a chat in it with no view
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

/// The program's file name.
const PROGRAM: &str = "herdr-acp";
/// How long a new chat may take to start listening.
const LISTEN_TIMEOUT: Duration = Duration::from_secs(20);

/// Where `herdr-acp` is: the setting, then the search path, then inside an
/// installed Herdr.
pub(crate) fn find(setting: Option<&Path>, path: &OsStr, home: Option<&Path>) -> Option<PathBuf> {
    if let Some(setting) = setting {
        return setting.is_file().then(|| setting.to_path_buf());
    }
    if let Some(found) = moonframe_providers::program::find(PROGRAM, path) {
        return Some(found);
    }
    let bundled = Path::new("Applications/Herdr.app/Contents/MacOS").join(PROGRAM);
    [
        Some(Path::new("/").join(&bundled)),
        home.map(|home| home.join(&bundled)),
    ]
    .into_iter()
    .flatten()
    .find(|candidate| candidate.is_file())
}

/// The agents installed here, as `herdr-acp agents` names them.
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
    /// The agent's session to continue.
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
    if let Some(session) = spawn.load {
        command.arg("--load").arg(session);
    }
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
            return Err(format!("herdr-acp ended before it listened ({status})"));
        }
        if started.elapsed() > LISTEN_TIMEOUT {
            end(pid);
            let _ = child.wait();
            return Err(
                "could not find herdr-acp's listening socket (needs /proc on Linux or lsof)"
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
