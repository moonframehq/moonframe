//! `moonframe mod new`: starting a package to publish.

use {
    super::git::git,
    moonframe_core::mod_name,
    std::{fs, path::Path},
};

/// Starts a package in the new folder `folder`: a git repository holding
/// one mod, ready to edit, link, and push.
pub(crate) fn create(folder: &Path) -> Result<String, String> {
    let name = folder
        .file_name()
        .and_then(|name| name.to_str())
        .map(mod_name)
        .ok_or_else(|| format!("{} has no name", folder.display()))?;
    if folder.exists()
        && fs::read_dir(folder)
            .map_err(|error| format!("{}: {error}", folder.display()))?
            .next()
            .is_some()
    {
        return Err(format!(
            "{} already exists and is not empty",
            folder.display()
        ));
    }
    fs::create_dir_all(folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    let write = |file: &str, text: String| {
        fs::write(folder.join(file), text).map_err(|error| format!("{file}: {error}"))
    };
    write("main.luau", MAIN.replace("{name}", &name))?;
    write(
        "README.md",
        README
            .replace("{name}", &name)
            .replace("{repository}", env!("CARGO_PKG_REPOSITORY")),
    )?;
    git_init(folder)?;
    Ok(name)
}

/// Makes `folder` a new git repository.
pub fn git_init(folder: &Path) -> Result<(), String> {
    git(folder, &["init", "--quiet"]).map(drop)
}

const MAIN: &str = r#"-- {name}: a mod for moonframe. `region` picks where the shell draws it:
-- left, right, main, bottom, status, or status-right.
return mod {
	region = "right",
	title = "{name}",

	init = function(previous)
		return previous or { count = 0 }
	end,

	update = function(state, msg)
		if msg == "add" then
			state.count += 1
		end
	end,

	view = function(state)
		return ui.panel {
			title = "{name}",
			ui.text(`Clicked {state.count} times`),
			ui.button { "Add", msg = "add" },
		}
	end,
}
"#;

/// A new package's README. `{repository}` is the app's own, from the
/// workspace's `repository` in Cargo.toml, so a fork links to itself.
const README: &str = "# {name}

A mod for [Moonframe]({repository}).

## Install

```sh
moonframe mod install github.com/<owner>/{name}
```

## Develop

```sh
moonframe mod link .   # the window shows your edits as you save
```
";
