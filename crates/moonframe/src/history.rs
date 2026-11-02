//! Undo for the window's own folder (the user's, or the profile's): a
//! snapshot each time its files change, and `moonframe history` to list,
//! undo, redo, or restore them from a terminal, as an agent would.
//! moonframe-history keeps them; this finds where.

use {
    crate::paths::{self, Folders},
    moonframe_history::{History, Snapshot},
    std::{
        process::ExitCode,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    },
};

pub(crate) const USAGE: &str = "\
moonframe history: undo changes to your folder (or a profile's)

Every change the window reloads for is kept as a snapshot, whoever made
it: you, an editor, or an agent. Undo writes the files back and the
window reloads with them.

Usage: moonframe [--profile NAME] history [COMMAND]

Commands:
  list [N]      the last N snapshots (default 20), newest first; * marks
                the one the folder is at
  undo          go back one snapshot
  redo          go forward one again, after an undo
  restore ID    go to the snapshot ID (as `list` shows it)
  show ID       what the snapshot ID changed, as a patch
  snapshot      take one now, if anything changed

settings.toml, databases, installed packages and other profiles are left
out: undo never changes them.";

/// The history of the window's own folder, if it has one and git runs.
pub(crate) fn open(folders: &Folders) -> Option<Arc<History>> {
    let own = folders.own()?;
    let state = paths::state(folders.profile.as_deref());
    let store = moonframe_history::store_in(state.parent()?, folders.profile.as_deref());
    History::open(&own, &store)
        .inspect_err(|error| tracing::warn!("no undo for {}: {error}", own.display()))
        .ok()
        .map(Arc::new)
}

/// `moonframe history <words>`.
pub(crate) fn run(folders: &Folders, words: &[String]) -> ExitCode {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    if matches!(words.first(), Some(&("help" | "-h" | "--help"))) {
        println!("{}", crate::cli::branded(USAGE));
        return ExitCode::SUCCESS;
    }
    let Some(history) = open(folders) else {
        eprintln!(
            "{} history: no folder to keep, or git cannot run; drop --no-user",
            paths::name()
        );
        return ExitCode::from(2);
    };
    let done = match words.as_slice() {
        [] | ["list"] => list(&history, 20),
        ["list", count] => match count.parse() {
            Ok(count) => list(&history, count),
            Err(_) => return usage(&format!("not a count: {count}")),
        },
        ["undo"] => history.undo().map(|undone| match undone {
            Some(undone) => println!("Undid {} ({})", undone.label, undone.short()),
            None => println!("Nothing to undo"),
        }),
        ["redo"] => history.redo().map(|redone| match redone {
            Some(redone) => println!("Redid {} ({})", redone.label, redone.short()),
            None => println!("Nothing to redo"),
        }),
        ["restore", id] => history
            .restore(id)
            .map(|at| println!("Restored {} ({})", at.label, at.short())),
        ["show", id] => history.show(id).map(|patch| println!("{patch}")),
        ["snapshot"] => history.capture("by hand").map(|taken| match taken {
            Some(taken) => println!("Took {} ({})", taken.label, taken.short()),
            None => println!("Nothing changed"),
        }),
        [word, ..] => return usage(&format!("unknown history command: {word}")),
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{} history: {error}", paths::name());
            ExitCode::FAILURE
        },
    }
}

fn usage(problem: &str) -> ExitCode {
    eprintln!("{problem}\n\n{}", crate::cli::branded(USAGE));
    ExitCode::from(2)
}

fn list(history: &History, count: usize) -> moonframe_history::Result<()> {
    let listed = history.list(count)?;
    if listed.is_empty() {
        println!("No snapshots yet of {}", history.folder().display());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs().cast_signed());
    for snapshot in &listed {
        println!("{}", line(snapshot, now));
    }
    Ok(())
}

/// `* 1a2b3c4d  5m ago  changed  mods/clock/init.luau`.
fn line(snapshot: &Snapshot, now: i64) -> String {
    format!(
        "{} {}  {:>8}  {:<14}  {}",
        if snapshot.current {
            '*'
        } else {
            ' '
        },
        snapshot.short(),
        ago(now - snapshot.created),
        snapshot.reason,
        snapshot.label
    )
}

/// "just now", "5m ago", "3h ago", "2d ago".
fn ago(seconds: i64) -> String {
    match seconds.max(0) {
        0..60 => "just now".to_owned(),
        seconds @ 60..3600 => format!("{}m ago", seconds / 60),
        seconds @ 3600..86_400 => format!("{}h ago", seconds / 3600),
        seconds => format!("{}d ago", seconds / 86_400),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ages_read_short() {
        assert_eq!(super::ago(-5), "just now");
        assert_eq!(super::ago(59), "just now");
        assert_eq!(super::ago(300), "5m ago");
        assert_eq!(super::ago(7200), "2h ago");
        assert_eq!(super::ago(200_000), "2d ago");
    }
}
