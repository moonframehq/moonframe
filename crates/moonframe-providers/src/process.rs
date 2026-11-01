//! Process providers: a command run in the provider's folder that prints one
//! JSON value per line on stdout and reads messages as
//! `{"from": "<mod>", "message": ...}`
//! lines on stdin. Its stderr goes to the app's log.

use {
    crate::{Context, Kind, Message, json},
    std::{
        io::{BufRead, BufReader, Read, Write},
        process::{Child, Command, Stdio},
        sync::mpsc::{self, RecvTimeoutError},
        thread,
        time::Duration,
    },
};

/// How often the provider's thread looks for output while it waits.
const POLL: Duration = Duration::from_millis(100);

/// A line the process printed, or the end of its output.
enum Line {
    Text(String),
    TooLong,
    End,
}

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
    let mut child = Command::new(program)
        .args(arguments)
        .current_dir(&context.dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot start `{program}`: {error}"))?;
    let result = drive(context, &mut child);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn drive(context: &Context, child: &mut Child) -> Result<(), String> {
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let stderr = child.stderr.take().ok_or("no stderr")?;
    let name = context.name.clone();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            tracing::info!(provider = %name, "{}", moonframe_core::printable(&line));
        }
    });
    let (lines, output) = mpsc::channel();
    thread::spawn(move || read_lines(stdout, &lines));
    loop {
        match context.messages.recv_timeout(POLL) {
            Ok(message) => {
                let line = line(&message);
                // A process that stopped reading is noticed when its output ends.
                let _ = writeln!(stdin, "{line}");
            },
            Err(RecvTimeoutError::Timeout) => {},
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
        if context.stopped() {
            return Ok(());
        }
        while let Ok(line) = output.try_recv() {
            match line {
                Line::Text(text) if text.trim().is_empty() => {},
                Line::Text(text) => match json::data(&text) {
                    Ok(data) => context.publish(data),
                    Err(problem) => return Err(problem.to_string()),
                },
                Line::TooLong => {
                    return Err(format!(
                        "printed a line longer than {} bytes",
                        json::MAX_JSON
                    ));
                },
                Line::End => {
                    let status = child.wait().map_err(|error| error.to_string())?;
                    return Err(format!("exited ({status})"));
                },
            }
        }
    }
}

/// Reads lines no longer than [`json::MAX_JSON`], so a runaway process
/// cannot fill memory with one line.
fn read_lines(stdout: impl Read, lines: &mpsc::Sender<Line>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut line = Vec::new();
        let limit = json::MAX_JSON as u64 + 1;
        let line = match reader.by_ref().take(limit).read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => Line::End,
            Ok(_) if line.len() as u64 >= limit && line.last() != Some(&b'\n') => Line::TooLong,
            Ok(_) => Line::Text(String::from_utf8_lossy(&line).into_owned()),
        };
        let end = !matches!(line, Line::Text(_));
        if lines.send(line).is_err() || end {
            return;
        }
    }
}

/// The line a message is written as: compact JSON, `from` first, so a
/// script can match a whole line exactly (the `herdr` provider does).
fn line(message: &Message) -> String {
    serde_json::json!({
        "from": message.from,
        "message": json::json(&message.data),
    })
    .to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, moonframe_core::Data};

    #[test]
    fn a_message_is_one_compact_line_with_its_sender_first() {
        let message = Message {
            from: "herdr".into(),
            data: Data::from("install"),
        };
        assert_eq!(line(&message), r#"{"from":"herdr","message":"install"}"#);
    }
}
