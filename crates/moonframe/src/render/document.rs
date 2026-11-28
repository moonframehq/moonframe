//! A `ui.document`: a box styled by the `document` rule, which the sealed
//! page's view fills (see `crate::browser::sealed`). Its HTML is its own,
//! or a file's, named and allowed as `ui.image` names and allows pictures:
//! in the app's folders or the image cache, where a provider writes a page
//! too large to publish.
//! Previews and platforms without a web view say what it would show.

use {
    super::{Cx, Draw, Painter, Place, browser::PageElement, problem, styling::*},
    crate::browser::SUPPORTED,
    gpui_kit::{component::ActiveTheme as _, *},
    moonframe_core::{Document, DocumentSource, MAX_HTML, Part},
    std::{
        cell::RefCell,
        collections::HashMap,
        path::{Path, PathBuf},
        rc::Rc,
        time::{Instant, SystemTime},
    },
};

/// The least room a document asks for.
const MIN_HEIGHT: f32 = 80.;

impl<'w> Draw<'w> for Document {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        if !cfg!(feature = "browser") {
            return problem(
                "documents need browser support, left out of this build; enable `browser`".into(),
                cx,
            );
        }
        let look = painter.look(Part::Document, &self.look, false, place);
        let theme = cx.theme();
        let frame = div()
            .id(place.id())
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h(px(MIN_HEIGHT))
            .overflow_hidden()
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .padded(&look.base.padding);
        let html = match &self.source {
            DocumentSource::Html(html) => Ok(Rc::<str>::from(html.as_str())),
            DocumentSource::File(src) => {
                let folder = painter.folder(place.owner).unwrap_or(Path::new("."));
                let workspace = painter.workspace;
                super::images::resolve_src(src, place.owner, folder, || {
                    roots(workspace.roots().layers(), crate::paths::images())
                })
                .ok_or_else(|| format!("`{src}` is no file this document may show"))
                .and_then(|path| read(&path))
            },
        };
        let html = match html {
            Ok(html) => html,
            Err(error) => return problem(format!("document `{}`: {error}", self.id), cx),
        };
        let dark = theme.is_dark();
        let browsers = match painter.browsers.as_deref_mut() {
            Some(browsers) if SUPPORTED && !painter.preview => browsers,
            _ => return placeholder(frame, painter.preview, cx),
        };
        match browsers.open_document(place.owner, self, &html, dark) {
            Ok(page) => match page.failed() {
                Some(error) => problem(format!("document `{}`: {error}", self.id), cx),
                None => frame.child(PageElement { page }).into_any_element(),
            },
            Err(error) => problem(format!("document `{}`: {error}", self.id), cx),
        }
    }
}

/// The folders a document's `src` may name a file in, as `ui.image` takes
/// a picture from: the app's `layers` and the image `cache` providers write
/// files for views to (a briefing, a preview converted from a shared file).
/// A file the user gave the mod is allowed besides.
fn roots<'a>(layers: impl Iterator<Item = &'a Path>, cache: PathBuf) -> Vec<PathBuf> {
    layers.map(Path::to_owned).chain([cache]).collect()
}

/// What shows where the document would be.
fn placeholder(frame: Stateful<Div>, preview: bool, cx: Cx) -> AnyElement {
    let note = if preview || SUPPORTED {
        "a document goes here"
    } else {
        "this platform has no web view for a document"
    };
    frame
        .items_center()
        .justify_center()
        .p_4()
        .text_color(cx.theme().muted_foreground)
        .child(div().text_sm().child(note))
        .into_any_element()
}

/// The most the files kept take, in bytes: past it, those read longest ago
/// go (the documents showing them read them again).
const KEPT_BYTES: usize = 32 * 1024 * 1024;

/// A file read: the length and time it had, and when it was last asked for.
struct Kept {
    stamp: (u64, Option<SystemTime>),
    html: Rc<str>,
    used: Instant,
}

thread_local! {
    /// Each file read: a file written again is read again, and one drawn
    /// each frame is read once.
    static FILES: RefCell<HashMap<PathBuf, Kept>> = RefCell::new(HashMap::new());
}

/// The HTML in the file at `path`, up to [`MAX_HTML`] bytes.
fn read(path: &Path) -> Result<Rc<str>, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_HTML as u64 {
        return Err(format!("larger than {MAX_HTML} bytes"));
    }
    let stamp = (metadata.len(), metadata.modified().ok());
    let now = Instant::now();
    if let Some(html) = FILES.with_borrow_mut(|files| {
        let kept = files.get_mut(path).filter(|kept| kept.stamp == stamp)?;
        kept.used = now;
        Some(kept.html.clone())
    }) {
        return Ok(html);
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let html: Rc<str> = String::from_utf8_lossy(&bytes).into();
    FILES.with_borrow_mut(|files| {
        files.insert(path.to_owned(), Kept {
            stamp,
            html: html.clone(),
            used: now,
        });
        let mut total: usize = files.values().map(|kept| kept.html.len()).sum();
        while total > KEPT_BYTES {
            let Some(oldest) = files
                .iter()
                .min_by_key(|(_, kept)| kept.used)
                .map(|(path, _)| path.clone())
            else {
                break;
            };
            if let Some(gone) = files.remove(&oldest) {
                total -= gone.html.len();
            }
        }
    });
    Ok(html)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::read, crate::browser::sealed, std::rc::Rc};

    #[test]
    fn a_file_is_read_once_until_it_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("acme.html");
        std::fs::write(&path, "<p>one</p>").unwrap();
        let first = read(&path).unwrap();
        assert!(Rc::ptr_eq(&first, &read(&path).unwrap()));
        std::fs::write(&path, "<p>two, longer</p>").unwrap();
        assert_eq!(&*read(&path).unwrap(), "<p>two, longer</p>");
    }

    #[test]
    fn files_read_long_ago_go_once_too_many_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        // Five of 7 MB: past the 32 MB kept.
        let big = "a".repeat(7 * 1024 * 1024);
        let paths: Vec<_> = (0..5)
            .map(|index| {
                let path = dir.path().join(format!("{index}.html"));
                std::fs::write(&path, &big).unwrap();
                read(&path).unwrap();
                path
            })
            .collect();
        super::FILES.with_borrow(|files| {
            assert!(!files.contains_key(&paths[0]));
            assert!(files.contains_key(&paths[4]));
            let total: usize = files.values().map(|kept| kept.html.len()).sum();
            assert!(total <= super::KEPT_BYTES);
        });
    }

    #[test]
    fn a_document_names_a_file_in_the_apps_folders_or_the_image_cache() {
        let (app, cache, elsewhere) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        for dir in [&app, &cache, &elsewhere] {
            std::fs::write(dir.path().join("acme.html"), "<p>acme</p>").unwrap();
        }
        let allowed = super::roots([app.path()].into_iter(), cache.path().to_owned());
        let resolve = |src: &std::path::Path| {
            moonframe_core::resolve_file(
                &src.to_string_lossy(),
                app.path(),
                None,
                &allowed,
                moonframe_core::MAX_HTML as u64,
            )
        };
        assert!(resolve(std::path::Path::new("acme.html")).is_some());
        assert!(resolve(&cache.path().join("acme.html")).is_some());
        assert!(resolve(&elsewhere.path().join("acme.html")).is_none());
    }

    #[test]
    fn a_sealed_document_starts_with_its_policy() {
        let html = sealed::sealed("<script>alert(1)</script><p>acme</p>");
        let policy = html.find("Content-Security-Policy").unwrap();
        assert!(policy < html.find("<script>").unwrap());
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("default-src 'none'"));
    }
}
