//! `moonframe mod <command>`: installing, updating, and sharing packages
//! from a terminal.

use {
    super::{Contents, Updated},
    std::{
        io::{self, BufRead as _, IsTerminal as _, Write as _},
        path::{Path, PathBuf},
        process::ExitCode,
    },
};

pub(crate) const USAGE: &str = "\
moonframe mod: mods others publish as git repositories

Usage: moonframe mod <COMMAND>

Commands:
  install <SOURCE>[@REF]  clone a package and enable it. SOURCE is
                          github.com/owner/repo, owner/repo (on GitHub), any
                          git URL, or a folder; REF a branch, tag, or commit
  list                    the installed packages, and what each holds
  update [NAME]           fetch the latest of each package, or of NAME
  remove <NAME>           uninstall a package
  enable <NAME>           lay a disabled package over the app again
  disable <NAME>          keep a package installed, but out of the window
  link <DIR>              use a folder in place as a package, to develop it
  new <DIR>               start a package to publish: a git repository with
                          one mod

NAME is a package's id, as `list` shows it, or its last part.

Options:
  --yes                   install providers without asking

Packages are cloned into packages/ in the user's folder and listed in its
packages.toml. Each is laid between the built-in app and the user's own
folder, so a mod of yours replaces a package's of the same name. A package
is one mod, with main.luau at its root, or laid out like the app folder:
mods/, providers/, components/, style.luau. Packages ship source only,
never provider.wasm or other binaries, so all they run can be reviewed
first: a wasm provider ships its crate, which the app builds. The window
picks up every change as it happens.

To publish a package, push its repository anywhere git can clone from and
share `moonframe mod install <where>`.";

/// Runs `args`, the words after `mod`, on the user's folder `user`.
pub fn run(user: &Path, args: &[String]) -> ExitCode {
    let yes = args.iter().any(|arg| arg == "--yes" || arg == "-y");
    let words: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|arg| !matches!(*arg, "--yes" | "-y"))
        .collect();
    let writers = super::git_writers(user);
    let approve_any = {
        let writers = writers.clone();
        move |contents: &Contents| approve(contents, &writers, yes)
    };
    // An app runs in a profile of its own; over the user's app it would
    // only add mods its shell has no place for.
    let approve_mod = move |contents: &Contents| match &contents.app {
        Some(app) => Err(format!(
            "this is the app {name}, not a mod: `moonframe app install` runs it as a \
             profile of its own",
            name = super::printable(&app.name)
        )),
        None => approve(contents, &writers, yes),
    };
    let done = match words.as_slice() {
        [] | ["help" | "-h" | "--help"] => {
            println!("{USAGE}");
            Ok(())
        },
        ["install", source] => {
            println!("Cloning {source}…");
            super::install(user, source, &approve_mod).map(|(id, contents)| {
                println!("Installed {id}: {contents}");
                println!("It is enabled; an open window shows it now.");
            })
        },
        ["link", folder] => super::link(user, folder, &approve_mod).map(|(id, contents)| {
            println!("Linked {id}: {contents}");
            println!("Edits to {folder} show in an open window as you save.");
        }),
        ["list"] => list(user),
        ["update"] => update(user, None, &approve_any),
        ["update", name] => update(user, Some(name), &approve_any),
        ["remove" | "uninstall", name] => {
            super::remove(user, name).map(|id| println!("Removed {id}"))
        },
        ["enable", name] => super::set_enabled(user, name, true).map(|id| println!("Enabled {id}")),
        ["disable", name] => {
            super::set_enabled(user, name, false).map(|id| println!("Disabled {id}"))
        },
        ["new", folder] => {
            let folder = PathBuf::from(folder);
            super::create(&folder).map(|name| {
                let shown = folder.display();
                println!("Started the package {name} in {shown}: main.luau and a README.");
                println!();
                println!("  moonframe mod link {shown}   see it in the window as you edit");
                println!("  cd {shown} && git add -A && git commit -m \"First version\"");
                println!("  gh repo create {name} --public --source . --push");
                println!();
                println!("Then anyone can: moonframe mod install github.com/<you>/{name}");
            })
        },
        _ => Err(format!(
            "unknown command: mod {}\n\n{USAGE}",
            words.join(" ")
        )),
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("moonframe mod: {problem}");
            ExitCode::FAILURE
        },
    }
}

fn list(user: &Path) -> Result<(), String> {
    let installed = super::installed(user)?;
    if installed.is_empty() {
        println!("No packages; `moonframe mod install github.com/owner/repo` adds one.");
        return Ok(());
    }
    for package in installed {
        let mut notes = Vec::new();
        if !package.enabled {
            notes.push("disabled".to_owned());
        }
        if package.link {
            notes.push("linked".to_owned());
        }
        if let Some(reference) = package.reference {
            notes.push(format!("@{reference}"));
        }
        if let Some(commit) = package.commit {
            notes.push(commit);
        }
        println!("{} ({})", package.id, notes.join(", "));
        match package.contents {
            Ok(contents) => println!("  {contents}"),
            Err(problem) => println!("  {problem}"),
        }
    }
    Ok(())
}

pub fn update(user: &Path, name: Option<&str>, approve: super::Approve) -> Result<(), String> {
    let mut failed = 0;
    for (id, updated) in super::update(user, name, approve)? {
        match updated {
            Ok(Updated::Current(commit)) => println!("{id}: up to date at {commit}"),
            Ok(Updated::Moved { from, to }) => println!("{id}: {from} → {to}"),
            Ok(Updated::Linked) => println!("{id}: linked, so always current"),
            Err(problem) => {
                failed += 1;
                eprintln!("{id}: {problem}");
            },
        }
    }
    match failed {
        0 => Ok(()),
        1 => Err("a package was not updated".to_owned()),
        _ => Err(format!("{failed} packages were not updated")),
    }
}

/// Asks before a package's providers may run code with the user's access,
/// or its mods may commit and push (named in git's `writers`); other mods,
/// components, and styles are Luau the app runs sandboxed. An app's
/// approval covers the packages it ships beside it, which its updates
/// change with it.
pub fn approve(contents: &Contents, writers: &[String], yes: bool) -> Result<(), String> {
    let mut trusted = Vec::new();
    let mut writing = Vec::new();
    for (from, part) in contents.parts() {
        let name = |name: &str| match from {
            Some(from) => format!("{} (in {})", super::printable(name), super::printable(from)),
            None => super::printable(name),
        };
        trusted.extend(
            part.trusted()
                .map(|(provider, how)| format!("{}: {how}", name(provider))),
        );
        writing.extend(part.writers(writers).map(name));
    }
    if (trusted.is_empty() && writing.is_empty()) || yes {
        return Ok(());
    }
    if !trusted.is_empty() {
        println!("This package has providers that run code as you:");
        for line in &trusted {
            println!("  {line}");
        }
    }
    if !writing.is_empty() {
        println!("This package replaces mods that may commit and push with git:");
        for name in &writing {
            println!("  {name}");
        }
    }
    if !io::stdin().is_terminal() {
        return Err("declined: pass --yes to allow them".to_owned());
    }
    print!("Allow them? [y/N] ");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    io::stdin()
        .lock()
        .read_line(&mut answer)
        .map_err(|error| error.to_string())?;
    match answer.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => Ok(()),
        _ => Err("declined".to_owned()),
    }
}
