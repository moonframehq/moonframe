//! **Build App Bundle** in the app menu (outside a release): builds the app
//! this window runs as a standalone release bundle, as `just bundle-app`
//! does, with `scripts/bundle-app.sh` from the source checkout the app runs
//! from, then shows it in Finder. A profile builds its own app; the starter
//! builds Moonframe.app.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// What to build, and from where.
#[derive(Clone, Debug)]
pub(crate) struct Build {
    /// The source checkout: the folder holding `scripts/bundle-app.sh`.
    pub(crate) checkout: PathBuf,
    pub(crate) profile: Option<String>,
    /// The user's folder, where the profile is found.
    pub(crate) user: Option<PathBuf>,
}

/// The script, from the checkout's root.
const SCRIPT: &str = "scripts/bundle-app.sh";

/// Where bundles go, from the checkout's root.
const OUT: &str = "target/bundle";

/// The checkout the built-in app `builtin` comes from, if it runs from
/// one: an installed bundle has no sources to build from.
pub(crate) fn checkout(builtin: &Path) -> Option<PathBuf> {
    let root = builtin.parent()?;
    root.join(SCRIPT).is_file().then(|| root.to_owned())
}

impl Build {
    /// Runs the build to the end: the bundle it made, or why it failed (the
    /// end of what the script printed). Blocks for as long as cargo takes.
    pub(crate) fn run(&self) -> Result<PathBuf, String> {
        let mut command = Command::new("bash");
        command
            .arg(SCRIPT)
            .arg(OUT)
            .args(self.profile.as_deref())
            .current_dir(&self.checkout);
        if let Some(user) = &self.user {
            command.env("MOONFRAME_USER", user);
        }
        tracing::info!("building the app bundle: {command:?}");
        let output = command
            .output()
            .map_err(|error| format!("cannot run {SCRIPT}: {error}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("{SCRIPT} failed:\n{stdout}{stderr}");
            let last = stderr
                .lines()
                .chain(stdout.lines())
                .rfind(|line| !line.trim().is_empty())
                .unwrap_or("it failed");
            return Err(last.trim().to_owned());
        }
        // The script prints the bundle's path last.
        let bundle = stdout
            .lines()
            .rfind(|line| !line.trim().is_empty())
            .map(|line| self.checkout.join(line.trim()))
            .ok_or_else(|| format!("{SCRIPT} printed no bundle"))?;
        tracing::info!("built {}", bundle.display());
        Ok(bundle)
    }
}

/// Shows `bundle` selected in Finder (on Linux, opens its folder).
pub(crate) fn reveal(bundle: &Path) {
    let result = if cfg!(target_os = "macos") {
        Command::new("open").arg("-R").arg(bundle).status()
    } else {
        Command::new("xdg-open").arg(bundle).status()
    };
    if let Err(error) = result {
        tracing::warn!("cannot show {}: {error}", bundle.display());
    }
}
