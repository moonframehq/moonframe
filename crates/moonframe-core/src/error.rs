use std::fmt::Display;

/// Why a module did not load, update, or render. Shown to whoever writes the
/// module, human or agent, so it says where the problem is.
///
/// One raised while reading a view also knows, in its [`Detail`], which
/// element and key it is about, so it can be reported as a
/// [`ViewDiagnostic`](crate::ViewDiagnostic). Two errors are equal when
/// their messages are.
#[derive(Clone, Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    message: String,
    detail: Option<Box<Detail>>,
}

/// Where in a view an error is, as the reader saw it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Detail {
    /// The whole path: `column[1].row.padding`.
    pub(crate) at: String,
    /// The problem without the path.
    pub(crate) problem: String,
    /// The element's path (`column[1].row`) and kind (`row`).
    pub(crate) element: Option<(String, &'static str)>,
    /// The key named outright, where the path does not end in it: an
    /// unknown key's.
    pub(crate) key: Option<String>,
    /// The value it was given, short.
    pub(crate) value: Option<String>,
    /// The line of the module that built the element.
    pub(crate) line: Option<u32>,
    pub(crate) suggestion: Option<String>,
    pub(crate) level: crate::ViewLevel,
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.message == other.message
    }
}

impl Eq for Error {}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            detail: None,
        }
    }

    /// A problem at a path in a value, such as `column[2].button`.
    pub(crate) fn at(at: &str, problem: impl Display) -> Self {
        let problem = problem.to_string();
        if at.is_empty() {
            return Self::new(problem);
        }
        Self {
            message: format!("{at}: {problem}"),
            detail: Some(Box::new(Detail {
                at: at.to_owned(),
                problem,
                ..Detail::default()
            })),
        }
    }

    pub(crate) fn context(self, context: impl Display) -> Self {
        Self {
            message: format!("{context}: {}", self.message),
            detail: self.detail,
        }
    }

    /// Whether it marks its element broken, rather than noting a
    /// deprecated key or what a strict check refuses.
    pub(crate) fn is_broken(&self) -> bool {
        self.detail()
            .is_none_or(|detail| detail.level == crate::ViewLevel::Broken)
    }

    pub(crate) fn detail(&self) -> Option<&Detail> {
        self.detail.as_deref()
    }

    /// Changes its detail, when it has one.
    pub(crate) fn with(mut self, change: impl FnOnce(&mut Detail)) -> Self {
        if let Some(detail) = &mut self.detail {
            change(detail);
        }
        self
    }

    /// The message with its suggestion, for the warning sign's tooltip.
    pub(crate) fn explained(&self) -> String {
        match self
            .detail()
            .and_then(|detail| detail.suggestion.as_deref())
        {
            Some(suggestion) => format!("{}\n{suggestion}", self.message),
            None => self.message.clone(),
        }
    }
}

impl From<mlua::Error> for Error {
    fn from(error: mlua::Error) -> Self {
        Self::new(error.to_string())
    }
}
