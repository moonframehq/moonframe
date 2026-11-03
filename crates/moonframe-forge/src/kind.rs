//! What sets GitHub and GitLab apart: where their APIs and OAuth endpoints
//! are, what a sign-in asks for, which CLI and environment variables may
//! hold a token, and what each calls itself.

/// A forge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    // keep-sorted start
    GitHub,
    GitLab,
    // keep-sorted end
}

impl Kind {
    /// Its provider's name, and the `kind` `app.git.remote` gives its
    /// remotes.
    pub fn name(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::GitLab => "gitlab",
        }
    }

    /// What people call it.
    pub fn title(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::GitLab => "GitLab",
        }
    }

    /// The host signed in to when the repository names none.
    pub fn default_host(self) -> &'static str {
        match self {
            Self::GitHub => "github.com",
            Self::GitLab => "gitlab.com",
        }
    }

    /// Whether a remote on `host` belongs to this forge: github.com, or
    /// gitlab.com, `$GITLAB_HOST`, or a host the settings name. Never a
    /// host that merely has `gitlab` in its name: anyone can register
    /// one, and a sign-in would be sent there.
    pub fn serves(self, host: &str, extra: &[String]) -> bool {
        match self {
            Self::GitHub => {
                host.eq_ignore_ascii_case("github.com")
                    || host.eq_ignore_ascii_case("www.github.com")
            },
            Self::GitLab => {
                same_host(host, "gitlab.com")
                    || env_host().is_some_and(|known| same_host(host, &known))
                    || extra.iter().any(|known| same_host(host, known))
            },
        }
    }

    /// Whether the environment's token (`GH_TOKEN`, `GITLAB_TOKEN`) is
    /// meant for `host`: `$GITLAB_HOST` overrides GitLab's default host.
    pub fn env_serves(self, host: &str) -> bool {
        self.env_serves_host(host, env_host().as_deref())
    }

    fn env_serves_host(self, host: &str, configured: Option<&str>) -> bool {
        let expected = match self {
            Self::GitHub => self.default_host(),
            Self::GitLab => configured.unwrap_or(self.default_host()),
        };
        same_host(host, expected)
    }

    /// Where a device sign-in starts.
    pub fn device_url(self, host: &str) -> String {
        match self {
            Self::GitHub => "https://github.com/login/device/code".to_owned(),
            Self::GitLab => format!("https://{host}/oauth/authorize_device"),
        }
    }

    /// Where a device sign-in, or a refresh, gets its token.
    pub fn token_url(self, host: &str) -> String {
        match self {
            Self::GitHub => "https://github.com/login/oauth/access_token".to_owned(),
            Self::GitLab => format!("https://{host}/oauth/token"),
        }
    }

    /// What a sign-in asks to read: private repositories, pull requests,
    /// checks and issues, and who the user is. Nothing it asks for writes.
    pub fn scopes(self) -> &'static str {
        match self {
            // OAuth apps have no read-only scope for private repositories.
            Self::GitHub => "repo read:org",
            Self::GitLab => "read_api read_user",
        }
    }

    /// Environment variables holding a token, first match wins.
    pub fn env(self) -> &'static [&'static str] {
        match self {
            Self::GitHub => &["GH_TOKEN", "GITHUB_TOKEN"],
            Self::GitLab => &["GITLAB_TOKEN"],
        }
    }

    /// The environment variable that overrides the settings' client id.
    pub fn client_env(self) -> &'static str {
        match self {
            Self::GitHub => "MOONFRAME_GITHUB_CLIENT_ID",
            Self::GitLab => "MOONFRAME_GITLAB_CLIENT_ID",
        }
    }

    /// The CLI whose sign-in may be used, and the arguments that print its
    /// token for `host`.
    pub fn cli(self, host: &str) -> (&'static str, Vec<String>) {
        match self {
            Self::GitHub => ("gh", vec![
                "auth".into(),
                "token".into(),
                "--hostname".into(),
                host.into(),
            ]),
            Self::GitLab => ("glab", vec![
                "config".into(),
                "get".into(),
                "token".into(),
                "--host".into(),
                host.into(),
            ]),
        }
    }
}

/// `$GITLAB_HOST`'s host, lowercased, without a scheme or path.
fn env_host() -> Option<String> {
    let value = std::env::var("GITLAB_HOST").ok()?;
    let value = value.trim();
    let value = value.split_once("://").map_or(value, |(_, rest)| rest);
    let host = value.split('/').next()?.to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// Whether `found` (a remote's `host[:port]`) is `known`. The port is part
/// of the host: the same name on another port can be another server, so a
/// self-hosted instance on a port is listed with it (`git.example.com:8443`)
/// and nothing else receives its sign-in.
pub fn same_host(found: &str, known: &str) -> bool {
    found.eq_ignore_ascii_case(known)
}

#[cfg(test)]
mod tests {
    use super::Kind;

    #[test]
    fn environment_tokens_have_exactly_one_host() {
        let gitlab = Kind::GitLab;
        assert!(gitlab.env_serves_host("gitlab.com", None));
        assert!(!gitlab.env_serves_host("gitlab.com", Some("git.corp.example")));
        assert!(gitlab.env_serves_host("GIT.corp.example", Some("git.corp.example")));
        assert!(!gitlab.env_serves_host("other.example", Some("git.corp.example")));
        assert!(Kind::GitHub.env_serves_host("github.com", Some("git.corp.example")));
    }
}
