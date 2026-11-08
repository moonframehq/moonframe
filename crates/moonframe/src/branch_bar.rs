//! A yellow bar along the bottom of a dev build made from a branch other
//! than `main`, naming the branch, worktree and commit, so it is plain which
//! checkout a window runs. Release builds and `main` never show it. It names
//! the project being run: Moonframe's checkout for Moonframe itself, and for
//! an app on the library the app's own, read once at launch.

use {
    crate::{checkout::Checkout, launch::AppSource},
    gpui_kit::*,
    std::{
        path::{Path, PathBuf},
        sync::OnceLock,
    },
};

const BRANCH: &str = env!("MOONFRAME_BUILD_BRANCH");
const COMMIT: &str = env!("MOONFRAME_BUILD_COMMIT");
const WORKTREE: &str = env!("MOONFRAME_BUILD_WORKTREE");

/// A warning yellow, and the dark text on it.
const YELLOW: u32 = 0xf2c94c;
const INK: u32 = 0x2b2100;

/// What the bar says, set at launch, so drawing a window never runs git.
static LABEL: OnceLock<Option<String>> = OnceLock::new();

/// Decides the bar for this run, after resolving the app's source folder.
pub(crate) fn init(hosted: bool, dir: Option<&Path>) {
    if cfg!(debug_assertions) {
        let _ = LABEL.set(text(&project(hosted, dir)));
    }
}

/// What the bar says, when this build should have one.
pub(crate) fn label() -> Option<String> {
    LABEL.get().cloned().flatten()
}

/// The selected app folder wins over the builder's default. Embedded files
/// have no source folder unless the host names its project explicitly:
/// neither the executable's location nor the materialized cache identifies it.
pub(crate) fn host_dir(
    source: &AppSource,
    selected: Option<&Path>,
    project: Option<&Path>,
) -> Option<PathBuf> {
    selected
        .or(match source {
            AppSource::Dir(dir) => Some(dir.as_path()),
            AppSource::Embedded(_) => project,
        })
        .map(Path::to_owned)
}

/// The starter keeps its build-time identity; a host reads its sources once.
fn project(hosted: bool, dir: Option<&Path>) -> Checkout {
    if hosted {
        return dir.map(Checkout::of).unwrap_or_default();
    }
    Checkout {
        branch: BRANCH.to_owned(),
        commit: COMMIT.to_owned(),
        worktree: WORKTREE.to_owned(),
    }
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
        super::{host_dir, project, text},
        crate::{
            checkout::{Checkout, git},
            launch::AppSource,
        },
        std::{path::Path, sync::Arc},
    };

    fn commit(dir: &Path) {
        git(
            dir,
            &[
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
            ],
        );
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

    #[test]
    fn a_selected_worktree_wins_over_the_default_app_and_project() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let repo = root.path().join("original");
        std::fs::create_dir_all(repo.join("app"))?;
        git(&repo, &["init", "--initial-branch", "main"]);
        commit(&repo);
        let source = AppSource::Dir(repo.join("app"));
        let label = |selected: Option<&Path>| {
            let dir = host_dir(&source, selected, Some(&repo));
            text(&project(true, dir.as_deref()))
        };
        assert_eq!(label(None), None);

        let linked = root.path().join("selected");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-b",
                "selected-feature",
                &linked.to_string_lossy(),
            ],
        );
        let app = linked.join("app");
        std::fs::create_dir_all(&app)?;
        let hash = git(&linked, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
        assert_eq!(
            label(Some(&app)),
            Some(format!("selected-feature · worktree selected · {hash}"))
        );
        // Selecting a folder outside git must not fall back to the builder.
        git(&repo, &["switch", "--create", "original-feature"]);
        assert_eq!(label(Some(root.path())), None);
        Ok(())
    }

    #[test]
    fn an_embedded_app_uses_its_explicit_project_not_the_executable() -> std::io::Result<()> {
        // This project is unrelated to the checkout containing the test binary.
        let repo = tempfile::tempdir()?;
        git(
            repo.path(),
            &["init", "--initial-branch", "embedded-feature"],
        );
        commit(repo.path());
        let source = AppSource::Embedded(Arc::new(|_| panic!("must not materialize files")));
        let label = |selected: Option<&Path>, project_dir: Option<&Path>| {
            let dir = host_dir(&source, selected, project_dir);
            text(&project(true, dir.as_deref()))
        };
        let hash = git(repo.path(), &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
        assert_eq!(
            label(None, Some(repo.path())),
            Some(format!("embedded-feature · {hash}"))
        );
        assert_eq!(label(None, None), None, "no executable-location fallback");

        let selected = tempfile::tempdir()?;
        git(selected.path(), &["init", "--initial-branch", "main"]);
        commit(selected.path());
        assert_eq!(label(Some(selected.path()), Some(repo.path())), None);
        git(repo.path(), &["switch", "--create", "main"]);
        assert_eq!(label(None, Some(repo.path())), None);
        Ok(())
    }
}
