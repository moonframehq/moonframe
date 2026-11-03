//! Prints what the git provider publishes for the repository `LAYOUTS_REPO`
//! (or the one two folders above the first argument), once, as JSON: for
//! comparing it with another provider's output by hand.

use {
    moonframe_core::Data,
    moonframe_providers::{Event, Handle, Native as _},
    std::{path::PathBuf, sync::Arc},
};

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let host = Handle::new(
        "git",
        &dir,
        Data::Nil,
        Arc::new(|event| {
            if let Event::Published { data, .. } = event {
                println!("{}", moonframe_providers::to_json(&data));
            }
        }),
    );
    let mut git = moonframe_git::Git::default();
    if let Err(problem) = git.start(&host) {
        eprintln!("{problem}");
    }
}
