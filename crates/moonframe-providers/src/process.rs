//! Process providers: a command run in the provider's folder that prints one
//! JSON value per line on stdout and reads messages as
//! `{"from": "<mod>", "message": ...}`
//! lines on stdin. Its stderr goes to the app's log.

use {
    crate::{
        Context, Kind, Publisher, Restart,
        outbox::Outbox,
        stream::{self, Ended, Pumped, Reporter},
    },
    std::{
        process::{Child, Command, Stdio},
        sync::{Arc, atomic::Ordering, mpsc},
        thread,
        time::{Duration, Instant},
    },
};

/// How long a process has after SIGTERM, on top of `stop-grace` after its
/// stdin closes, before it is killed.
pub(crate) const TERM_GRACE: Duration = Duration::from_millis(250);
/// How often a stopping process is looked at.
const STOPPING_POLL: Duration = Duration::from_millis(5);
/// The first wait before starting a process again; each doubles, up to
/// [`SLOWEST`], until one stays up for [`SETTLED`].
const QUICKEST: Duration = Duration::from_millis(500);
const SLOWEST: Duration = Duration::from_secs(30);
const SETTLED: Duration = Duration::from_secs(60);

pub(crate) fn run(context: &Context) {
    if let Some(left) = &context.replaces {
        // Bounded: `finish` runs out of steps.
        while !left.load(Ordering::Relaxed) {
            thread::sleep(STOPPING_POLL);
        }
    }
    let mut reporter = Reporter::new(context);
    let mut delay = QUICKEST;
    while !context.stopped() {
        let started = Instant::now();
        let result = serve(context, &mut reporter);
        reporter.connected(false);
        let Err(problem) = result else {
            return;
        };
        context.fail(problem);
        if context.manifest.restart != Restart::OnFailure || !cfg!(feature = "restart") {
            return;
        }
        if started.elapsed() >= SETTLED {
            delay = QUICKEST;
        }
        if !stream::wait(context, delay, &mut reporter) {
            return;
        }
        delay = (delay * 2).min(SLOWEST);
    }
}

fn serve(context: &Context, reporter: &mut Reporter) -> Result<(), String> {
    let Kind::Process { command } = &context.manifest.kind else {
        return Err("not a process provider".to_owned());
    };
    let (program, arguments) = command.split_first().ok_or("an empty `command`")?;
    let mut child = Command::new(program);
    if let Some(profile) = crate::PROFILE.get() {
        // Which app it serves, to keep state of its own per app.
        child.env("MOONFRAME_PROFILE", profile);
    }
    if let Some(dir) = crate::PAYLOAD.get() {
        // What the bundle carries for it: a Python, an engine.
        child.env("MOONFRAME_PAYLOAD", dir);
    }
    if let Some(dir) = crate::launch_dir() {
        // Where the app was started, for one that shows that project.
        child.env("MOONFRAME_LAUNCH_DIR", dir);
    }
    if let Some(dir) = crate::project() {
        // The repository of the terminal the user last used.
        child.env("MOONFRAME_PROJECT", dir);
    }
    // A group of its own, for stopping to reach what it started as well: a
    // shell script's Python, an engine's helpers.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut child, 0);
    let mut child = child
        .args(arguments)
        .current_dir(&context.dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot start `{program}`: {error}"))?;
    let outbox = Arc::new(Outbox::new(&context.manifest));
    reporter.connected(true);
    let result = drive(context, &mut child, &outbox, reporter);
    finish(&mut child, &outbox, context.manifest.stop_grace);
    reporter.carry(outbox.load().1);
    result
}

/// Stops `child` and what it started, in order, each step only if the last
/// did not do it: closing its stdin, which a provider should take as its
/// cue to leave and may use `stop-grace` for; SIGTERM; then killing them.
fn finish(child: &mut Child, outbox: &Outbox, grace: Duration) {
    outbox.close();
    if gone(child, grace, || false) {
        return;
    }
    terminate(child);
    if !gone(child, TERM_GRACE, || false) {
        kill(child);
    }
}

/// Whether `child` and what it started ended within `time`, unless the
/// caller interrupts the wait to start shutdown's own grace period.
fn gone(child: &mut Child, time: Duration, interrupted: impl Fn() -> bool) -> bool {
    let until = Instant::now() + time;
    loop {
        if !matches!(child.try_wait(), Ok(None)) && !lingering(child) {
            return true;
        }
        if interrupted() || Instant::now() >= until {
            return false;
        }
        thread::sleep(STOPPING_POLL);
    }
}

/// Sends `signal` to the group `child` leads; whether anyone is there.
#[cfg(unix)]
#[allow(unsafe_code, reason = "sending a signal; see SAFETY")]
fn signal_group(child: &Child, signal: libc::c_int) -> bool {
    libc::pid_t::try_from(child.id()).is_ok_and(|pid| {
        // SAFETY: `kill` takes plain integers. The group keeps the leader's
        // id for as long as anyone is in it, and the moments between the
        // last one leaving and this call are too few for it to be reused.
        unsafe { libc::kill(-pid, signal) == 0 }
    })
}

/// Whether anything `child` started is still there.
#[cfg(unix)]
fn lingering(child: &Child) -> bool {
    signal_group(child, 0)
}

#[cfg(not(unix))]
fn lingering(_: &Child) -> bool {
    false
}

#[cfg(unix)]
fn terminate(child: &Child) {
    signal_group(child, libc::SIGTERM);
}

#[cfg(not(unix))]
fn terminate(_: &Child) {}

fn kill(child: &mut Child) {
    #[cfg(unix)]
    signal_group(child, libc::SIGKILL);
    let _ = child.kill();
    let _ = child.wait();
}

fn drive(
    context: &Context,
    child: &mut Child,
    outbox: &Arc<Outbox>,
    reporter: &mut Reporter,
) -> Result<(), String> {
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
    let pumped = stream::pump(context, outbox, &end, reporter, || {
        !matches!(child.try_wait(), Ok(None))
    });
    match pumped {
        Pumped::Stopped => Ok(()),
        Pumped::Ended(Ended::Problem(problem)) => Err(problem),
        Pumped::Ended(Ended::Closed) => {
            // Allow a normal exit to report its status, but leave this
            // wait as soon as shutdown asks: `finish` owns the stop grace.
            gone(child, context.manifest.stop_grace, || context.stopped());
            Err(match child.try_wait() {
                Ok(Some(status)) => format!("exited ({status})"),
                _ => "closed its output".to_owned(),
            })
        },
    }
}
