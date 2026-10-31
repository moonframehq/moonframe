//! What runs the app's terminals, chosen by what this machine has:
//!
//! 1. **herdr-core** (`hc`), when installed: it keeps terminals running
//!    and streams their output; started if it isn't running.
//! 2. **tmux**, when installed: a session per terminal on the app's own
//!    server.
//! 3. **pty**: the login shell on a local pseudo-terminal, always there;
//!    it ends with the app.
//!
//! The first two outlive the app, and a terminal attaches to the same
//! program again on the next launch. Each is a
//! [`moonframe_terminal::Backend`], so the app never depends on one.

#[cfg(feature = "herdr-core")]
mod herdr_core;
mod program;
mod tmux;

#[cfg(feature = "herdr-core")]
pub use herdr_core::HerdrCore;
pub use tmux::Tmux;

use {
    moonframe_terminal::{Backend, Open, Opened, Pty},
    std::sync::{Arc, OnceLock},
};

/// Which backend a terminal asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Preference {
    /// The best one this machine has.
    #[default]
    Auto,
    HerdrCore,
    Tmux,
    /// A local shell that ends with the app.
    Pty,
}

/// The backends this machine has, found on first use.
#[derive(Default)]
pub struct Backends {
    herdr_core: OnceLock<Option<Arc<dyn Backend>>>,
    tmux: OnceLock<Option<Arc<dyn Backend>>>,
}

impl Backends {
    pub fn new() -> Self {
        Self::default()
    }

    /// herdr-core, when it is installed and starts. Blocks on first use.
    pub fn herdr_core(&self) -> Option<Arc<dyn Backend>> {
        self.herdr_core
            .get_or_init(|| {
                #[cfg(feature = "herdr-core")]
                {
                    HerdrCore::connect().map(|core| Arc::new(core) as Arc<dyn Backend>)
                }
                #[cfg(not(feature = "herdr-core"))]
                None
            })
            .clone()
    }

    /// tmux, when it is installed. Blocks on first use.
    pub fn tmux(&self) -> Option<Arc<dyn Backend>> {
        self.tmux
            .get_or_init(|| Tmux::find().map(|tmux| Arc::new(tmux) as Arc<dyn Backend>))
            .clone()
    }

    /// The backends to try for `preference`, best first; the local shell is
    /// always last, so a terminal opens whatever this machine lacks.
    pub fn candidates(&self, preference: Preference) -> Vec<Arc<dyn Backend>> {
        let mut candidates = match preference {
            Preference::Pty => Vec::new(),
            Preference::Tmux => [self.tmux(), self.herdr_core()]
                .into_iter()
                .flatten()
                .collect(),
            Preference::HerdrCore | Preference::Auto => [self.herdr_core(), self.tmux()]
                .into_iter()
                .flatten()
                .collect(),
        };
        candidates.push(Arc::new(Pty));
        candidates
    }

    /// Opens `open` on the best backend that manages it, and says which.
    pub fn open(
        &self,
        preference: Preference,
        open: &Open,
    ) -> Result<(Opened, &'static str), String> {
        let mut problems = Vec::new();
        for backend in self.candidates(preference) {
            match backend.open(open) {
                Ok(opened) => {
                    if !problems.is_empty() {
                        tracing::warn!("terminal {}: {}", open.key, problems.join("; "));
                    }
                    return Ok((opened, backend.name()));
                },
                Err(problem) => problems.push(format!("{}: {problem}", backend.name())),
            }
        }
        Err(problems.join("; "))
    }
}

/// Whether herdr-core is installed here, running or not.
pub fn herdr_core_installed() -> bool {
    #[cfg(feature = "herdr-core")]
    {
        herdr_core::installed().is_some()
    }
    #[cfg(not(feature = "herdr-core"))]
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
