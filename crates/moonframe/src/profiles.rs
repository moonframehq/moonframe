//! `moonframe app <command>`: whole apps, each run in a profile of its own.
//!
//! A profile is a folder in `profiles/` of the user's folder, laid out like
//! the user's folder itself: its own `packages.toml`, packages, mods, and
//! settings. An app package (one with a `moonframe.toml`, see `packages`)
//! installed into it brings the shell and the mods; the built-in app under
//! it lends its providers, components, and styles. `moonframe --profile
//! NAME` opens it, as an app of its own beside any other.

use {
    crate::{
        packages::{self, Contents},
        paths,
    },
    std::{
        fs,
        path::{Path, PathBuf},
        process::ExitCode,
    },
};

pub(crate) const USAGE: &str = "\
moonframe app: whole apps others publish, each run as a profile of its own

Usage: moonframe app <COMMAND>

Commands:
  install <SOURCE>[@REF]  clone an app and the packages it requires into a
                          new profile. SOURCE is as `moonframe mod install`
                          takes it
  list                    the profiles, and the app each runs
  update <NAME>           fetch the latest of a profile's app and packages,
                          and install what it newly requires
  remove <NAME>           delete a profile, with its packages
  link <DIR>              run a folder in place as a profile's app, to
                          develop it
  new <DIR>               start an app to publish: a git repository with a
                          moonframe.toml, a shell, and one mod
  paths <NAME>            what a bundle of the profile holds, one per line:
                          name, title, its app folder, its icon, each package
                          (`just bundle-app NAME` reads it)

Options:
  --as NAME               the profile's name, for install and link (default:
                          the name in the app's moonframe.toml)
  --yes                   install providers without asking
  --force                 remove a profile holding files of your own

Run a profile with `moonframe --profile NAME`; `moonframe --profile NAME
mod <COMMAND>` manages the packages in it. Each profile keeps its own mods,
packages, and theme; your style.luau follows you into all of them, and the
starter app in your folder stays as it is.

An app is a repository laid out like the app folder (main.luau, mods/,
providers/, components/, style.luau) with a moonframe.toml at its root:

  [app]
  name = \"pi\"                         # the profile it installs as
  title = \"Pi\"                        # its name in the menu bar and bundle
  icon = \"icon.png\"                   # its icon, a PNG (default icon.png)
  keep = [\"terminal\"]                 # built-in mods it shows
  requires = [\"owner/repo@v1\"]        # packages it installs with it; a path
                                      # such as \"../shared\" is one shipped
                                      # beside it in its repository, used
                                      # in place
  version = \"1.0.0\"                   # its version, for bundles and updates
  updates = \"owner/repo\"              # GitHub releases to update from:
                                      # Check for Updates in the app menu";

/// The words that make `app` this command rather than a folder's name.
pub(crate) const COMMANDS: &[&str] = &[
    "install",
    "list",
    "update",
    "remove",
    "uninstall",
    "link",
    "new",
    "paths",
    "help",
    "-h",
    "--help",
    "--as",
    "--yes",
    "-y",
    "--force",
];

/// Runs `args`, the words after `app`, on the user's folder `user`.
pub(crate) fn run(user: &Path, args: &[String]) -> ExitCode {
    let mut yes = false;
    let mut force = false;
    let mut name = None;
    let mut words = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--yes" | "-y" => yes = true,
            "--force" => force = true,
            "--as" => match args.next() {
                Some(value) => name = Some(value.as_str()),
                None => return fail("--as needs a name".to_owned()),
            },
            word => words.push(word),
        }
    }
    if let Some(name) = name
        && !packages::valid_name(name)
    {
        return fail(format!(
            "`{name}` is not a profile name: use a-z, 0-9, - and _"
        ));
    }
    let done = match words.as_slice() {
        [] | ["help" | "-h" | "--help"] => {
            println!("{USAGE}");
            Ok(())
        },
        ["install", source] => install(user, source, name, yes),
        ["link", folder] => link(user, folder, name, yes),
        ["list"] => list(user),
        ["update", profile] => update(user, profile, yes),
        ["remove" | "uninstall", profile] => remove(user, profile, force),
        ["new", folder] => create(Path::new(folder)),
        ["paths", profile] => print_paths(user, profile),
        _ => Err(format!(
            "unknown command: app {}\n\n{USAGE}",
            words.join(" ")
        )),
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => fail(problem),
    }
}

fn fail(problem: String) -> ExitCode {
    eprintln!("moonframe app: {problem}");
    ExitCode::FAILURE
}

/// Asks as `moonframe mod` does, and refuses anything but an app.
/// `writers` are the mods git lets commit and push in the profile.
fn approve_app(contents: &Contents, writers: &[String], yes: bool) -> Result<(), String> {
    if contents.app.is_none() {
        return Err(
            "this is a package of mods, not an app (it has no moonframe.toml): \
             `moonframe mod install` adds it to your app, or `moonframe --profile NAME mod \
             install` to a profile's"
                .to_owned(),
        );
    }
    packages::approve(contents, writers, yes)
}

/// Asks as `moonframe mod` does, and refuses a second app.
fn approve_mod(contents: &Contents, writers: &[String], yes: bool) -> Result<(), String> {
    if contents.app.is_some() {
        return Err("an app cannot require another app".to_owned());
    }
    packages::approve(contents, writers, yes)
}

/// Clones the app into a profile being made, moves the profile into place
/// under the app's name, then installs what the app requires.
fn install(user: &Path, source: &str, name: Option<&str>, yes: bool) -> Result<(), String> {
    let profiles = paths::profiles_folder(user);
    if let Some(name) = name {
        refuse_taken(user, name)?;
    }
    let incoming = profiles.join(format!(".incoming-{}", std::process::id()));
    let _ = fs::remove_dir_all(&incoming);
    println!("Cloning {source}…");
    // A new profile: git's writers are the built-in ones.
    let writers = packages::git_writers(&incoming);
    let installed = packages::install(&incoming, source, &|contents| {
        approve_app(contents, &writers, yes)?;
        match name {
            Some(_) => Ok(()),
            None => refuse_taken(user, &app_name(contents)),
        }
    });
    let (id, contents) = match installed {
        Ok(installed) => installed,
        Err(problem) => {
            let _ = fs::remove_dir_all(&incoming);
            return Err(problem);
        },
    };
    let name = name.map_or_else(|| app_name(&contents), str::to_owned);
    let profile = paths::profile(user, &name);
    if let Err(error) = fs::rename(&incoming, &profile) {
        let _ = fs::remove_dir_all(&incoming);
        return Err(format!("{}: {error}", profile.display()));
    }
    println!("Installed {id} as the profile {name}: {contents}");
    let folder = packages::enabled_layers(&profile, true)
        .1
        .map(|app| app.path);
    let missing = install_requires(&profile, folder.as_deref(), &contents, yes);
    println!();
    println!("  moonframe --profile {name}   open it");
    if missing > 0 {
        return Err(format!(
            "{missing} required package(s) were not installed; \
             `moonframe app update {name}` tries again"
        ));
    }
    Ok(())
}

/// Uses `folder` in place as the app of a new profile.
fn link(user: &Path, folder: &str, name: Option<&str>, yes: bool) -> Result<(), String> {
    let manifest = packages::App::read(&PathBuf::from(folder))?
        .ok_or_else(|| format!("{folder} is not an app: it has no moonframe.toml"))?;
    let name = name.map_or(manifest.name, str::to_owned);
    refuse_taken(user, &name)?;
    let profile = paths::profile(user, &name);
    let writers = packages::git_writers(&profile);
    let linked = packages::link(&profile, folder, &|contents| {
        approve_app(contents, &writers, yes)
    });
    let (id, contents) = match linked {
        Ok(linked) => linked,
        Err(problem) => {
            let _ = fs::remove_dir_all(&profile);
            return Err(problem);
        },
    };
    println!("Linked {id} as the profile {name}: {contents}");
    let folder = packages::enabled_layers(&profile, true)
        .1
        .map(|app| app.path);
    let missing = install_requires(&profile, folder.as_deref(), &contents, yes);
    println!();
    println!("  moonframe --profile {name}   open it; edits show as you save");
    match missing {
        0 => Ok(()),
        _ => Err(format!("{missing} required package(s) were not installed")),
    }
}

/// The profile name an app asks for.
fn app_name(contents: &Contents) -> String {
    contents
        .app
        .as_ref()
        .map(|app| app.name.clone())
        .unwrap_or_default()
}

/// Refuses a name that is taken or that is not a profile name, before any
/// path is made from it.
fn refuse_taken(user: &Path, name: &str) -> Result<(), String> {
    if !packages::valid_name(name) {
        return Err(format!(
            "`{}` is not a profile name: use a-z, 0-9, - and _",
            name.escape_debug()
        ));
    }
    if paths::profile(user, name).exists() {
        return Err(format!(
            "a profile `{name}` exists; pick another with --as NAME, or \
             `moonframe app remove {name}` first"
        ));
    }
    Ok(())
}

/// Installs each package the app in `folder` requires that the profile
/// lacks; how many failed. A requirement given as a path from the app's
/// folder (`./x`, `../x`) is a package shipped beside it, inside the app's
/// repository, used in place.
fn install_requires(
    profile: &Path,
    folder: Option<&Path>,
    contents: &Contents,
    yes: bool,
) -> usize {
    let Some(app) = &contents.app else {
        return 0;
    };
    let mut failed = 0;
    let writers = packages::git_writers(profile);
    for source in &app.requires {
        let done = packages::beside(folder, source).and_then(|beside| match beside {
            Some(path) => {
                if packages::linked(profile, &path)? {
                    return Ok(None);
                }
                let shown = path.display().to_string();
                println!("Linking {shown}, which it requires…");
                packages::link(profile, &shown, &|contents| {
                    approve_mod(contents, &writers, yes)
                })
                .map(Some)
            },
            None => {
                if packages::has(profile, &packages::source_id(source)?)? {
                    return Ok(None);
                }
                println!("Cloning {source}, which it requires…");
                packages::install(profile, source, &|contents| {
                    approve_mod(contents, &writers, yes)
                })
                .map(Some)
            },
        });
        match done {
            Ok(Some((id, contents))) => println!("Installed {id}: {contents}"),
            Ok(None) => {},
            Err(problem) => {
                failed += 1;
                eprintln!("{}: {problem}", packages::printable(source));
            },
        }
    }
    failed
}

fn list(user: &Path) -> Result<(), String> {
    let names = paths::profiles(user);
    if names.is_empty() {
        println!("No profiles; `moonframe app install github.com/owner/repo` adds one.");
        return Ok(());
    }
    for name in names {
        let profile = paths::profile(user, &name);
        let installed = packages::installed(&profile)?;
        let app = installed
            .iter()
            .find(|package| package.contents.as_ref().is_ok_and(|c| c.app.is_some()));
        match app {
            Some(app) => {
                let mut notes = Vec::new();
                if app.link {
                    notes.push("linked".to_owned());
                }
                if let Some(reference) = &app.reference {
                    notes.push(format!("@{reference}"));
                }
                if let Some(commit) = &app.commit {
                    notes.push(commit.clone());
                }
                println!("{name}: {} ({})", app.id, notes.join(", "));
            },
            None => println!("{name}: the built-in app"),
        }
        let others = installed.len() - usize::from(app.is_some());
        if others > 0 {
            println!("  and {others} package(s); `moonframe --profile {name} mod list`");
        }
    }
    Ok(())
}

fn update(user: &Path, name: &str, yes: bool) -> Result<(), String> {
    let profile = existing(user, name)?;
    let writers = packages::git_writers(&profile);
    let updated = packages::print_update(&profile, None, &|contents| {
        // The app may change its requirements; its kind may not change.
        match contents.app {
            Some(_) => packages::approve(contents, &writers, yes),
            None => approve_mod(contents, &writers, yes),
        }
    });
    // What the app requires now, which may be new.
    let mut missing = 0;
    let folder = packages::enabled_layers(&profile, true)
        .1
        .map(|app| app.path);
    for package in packages::installed(&profile)? {
        if let Ok(contents) = &package.contents
            && contents.app.is_some()
        {
            missing += install_requires(&profile, folder.as_deref(), contents, yes);
        }
    }
    updated?;
    match missing {
        0 => Ok(()),
        _ => Err(format!("{missing} required package(s) were not installed")),
    }
}

/// Deletes a profile. One holding the user's own mods, stylesheet, or
/// other files needs `force`, so a removal never takes their work.
fn remove(user: &Path, name: &str, force: bool) -> Result<(), String> {
    let profile = existing(user, name)?;
    let own: Vec<String> = fs::read_dir(&profile)
        .map_err(|error| format!("{}: {error}", profile.display()))?
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|file| {
            !matches!(
                file.as_str(),
                "packages" | "packages.toml" | "settings.toml"
            )
        })
        .collect();
    if !own.is_empty() && !force {
        return Err(format!(
            "{} holds files of yours ({}); pass --force to delete them too",
            profile.display(),
            own.join(", ")
        ));
    }
    fs::remove_dir_all(&profile).map_err(|error| format!("{}: {error}", profile.display()))?;
    println!("Removed the profile {name}");
    Ok(())
}

/// What `scripts/bundle-app.sh` stages: tab-separated kinds and values.
fn print_paths(user: &Path, name: &str) -> Result<(), String> {
    let profile = existing(user, name)?;
    let (packages, app) = packages::enabled_layers(&profile, true);
    let app = app.ok_or_else(|| format!("the profile {name} runs no app package"))?;
    let manifest = packages::App::read(&app.path)?
        .ok_or_else(|| format!("{} has no moonframe.toml", app.path.display()))?;
    println!("name\t{}", manifest.name);
    println!(
        "title\t{}",
        manifest
            .title
            .clone()
            .unwrap_or_else(|| manifest.name.clone())
    );
    println!("app\t{}", app.path.display());
    if let Some(version) = &manifest.version {
        println!("version\t{version}");
    }
    let icon = app
        .path
        .join(manifest.icon.as_deref().unwrap_or("icon.png"));
    if icon.is_file() {
        println!("icon\t{}", icon.display());
    }
    for package in packages {
        println!("package\t{}", package.display());
    }
    Ok(())
}

fn existing(user: &Path, name: &str) -> Result<PathBuf, String> {
    let profile = paths::profile(user, name);
    if !packages::valid_name(name) || !profile.is_dir() {
        return Err(format!("no profile `{name}`; see `moonframe app list`"));
    }
    Ok(profile)
}

/// Starts an app in the new folder `folder`: a git repository with a
/// manifest, a shell, and one mod, ready to link and push.
fn create(folder: &Path) -> Result<(), String> {
    let name = folder
        .file_name()
        .and_then(|name| name.to_str())
        .map(moonframe_core::mod_name)
        .ok_or_else(|| format!("{} has no name", folder.display()))?;
    if fs::read_dir(folder).is_ok_and(|mut entries| entries.next().is_some()) {
        return Err(format!(
            "{} already exists and is not empty",
            folder.display()
        ));
    }
    let write = |file: &str, text: &str| {
        let path = folder.join(file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::write(&path, text.replace("{name}", &name))
            .map_err(|error| format!("{}: {error}", path.display()))
    };
    write(packages::MANIFEST, MANIFEST)?;
    write("main.luau", SHELL)?;
    write("mods/home/main.luau", HOME)?;
    write("style.luau", STYLE)?;
    write(
        "README.md",
        &README.replace("{repository}", env!("CARGO_PKG_REPOSITORY")),
    )?;
    packages::git_init(folder)?;
    let shown = folder.display();
    println!("Started the app {name} in {shown}: moonframe.toml, main.luau, mods/home.");
    println!();
    println!("  moonframe app link {shown}   make it a profile, edited live");
    println!("  moonframe --profile {name}        open it");
    println!();
    println!("Then push it, and anyone can: moonframe app install github.com/<you>/{name}");
    Ok(())
}

const MANIFEST: &str = r#"[app]
# The profile it installs as: `moonframe --profile {name}` opens it.
name = "{name}"
# Built-in mods it shows, such as "terminal"; it brings every other.
keep = []
# Packages it installs with it, as `moonframe mod install` takes them.
requires = []
# Its version: its bundle's, and what Check for Updates compares with.
version = "0.1.0"
# The GitHub repository it is released from: the app menu gets Check for
# Updates. Release with `just release-app {name}`; see docs/updates.md.
# updates = "you/{name}"
"#;

const SHELL: &str = r#"-- {name}'s shell: lays out the whole window. Mods fill the regions it
-- draws; the built-in providers (git, files, agents, system) and
-- components are there to use.
return mod {
	init = function(previous)
		return previous or { left = true }
	end,

	update = function(state, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		end
	end,

	view = function(state)
		return ui.column {
			grow = true,
			gap = "none",
			ui.hsplit {
				ui.pane { size = 260, min = 180, max = 480, fixed = true, visible = state.left, surface = "sidebar", ui.region "left" },
				ui.pane { ui.region "main" },
				ui.pane { size = 320, min = 200, max = 560, fixed = true, surface = "sidebar", ui.region "right" },
			},
		}
	end,
}
"#;

const HOME: &str = r#"-- The first mod of {name}: it fills the main region.
return mod {
	region = "main",
	title = "{name}",

	view = function()
		return ui.panel {
			title = "{name}",
			ui.text "Lay out the window in main.luau, and add mods in mods/.",
		}
	end,
}
"#;

const STYLE: &str = "-- {name}'s styles, merged over the built-in stylesheet.
return style {}
";

const README: &str = "# {name}

An app for [Moonframe]({repository}).

## Install

```sh
moonframe app install github.com/<owner>/{name}
moonframe --profile {name}
```

## Develop

```sh
moonframe app link .          # a profile running this folder in place
moonframe --profile {name}    # the window shows your edits as you save
```
";
