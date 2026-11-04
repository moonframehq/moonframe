//! `moonframe doctor`: finds the files in the user's folder that keep the
//! app from loading, and with `--fix` moves them aside, so the app's own
//! come back. A file there wins over the one at the same path in the app,
//! so taking it away is always a way back. The window reloads as they go.

use {
    crate::paths::Folders,
    moonframe_core::SHELL,
    std::{
        fs,
        path::{Path, PathBuf},
        process::ExitCode,
        time::{SystemTime, UNIX_EPOCH},
    },
};

pub(crate) const USAGE: &str = "\
moonframe doctor: find the files in your folder that break the app

A file in your folder (or a profile's) replaces the one at the same path
in the app. Doctor loads the app and names the ones with a problem.

Usage: moonframe [--profile NAME] doctor [--fix]

Options:
  --fix         move them to .broken/ in your folder, so the app's own come
                back; a running window reloads without them";

/// Where broken files are moved to in the user's folder.
const ASIDE: &str = ".broken";

pub(crate) fn run(folders: &Folders, words: &[String]) -> ExitCode {
    let fix = match words {
        [] => false,
        [word] if word == "--fix" => true,
        [word] if word == "help" || word == "--help" || word == "-h" => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        },
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        },
    };
    let Some(own) = folders.own() else {
        eprintln!("doctor: there is no user folder to look in; drop --no-user");
        return ExitCode::from(2);
    };
    let problems = crate::cli::problems(folders.roots());
    let (mut blamed, others): (Vec<_>, Vec<_>) = problems
        .into_iter()
        .map(|(name, error)| (override_of(&own, &name, &error), name, error))
        .partition(|(file, ..)| file.is_some());
    blamed.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, name, error) in &others {
        println!("{name}: {error}");
    }
    if blamed.is_empty() {
        if others.is_empty() {
            println!("ok: nothing in {} breaks the app", own.display());
            return ExitCode::SUCCESS;
        }
        println!("none of these come from a file in {}", own.display());
        return ExitCode::FAILURE;
    }
    let mut files: Vec<PathBuf> = Vec::new();
    for (file, name, error) in blamed {
        println!("{name}: {error}");
        if let Some(file) = file
            && !files.contains(&file)
        {
            files.push(file);
        }
    }
    if !fix {
        for file in &files {
            println!("broken: {}", own.join(file).display());
        }
        println!("`moonframe doctor --fix` moves them aside");
        return ExitCode::FAILURE;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_secs());
    let aside = Path::new(ASIDE).join(stamp.to_string());
    let mut failed = false;
    for file in &files {
        match move_aside(&own, file, &aside) {
            Ok(to) => println!("moved {} to {}", file.display(), to.display()),
            Err(error) => {
                eprintln!("cannot move {}: {error}", file.display());
                failed = true;
            },
        }
    }
    if failed || !others.is_empty() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// What in the folder `own` a problem named `name` comes from, by its path
/// from `own`, if anything. The error says first: a mod that uses a broken
/// component fails with the component's error, and is not itself to blame.
/// A mounted copy (`editor#1`) is its mod's.
fn override_of(own: &Path, name: &str, error: &str) -> Option<PathBuf> {
    let place = if let Some(component) = component_in(error) {
        format!("components/{component}.luau")
    } else if let Some(component) = name.strip_suffix(".luau") {
        format!("components/{component}.luau")
    } else if let Some(provider) = name.strip_prefix("provider ") {
        format!("providers/{provider}")
    } else if name == SHELL {
        "main.luau".to_owned()
    } else if name == "style" {
        "style.luau".to_owned()
    } else {
        format!("mods/{}", name.split('#').next().unwrap_or(name))
    };
    Some(PathBuf::from(place)).filter(|place| own.join(place).exists())
}

/// The component an error's `components/<name>:<line>` names.
fn component_in(error: &str) -> Option<&str> {
    let (_, rest) = error.split_once("components/")?;
    let (name, _) = rest.split_once(':')?;
    let name = name.strip_suffix(".luau").unwrap_or(name);
    (!name.is_empty() && !name.contains(char::is_whitespace)).then_some(name)
}

/// Moves `file` to the same path under `aside`, both in `own`.
fn move_aside(own: &Path, file: &Path, aside: &Path) -> std::io::Result<PathBuf> {
    let to = own.join(aside).join(file);
    if let Some(folder) = to.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::rename(own.join(file), &to)?;
    Ok(to)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_problem_is_blamed_on_the_file_at_its_path() {
        let dir = tempfile::tempdir().unwrap();
        let own = dir.path();
        for folder in ["mods/feedback", "providers/acp", "components/demos"] {
            fs::create_dir_all(own.join(folder)).unwrap();
        }
        for file in [
            "main.luau",
            "style.luau",
            "components/card.luau",
            "components/demos/card.luau",
        ] {
            fs::write(own.join(file), "").unwrap();
        }
        let blamed =
            |name, error| override_of(own, name, error).map(|file| file.display().to_string());
        assert_eq!(blamed("feedback", "x").as_deref(), Some("mods/feedback"));
        assert_eq!(blamed("feedback#1", "x").as_deref(), Some("mods/feedback"));
        assert_eq!(
            blamed("provider acp", "x").as_deref(),
            Some("providers/acp")
        );
        assert_eq!(blamed(SHELL, "x").as_deref(), Some("main.luau"));
        assert_eq!(blamed("style", "x").as_deref(), Some("style.luau"));
        assert_eq!(
            blamed("card.luau", "x").as_deref(),
            Some("components/card.luau")
        );
        assert_eq!(
            blamed("demos/card.luau", "x").as_deref(),
            Some("components/demos/card.luau")
        );
        // The app's own problem: nothing of the user's to move.
        assert_eq!(blamed("status", "x"), None);
    }

    #[test]
    fn a_mod_is_not_blamed_for_the_component_that_broke_it() {
        let dir = tempfile::tempdir().unwrap();
        let own = dir.path();
        fs::create_dir_all(own.join("mods/feedback")).unwrap();
        fs::create_dir_all(own.join("components")).unwrap();
        fs::write(own.join("components/card.luau"), "").unwrap();
        let error = "syntax error: components/card:1: Expected identifier";
        assert_eq!(
            override_of(own, "feedback", error),
            Some("components/card.luau".into())
        );
        // The app's own component is broken: the user's mod stays.
        let error = "syntax error: components/other:1: Expected identifier";
        assert_eq!(override_of(own, "feedback", error), None);
    }

    #[test]
    fn moving_aside_keeps_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let own = dir.path();
        fs::create_dir_all(own.join("mods/feedback")).unwrap();
        fs::write(own.join("mods/feedback/main.luau"), "x").unwrap();
        let to = move_aside(own, Path::new("mods/feedback"), Path::new(".broken/1")).unwrap();
        assert!(!own.join("mods/feedback").exists());
        assert_eq!(fs::read_to_string(to.join("main.luau")).unwrap(), "x");
    }
}
