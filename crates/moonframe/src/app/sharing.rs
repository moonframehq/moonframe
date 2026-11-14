//! `app:share`: the system's share menu for what a mod offers, at the
//! pointer, and the service the user picked back to the mod.

use {
    super::*,
    crate::share::{self, Items},
    moonframe_core::Share,
};

impl MoonframeApp {
    /// Shows the share menu the mod `from` asked for. Its files must be
    /// the app's own (its folders, its cache and state) or the user's
    /// choice for that mod (`app:pick`, a drop); anything else is refused.
    pub(super) fn share_request(
        &mut self,
        from: &str,
        request: Share,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.built_with(from, cfg!(feature = "share"), "share") {
            return;
        }
        let shown = request
            .files
            .iter()
            .map(|src| self.shared_file(from, src))
            .collect::<Result<Vec<_>, _>>()
            .and_then(|files| Ok((files, share::anchor(window)?)));
        let (files, anchor) = match shown {
            Ok(shown) => shown,
            Err(problem) => {
                self.workspace
                    .refuse(from, Error::new(format!("`app:share`: {problem}")));
                return;
            },
        };
        let items = Items {
            url: request.url.clone(),
            text: request.text.clone(),
            files,
        };
        let from = from.to_owned();
        // The picker may track like a menu: never inside an update.
        cx.spawn(async move |this, cx| {
            let picked = share::show(&anchor, items).await.ok().flatten();
            if let Some(answer) = request.answer(picked) {
                let _ = this.update(cx, |app, cx| app.deliver(&from, answer, cx));
            }
        })
        .detach();
    }

    /// The file `src` names for the mod `from`, if it may share it.
    fn shared_file(&self, from: &str, src: &str) -> Result<PathBuf, String> {
        let mut allowed: Vec<PathBuf> = self
            .workspace
            .roots()
            .layers()
            .map(Path::to_owned)
            .collect();
        allowed.push(paths::cache());
        allowed.extend(
            paths::state(self.folders.profile.as_deref())
                .parent()
                .map(Path::to_owned),
        );
        let home = std::env::home_dir();
        shareable(
            src,
            self.mod_folder(from),
            home.as_deref(),
            &allowed,
            |real| crate::render::images::is_granted(from, real),
        )
    }
}

/// The file `src` names for a mod in `folder`, if it may share it: one in
/// `allowed` (the app's folders, its cache and state), or one the user
/// gave the mod (`granted`).
fn shareable(
    src: &str,
    folder: &Path,
    home: Option<&Path>,
    allowed: &[PathBuf],
    granted: impl Fn(&Path) -> bool,
) -> Result<PathBuf, String> {
    if let Some(file) = moonframe_core::resolve_file(src, folder, home, allowed, u64::MAX) {
        return Ok(file);
    }
    let path = match (home, Path::new(src).strip_prefix("~")) {
        (Some(home), Ok(rest)) => home.join(rest),
        _ => folder.join(src),
    };
    path.canonicalize()
        .ok()
        .filter(|real| real.is_file() && granted(real))
        .ok_or_else(|| {
            format!(
                "`{src}` is no file in the app's folders, its cache or state, or one the user \
                 picked for this mod"
            )
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::shareable;

    #[test]
    fn a_mod_shares_the_apps_files_and_what_the_user_gave_it() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(app.join("mods/news")).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(app.join("article.pdf"), "pdf").unwrap();
        std::fs::write(elsewhere.join("secret.txt"), "no").unwrap();
        std::fs::write(elsewhere.join("picked.txt"), "yes").unwrap();
        let folder = app.join("mods/news");
        let allowed = [app.clone()];
        let picked = elsewhere.join("picked.txt").canonicalize().unwrap();
        let share = |src: &str| {
            shareable(src, &folder, Some(dir.path()), &allowed, |real| {
                real == picked
            })
        };
        assert!(share("../../article.pdf").is_ok(), "the app's own");
        assert!(share(&app.join("article.pdf").to_string_lossy()).is_ok());
        let problem = share(&elsewhere.join("secret.txt").to_string_lossy()).unwrap_err();
        assert!(
            problem.contains("no file in the app's folders"),
            "{problem}"
        );
        assert!(
            share("../../../elsewhere/secret.txt").is_err(),
            "no way out"
        );
        assert!(share("~/elsewhere/picked.txt").is_ok(), "the user's pick");
        assert!(share("../../missing.pdf").is_err());
        assert!(share("../..").is_err(), "a folder is no file");
    }
}
