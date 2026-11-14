//! Undo for the folders a change to the app lands in: a snapshot each time
//! their files change, and `moonframe history` to list, undo, redo, or
//! restore them from a terminal, as an agent would. moonframe-history keeps
//! them; [`Kept`] finds which folders and lays them on one timeline.

mod kept;

pub(crate) use kept::{Kept, open_with_sources};
use {
    crate::paths::{self, Folders},
    moonframe_history::Snapshot,
    std::{
        path::{Path, PathBuf},
        process::ExitCode,
        time::{SystemTime, UNIX_EPOCH},
    },
};

pub(crate) const USAGE: &str = "\
moonframe history: undo changes to your app

Every change the window reloads for is kept as a snapshot, whoever made
it: you, an editor, or an agent. That covers your folder (or a profile's),
the source of a linked app, and app/ when running from a checkout. Undo
writes the files back and the window reloads with them.

Usage: moonframe [--profile NAME] history [COMMAND]

Commands:
  list [N]      the last N snapshots (default 20), newest first; * marks
                the one the folder is at
  undo          go back one snapshot
  undo --run [ID]
                go back to before the agent's run ID (default: the last),
                however many files it changed
  redo          go forward one again, after an undo
  restore ID    go to the snapshot ID (as `list` shows it)
  show ID       what the snapshot ID changed, as a patch
  snapshot      take one now, if anything changed

settings.toml, databases, installed packages and other profiles are left
out: undo never changes them. User-selected miniapp source folders are kept
when they are active in the window.";

/// `moonframe history <words>`.
pub(crate) fn run(folders: &Folders, sources: &[PathBuf], words: &[String]) -> ExitCode {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    if matches!(words.first(), Some(&("help" | "-h" | "--help"))) {
        println!("{}", crate::cli::branded(USAGE));
        return ExitCode::SUCCESS;
    }
    let Some(history) = open_with_sources(folders, sources) else {
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
        ["undo", "--run"] => undo_run(&history, None),
        ["undo", "--run", id] => undo_run(&history, Some(id)),
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
        ["snapshot"] => history.capture("by hand").map(|taken| {
            for snapshot in &taken {
                println!("Took {} ({})", snapshot.label, snapshot.short());
            }
            if taken.is_empty() {
                println!("Nothing changed");
            }
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

fn undo_run(history: &Kept, id: Option<&str>) -> moonframe_history::Result<()> {
    let Some(run) = id.map(str::to_owned).or_else(|| history.last_run()) else {
        println!("No agent's run to undo");
        return Ok(());
    };
    let restored = history.undo_run(&run)?;
    if restored == 0 {
        println!("No run {run} to undo");
    } else {
        println!("Went back to before run {run}");
    }
    Ok(())
}

fn list(history: &Kept, count: usize) -> moonframe_history::Result<()> {
    let listed = history.list(count)?;
    if listed.is_empty() {
        println!("No snapshots yet of {}", history.folder().display());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs().cast_signed());
    let several = history.several();
    for (folder, snapshot) in &listed {
        let line = line(snapshot, now);
        if several {
            println!("{line}  [{}]", short_folder(folder));
        } else {
            println!("{line}");
        }
    }
    Ok(())
}

/// The folder's last part, enough to tell a timeline's folders apart.
fn short_folder(folder: &Path) -> String {
    folder.file_name().map_or_else(
        || folder.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
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
