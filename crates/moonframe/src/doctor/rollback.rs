//! `moonframe doctor --rollback`: takes the app back to the newest moment
//! it loaded. Each moment is loaded from a copy first, so the folders and a
//! running window only ever see the one that passed. Taking a folder back
//! goes through the history, so `moonframe history redo` undoes it.

use {
    crate::{history::Kept, paths::Folders},
    moonframe_core::Roots,
    moonframe_history::{History, Snapshot},
    std::process::ExitCode,
};

/// Snapshots read from each folder, and moments tried.
const LOOK_BACK: usize = 200;
const MOST_TRIED: usize = 50;

pub(super) fn run(folders: &Folders) -> ExitCode {
    let Some(kept) = crate::history::open(folders) else {
        eprintln!("doctor: no history to go back in: git cannot run, or there is no folder");
        return ExitCode::from(2);
    };
    let roots = folders.roots();
    let problems = crate::cli::problems(roots.clone());
    if problems.is_empty() {
        println!("ok: the app loads, so there is nothing to roll back");
        return ExitCode::SUCCESS;
    }
    for (name, error) in &problems {
        println!("{name}: {error}");
    }
    match find_and_restore(&kept, &roots) {
        Ok(true) => {
            println!("`moonframe history redo` goes forward again");
            ExitCode::SUCCESS
        },
        Ok(false) => {
            println!(
                "no snapshot loads without a problem, so nothing changed; the problem may not \
                 come from these folders"
            );
            ExitCode::FAILURE
        },
        Err(error) => {
            eprintln!("doctor: {error}");
            ExitCode::FAILURE
        },
    }
}

fn find_and_restore(kept: &Kept, roots: &Roots) -> moonframe_history::Result<bool> {
    kept.capture("before rollback")?;
    let mut lines = Vec::new();
    for history in kept.histories() {
        lines.push(line(history)?);
    }
    for at in moments(&lines).into_iter().take(MOST_TRIED) {
        let picks = picks(&lines, at);
        if !loads(kept.histories(), &picks, roots)? {
            continue;
        }
        for (history, pick) in kept.histories().iter().zip(&picks) {
            if let Some(pick) = pick.filter(|pick| !pick.current) {
                history.restore(&pick.id)?;
                println!(
                    "rolled {} back to {} ({})",
                    history.folder().display(),
                    pick.short(),
                    pick.label
                );
            }
        }
        return Ok(true);
    }
    Ok(false)
}

/// A folder's snapshots from where it is now back, newest first.
fn line(history: &History) -> moonframe_history::Result<Vec<Snapshot>> {
    let listed = history.list(LOOK_BACK)?;
    Ok(listed
        .into_iter()
        .skip_while(|snapshot| !snapshot.current)
        .collect())
}

/// The times any folder changed, newest first: the moments to try.
fn moments(lines: &[Vec<Snapshot>]) -> Vec<i64> {
    let mut moments: Vec<i64> = lines.iter().flatten().map(|one| one.created).collect();
    moments.sort_unstable_by_key(|at| std::cmp::Reverse(*at));
    moments.dedup();
    moments
}

/// Each folder's snapshot at the time `at`: its newest by then, or its
/// oldest if it was first kept later.
fn picks(lines: &[Vec<Snapshot>], at: i64) -> Vec<Option<&Snapshot>> {
    lines
        .iter()
        .map(|line| {
            let by_then = line.iter().find(|snapshot| snapshot.created <= at);
            by_then.or_else(|| line.last())
        })
        .collect()
}

/// Whether the app loads with each folder at its pick, read from a copy.
/// With every folder where it is now there is nothing new to try.
fn loads(
    histories: &[History],
    picks: &[Option<&Snapshot>],
    roots: &Roots,
) -> moonframe_history::Result<bool> {
    let copies = tempfile::tempdir()?;
    let mut roots = roots.clone();
    let mut differs = false;
    for (index, (history, pick)) in histories.iter().zip(picks).enumerate() {
        if let Some(pick) = pick.filter(|pick| !pick.current) {
            let copy = copies.path().join(index.to_string());
            history.export(&pick.id, &copy)?;
            roots = roots.moved(history.folder(), &copy);
            differs = true;
        }
    }
    Ok(differs && crate::cli::problems(roots).is_empty())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {
        super::*,
        std::{fs, path::Path},
    };

    fn snapshot(id: &str, created: i64, current: bool) -> Snapshot {
        Snapshot {
            id: id.to_owned(),
            created,
            label: id.to_owned(),
            reason: "changed".to_owned(),
            current,
        }
    }

    #[test]
    fn a_moment_takes_each_folder_as_it_was_by_then() {
        let own = vec![snapshot("c", 30, true), snapshot("a", 10, false)];
        let source = vec![snapshot("d", 20, true)];
        let lines = [own, source];
        assert_eq!(moments(&lines), [30, 20, 10]);
        let ids = |at| -> Vec<&str> {
            picks(&lines, at)
                .into_iter()
                .map(|pick| pick.unwrap().id.as_str())
                .collect()
        };
        assert_eq!(ids(25), ["a", "d"]);
        // First kept after that moment: its oldest is the nearest there is.
        assert_eq!(ids(10), ["a", "d"]);
    }

    /// The real app, with `user` over it.
    fn window(user: &Path) -> Folders {
        Folders {
            builtin: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app"),
            host: None,
            user: Some(user.to_owned()),
            profile: None,
            bundled: None,
        }
    }

    #[test]
    fn goes_back_past_broken_moments_to_the_newest_that_loads() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("user");
        fs::create_dir_all(&user).unwrap();
        let store = dir.path().join("own.git");
        let history = History::open(&user, &store).unwrap();
        let style = user.join("style.luau");
        fs::write(&style, "return {}\n").unwrap();
        history.capture("launch").unwrap();
        // Git's times are in seconds, so a moment needs one of its own.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        fs::write(&style, "return {\n").unwrap();
        history.capture("changed").unwrap();
        let folders = window(&user);
        let roots = folders.roots();
        let kept = Kept::of(vec![history]);
        assert!(find_and_restore(&kept, &roots).unwrap());
        assert_eq!(fs::read_to_string(&style).unwrap(), "return {}\n");
    }
}
