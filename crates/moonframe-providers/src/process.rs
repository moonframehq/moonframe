//! Process providers: a command run in the provider's folder that prints one
//! JSON value per line on stdout and reads messages as
//! `{"from": "<mod>", "message": ...}`
//! lines on stdin. Its stderr goes to the app's log.

use {
    crate::{
        Context, Kind, Publisher,
        outbox::Outbox,
        stream::{self, Ended, Pumped, Reporter},
    },
    std::{
        process::{Child, Command, Stdio},
        sync::{Arc, mpsc},
        thread,
        time::{Duration, Instant},
    },
};

/// How long a process has after SIGTERM, on top of `stop-grace` after its
/// stdin closes, before it is killed.
pub(crate) const TERM_GRACE: Duration = Duration::from_millis(250);
/// How often a stopping process is looked at.
const STOPPING_POLL: Duration = Duration::from_millis(5);

pub(crate) fn run(context: &Context) {
    if let Err(problem) = serve(context) {
        context.fail(problem);
    }
}

fn serve(context: &Context) -> Result<(), String> {
    let Kind::Process { command } = &context.manifest.kind else {
        return Err("not a process provider".to_owned());
    };
    let (program, arguments) = command.split_first().ok_or("an empty `command`")?;
    let mut child = Command::new(program);
    if let Some(profile) = crate::PROFILE.get() {
        // Which app it serves, to keep state of its own per app.
        child.env("MOONFRAME_PROFILE", profile);
    }
    if let Some(dir) = crate::launch_dir() {
        // Where the app was started, for one that shows that project.
        child.env("MOONFRAME_LAUNCH_DIR", dir);
    }
    if let Some(dir) = crate::project() {
        // The repository of the terminal the user last used.
        child.env("MOONFRAME_PROJECT", dir);
    }
    let mut child = child
        .args(arguments)
        .current_dir(&context.dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot start `{program}`: {error}"))?;
    let outbox = Arc::new(Outbox::new(&context.manifest));
    let result = drive(context, &mut child, &outbox);
    finish(&mut child, &outbox, context.manifest.stop_grace);
    result
}

/// Stops `child` in order, each step only if the last did not do it:
/// closing its stdin, which a provider should take as its cue to leave
/// and may use `stop-grace` for; SIGTERM; then killing it.
fn finish(child: &mut Child, outbox: &Outbox, grace: Duration) {
    outbox.close();
    if gone(child, grace) {
        return;
    }
    terminate(child);
    if !gone(child, TERM_GRACE) {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Whether `child` ended within `time`.
fn gone(child: &mut Child, time: Duration) -> bool {
    let until = Instant::now() + time;
    loop {
        if !matches!(child.try_wait(), Ok(None)) {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        thread::sleep(STOPPING_POLL);
    }
}

#[cfg(unix)]
#[allow(unsafe_code, reason = "sending a signal; see SAFETY")]
fn terminate(child: &Child) {
    if let Ok(pid) = libc::pid_t::try_from(child.id()) {
        // SAFETY: `kill` takes plain integers. The child has not been
        // waited for, so its pid still names it.
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
}

#[cfg(not(unix))]
fn terminate(_: &Child) {}

fn drive(context: &Context, child: &mut Child, outbox: &Arc<Outbox>) -> Result<(), String> {
    let stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let stderr = child.stderr.take().ok_or("no stderr")?;
    let name = context.name.clone();
    thread::spawn(move || {
        crate::lines::log_lines(stderr, |line| {
            tracing::info!(provider = %name, "{}", moonframe_core::printable(&line));
        });
    });
    // Each on a thread of its own: output is published the moment it is
    // read, and a process that never reads its messages fills the pipe,
    // which must hold up nothing here. Closing the outbox ends the writer
    // and with it stdin; killing the process ends either if stuck.
    let (ended, end) = mpsc::channel();
    let publisher = Publisher::of(context);
    let stop = context.stop.clone();
    thread::spawn(move || {
        let _ = ended.send(stream::publish_lines(stdout, &publisher, &stop));
    });
    let writing = outbox.clone();
    thread::spawn(move || stream::write_lines(stdin, &writing));
    let mut reporter = Reporter::new(context);
    match stream::pump(context, outbox, &end, &mut reporter) {
        Pumped::Stopped => Ok(()),
        Pumped::Ended(Ended::Problem(problem)) => Err(problem),
        Pumped::Ended(Ended::Closed) => {
            let status = child.wait().map_err(|error| error.to_string())?;
            Err(format!("exited ({status})"))
        },
    }
}
