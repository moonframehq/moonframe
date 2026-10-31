use std::fmt::Display;

/// Why a module did not load, update, or render. Shown to whoever writes the
/// module, human or agent, so it says where the problem is.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Error(String);

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    /// A problem at a path in a value, such as `column[2].button`.
    pub(crate) fn at(at: &str, problem: impl Display) -> Self {
        if at.is_empty() {
            Self(problem.to_string())
        } else {
            Self(format!("{at}: {problem}"))
        }
    }

    pub(crate) fn context(self, context: impl Display) -> Self {
        Self(format!("{context}: {}", self.0))
    }
}

impl From<mlua::Error> for Error {
    fn from(error: mlua::Error) -> Self {
        Self(error.to_string())
    }
}
