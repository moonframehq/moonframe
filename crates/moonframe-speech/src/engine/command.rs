//! A speech-to-text program the user installed: whisper.cpp, parakeet, a
//! script of their own. It is run without a shell, with `{file}` in its
//! arguments replaced by the take's path, and what it prints is the text.

use {
    super::{Cancel, expand},
    moonframe_providers::program,
    serde_json::Value,
    std::{
        io::Read,
        path::{Path, PathBuf},
        process::{Command as Process, Stdio},
        thread,
        time::{Duration, Instant},
    },
};

/// How often a running program is looked at.
const POLL: Duration = Duration::from_millis(50);
const TIMEOUT: Duration = Duration::from_secs(120);
/// How much of a failing program's stderr reaches the user.
const MAX_ERROR: usize = 400;

#[derive(Clone, Debug)]
pub(super) struct Command {
    program: String,
    arguments: Vec<String>,
    timeout: Duration,
}

impl Command {
    pub(super) fn read(value: &Value) -> Result<Self, String> {
        let mut words = value
            .get("command")
            .and_then(Value::as_array)
            .ok_or("needs a `command` list")?
            .iter()
            .map(|word| word.as_str().map(expand).ok_or("`command` holds only text"))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter();
        let program = words.next().ok_or("`command` is empty")?;
        let arguments: Vec<String> = words.collect();
        if !arguments.iter().any(|word| word.contains("{file}")) {
            return Err("`command` must pass the recording as `{file}`".into());
        }
        let timeout = value
            .get("timeout")
            .and_then(Value::as_u64)
            .map_or(TIMEOUT, Duration::from_secs);
        Ok(Self {
            program,
            arguments,
            timeout,
        })
    }

    fn resolve(&self) -> Option<PathBuf> {
        let path = Path::new(&self.program);
        if path.is_absolute() {
            return path.is_file().then(|| path.to_owned());
        }
        program::find(&self.program, &program::default_search_path())
    }

    pub(super) fn check(&self) -> Option<String> {
        if self.resolve().is_none() {
            return Some(format!("`{}` isn't installed", self.program));
        }
        // A model named by path that isn't there fails every take.
        let home = std::env::var("HOME").unwrap_or_default();
        self.arguments
            .iter()
            .find(|word| !home.is_empty() && word.starts_with(&home) && !Path::new(word).exists())
            .map(|word| format!("`{word}` is missing"))
    }

    pub(super) fn transcribe(&self, file: &Path, cancel: &Cancel) -> Result<String, String> {
        let program = self
            .resolve()
            .ok_or_else(|| format!("`{}` isn't installed", self.program))?;
        let path = file.to_string_lossy();
        let mut child = Process::new(program)
            .args(
                self.arguments
                    .iter()
                    .map(|word| word.replace("{file}", &path)),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Could not run `{}`: {error}", self.program))?;
        // Read while it runs, or a full pipe would stall it.
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if cancel.is_set() => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Cancelled".into());
                },
                Ok(None) if started.elapsed() >= self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("`{}` took too long", self.program));
                },
                Ok(None) => thread::sleep(POLL),
                Err(error) => return Err(format!("Lost `{}`: {error}", self.program)),
            }
        };
        let text = stdout.join().unwrap_or_default();
        if status.success() {
            return Ok(text);
        }
        let error = stderr.join().unwrap_or_default();
        let error = error.trim();
        let tail = error
            .char_indices()
            .rev()
            .nth(MAX_ERROR)
            .map_or(error, |(start, _)| &error[start..]);
        Err(format!("`{}` failed: {status}. {tail}", self.program)
            .trim()
            .to_owned())
    }
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}
