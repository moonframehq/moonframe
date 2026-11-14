//! Pages rendered to PDF files for mods (`app:pdf`): the page loads in an
//! offscreen web view of its own, then prints to paper-sized pages in a
//! file, as a browser's Save as PDF does. On macOS that is WebKit, part of
//! the `browser` feature; there is none elsewhere yet.

#[cfg(all(feature = "browser", target_os = "macos"))]
mod macos;

use {
    futures::channel::oneshot,
    moonframe_core::Paper,
    std::{path::PathBuf, time::Duration},
};

/// How long a page has to load and print before it is given up.
pub(crate) const TIMEOUT: Duration = Duration::from_secs(30);

/// A page to make into a PDF, its file found.
#[cfg_attr(not(all(feature = "browser", target_os = "macos")), allow(dead_code))]
pub(crate) enum Source {
    /// HTML shown with no script and nothing loaded but `data:`.
    Html(String),
    /// An `.html` file, read as `Html` is.
    HtmlFile(PathBuf),
    /// A web page, with its script, in a session of its own.
    Url(String),
}

/// A PDF being made: `done` says whether the file was written.
pub(crate) struct Job {
    pub(crate) id: u64,
    pub(crate) done: oneshot::Receiver<Result<(), String>>,
}

/// Starts making `source` into a PDF at `out`, on `paper` (else the
/// system's). Call [`finish`] with its id once `done` answers or the wait
/// is given up, to let its view go; one given up mid-print is kept until
/// its print is over, which answers `done`, and needs a second call then.
#[cfg(all(feature = "browser", target_os = "macos"))]
pub(crate) fn render(source: Source, paper: Option<Paper>, out: PathBuf) -> Result<Job, String> {
    macos::render(source, paper, out)
}

#[cfg(all(feature = "browser", target_os = "macos"))]
pub(crate) fn finish(id: u64) {
    macos::finish(id);
}

#[cfg(not(all(feature = "browser", target_os = "macos")))]
pub(crate) fn render(_: Source, _: Option<Paper>, _: PathBuf) -> Result<Job, String> {
    Err(if cfg!(feature = "browser") {
        "PDFs are made on macOS only, so far".to_owned()
    } else {
        "this app was built without the `browser` feature".to_owned()
    })
}

#[cfg(not(all(feature = "browser", target_os = "macos")))]
pub(crate) fn finish(_: u64) {}
