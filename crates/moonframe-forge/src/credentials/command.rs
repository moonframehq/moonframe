//! One deadline covers both the CLI and EOF on its output pipe. A helper
//! retaining stdout after the CLI exits must not strand the fetch worker.

use {
    crate::http::Failure,
    std::{
        io::Read,
        process::{Child, Command, Stdio},
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    },
};

pub(super) fn run_for(
    mut command: Command,
    capture: bool,
    timeout: Duration,
) -> Result<(bool, Vec<u8>), Failure> {
    let deadline = Instant::now() + timeout;
    command.stdout(if capture {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = spawn(&mut command, deadline)
        .map_err(|error| Failure::Network(format!("cannot start the forge CLI: {error}")))?;
    drop(command);
    let result = collect(&mut child, deadline);
    if result.is_err() {
        stop(&mut child);
    }
    result
}

/// Starts `command`, again a little later while its program is busy being
/// written (`ETXTBSY`): the CLI being replaced by an update, or, in a
/// process that starts others, one that forked while the file was open.
fn spawn(command: &mut Command, deadline: Instant) -> std::io::Result<Child> {
    loop {
        match command.spawn() {
            Err(error)
                if error.kind() == std::io::ErrorKind::ExecutableFileBusy
                    && Instant::now() + BUSY_WAIT < deadline =>
            {
                thread::sleep(BUSY_WAIT);
            },
            started => return started,
        }
    }
}

/// How long to wait before starting a busy program again.
const BUSY_WAIT: Duration = Duration::from_millis(10);

fn collect(child: &mut Child, deadline: Instant) -> Result<(bool, Vec<u8>), Failure> {
    let reader = child
        .stdout
        .take()
        .map(|stdout| {
            let (send, receive) = mpsc::channel();
            thread::Builder::new()
                .name("forge-cli-output".into())
                .spawn(move || {
                    let mut bytes = Vec::new();
                    let result = stdout.take(8193).read_to_end(&mut bytes).map(|_| bytes);
                    let _ = send.send(result);
                })
                .map(|_| receive)
        })
        .transpose()
        .map_err(|_| Failure::Network("cannot read the forge CLI output".into()))?;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(20));
            },
            Ok(None) => return Err(timed_out()),
            Err(_) => return Err(Failure::Network("cannot wait for the forge CLI".into())),
        }
    };
    let bytes = match reader {
        Some(reader) => reader
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| timed_out())?
            .map_err(|_| Failure::Network("cannot read the forge CLI output".into()))?,
        None => Vec::new(),
    };
    if bytes.len() > 8192 {
        return Err(Failure::Body("the forge CLI output is too large".into()));
    }
    Ok((status.success(), bytes))
}

fn timed_out() -> Failure {
    Failure::Network("the forge CLI timed out".into())
}

fn stop(child: &mut Child) {
    #[cfg(unix)]
    if let Some(group) = rustix::process::Pid::from_raw(child.id() as i32) {
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use {
        super::*,
        std::{io::Write, os::unix::fs::PermissionsExt},
    };

    #[test]
    #[allow(clippy::unwrap_used)]
    fn a_program_still_being_written_starts_once_it_is_closed() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("cli");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"#!/bin/sh\nprintf ready\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        // Open for writing a little longer: starting it is ETXTBSY until then.
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            drop(file);
        });
        let (ok, out) = run_for(Command::new(&path), true, Duration::from_secs(5)).unwrap();
        writer.join().unwrap();
        assert!(ok);
        assert_eq!(out, b"ready");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn a_program_that_stays_busy_fails_by_the_deadline() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("cli");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"#!/bin/sh\nprintf ready\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let started = Instant::now();
        let failed = run_for(Command::new(&path), true, Duration::from_millis(200));
        drop(file);
        assert!(
            matches!(&failed, Err(Failure::Network(why)) if why.contains("busy")),
            "{failed:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }
}
