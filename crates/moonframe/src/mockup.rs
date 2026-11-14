//! `moonframe mockup`: ideas for a UI drawn with the app's own components
//! from one Luau file, checked, rendered to PNGs in light and dark, laid
//! out on a page for Sheprd (or any browser) to pick from, or shown live
//! in a tab of the user's window.
//!
//! The file returns `{ ask, width?, height?, variants = { { name, note?,
//! body, width?, height? } } }`; a `body` is an element, a list of them,
//! or `function(app)` returning either.

#[cfg(feature = "capture")]
mod draw;
mod source;

use {
    moonframe_core::{
        Roots,
        entries::{CheckOptions, RootPolicy, Severity, check_in},
    },
    std::{
        path::{Path, PathBuf},
        process::ExitCode,
    },
};

/// The words `moonframe mockup` takes first.
pub(crate) const COMMANDS: &[&str] = &["check", "close", "help", "open", "page", "render"];

const USAGE: &str = "\
Usage: moonframe mockup check FILE      check every variant against the app's components
       moonframe mockup render FILE [--out DIR] [--light | --dark]
                                         draw each variant to DIR/a.png, a-dark.png, ...
       moonframe mockup page FILE [--out DIR] [--open]
                                         render, then write DIR/index.html to pick from
                                         (--open shows it in Sheprd)
       moonframe mockup open FILE       show the variants live in a Mockups tab
       moonframe mockup close           close that tab and remove its mod

FILE returns { ask, width?, height?, variants = { { name, note?, body } } }.
DIR is FILE's folder/<name>.mockup unless --out says otherwise.";

/// Runs `moonframe mockup <words>` over the app's `roots`, with `own` the
/// user's (or the profile's) folder.
pub(crate) fn run(roots: &Roots, own: Option<&Path>, words: &[String]) -> ExitCode {
    let result = match words {
        _ if words.iter().any(|word| word == "--profile") => {
            Err("--profile goes before `mockup`: moonframe --profile NAME mockup ...".to_owned())
        },
        [command, file, rest @ ..] if command != "close" => {
            let file = PathBuf::from(file);
            match command.as_str() {
                "check" if rest.is_empty() => {
                    check(roots, &file).map(|()| println!("ok: {}", file.display()))
                },
                "render" => render_command(roots, &file, rest),
                "page" => page_command(roots, &file, rest),
                "open" if rest.is_empty() => open(roots, own, &file),
                _ => Err(USAGE.to_owned()),
            }
        },
        [command] if command == "close" => close(own),
        [command] if command == "help" => {
            println!("{USAGE}");
            Ok(())
        },
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        },
    }
}

fn read(file: &Path) -> Result<String, String> {
    std::fs::read_to_string(file).map_err(|error| format!("{}: {error}", file.display()))
}

/// Checks every variant, strictly, against the app's components and
/// stylesheet; prints each problem against `file`'s own lines.
fn check(roots: &Roots, file: &Path) -> Result<(), String> {
    let text = read(file)?;
    let work = tempfile::tempdir().map_err(|error| error.to_string())?;
    let entry = work.path().join("mockup");
    std::fs::create_dir(&entry).map_err(|error| error.to_string())?;
    std::fs::write(entry.join("main.luau"), source::entry(&text, 0, "light"))
        .map_err(|error| error.to_string())?;
    let options = CheckOptions {
        name: Some("mockup:check".to_owned()),
        roots: Some(roots.clone()),
        strict: true,
        ..CheckOptions::default()
    };
    let mut errors = 0;
    for mut problem in check_in(&entry, &RootPolicy::default(), &options) {
        if problem.file.starts_with(work.path()) {
            problem.file = file.to_owned();
        }
        let message = problem.to_string().replace(
            &entry.join("main.luau").display().to_string(),
            &file.display().to_string(),
        );
        eprintln!("{message}");
        errors += usize::from(problem.severity == Severity::Error);
    }
    match errors {
        0 => Ok(()),
        count => Err(format!(
            "{}: {count} problem(s) to fix first",
            file.display()
        )),
    }
}

#[cfg(feature = "capture")]
use draw::{page_command, render_command};

#[cfg(not(feature = "capture"))]
fn render_command(_: &Roots, _: &Path, _: &[String]) -> Result<(), String> {
    Err(NO_CAPTURE.to_owned())
}

#[cfg(not(feature = "capture"))]
fn page_command(_: &Roots, _: &Path, _: &[String]) -> Result<(), String> {
    Err(NO_CAPTURE.to_owned())
}

#[cfg(not(feature = "capture"))]
const NO_CAPTURE: &str = "this build draws no PNGs: in the moonframe repository run \
`just mockup render FILE` (a build with the `capture` feature), or show the \
variants live with `moonframe mockup open FILE`";

/// The live mod's file in `own`.
fn live_file(own: Option<&Path>) -> Result<PathBuf, String> {
    let own = own.ok_or("the Mockups tab lives in the user's folder: drop --no-user")?;
    Ok(own.join("mods").join("mockup").join("main.luau"))
}

/// Writes the live mod for `file`; the window loads it and brings up its tab.
fn open(roots: &Roots, own: Option<&Path>, file: &Path) -> Result<(), String> {
    check(roots, file)?;
    let file = std::path::absolute(file).map_err(|error| error.to_string())?;
    let text = read(&file)?;
    // New on every run, not only when the file changes, so `open` brings
    // the tab back after the user closed it.
    let round = {
        use std::hash::{Hash as _, Hasher as _};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };
    let live = live_file(own)?;
    if let Some(dir) = live.parent() {
        std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    }
    let source = source::live(&text, &file.display().to_string(), &round, false);
    std::fs::write(&live, source).map_err(|error| format!("{}: {error}", live.display()))?;
    println!(
        "open: a Mockups tab shows {} ({})",
        file.display(),
        live.display()
    );
    Ok(())
}

/// Closes the tab through its mod, waits for the window to see that, then
/// removes the mod: removed first, it could no longer close its tab.
fn close(own: Option<&Path>) -> Result<(), String> {
    let live = live_file(own)?;
    let Ok(text) = std::fs::read_to_string(&live) else {
        println!("no Mockups tab to close");
        return Ok(());
    };
    let closing = source::closing(&text).ok_or_else(|| {
        format!(
            "{}: not a mod `moonframe mockup open` wrote; remove its folder by hand",
            live.display()
        )
    })?;
    std::fs::write(&live, closing).map_err(|error| format!("{}: {error}", live.display()))?;
    std::thread::sleep(std::time::Duration::from_secs(2));
    if let Some(dir) = live.parent() {
        std::fs::remove_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    }
    println!("closed the Mockups tab and removed its mod");
    Ok(())
}
