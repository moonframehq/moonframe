//! Socket providers: a program that runs on its own, reached over a Unix
//! socket it listens on. The lines are those of a process provider, JSON
//! one way and `{"from": "<mod>", "message": ...}` the other, but the app
//! does not own the program: it connects when it can, connects again when
//! the link drops, and leaves it running when it quits or reloads.
//!
//! Each connection starts with `{"from":"","message":{"connected":true}}`,
//! from the app itself, so the program can say how things really stand.
//! Messages sent while there is no link are dropped and counted, never
//! kept: a command held back would run at some later, unknown moment.

use {
    crate::{
        Context, Kind, Message, Publisher,
        outbox::Outbox,
        stream::{self, Ended, Pumped, Reporter},
    },
    moonframe_core::Data,
    std::{
        collections::BTreeMap, net::Shutdown, os::unix::net::UnixStream, path::Path, sync::mpsc,
        thread, time::Duration,
    },
};

/// The first wait before connecting again; each doubles, up to [`SLOWEST`].
const QUICKEST: Duration = Duration::from_millis(100);
const SLOWEST: Duration = Duration::from_secs(2);

pub(crate) fn run(context: &Context) {
    let Kind::Socket { path } = &context.manifest.kind else {
        context.fail("not a socket provider");
        return;
    };
    let path = context.dir.join(expanded(path));
    let mut reporter = Reporter::new(context);
    let mut delay = QUICKEST;
    reporter.connected(false);
    loop {
        if context.stopped() {
            return;
        }
        match UnixStream::connect(&path) {
            Ok(stream) => {
                delay = QUICKEST;
                if serve(context, stream, &mut reporter) {
                    return;
                }
            },
            Err(error) => {
                tracing::debug!(provider = %context.name, "cannot connect: {error}");
            },
        }
        if !stream::wait(context, delay, &mut reporter) {
            return;
        }
        delay = (delay * 2).min(SLOWEST);
    }
}

/// `path` with a leading `~/` as the user's home.
fn expanded(path: &str) -> std::path::PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => Path::new(&home).join(rest),
        _ => path.into(),
    }
}

/// Carries one connection; whether the provider should stop.
fn serve(context: &Context, stream: UnixStream, reporter: &mut Reporter) -> bool {
    let Ok(reader) = stream.try_clone() else {
        return false;
    };
    let Ok(writer) = stream.try_clone() else {
        return false;
    };
    let outbox = std::sync::Arc::new(Outbox::new(&context.manifest));
    outbox.push(&Message {
        from: String::new(),
        data: Data::Map(BTreeMap::from([("connected".to_owned(), Data::Bool(true))])),
    });
    let (ended, end) = mpsc::channel();
    let publisher = Publisher::of(context);
    let stop = context.stop.clone();
    thread::spawn(move || {
        let _ = ended.send(stream::publish_lines(reader, &publisher, &stop));
    });
    let writing = outbox.clone();
    thread::spawn(move || stream::write_lines(writer, &writing));
    reporter.connected(true);
    let pumped = stream::pump(context, &outbox, &end, reporter, || false);
    // Wakes the reader and a writer stuck on a full socket.
    let _ = stream.shutdown(Shutdown::Both);
    outbox.close();
    reporter.carry(outbox.load().1);
    reporter.connected(false);
    match pumped {
        Pumped::Stopped => true,
        Pumped::Ended(Ended::Closed) => false,
        Pumped::Ended(Ended::Problem(problem)) => {
            tracing::warn!(provider = %context.name, "dropping the link: {problem}");
            false
        },
    }
}
