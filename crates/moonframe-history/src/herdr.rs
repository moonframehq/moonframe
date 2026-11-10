//! Herdr, when it is about: a folder kept in a git checkout (a dotfiles
//! repository, a linked app's source) gets a Herdr checkpoint before the
//! history writes into it, so Herdr UI's checkpoints, beside its agents'
//! turns, can bring back what an undo or restore replaced.
//!
//! Moonframe's history works the same without it; this only adds to it.
//! A checkpoint that cannot be taken is logged and the restore goes on.

use {
    herdr_vcs::checkpoint::{Checkout, direct},
    std::path::Path,
};

/// Takes a Herdr checkpoint of the checkout `folder` is in, labelled for
/// `why`, when Herdr is about and there is such a checkout.
pub(crate) fn checkpoint(folder: &Path, why: &str) {
    if !present() {
        return;
    }
    let Some(checkout) = checkout(folder) else {
        return;
    };
    let label = format!("Moonframe: before {why}");
    match direct::create(&checkout, &label, direct::here) {
        Ok(Some(id)) => tracing::info!("took Herdr checkpoint {id} of {}", checkout.repo_key),
        Ok(None) => {},
        Err(error) => tracing::warn!("no Herdr checkpoint before the {why}: {error}"),
    }
}

/// Herdr runs this process's terminal, or is installed.
fn present() -> bool {
    std::env::var_os("HERDR_ENV").is_some_and(|value| !value.is_empty())
        || (cfg!(target_os = "macos") && Path::new("/Applications/Herdr.app").is_dir())
}

/// The checkout `folder` is in, on its branch, as Herdr names it.
fn checkout(folder: &Path) -> Option<Checkout> {
    let ask = |args: &[&str]| {
        let output = crate::git::plain(folder).args(args).output().ok()?;
        let text = String::from_utf8(output.stdout).ok()?;
        output
            .status
            .success()
            .then(|| text.trim().to_owned())
            .filter(|text| !text.is_empty())
    };
    let key = ask(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
    let branch = ask(&["symbolic-ref", "--short", "-q", "HEAD"])?;
    Checkout::new(herdr_usage::Host::Local, &key, Some(&branch))
}
