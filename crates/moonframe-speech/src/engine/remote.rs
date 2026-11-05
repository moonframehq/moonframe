//! A speech-to-text service with OpenAI's `audio/transcriptions` request:
//! OpenAI, Groq, or a server of the user's own. The key stays here, read
//! from an environment variable named in the settings, never from a mod.

use {
    serde_json::Value,
    std::{path::Path, time::Duration},
    ureq::Agent,
};

const TIMEOUT: Duration = Duration::from_secs(60);
const BOUNDARY: &str = "moonframe-speech-7d1f3a9c";
/// The longest error from a service shown to the user.
const MAX_ERROR: usize = 300;

#[derive(Clone, Debug)]
pub(super) struct Remote {
    url: String,
    model: String,
    key_env: Option<String>,
    language: Option<String>,
}

impl Remote {
    pub(super) fn read(value: &Value) -> Result<Self, String> {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        let url = text("url").ok_or("needs a `url`")?;
        let url = url.trim_end_matches('/').to_owned();
        if !secure(&url) {
            return Err("`url` must be https, or http on this machine".into());
        }
        let key_env = text("key_env");
        if key_env.as_deref().is_some_and(|name| !variable(name)) {
            return Err("`key_env` must be a variable's name".into());
        }
        Ok(Self {
            url,
            model: text("model").ok_or("needs a `model`")?,
            key_env,
            language: text("language"),
        })
    }

    pub(super) fn check(&self) -> Option<String> {
        let name = self.key_env.as_deref()?;
        key(name)
            .is_none()
            .then(|| format!("Set {name} to use this engine"))
    }

    pub(super) fn transcribe(&self, file: &Path) -> Result<String, String> {
        let audio =
            std::fs::read(file).map_err(|error| format!("Could not read the take: {error}"))?;
        let agent: Agent = Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .http_status_as_error(false)
            .build()
            .into();
        let mut request = agent
            .post(format!("{}/audio/transcriptions", self.url))
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={BOUNDARY}"),
            );
        if let Some(name) = &self.key_env {
            let key = key(name).ok_or_else(|| format!("Set {name} to use this engine"))?;
            request = request.header("Authorization", format!("Bearer {key}"));
        }
        let mut response = request
            .send(&self.body(&audio)[..])
            .map_err(|error| format!("Could not reach the speech service: {error}"))?;
        let status = response.status();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|error| format!("Could not read the speech service: {error}"))?;
        let answer: Option<Value> = serde_json::from_str(&body).ok();
        if !status.is_success() {
            return Err(failure(status.as_u16(), answer.as_ref()));
        }
        answer
            .as_ref()
            .and_then(|answer| answer.get("text"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "The speech service sent no text".into())
    }

    fn body(&self, audio: &[u8]) -> Vec<u8> {
        let mut body = Vec::with_capacity(audio.len() + 512);
        let mut field = |name: &str, value: &str| {
            body.extend_from_slice(
                format!(
                    "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n\
                     {value}\r\n"
                )
                .as_bytes(),
            );
        };
        field("model", &self.model);
        field("response_format", "json");
        if let Some(language) = &self.language {
            field("language", language);
        }
        body.extend_from_slice(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; \
                 filename=\"speech.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(audio);
        body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
        body
    }
}

fn failure(status: u16, answer: Option<&Value>) -> String {
    let message = answer
        .and_then(|answer| answer.pointer("/error/message"))
        .and_then(Value::as_str)
        .map(|message| message.chars().take(MAX_ERROR).collect::<String>());
    match message {
        Some(message) => format!("The speech service refused ({status}): {message}"),
        None => format!("The speech service refused ({status})"),
    }
}

/// Voice and key go in the clear over plain http, so only to this machine.
fn secure(url: &str) -> bool {
    if url.starts_with("https://") {
        return true;
    }
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or_default();
    let host = host.rsplit_once(':').map_or(host, |(host, _)| host);
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

fn variable(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || letter == '_')
}

/// The variable's value. An app opened from the Dock doesn't have what the
/// user's shell profile exports, so the login shell is asked after this
/// process. `name` was checked by [`variable`], and goes as an argument.
fn key(name: &str) -> Option<String> {
    let found = std::env::var(name).ok().or_else(|| login_shell(name))?;
    let found = found.trim().to_owned();
    (!found.is_empty()).then_some(found)
}

#[cfg(unix)]
fn login_shell(name: &str) -> Option<String> {
    let shell = std::env::var_os("SHELL")?;
    let output = std::process::Command::new(shell)
        .args(["-l", "-c", "printenv \"$0\"", name])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    // A profile may print before the command, so only the last line counts.
    output
        .status
        .success()
        .then(|| text.lines().last().map(str::to_owned))
        .flatten()
}

#[cfg(not(unix))]
fn login_shell(_: &str) -> Option<String> {
    None
}
