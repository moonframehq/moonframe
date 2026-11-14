//! The Developer tools: the `devtools` mod, docked on the right of the
//! main window (drawn around the shell, so no app's layout has to make
//! room) or in a window of its own, in Developer Mode only. It hears each
//! pick on `app/selection`, and asks the app for a mod's source and state
//! with `app:devtools`, which only it may send. A save is checked to be a
//! Luau file of the app's own folders, and goes to the user's copy when
//! the file ships in the signed app; the window then reloads as for any
//! change.

mod files;

use {
    super::*,
    moonframe_core::{DEVTOOLS, Devtools, DevtoolsPlace, FEEDBACK, with_fields},
    std::collections::BTreeMap,
};

/// The dock's width when it first opens, and the range dragging keeps it in.
const WIDTH: f32 = 460.;
const MIN_WIDTH: f32 = 320.;
const MAX_WIDTH: f32 = 960.;
/// The narrowest dock that lists the agent sessions beside the open one.
const WIDE: f32 = 440.;
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
        self.tell_place();
        if place != DevtoolsPlace::Closed
            && let Some(selected) = self.inspector.selected.clone()
        {
            self.tell_devtools(&selected);
        }
        cx.notify();
    }

    /// Whether the tools have room for the agent sessions beside the open
    /// one: in a window, or a dock at least [`WIDE`].
    fn devtools_wide(&self) -> bool {
        match self.devtools.place {
            DevtoolsPlace::Window => true,
            DevtoolsPlace::Dock => self.devtools.width >= WIDE,
            DevtoolsPlace::Closed => false,
        }
    }

    /// Tells the mod, on `app/devtools`, where the tools show, whether
    /// they are wide, and whether the app has agents.
    fn tell_place(&mut self) {
        // Agent sessions are the feedback mod's runs: without it, no Agent tab.
        let agent = self.workspace.slot(FEEDBACK).is_some();
        let fields = BTreeMap::from([
            ("agent".to_owned(), Data::Bool(agent)),
            ("place".to_owned(), Data::from(self.devtools.place.name())),
            ("wide".to_owned(), Data::Bool(self.devtools_wide())),
        ]);
        self.workspace
            .app_topic("devtools", DEVTOOLS, Data::Map(fields));
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
        let pick = inspect::pick(picked, &self.workspace, Some(DEVTOOLS));
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
                    Ok(file) => files::read(&file, &self.source_roots()),
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
                    .and_then(|file| files::save(&file, &text, &self.source_roots()))
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
        // By name here; `files` checks what the folder really is.
        if !self
            .workspace
            .roots()
            .layers()
            .any(|layer| file.starts_with(layer))
        {
            return Err("that file is not in the app's folders".to_owned());
        }
        Ok(file)
    }

    /// The folders a source file may be read from and saved to: the app's,
    /// and the user's copies of a signed app's files.
    fn source_roots(&self) -> Vec<PathBuf> {
        self.workspace
            .roots()
            .layers()
            .map(Path::to_path_buf)
            .chain(moonframe_providers::installed().map(|installed| installed.folder.clone()))
            .collect()
    }

    /// Whether the `devtools` mod loaded is the app's own (the built-in
    /// one, the app's, or the user's), not one an installed package
    /// brought: only that one may read and save the app's files.
    fn devtools_trusted(&self) -> bool {
        let Some(source) = self.workspace.slot(DEVTOOLS).and_then(|slot| slot.source()) else {
            return false;
        };
        self.workspace.roots().in_app_layer(source)
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
                            let was = this.devtools_wide();
                            this.devtools.width = (width + moved).clamp(MIN_WIDTH, MAX_WIDTH);
                            if this.devtools_wide() != was {
                                this.tell_place();
                            }
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
