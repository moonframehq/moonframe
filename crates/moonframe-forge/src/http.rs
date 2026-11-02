//! Bounded HTTP against a forge: every answer has a deadline and a size
//! cap, tokens travel only in the `Authorization` header, and what goes
//! wrong is told apart (signed out, rate limited, refused) without ever
//! echoing a body that might hold a secret.

use {
    secrecy::{ExposeSecret, SecretString},
    serde_json::Value,
    std::{fmt, time::Duration},
};

/// The largest answer read, in bytes.
const LIMIT: u64 = 4 * 1024 * 1024;
/// The largest picture downloaded (an avatar).
pub(crate) const PICTURE_LIMIT: u64 = 512 * 1024;
/// How much of an error's `message` is kept.
const MESSAGE: usize = 200;

/// Why a request gave nothing usable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    /// 401: the token is missing, expired or revoked.
    Unauthorized,
    /// 403 or 429 with the rate limit spent.
    RateLimited,
    /// Any other status, with the forge's own short message when it gave
    /// one.
    Status(u16, String),
    /// No answer: offline, DNS, TLS, a timeout.
    Network(String),
    /// An answer that was too large or not the JSON expected.
    Body(String),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized => write!(f, "the sign-in was refused; sign in again"),
            Self::RateLimited => write!(f, "rate limited; trying again later"),
            Self::Status(status, message) if message.is_empty() => {
                write!(f, "the server answered {status}")
            },
            Self::Status(status, message) => write!(f, "{message} ({status})"),
            Self::Network(problem) => write!(f, "cannot reach the server: {problem}"),
            Self::Body(problem) => write!(f, "unexpected answer: {problem}"),
        }
    }
}

/// A client whose every answer arrives within a few seconds, which never
/// follows a redirect (a token must not travel to another host) and
/// reads every status itself.
pub(crate) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(30)))
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent("Moonframe (+https://moonframe.dev)")
        .build()
        .new_agent()
}

fn bearer(token: &SecretString) -> String {
    format!("Bearer {}", token.expose_secret())
}

/// GET `url` as JSON, signed with `token`.
pub(crate) fn get(agent: &ureq::Agent, url: &str, token: &SecretString) -> Result<Value, Failure> {
    let response = agent
        .get(url)
        .header("Accept", "application/json")
        .header("Authorization", bearer(token))
        .call();
    answer(response)
}

/// POST `body` as JSON to `url`, signed with `token`.
pub(crate) fn post(
    agent: &ureq::Agent,
    url: &str,
    token: &SecretString,
    body: &Value,
) -> Result<Value, Failure> {
    let response = agent
        .post(url)
        .header("Accept", "application/json")
        .header("Authorization", bearer(token))
        .header("Content-Type", "application/json")
        .send(body.to_string());
    answer(response)
}

/// POST a form to an OAuth endpoint, unsigned. The answer's JSON comes
/// back whatever the status: OAuth says what went wrong in its `error`
/// field, with 200 (GitHub) or 400 (GitLab).
pub(crate) fn form(
    agent: &ureq::Agent,
    url: &str,
    fields: &[(&str, &str)],
) -> Result<Value, Failure> {
    let mut response = agent
        .post(url)
        .header("Accept", "application/json")
        .send_form(fields.iter().copied())
        .map_err(|error| Failure::Network(error.to_string()))?;
    let status = response.status().as_u16();
    let value = read(&mut response)?;
    if status == 200 || (400..500).contains(&status) && value.get("error").is_some() {
        Ok(value)
    } else {
        Err(Failure::Status(status, message(&value)))
    }
}

/// A picture at `url`, unsigned, of at most [`PICTURE_LIMIT`] bytes.
pub(crate) fn picture(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, Failure> {
    let mut response = agent
        .get(url)
        .call()
        .map_err(|error| Failure::Network(error.to_string()))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(Failure::Status(status, String::new()));
    }
    response
        .body_mut()
        .with_config()
        .limit(PICTURE_LIMIT)
        .read_to_vec()
        .map_err(|error| Failure::Body(error.to_string()))
}

fn answer(
    response: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<Value, Failure> {
    let mut response = response.map_err(|error| Failure::Network(error.to_string()))?;
    let status = response.status().as_u16();
    let spent = response
        .headers()
        .get("x-ratelimit-remaining")
        .or_else(|| response.headers().get("ratelimit-remaining"))
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim() == "0");
    match status {
        200..=299 => read(&mut response),
        401 => Err(Failure::Unauthorized),
        429 => Err(Failure::RateLimited),
        403 if spent => Err(Failure::RateLimited),
        _ => {
            let value = read(&mut response).unwrap_or(Value::Null);
            Err(Failure::Status(status, message(&value)))
        },
    }
}

fn read(response: &mut ureq::http::Response<ureq::Body>) -> Result<Value, Failure> {
    let text = response
        .body_mut()
        .with_config()
        .limit(LIMIT)
        .read_to_string()
        .map_err(|error| Failure::Body(error.to_string()))?;
    // The body may hold a token, so only the parser's complaint is kept.
    serde_json::from_str(&text).map_err(|error| Failure::Body(error.to_string()))
}

/// The forge's own short explanation in an error answer: `message`
/// (GitHub, GitLab) or `error_description` (OAuth), cut short.
pub(crate) fn message(value: &Value) -> String {
    let text = value
        .get("message")
        .or_else(|| value.get("error_description"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    text.chars()
        .filter(|c| !c.is_control())
        .take(MESSAGE)
        .collect()
}
