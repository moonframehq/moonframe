//! `app:pdf`: a page made into a PDF under the app's own folders, and the
//! file's path (or why not) back to the mod.

use {
    super::*,
    crate::pdf::{self, Source},
    moonframe_core::{Pdf, PdfSource},
};

/// Largest `.html` file read for a PDF.
const MAX_HTML_FILE: u64 = 64 * 1024 * 1024;

impl MoonframeApp {
    /// Makes the PDF the mod `from` asked for. Its files must be under the
    /// app's cache or state folders; the PDF is written beside where it
    /// goes, then put in place, so a half-written one is never seen.
    pub(super) fn pdf_request(&mut self, from: &str, request: Pdf, cx: &mut Context<Self>) {
        let roots = self.own_data_folders();
        let started = (|| {
            let out = pdf_path(&request.pdf, &roots)?;
            let source = match &request.source {
                PdfSource::Html(html) => Source::Html(html.clone()),
                PdfSource::Url(url) => Source::Url(url.clone()),
                PdfSource::HtmlFile(file) => Source::HtmlFile(html_file(file, &roots)?),
            };
            let partial = partial_path(&out);
            let job = pdf::render(source, request.paper, partial.clone())?;
            Ok::<_, String>((job, partial, out))
        })();
        let (job, partial, out) = match started {
            Ok(started) => started,
            Err(problem) => {
                self.workspace
                    .refuse(from, Error::new(format!("`app:pdf`: {problem}")));
                return;
            },
        };
        let from = from.to_owned();
        cx.spawn(async move |this, cx| {
            let timeout = cx.background_executor().timer(pdf::TIMEOUT);
            let (done, still_printing) =
                match futures::future::select(job.done, Box::pin(timeout)).await {
                    futures::future::Either::Left((done, _)) => (
                        done.unwrap_or_else(|_| Err("the PDF was given up".to_owned())),
                        None,
                    ),
                    futures::future::Either::Right((_, done)) => (
                        Err(format!(
                            "the page took longer than {} s",
                            pdf::TIMEOUT.as_secs()
                        )),
                        Some(done),
                    ),
                };
            pdf::finish(job.id);
            let written = done.and_then(|()| {
                std::fs::rename(&partial, &out)
                    .map(|()| out.to_string_lossy().into_owned())
                    .map_err(|error| format!("cannot write {}: {error}", out.display()))
            });
            if written.is_err() {
                let _ = std::fs::remove_file(&partial);
            }
            if let Err(problem) = &written {
                tracing::warn!("{from}: `app:pdf`: {problem}");
            }
            if let Some(answer) = request.answer(written) {
                let _ = this.update(cx, |app, cx| app.deliver(&from, answer, cx));
            }
            // One given up mid-print is let go once its print is over, and
            // what that wrote with it.
            if let Some(done) = still_printing {
                let _ = done.await;
                pdf::finish(job.id);
                let _ = std::fs::remove_file(&partial);
            }
        })
        .detach();
    }

    /// The app's cache folder, then its state folder: where it keeps what
    /// it makes, and where a mod's PDFs and their pages may be.
    fn own_data_folders(&self) -> Vec<PathBuf> {
        let mut roots = vec![paths::cache()];
        roots.extend(
            paths::state(self.folders.profile.as_deref())
                .parent()
                .map(Path::to_owned),
        );
        roots
    }
}

/// `name` made a path under the first of `roots` when relative, else as
/// written (`~` the home folder); never with `..`.
fn under(name: &str, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let path = Path::new(name);
    if path
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(format!("`{name}` may not go up a folder (..)"));
    }
    let home = std::env::home_dir();
    Ok(match (path.strip_prefix("~"), home) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ if path.is_absolute() => path.to_owned(),
        _ => roots
            .first()
            .ok_or("the app has no cache folder")?
            .join(path),
    })
}

/// Whether `real` (canonical) lies in one of `roots`.
fn inside(real: &Path, roots: &[PathBuf]) -> bool {
    roots
        .iter()
        .filter_map(|root| root.canonicalize().ok())
        .any(|root| real.starts_with(root))
}

/// Where the PDF `name` goes: under the app's cache or state folders, its
/// folder made if need be, and not a folder itself.
fn pdf_path(name: &str, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let path = under(name, roots)?;
    let refuse = || format!("`{name}` is not under the app's cache or state folders");
    let (Some(folder), Some(file)) = (path.parent(), path.file_name()) else {
        return Err(refuse());
    };
    let Some(root) = roots.iter().find(|root| folder.starts_with(root)) else {
        return Err(refuse());
    };
    std::fs::create_dir_all(root)
        .map_err(|error| format!("cannot make {}: {error}", root.display()))?;
    // Only made inside them: the deepest folder already there, links
    // followed, must be inside before the rest are made, so none is ever
    // made outside through a link.
    let existing = folder
        .ancestors()
        .find(|folder| folder.symlink_metadata().is_ok())
        .unwrap_or(root);
    let real = existing.canonicalize().map_err(|_| refuse())?;
    if !inside(&real, roots) {
        return Err(refuse());
    }
    std::fs::create_dir_all(folder)
        .map_err(|error| format!("cannot make {}: {error}", folder.display()))?;
    let folder = folder.canonicalize().map_err(|error| error.to_string())?;
    if !inside(&folder, roots) {
        return Err(refuse());
    }
    let out = folder.join(file);
    if out.is_dir() {
        return Err(format!("`{name}` is a folder"));
    }
    Ok(out)
}

/// The `.html` file `name` names, if it is the app's.
fn html_file(name: &str, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let real = under(name, roots)?
        .canonicalize()
        .map_err(|_| format!("no file `{name}`"))?;
    let size = real
        .metadata()
        .map(|meta| meta.is_file().then_some(meta.len()));
    match size {
        Ok(Some(size)) if size <= MAX_HTML_FILE && inside(&real, roots) => Ok(real),
        Ok(Some(size)) if size > MAX_HTML_FILE => Err(format!("`{name}` is over 64 MiB")),
        _ => Err(format!(
            "`{name}` is no file under the app's cache or state folders"
        )),
    }
}

/// Where the PDF is printed before it is put in place: beside it, hidden,
/// and named for this request alone, so two making the same PDF never
/// write one file.
fn partial_path(out: &Path) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let name = out
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let request = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    out.with_file_name(format!(".{name}.{}.{request}.partial", std::process::id()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {
        super::{html_file, partial_path, pdf_path},
        std::path::PathBuf,
    };

    #[test]
    fn a_pdf_and_its_page_are_the_apps_own() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let state = dir.path().join("state");
        let elsewhere = dir.path().join("elsewhere");
        for folder in [&cache, &state, &elsewhere] {
            std::fs::create_dir_all(folder).unwrap();
        }
        let roots = [cache.clone(), state.clone()];
        let out = pdf_path("articles/a.pdf", &roots).unwrap();
        assert_eq!(out, cache.canonicalize().unwrap().join("articles/a.pdf"));
        assert!(cache.join("articles").is_dir(), "its folder is made");
        let in_state = state.join("b.pdf");
        assert!(pdf_path(&in_state.to_string_lossy(), &roots).is_ok());
        for bad in [
            "../escape.pdf".to_owned(),
            elsewhere.join("c.pdf").to_string_lossy().into_owned(),
            "/etc/x.pdf".to_owned(),
        ] {
            assert!(pdf_path(&bad, &roots).is_err(), "{bad}");
        }
        assert!(!elsewhere.join("c.pdf").exists());
        // A link out of the cache is followed and refused, and no folder is
        // made through it.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&elsewhere, cache.join("out")).unwrap();
            assert!(pdf_path("out/d.pdf", &roots).is_err());
            assert!(pdf_path("out/new/deeper/d.pdf", &roots).is_err());
            assert!(!elsewhere.join("new").exists(), "nothing made outside");
            std::os::unix::fs::symlink(dir.path().join("gone"), cache.join("dangling")).unwrap();
            assert!(pdf_path("dangling/e.pdf", &roots).is_err());
            assert!(!dir.path().join("gone").exists());
        }
        // The cache itself is made if it is not there yet.
        let fresh = [dir.path().join("fresh/cache")];
        assert!(pdf_path("f.pdf", &fresh).is_ok());
        assert!(fresh[0].is_dir());
        std::fs::create_dir_all(cache.join("dir.pdf")).unwrap();
        assert!(pdf_path("dir.pdf", &roots).is_err(), "a folder");

        std::fs::write(cache.join("page.html"), "<p>hi</p>").unwrap();
        std::fs::write(elsewhere.join("page.html"), "<p>no</p>").unwrap();
        assert!(html_file("page.html", &roots).is_ok());
        assert!(html_file(&elsewhere.join("page.html").to_string_lossy(), &roots).is_err());
        assert!(html_file("missing.html", &roots).is_err());

        let partial = partial_path(&PathBuf::from("/c/articles/a.pdf"));
        assert!(partial.starts_with("/c/articles"));
        assert!(
            partial
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".a.pdf.")
        );
        assert_ne!(
            partial,
            partial_path(&PathBuf::from("/c/articles/a.pdf")),
            "each request prints to a file of its own"
        );
    }
}
