//! OAuth's device authorization grant (RFC 8628), which both forges speak:
//! the app asks for a code, the user enters it on the forge's site, and the
//! app polls until the forge hands it a token. It needs the public client
//! id of an OAuth app (GitHub) or application (GitLab) with the device
//! flow enabled, which each app built on Moonframe registers as its own.

use {
    crate::{
        credentials::{Credential, Source, now},
        http::{self, Failure},
        kind::Kind,
    },
    secrecy::{ExposeSecret, SecretString},
    serde_json::Value,
    std::time::Duration,
};

const GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// The code the user enters, and where.
pub struct Code {
    pub device: SecretString,
    pub user: String,
    pub url: String,
    /// The same page with the code filled in, when the forge offers one.
    pub complete_url: Option<String>,
    /// Unix seconds.
    pub expires: u64,
    pub interval: Duration,
}

/// Asks `host` for a device code.
pub fn start(agent: &ureq::Agent, kind: Kind, host: &str, client: &str) -> Result<Code, Failure> {
    let answer = http::form(agent, &kind.device_url(host), &[
        ("client_id", client),
        ("scope", kind.scopes()),
    ])?;
    if let Some(error) = answer.get("error").and_then(Value::as_str) {
        return Err(Failure::Status(400, start_error(kind, error, &answer)));
    }
    let text = |key: &str| answer.get(key).and_then(Value::as_str).map(str::to_owned);
    let device = text("device_code").filter(|code| crate::credentials::plausible(code));
    let user = text("user_code").filter(|code| {
        (1..=32).contains(&code.len())
            && code
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    });
    // The page the user is sent to must be on the forge's own host.
    let url = text("verification_uri").filter(|url| on_host(url, kind, host));
    let (Some(device), Some(user), Some(url)) = (device, user, url) else {
        return Err(Failure::Body("the device code answer is incomplete".into()));
    };
    let expires_in = answer
        .get("expires_in")
        .and_then(Value::as_u64)
        .unwrap_or(900)
        .clamp(60, 1800);
    let interval = answer
        .get("interval")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .clamp(1, 60);
    Ok(Code {
        device: SecretString::from(device),
        user,
        complete_url: text("verification_uri_complete").filter(|url| on_host(url, kind, host)),
        url,
        expires: now() + expires_in,
        interval: Duration::from_secs(interval),
    })
}

fn on_host(url: &str, kind: Kind, host: &str) -> bool {
    let expected = match kind {
        Kind::GitHub => "github.com",
        Kind::GitLab => host,
    };
    url.strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|found| found.eq_ignore_ascii_case(expected))
}

/// What one poll found.
pub enum Poll {
    // keep-sorted start
    Done(Credential),
    /// Denied, expired, or refused; the words say which.
    Failed(String),
    /// The user has not entered the code yet; poll again after this long.
    Wait(Duration),
    // keep-sorted end
}

/// Asks once whether the user entered `code`.
pub fn poll(agent: &ureq::Agent, kind: Kind, host: &str, client: &str, code: &mut Code) -> Poll {
    let answer = match http::form(agent, &kind.token_url(host), &[
        ("client_id", client),
        ("device_code", code.device.expose_secret()),
        ("grant_type", GRANT),
    ]) {
        Ok(answer) => answer,
        // A blip: keep waiting, the code is still good.
        Err(Failure::Network(_)) => return Poll::Wait(code.interval),
        Err(failure) => return Poll::Failed(failure.to_string()),
    };
    polled(&answer, &mut code.interval)
}

fn polled(answer: &Value, interval: &mut Duration) -> Poll {
    match answer.get("error").and_then(Value::as_str) {
        Some("authorization_pending") => Poll::Wait(*interval),
        Some("slow_down") => {
            *interval += Duration::from_secs(5);
            Poll::Wait(*interval)
        },
        Some("expired_token") => Poll::Failed("the code expired; start again".into()),
        Some("access_denied") => Poll::Failed("the sign-in was declined".into()),
        Some(_) => Poll::Failed(oauth_error(answer)),
        None => match token(answer, Source::Saved) {
            Some(credential) => Poll::Done(credential),
            None => Poll::Failed("the forge sent no token".into()),
        },
    }
}

/// Renews an expiring OAuth token with its refresh token.
pub fn refresh(
    agent: &ureq::Agent,
    kind: Kind,
    host: &str,
    client: &str,
    credential: &Credential,
) -> Result<Credential, Failure> {
    let Some(refresh) = &credential.refresh else {
        return Err(Failure::Unauthorized);
    };
    let answer = http::form(agent, &kind.token_url(host), &[
        ("client_id", client),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh.expose_secret()),
    ])?;
    refreshed(&answer, credential.source)
}

fn refreshed(answer: &Value, source: Source) -> Result<Credential, Failure> {
    match answer.get("error").and_then(Value::as_str) {
        Some("invalid_grant" | "invalid_token" | "access_denied") => Err(Failure::Unauthorized),
        Some("slow_down") => Err(Failure::RateLimited),
        Some(_) => Err(Failure::Status(400, oauth_error(answer))),
        None => token(answer, source)
            .ok_or_else(|| Failure::Body("the renewal answer has no token".into())),
    }
}

/// The token in an OAuth answer.
fn token(answer: &Value, source: Source) -> Option<Credential> {
    let text = |key: &str| {
        answer
            .get(key)
            .and_then(Value::as_str)
            .filter(|token| crate::credentials::plausible(token))
            .map(|token| SecretString::from(token.to_owned()))
    };
    Some(Credential {
        token: text("access_token")?,
        refresh: text("refresh_token"),
        expires: answer
            .get("expires_in")
            .and_then(Value::as_u64)
            .map(|seconds| now() + seconds),
        source,
    })
}

/// Why the forge would not start a device sign-in, as the step that fixes
/// it: the OAuth application's settings, which the app's author controls.
pub(crate) fn start_error(kind: Kind, error: &str, answer: &Value) -> String {
    match (kind, error) {
        (_, "invalid_client") => format!(
            "{} does not know this app's OAuth client, or it is confidential: \
             check its client id, and untick Confidential",
            kind.title()
        ),
        (Kind::GitLab, "access_denied") => "GitLab refused device sign-in for this application: \
             tick \"Device authorization grant\" in its settings"
            .to_owned(),
        _ => oauth_error(answer),
    }
}

fn oauth_error(answer: &Value) -> String {
    let words = http::message(answer);
    if words.is_empty() {
        let code: String = answer
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("refused")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .take(64)
            .collect();
        format!("the forge refused the sign-in ({code})")
    } else {
        words
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn only_a_definitive_oauth_refusal_signs_out() {
        for error in ["invalid_grant", "invalid_token", "access_denied"] {
            assert!(matches!(
                refreshed(&json!({ "error": error }), Source::Saved),
                Err(Failure::Unauthorized)
            ));
        }
        for error in ["temporarily_unavailable", "server_error", "invalid_client"] {
            assert!(matches!(
                refreshed(&json!({ "error": error }), Source::Saved),
                Err(Failure::Status(_, _))
            ));
        }
        assert!(matches!(
            refreshed(&json!({ "error": "slow_down" }), Source::Saved),
            Err(Failure::RateLimited)
        ));
        assert!(matches!(
            refreshed(&json!({}), Source::Saved),
            Err(Failure::Body(_))
        ));
    }

    #[test]
    fn slowdowns_accumulate_and_pending_keeps_the_increased_interval() {
        let mut interval = Duration::from_secs(5);
        for (error, seconds) in [
            ("slow_down", 10),
            ("authorization_pending", 10),
            ("slow_down", 15),
            ("slow_down", 20),
        ] {
            assert!(matches!(polled(&json!({ "error": error }), &mut interval),
                Poll::Wait(wait) if wait == Duration::from_secs(seconds)));
        }
    }
}
