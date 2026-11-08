//! The native providers a Rune build has: the same crates the GPUI app links,
//! none of which touches GPUI. `demos` is left out, as it opens apps on the
//! GPUI binary.

use {
    moonframe_providers::{Native, Natives},
    std::sync::Arc,
};

pub(crate) fn natives() -> Natives {
    let built_in: &[(&str, fn() -> Box<dyn Native>)] = &[
        // keep-sorted start
        ("acp", moonframe_acp::native),
        ("chat", moonframe_chat::native),
        ("docs", moonframe_docs::native),
        ("git", moonframe_git::native),
        ("github", moonframe_forge::github),
        ("gitlab", moonframe_forge::gitlab),
        ("history", moonframe_transcripts::native),
        ("library", moonframe_documents::native),
        ("pi", moonframe_pi::pi),
        ("planner", moonframe_planner::native),
        ("procs", moonframe_procs::native),
        ("projects", moonframe_projects::native),
        ("repo", moonframe_pi::repo),
        ("shell", moonframe_shell::native),
        ("speech", moonframe_speech::native),
        ("usage", moonframe_usage::native),
        // keep-sorted end
    ];
    built_in
        .iter()
        .map(|&(name, make)| {
            let factory: Arc<dyn Fn() -> Box<dyn Native> + Send + Sync> = Arc::new(make);
            (name.to_owned(), factory)
        })
        .collect()
}
