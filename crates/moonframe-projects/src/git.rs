//! Running git and `gh`. An app opened from the Dock has no shell's `PATH`,
//! so programs are looked for where they install themselves too.

use {
    moonframe_providers::program::default_search_path,
    std::{
        path::Path,
        process::{Command, Stdio},
    },
};

/// `program`, finding what it runs where a shell would.
pub(crate) fn command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.env("PATH", default_search_path());
    command.stdin(Stdio::null());
    command
}

/// git in `dir`, for reading, its output or `None` when it fails. Names need
/// not be UTF-8; they print with a replacement character.
pub(crate) fn git(dir: &str, args: &[&str]) -> Option<String> {
    let done = command("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stderr(Stdio::null())
        .output()
        .ok()?;
    done.status
        .success()
        .then(|| String::from_utf8_lossy(&done.stdout).into_owned())
}

/// Runs a command in `dir`: whether it passed, and what it said.
pub(crate) fn run(dir: &str, program: &str, args: &[&str]) -> (bool, String) {
    let done = command(program)
        .args(args)
        .current_dir(Path::new(dir))
        .env("GIT_TERMINAL_PROMPT", "0")
        .output();
    let Ok(done) = done else {
        return (false, String::new());
    };
    let mut text = String::from_utf8_lossy(&done.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&done.stderr));
    (done.status.success(), text.trim_end().to_owned())
}
