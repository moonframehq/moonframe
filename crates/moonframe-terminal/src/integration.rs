//! Teaches the user's shell to say what it is doing (see [`Signal`]): when
//! a command starts and ends, and its folder. Shells print none of this by
//! themselves, so a terminal starts with an environment that loads a small
//! script, as Ghostty, kitty and VS Code do.
//!
//! - **zsh**: `ZDOTDIR` leads to a `.zshenv` that puts it back, then loads
//!   the user's files as usual.
//! - **fish**: a `vendor_conf.d` script, found through `XDG_DATA_DIRS`.
//! - **bash**: cannot be reached this way; one line in `~/.bashrc` does it.
//!
//! `MOONFRAME_SHELL_INTEGRATION=0` in the app's environment turns it off.
//!
//! [`Signal`]: crate::Signal

use std::{ffi::OsStr, fs, io, path::Path};

/// Where the scripts are, for the line in `~/.bashrc`, and the switch.
const FOLDER: &str = "MOONFRAME_SHELL_INTEGRATION";

/// Each script, by the path it has under the folder of [`install`].
const SCRIPTS: [(&str, &str); 3] = [
    ("bash/moonframe.bash", include_str!("shell/moonframe.bash")),
    (
        "fish/fish/vendor_conf.d/moonframe.fish",
        include_str!("shell/moonframe.fish"),
    ),
    ("zsh/.zshenv", include_str!("shell/zshenv")),
];

/// Writes the scripts under `folder`, and returns the environment a new
/// terminal starts with to load them; nothing when it is turned off.
///
/// Shells kept by tmux or herdr-core outlive the app, but read a script
/// only as they start, so replacing one never touches them.
pub fn install(folder: &Path) -> io::Result<Vec<(String, String)>> {
    let read = |name: &str| std::env::var(name).ok();
    if read(FOLDER).as_deref() == Some("0") {
        return Ok(Vec::new());
    }
    for (path, text) in SCRIPTS {
        let path = folder.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Whole or not at all: another window may be starting a shell.
        let partial = path.with_extension(format!("{}.partial", std::process::id()));
        fs::write(&partial, text)?;
        fs::rename(&partial, &path)?;
    }
    let shell = read("SHELL").unwrap_or_default();
    Ok(environment(&shell, folder, read))
}

/// The variables that load the scripts in `folder` into `shell`, given the
/// app's own environment.
pub(crate) fn environment(
    shell: &str,
    folder: &Path,
    read: impl Fn(&str) -> Option<String>,
) -> Vec<(String, String)> {
    let text = |path: &Path| path.to_string_lossy().into_owned();
    let mut variables = vec![(FOLDER.to_owned(), text(&folder.join("bash")))];
    let pair = |name: &str, value: String| (name.to_owned(), value);
    match Path::new(shell).file_name().and_then(OsStr::to_str) {
        Some("fish") => {
            let data = text(&folder.join("fish"));
            // Unset means these two, which setting it would lose.
            let rest = read("XDG_DATA_DIRS")
                .filter(|dirs| !dirs.is_empty())
                .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
            variables.push(pair("XDG_DATA_DIRS", format!("{data}:{rest}")));
            variables.push(pair("MOONFRAME_FISH_DATA", data));
        },
        Some("zsh") => {
            variables.push(pair("ZDOTDIR", text(&folder.join("zsh"))));
            if let Some(zdotdir) = read("ZDOTDIR") {
                variables.push(pair("MOONFRAME_ZDOTDIR", zdotdir));
            }
        },
        _ => {},
    }
    variables
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "integration/tests.rs"]
mod tests;
