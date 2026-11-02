//! `app:pick`: the system's open, folder and save panels.

use {
    super::*,
    moonframe_core::{Pick, PickKind},
};

impl MoonframeApp {
    /// Opens the panel the mod `from` asked for; its answer goes back to
    /// that mod as `pick.reply`. What the user picks may then show in
    /// `ui.image`, since they chose it.
    pub(super) fn pick(&mut self, from: &str, pick: Pick, cx: &mut Context<Self>) {
        let from = from.to_owned();
        let save = match pick.kind {
            PickKind::Save => {
                let folder = pick
                    .folder
                    .as_deref()
                    .map(expand)
                    .or_else(std::env::home_dir)
                    .unwrap_or_else(|| PathBuf::from("."));
                Some(cx.prompt_for_new_path(&folder, pick.name.as_deref()))
            },
            PickKind::Open | PickKind::Folder => None,
        };
        let open = save.is_none().then(|| {
            cx.prompt_for_paths(PathPromptOptions {
                files: pick.kind == PickKind::Open,
                directories: pick.kind == PickKind::Folder,
                multiple: pick.multiple,
                prompt: pick.prompt.clone().map(SharedString::from),
            })
        });
        cx.spawn(async move |this, cx| {
            let picked = match (save, open) {
                (Some(save), _) => save
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .flatten()
                    .map(|path| vec![path]),
                (None, Some(open)) => open.await.ok().and_then(Result::ok).flatten(),
                (None, None) => None,
            };
            if pick.kind != PickKind::Save
                && let Some(paths) = &picked
            {
                crate::render::images::grant(paths);
            }
            let answer = pick.answer(picked);
            let _ = this.update(cx, |app, cx| app.deliver(&from, answer, cx));
        })
        .detach();
    }
}

/// `~` and `~/…` are the home folder.
fn expand(folder: &str) -> PathBuf {
    match (folder.strip_prefix('~'), std::env::home_dir()) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            home.join(rest.trim_start_matches('/'))
        },
        _ => PathBuf::from(folder),
    }
}
