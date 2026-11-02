//! The `github` and `gitlab` native providers: the app repository's pull
//! (merge) requests, their checks and line counts, issues, and the pull
//! request of every branch its worktrees have out, read over each forge's
//! own API, as `app.github` and `app.gitlab`.
//!
//! Signing in is the provider's, drawing it is the app's. The provider
//! publishes where the sign-in stands as `auth`, and takes these messages
//! from the mods in provider.toml's `[settings] accounts`, answering each
//! with `{ reply = id, action, ok, message | error }`:
//! - `{ sign_in = true }` starts a device sign-in: `auth.code` and
//!   `auth.url` are what the user enters, and where; the answer comes once
//!   they did (or it expired, or was declined). Needs the client id of an
//!   OAuth app with the device flow enabled (`client_id`).
//! - `{ token = "<token>" }` (or `{ token = true, value = "<token>" }`,
//!   as a `ui.input`'s `submit` sends it) checks a pasted token and saves
//!   it.
//! - `{ use_cli = true }` uses `gh`'s (or `glab`'s) own sign-in, read from
//!   it again each time.
//! - `{ cancel = true }` stops a device sign-in; `{ sign_out = true }`
//!   forgets the saved sign-in.
//!
//! Any other message, from any mod, reads the forge again. Tokens never
//! leave the provider: mods only ever see the login.

// keep-sorted start
mod credentials;
mod device;
mod github;
mod gitlab;
mod http;
mod kind;
mod provider;
mod repo;
mod shape;
mod work;
// keep-sorted end

use moonframe_providers::Native;

/// The `github` provider, for the app's registry.
pub fn github() -> Box<dyn Native> {
    Box::new(provider::Forge::new(kind::Kind::GitHub))
}

/// The `gitlab` provider, for the app's registry.
pub fn gitlab() -> Box<dyn Native> {
    Box::new(provider::Forge::new(kind::Kind::GitLab))
}

/// What a fetch asks the forge about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) host: String,
    /// `owner/repo`, or GitLab's `group/sub/repo`.
    pub(crate) path: String,
    pub(crate) branch: String,
    pub(crate) branches: Vec<String>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
