//! A `ui.document`: a box styled by the `document` rule, which the sealed
//! page's view fills (see `crate::browser::sealed`). Its HTML is its own,
//! or a file's, named and allowed as `ui.image` names and allows pictures.
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
        time::SystemTime,
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
                    workspace.roots().layers().map(Path::to_owned).collect()
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

thread_local! {
    /// Each file read, with the length and time it had: a file written again
    /// is read again, and one drawn each frame is read once.
    static FILES: RefCell<HashMap<PathBuf, (u64, Option<SystemTime>, Rc<str>)>> =
        RefCell::new(HashMap::new());
}

/// The HTML in the file at `path`, up to [`MAX_HTML`] bytes.
fn read(path: &Path) -> Result<Rc<str>, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_HTML as u64 {
        return Err(format!("larger than {MAX_HTML} bytes"));
    }
    let stamp = (metadata.len(), metadata.modified().ok());
    if let Some(html) = FILES.with_borrow(|files| {
        files
            .get(path)
            .filter(|(len, at, _)| (*len, *at) == stamp)
            .map(|(.., html)| html.clone())
    }) {
        return Ok(html);
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let html: Rc<str> = String::from_utf8_lossy(&bytes).into();
    FILES.with_borrow_mut(|files| {
        files.insert(path.to_owned(), (stamp.0, stamp.1, html.clone()));
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
    fn a_sealed_document_starts_with_its_policy() {
        let html = sealed::sealed("<script>alert(1)</script><p>acme</p>");
        let policy = html.find("Content-Security-Policy").unwrap();
        assert!(policy < html.find("<script>").unwrap());
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("default-src 'none'"));
    }
}
