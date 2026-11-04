//! A yellow bar along the bottom of a dev build made from a branch other
//! than `main`, naming the branch, worktree and commit, so it is plain which
//! checkout a window runs. Release builds and `main` never show it. It names
//! the project being run: Moonframe's checkout for Moonframe itself, and for
//! an app on the library the app's own, read once at launch.

use {
    crate::{checkout::Checkout, launch::AppSource},
    gpui_kit::*,
    std::sync::OnceLock,
};

const BRANCH: &str = env!("MOONFRAME_BUILD_BRANCH");
const COMMIT: &str = env!("MOONFRAME_BUILD_COMMIT");
const WORKTREE: &str = env!("MOONFRAME_BUILD_WORKTREE");

/// A warning yellow, and the dark text on it.
const YELLOW: u32 = 0xf2c94c;
const INK: u32 = 0x2b2100;

/// What the bar says, set at launch, so drawing a window never runs git.
static LABEL: OnceLock<Option<String>> = OnceLock::new();

/// Decides the bar for this run; `host` is the app's folder, for an app on
/// the library.
pub(crate) fn init(host: Option<&AppSource>) {
    if cfg!(debug_assertions) {
        let _ = LABEL.set(text(&project(host)));
    }
}

/// What the bar says, when this build should have one.
pub(crate) fn label() -> Option<String> {
    LABEL.get().cloned().flatten()
}

/// The checkout of the project being run. An app's sources hold its folder;
/// one with only embedded files runs from its `target` folder, in the same
/// checkout.
fn project(host: Option<&AppSource>) -> Checkout {
    let dir = match host {
        None => {
            return Checkout {
                branch: BRANCH.to_owned(),
                commit: COMMIT.to_owned(),
                worktree: WORKTREE.to_owned(),
            };
        },
        Some(AppSource::Dir(dir)) => Some(dir.clone()),
        Some(AppSource::Embedded(_)) => std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(ToOwned::to_owned)),
    };
    dir.map(|dir| Checkout::of(&dir)).unwrap_or_default()
}

/// The bar's text for `checkout`: none on `main`, or outside git.
fn text(checkout: &Checkout) -> Option<String> {
    let Checkout {
        branch,
        commit,
        worktree,
    } = checkout;
    if matches!(branch.as_str(), "main" | "master") || (branch.is_empty() && commit.is_empty()) {
        return None;
    }
    let branch = if branch.is_empty() {
        "detached HEAD"
    } else {
        branch
    };
    let mut parts = vec![branch.to_owned()];
    if !worktree.is_empty() {
        parts.push(format!("worktree {worktree}"));
    }
    if !commit.is_empty() {
        parts.push(commit.clone());
    }
    Some(parts.join(" · "))
}

pub(crate) fn render(label: String) -> AnyElement {
    div()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_0p5()
        .bg(rgb(YELLOW))
        .text_color(rgb(INK))
        .text_xs()
        .child(div().font_weight(FontWeight::MEDIUM).child(label))
        .child("dev build")
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use {
        super::text,
        crate::checkout::{Checkout, git},
        std::path::Path,
    };

    fn commit(dir: &Path) {
        git(dir, &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "--message",
            "first",
        ]);
    }

    #[test]
    fn the_bar_names_a_projects_branch_but_not_its_main() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let repo = root.path().join("project");
        std::fs::create_dir_all(repo.join("app"))?;
        assert_eq!(text(&Checkout::of(&repo)), None, "outside git");

        git(&repo, &["init", "--initial-branch", "main"]);
        commit(&repo);
        assert_eq!(text(&Checkout::of(&repo.join("app"))), None, "on main");

        git(&repo, &["switch", "--create", "feature"]);
        let hash = git(&repo, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
        assert_eq!(
            text(&Checkout::of(&repo.join("app"))),
            Some(format!("feature · {hash}"))
        );

        git(&repo, &["switch", "--detach"]);
        assert_eq!(
            text(&Checkout::of(&repo)),
            Some(format!("detached HEAD · {hash}"))
        );

        let linked = root.path().join("linked");
        let path = linked.to_string_lossy();
        git(&repo, &["worktree", "add", "-b", "side", &path]);
        assert_eq!(
            text(&Checkout::of(&linked)),
            Some(format!("side · worktree linked · {hash}"))
        );
        Ok(())
    }
}
