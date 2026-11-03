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
    let mut child = command
        .spawn()
        .map_err(|_| Failure::Network("cannot start the forge CLI".into()))?;
    drop(command);
    let result = collect(&mut child, deadline);
    if result.is_err() {
        stop(&mut child);
    }
    result
}

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
