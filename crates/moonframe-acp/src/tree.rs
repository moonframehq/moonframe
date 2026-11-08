//! An agent's whole process tree. An agent may be a launcher, such as
//! `npx`, that starts the real adapter as its child, so killing the agent
//! alone could leave the adapter running. Each agent starts in a process
//! group of its own, and stopping it signals the group.

use std::process::Command;

/// Starts what `command` runs in a process group of its own.
pub(crate) fn own_group(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(not(unix))]
    let _ = command;
}

/// Asks the group led by `pid` to stop, or with `force` makes it.
#[cfg(unix)]
#[allow(unsafe_code, reason = "sending a signal; see SAFETY")]
pub(crate) fn signal(pid: u32, force: bool) {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return;
    };
    if pid <= 0 {
        return;
    }
    let signal = if force {
        libc::SIGKILL
    } else {
        libc::SIGTERM
    };
    // SAFETY: `kill` takes plain integers. The group is the agent's own,
    // made at its start, and lives while any process of it does.
    unsafe {
        libc::kill(-pid, signal);
    }
}

/// Elsewhere the agent itself is killed, as before.
#[cfg(not(unix))]
pub(crate) fn signal(_pid: u32, _force: bool) {}
