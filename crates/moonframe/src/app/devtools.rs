//! The Developer tools: the `devtools` mod, docked on the right of the
//! main window (drawn around the shell, so no app's layout has to make
//! room) or in a window of its own, in Developer Mode only. It hears each
//! pick on `app/selection`, and asks the app for a mod's source and state
//! with `app:devtools`, which only it may send. A save is checked to be a
//! Luau file of the app's own folders, and goes to the user's copy when
//! the file ships in the signed app; the window then reloads as for any
//! change.

use {
    super::*,
    moonframe_core::{DEVTOOLS, Devtools, DevtoolsPlace, FEEDBACK, MAX_SOURCE, with_fields},
    std::collections::BTreeMap,
};

/// The dock's width when it first opens, and the range dragging keeps it in.
const WIDTH: f32 = 460.;
const MIN_WIDTH: f32 = 320.;
const MAX_WIDTH: f32 = 960.;
/// Width of the strip between the content and the dock that resizes it.
const HANDLE: f32 = 6.;

/// Where the tools show, and how wide the dock is.
pub(super) struct Tools {
    place: DevtoolsPlace,
    width: f32,
    /// While the dock's edge is dragged: where the pointer went down, and
    /// the width then.
    resizing: Option<(Pixels, f32)>,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            place: DevtoolsPlace::Closed,
            width: WIDTH,
            resizing: None,
        }
    }
}

impl MoonframeApp {
    /// Whether the dock shows in the main window.
    pub(crate) fn devtools_docked(&self) -> bool {
        self.devtools.place == DevtoolsPlace::Dock && self.devtools_available()
    }

    /// Whether the tools show anywhere, to take the inspector's picks.
    pub(crate) fn devtools_open(&self) -> bool {
        self.devtools.place != DevtoolsPlace::Closed && self.devtools_available()
    }

    fn devtools_available(&self) -> bool {
        !crate::release() && self.workspace.slot(DEVTOOLS).is_some()
    }

    /// View > Developer Tools: docks them, or closes them where they are.
    pub(crate) fn toggle_devtools(&mut self, cx: &mut Context<Self>) {
        if self.devtools_open() {
            self.place_devtools(DevtoolsPlace::Closed, cx);
        } else if !self.devtools_available() {
            self.show_toast("Developer tools need Developer Mode", cx);
        } else {
            self.place_devtools(DevtoolsPlace::Dock, cx);
        }
    }

    /// Moves the tools to `place`, and tells the mod where it now is, with
    /// the selection when there is one.
    pub(crate) fn place_devtools(&mut self, place: DevtoolsPlace, cx: &mut Context<Self>) {
        let was = std::mem::replace(&mut self.devtools.place, place);
        if was == DevtoolsPlace::Window && place != DevtoolsPlace::Window {
            self.window_request(DEVTOOLS, WindowRequest::Close(DEVTOOLS.to_owned()), cx);
        }
        if place == DevtoolsPlace::Window {
            self.show_mod_window(DEVTOOLS, None, cx);
        }
        // Agent sessions are the feedback mod's runs: without it, no Agent tab.
        let agent = self.workspace.slot(FEEDBACK).is_some();
        let fields = BTreeMap::from([
            ("agent".to_owned(), Data::Bool(agent)),
            ("place".to_owned(), Data::from(place.name())),
        ]);
        self.workspace
            .app_topic("devtools", DEVTOOLS, Data::Map(fields));
        if place != DevtoolsPlace::Closed
            && let Some(selected) = self.inspector.selected.clone()
        {
            self.tell_devtools(&selected);
        }
        cx.notify();
    }

    /// The tools' window was closed by the user: they are closed.
    pub(super) fn devtools_window_closed(&mut self, cx: &mut Context<Self>) {
        if self.devtools.place == DevtoolsPlace::Window {
            self.place_devtools(DevtoolsPlace::Closed, cx);
        }
    }

    /// Developer Mode went off: the tools go with it.
    pub(super) fn close_devtools(&mut self, cx: &mut Context<Self>) {
        if self.devtools.place != DevtoolsPlace::Closed {
            self.place_devtools(DevtoolsPlace::Closed, cx);
        }
    }

    /// Hands a pick to the tools, on `app/selection`: the element, its mod,
    /// file and line, the elements around it (outermost first, each with
    /// the path `{ select = path }` picks it by), its size, and the report
    /// agents read.
    pub(super) fn tell_devtools(&mut self, picked: &Placed) {
        if !self.devtools_open() {
            return;
        }
        let pick = inspect::pick(picked, &self.workspace);
        let (_, file, line) = inspect::source_of(picked, &self.workspace);
        let trail: Vec<Data> = picked
            .crumb
            .trail()
            .iter()
            .map(|crumb| {
                let mut fields =
                    BTreeMap::from([("label".to_owned(), Data::from(crumb.label.as_str()))]);
                if let Some(path) = &crumb.path {
                    fields.insert("path".to_owned(), Data::from(path.as_str()));
                }
                if let Some(line) = crumb.line {
                    fields.insert("line".to_owned(), Data::Integer(i64::from(line)));
                }
                Data::Map(fields)
            })
            .collect();
        let size = picked.bounds.size;
        let mut fields = BTreeMap::from([
            ("element".to_owned(), Data::from(pick.element)),
            ("mod".to_owned(), Data::from(pick.owner)),
            ("at".to_owned(), Data::from(pick.at)),
            ("report".to_owned(), Data::from(pick.report)),
            ("path".to_owned(), Data::from(picked.path.as_str())),
            ("trail".to_owned(), Data::List(trail)),
            (
                "width".to_owned(),
                Data::Number(f64::from(f32::from(size.width)).round()),
            ),
            (
                "height".to_owned(),
                Data::Number(f64::from(f32::from(size.height)).round()),
            ),
        ]);
        if let Some(file) = file {
            fields.insert("file".to_owned(), Data::from(file.display().to_string()));
        }
        if let Some(line) = line {
            fields.insert("line".to_owned(), Data::Integer(i64::from(line)));
        }
        self.workspace
            .app_topic("selection", DEVTOOLS, Data::Map(fields));
    }

    /// Does what the tools asked with `app:devtools`; no other mod may.
    pub(super) fn devtools_request(
        &mut self,
        from: &str,
        request: Devtools,
        cx: &mut Context<Self>,
    ) {
        if from != DEVTOOLS || !self.devtools_trusted() {
            self.workspace.refuse(
                from,
                Error::new(format!(
                    "`app:devtools` is for the Developer tools (mods/{DEVTOOLS}) only"
                )),
            );
            return;
        }
        if crate::release() {
            tracing::warn!("`app:devtools` asked outside Developer Mode");
            return;
        }
        match request {
            Devtools::Place(place) => self.place_devtools(place, cx),
            Devtools::Select(path) => self.select_path(&path, cx),
            Devtools::Read { path, reply } => {
                let answer = match self.source_file(&path) {
                    Ok(file) => read_source(&file),
                    Err(problem) => Err(problem),
                };
                let fields = match answer {
                    Ok(text) => BTreeMap::from([
                        ("path".to_owned(), Data::from(path)),
                        ("text".to_owned(), Data::from(text)),
                    ]),
                    Err(problem) => failed(&path, problem),
                };
                self.deliver(from, with_fields(&reply, fields), cx);
            },
            Devtools::Save { path, text, reply } => {
                let fields = match self
                    .source_file(&path)
                    .and_then(|file| save_source(&file, &text))
                {
                    Ok(written) => BTreeMap::from([
                        ("path".to_owned(), Data::from(path)),
                        ("ok".to_owned(), Data::Bool(true)),
                        (
                            "written".to_owned(),
                            Data::from(written.display().to_string()),
                        ),
                    ]),
                    Err(problem) => failed(&path, problem),
                };
                self.deliver(from, with_fields(&reply, fields), cx);
            },
            Devtools::State { module, reply } => {
                let mut fields = BTreeMap::from([("mod".to_owned(), Data::from(module.as_str()))]);
                match self.workspace.slot(&module) {
                    None => {
                        fields.insert(
                            "error".to_owned(),
                            Data::from(format!("no mod named `{module}`")),
                        );
                    },
                    Some(slot) => {
                        match slot.state() {
                            Some(Ok(state)) => {
                                fields.insert("state".to_owned(), state);
                            },
                            Some(Err(problem)) => {
                                fields.insert("error".to_owned(), Data::from(problem.to_string()));
                            },
                            None => {},
                        }
                        if let Some(problem) = slot.error() {
                            fields.insert("problem".to_owned(), Data::from(problem.to_string()));
                        }
                    },
                }
                self.deliver(from, with_fields(&reply, fields), cx);
            },
        }
    }

    /// `path` when it is a Luau file in one of the app's folders.
    fn source_file(&self, path: &str) -> Result<PathBuf, String> {
        let file = PathBuf::from(path);
        let luau = file
            .extension()
            .is_some_and(|extension| extension == "luau");
        let climbs = file
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir));
        if !file.is_absolute() || !luau || climbs {
            return Err("only a Luau file of the app's folders, by its whole path".to_owned());
        }
        // Resolved, so a link inside the app's folders cannot lead out.
        let resolved = file
            .canonicalize()
            .map_err(|error| format!("cannot read it: {error}"))?;
        if !self.workspace.roots().layers().any(|layer| {
            layer
                .canonicalize()
                .is_ok_and(|layer| resolved.starts_with(layer))
        }) {
            return Err("that file is not in the app's folders".to_owned());
        }
        // The path as asked: the pick and the user's copy are named by it.
        Ok(file)
    }

    /// Whether the `devtools` mod loaded is the app's own (the built-in
    /// one, the app's, or the user's), not one an installed package
    /// brought: only that one may read and save the app's files.
    fn devtools_trusted(&self) -> bool {
        let Some(source) = self.workspace.slot(DEVTOOLS).and_then(|slot| slot.source()) else {
            return false;
        };
        self.workspace
            .roots()
            .app_layers()
            .any(|layer| source.starts_with(layer))
    }

    /// The dock beside `body`, the shell, when `tools` is drawn: a strip
    /// between them drags its width.
    pub(super) fn with_dock(
        &self,
        body: AnyElement,
        tools: Option<AnyElement>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(tools) = tools else {
            return body;
        };
        let theme = cx.theme();
        let width = self.devtools.width;
        let resizing = self.devtools.resizing.is_some();
        let handle = div()
            .id("devtools-edge")
            .w(px(HANDLE))
            .h_full()
            .flex_none()
            .cursor_col_resize()
            .bg(if resizing {
                theme.primary.opacity(0.5)
            } else {
                theme.border
            })
            .hover(|style| style.bg(theme.primary.opacity(0.5)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.devtools.resizing = Some((event.position.x, this.devtools.width));
                    cx.notify();
                }),
            );
        // While dragging, a layer over the window follows the pointer
        // wherever it goes, and lets go anywhere.
        let drag = resizing.then(|| {
            deferred(
                div()
                    .id("devtools-resize")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .cursor_col_resize()
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        if let Some((from, width)) = this.devtools.resizing {
                            let moved = f32::from(from - event.position.x);
                            this.devtools.width = (width + moved).clamp(MIN_WIDTH, MAX_WIDTH);
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| {
                            this.devtools.resizing = None;
                            cx.notify();
                        }),
                    ),
            )
            .with_priority(1)
        });
        div()
            .relative()
            .size_full()
            .flex()
            .flex_row()
            .child(div().flex_1().min_w_0().h_full().child(body))
            .child(handle)
            .child(
                div()
                    .w(px(width))
                    .flex_none()
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .bg(theme.background)
                    .child(tools),
            )
            .children(drag)
            .into_any_element()
    }
}

/// The answer's fields when `path` could not be read or written.
fn failed(path: &str, problem: String) -> BTreeMap<String, Data> {
    BTreeMap::from([
        ("path".to_owned(), Data::from(path)),
        ("ok".to_owned(), Data::Bool(false)),
        ("error".to_owned(), Data::from(problem)),
    ])
}

/// The text of `file`: the user's copy of it when there is one, as that is
/// what the app runs.
fn read_source(file: &Path) -> Result<String, String> {
    let file = user_copy(file)
        .filter(|copy| copy.is_file())
        .unwrap_or_else(|| file.to_path_buf());
    let size = std::fs::metadata(&file)
        .map_err(|error| error.to_string())?
        .len();
    if size > MAX_SOURCE as u64 {
        return Err(format!("it is over {MAX_SOURCE} bytes"));
    }
    std::fs::read_to_string(&file).map_err(|error| error.to_string())
}

/// Writes `text` to `file`, or to the user's copy of it when it ships in
/// the signed app, which that copy then replaces. A temporary file and a
/// rename, so the window never reloads half a file.
fn save_source(file: &Path, text: &str) -> Result<PathBuf, String> {
    let target = user_copy(file).unwrap_or_else(|| file.to_path_buf());
    let folder = target
        .parent()
        .ok_or_else(|| "that file has no folder".to_owned())?;
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = folder.join(format!(".{name}.devtools"));
    std::fs::write(&temporary, text)
        .and_then(|()| std::fs::rename(&temporary, &target))
        .map_err(|error| {
            let _ = std::fs::remove_file(&temporary);
            error.to_string()
        })?;
    tracing::info!("devtools saved {}", target.display());
    Ok(target)
}

/// Where the user's copy of `file` goes when it ships in the signed app.
fn user_copy(file: &Path) -> Option<PathBuf> {
    let folder = &moonframe_providers::installed()?.folder;
    Some(folder.join(paths::overlay(file)?))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{read_source, save_source};

    #[test]
    fn a_save_replaces_the_whole_file_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("mods/a/main.luau");
        let written = save_source(&file, "return mod {}").unwrap();
        assert_eq!(written, file);
        assert_eq!(read_source(&file).unwrap(), "return mod {}");
        save_source(&file, "return mod { title = \"A\" }").unwrap();
        assert_eq!(read_source(&file).unwrap(), "return mod { title = \"A\" }");
        // No temporary file is left beside it.
        let names: Vec<_> = std::fs::read_dir(file.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
    }
}
