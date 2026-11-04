//! Loading a module into its [`Slot`] and drawing its view.

use {super::*, std::fs};

impl Slot {
    pub fn view(&self) -> Option<&Node> {
        self.view.as_ref()
    }

    pub fn error(&self) -> Option<&Error> {
        self.load_error
            .as_ref()
            .or(self.update_error.as_ref())
            .or(self.view_error.as_ref())
    }

    /// What is wrong with the children marked broken in the view.
    pub fn broken(&self) -> &[Error] {
        &self.broken
    }

    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    pub fn meta(&self) -> Option<&Meta> {
        self.module.as_ref().map(Module::meta)
    }

    /// The keys its mod takes anywhere in the window (`keys` in `mod {}`).
    pub fn keys(&self) -> Option<&crate::Keys> {
        self.module.as_ref().map(Module::keys)
    }

    /// The file the module comes from.
    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    /// Loads the first of `paths` (highest layer first) that works. A module
    /// that never loaded falls back to the same file in a lower layer, the
    /// app's own, so a broken copy in the user's folder shows the built-in
    /// version under its error rather than nothing. One that loaded before
    /// keeps its last good version, as always.
    pub(super) fn load_layered(&mut self, name: &str, paths: &[PathBuf], library: &Library) {
        let Some((top, lower)) = paths.split_first() else {
            return;
        };
        self.load(name, top, library);
        if self.module.is_some() || self.load_error.is_none() {
            return;
        }
        let Some(error) = self.load_error.take() else {
            return;
        };
        for path in lower {
            self.load(name, path, library);
            if self.module.is_some() {
                self.load_error = Some(Error::new(format!(
                    "{}: {error} (showing {} until it loads)",
                    top.display(),
                    path.display()
                )));
                return;
            }
        }
        self.load_error = Some(error);
    }

    fn load(&mut self, name: &str, path: &Path, library: &Library) {
        match fs::read_to_string(path) {
            Ok(source) => self.load_source(name, &source, Some(path), library),
            Err(error) => {
                self.source = Some(path.to_owned());
                self.load_error = Some(Error::new(format!(
                    "cannot read {}: {error}",
                    path.display()
                )));
            },
        }
    }

    /// Loads `source`, which came from `path` if it is a file's.
    pub(super) fn load_source(
        &mut self,
        name: &str,
        source: &str,
        path: Option<&Path>,
        library: &Library,
    ) {
        self.source = path.map(Path::to_owned);
        let (previous, lost) = self.previous();
        let props = (self.props != Data::Nil).then_some(&self.props);
        let loaded = Module::load(name, source, previous.as_ref(), props, library);
        self.loaded(loaded, lost);
    }

    /// Loads an instance of the component `component`, keeping its state
    /// across reloads like a mod's.
    pub(super) fn load_component(&mut self, key: &str, component: &str, library: &Library) {
        self.source = library.path(component);
        let (previous, lost) = self.previous();
        let props = (self.props != Data::Nil).then_some(&self.props);
        let loaded = Module::load_component(key, component, previous.as_ref(), props, library);
        self.loaded(loaded, lost);
    }

    /// The state a load starts from: a reload keeps the running state, a
    /// first load takes the saved one. With why the running state could not
    /// be kept, when it could not.
    fn previous(&mut self) -> (Option<Data>, Option<Error>) {
        match self.module.as_ref().map(Module::state) {
            Some(Ok(state)) => (Some(state), None),
            Some(Err(error)) => (None, Some(lost_state(error))),
            None => (self.restored.take(), None),
        }
    }

    fn loaded(&mut self, loaded: Result<Module, Error>, lost: Option<Error>) {
        match loaded {
            Ok(module) => {
                self.module = Some(module);
                self.fresh = true;
                self.dirty = true;
                self.load_error = None;
                self.update_error = lost;
            },
            Err(error) => self.load_error = Some(error),
        }
    }

    /// Draws it with `props` from now on.
    pub(super) fn set_props(&mut self, props: &Data) {
        if self.props == *props {
            return;
        }
        self.props = props.clone();
        self.dirty = true;
        if let Some(module) = &mut self.module
            && let Err(error) = module.set_props(props)
        {
            self.view_error = Some(error);
        }
    }

    /// Runs its view, unless nothing it was drawn from has changed;
    /// whether it ran.
    pub(super) fn render(
        &mut self,
        app: &Rc<AppData>,
        versions: &Versions,
        sheet: &Stylesheet,
        style: u64,
        window: Option<WindowState>,
    ) -> bool {
        let Some(module) = &self.module else {
            return false;
        };
        let current = self.drawn.as_ref().is_some_and(|drawn| {
            drawn.style_generation == style
                && versions.current(&drawn.stamp)
                && (drawn.window == window || !drawn.stamp.reads("window"))
        });
        if current && !self.dirty {
            return false;
        }
        self.dirty = false;
        let scoped;
        let app = if let Some(window) = window {
            let mut data = (**app).clone();
            data.insert("window".to_owned(), window.data());
            scoped = Rc::new(data);
            &scoped
        } else {
            app
        };
        match module
            .rules(sheet)
            .and_then(|rules| Ok((rules, module.view(app, sheet, &self.lists)?)))
        {
            Ok((rules, (view, broken, reads))) => {
                self.rules = rules;
                self.view = Some(view);
                self.broken = broken;
                self.view_error = None;
                self.drawn = Some(Drawn {
                    style_generation: style,
                    stamp: versions.stamp(&reads),
                    window,
                });
            },
            Err(error) => {
                self.view_error = Some(error);
                // Tried again on the next render, whatever changes.
                self.drawn = None;
            },
        }
        true
    }
}
