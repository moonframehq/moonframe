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
    for picks in moments(&lines).into_iter().take(MOST_TRIED) {
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

/// Where each folder was at each moment, newest first: from now, each step
/// takes whichever folder changed last back one snapshot. No snapshot is
/// passed over, however many share a time, and a folder at its oldest stays
/// there as the others go on.
fn moments(lines: &[Vec<Snapshot>]) -> Vec<Vec<Option<&Snapshot>>> {
    let mut steps = vec![0; lines.len()];
    let mut moments = Vec::new();
    loop {
        let at = lines.iter().zip(&steps);
        moments.push(at.map(|(line, step)| line.get(*step)).collect());
        let older = lines
            .iter()
            .zip(&steps)
            .enumerate()
            .filter_map(|(folder, (line, step))| {
                line.get(step + 1)?;
                Some((line.get(*step)?.taken, folder))
            });
        let Some((_, folder)) = older.max() else {
            return moments;
        };
        if let Some(step) = steps.get_mut(folder) {
            *step += 1;
        }
    }
}

/// Whether the app loads with each folder at its pick, read from a copy
/// that holds what going back would leave in the folder, ignored files too.
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

    fn snapshot(id: &str, taken: i64, current: bool) -> Snapshot {
        Snapshot {
            id: id.to_owned(),
            created: taken / 1_000_000,
            taken,
            label: id.to_owned(),
            reason: "changed".to_owned(),
            current,
        }
    }

    fn ids<'a>(moments: &[Vec<Option<&'a Snapshot>>]) -> Vec<Vec<&'a str>> {
        let ids = |moment: &Vec<Option<&'a Snapshot>>| -> Vec<&'a str> {
            moment
                .iter()
                .map(|pick| pick.unwrap().id.as_str())
                .collect()
        };
        moments.iter().map(ids).collect()
    }

    #[test]
    fn each_moment_takes_back_whichever_folder_changed_last() {
        let own = vec![snapshot("c", 30, true), snapshot("a", 10, false)];
        let source = vec![snapshot("d", 20, true), snapshot("b", 5, false)];
        let lines = [own, source];
        assert_eq!(ids(&moments(&lines)), [["c", "d"], ["a", "d"], ["a", "b"]]);
    }

    #[test]
    fn a_folder_kept_later_stays_at_its_oldest() {
        let own = vec![snapshot("c", 30, true), snapshot("a", 10, false)];
        let source = vec![snapshot("d", 20, true)];
        assert_eq!(ids(&moments(&[own, source])), [["c", "d"], ["a", "d"]]);
    }

    #[test]
    fn snapshots_of_one_time_are_each_a_moment() {
        let line = vec![
            snapshot("broken", 10, true),
            snapshot("good", 10, false),
            snapshot("older", 10, false),
        ];
        assert_eq!(ids(&moments(&[line])), [["broken"], ["good"], ["older"]]);
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
        // In the same second, more often than not: each is a moment.
        fs::write(&style, "return {\n").unwrap();
        history.capture("changed").unwrap();
        let folders = window(&user);
        let roots = folders.roots();
        let kept = Kept::of(vec![history]);
        assert!(find_and_restore(&kept, &roots).unwrap());
        assert_eq!(fs::read_to_string(&style).unwrap(), "return {}\n");
    }

    #[test]
    fn an_ignored_file_that_breaks_the_app_counts_in_every_moment() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("user");
        fs::create_dir_all(user.join("mods/local")).unwrap();
        fs::write(user.join(".gitignore"), "mods/local/\n").unwrap();
        let history = History::open(&user, &dir.path().join("own.git")).unwrap();
        let style = user.join("style.luau");
        fs::write(&style, "return {}\n").unwrap();
        history.capture("launch").unwrap();
        fs::write(&style, "return { }\n").unwrap();
        history.capture("changed").unwrap();
        fs::write(user.join("mods/local/main.luau"), "return {\n").unwrap();
        let folders = window(&user);
        let roots = folders.roots();
        let kept = Kept::of(vec![history]);
        // Going back leaves the mod broken, so no snapshot would help.
        assert!(!find_and_restore(&kept, &roots).unwrap());
        assert_eq!(fs::read_to_string(&style).unwrap(), "return { }\n");
    }
}
