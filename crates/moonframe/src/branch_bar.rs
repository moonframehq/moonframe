//! A yellow bar along the bottom of a dev build made from a branch other
//! than `main`, naming the branch, worktree and commit, so it is plain which
//! checkout a window runs. Release builds and `main` never show it. In an
//! app on the library it names Moonframe's checkout, and only a branch of
//! one: a git dependency's checkout is detached, and says nothing.

use gpui_kit::*;

const BRANCH: &str = env!("MOONFRAME_BUILD_BRANCH");
const COMMIT: &str = env!("MOONFRAME_BUILD_COMMIT");
const WORKTREE: &str = env!("MOONFRAME_BUILD_WORKTREE");

/// A warning yellow, and the dark text on it.
const YELLOW: u32 = 0xf2c94c;
const INK: u32 = 0x2b2100;

/// What the bar says, when this build should have one.
pub(crate) fn label() -> Option<String> {
    let on_main = matches!(BRANCH, "main" | "master");
    let hosted = crate::identity().hosted;
    if !cfg!(debug_assertions) || on_main || (hosted && BRANCH.is_empty()) {
        return None;
    }
    let branch = if BRANCH.is_empty() {
        "detached HEAD"
    } else {
        BRANCH
    };
    let mut parts = vec![if hosted {
        format!("moonframe {branch}")
    } else {
        branch.to_owned()
    }];
    if !WORKTREE.is_empty() {
        parts.push(format!("worktree {WORKTREE}"));
    }
    if !COMMIT.is_empty() {
        parts.push(COMMIT.to_owned());
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
