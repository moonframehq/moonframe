//! [`Moonframe`]: how an app built on this library says who it is and what
//! it adds, then runs.

use {
    crate::{
        Identity,
        hosted::{HostView, Registry, ViewInit},
    },
    gpui_kit::{Context, Window},
    moonframe_providers::{Native, Natives},
    std::{
        io,
        path::{Path, PathBuf},
        process::ExitCode,
        sync::Arc,
    },
};

/// An app on Moonframe: its name and its parts, then [`run`](Self::run).
///
/// Its layers are, lowest first: the toolbox (Moonframe's components,
/// providers and stylesheet, with none of the starter's mods unless it
/// [keeps](Self::keep) them), the packages the user installed, the app's
/// own folder, and the user's `~/.config/<name>` on top.
pub struct Moonframe {
    pub(crate) identity: Identity,
    pub(crate) app: Option<AppSource>,
    pub(crate) keep: Vec<String>,
    pub(crate) natives: Natives,
    pub(crate) views: Registry,
    pub(crate) log: String,
}

/// Where the host's own app folder comes from.
#[derive(Clone)]
pub(crate) enum AppSource {
    /// A folder on disk: the app's sources while it is developed.
    Dir(PathBuf),
    /// Files compiled into the binary, written to the cache to run.
    Embedded(Arc<dyn Fn(&Path) -> io::Result<PathBuf> + Send + Sync>),
}

impl Moonframe {
    /// The app `name` (a-z, 0-9, `-` and `_`): its folders are
    /// `~/.config/<name>`, `~/.cache/<name>` and `~/.local/state/<name>`.
    /// Its title is the name until [`title`](Self::title) says another,
    /// and its id `dev.moonframe.<name>`.
    pub fn new(name: &str) -> Self {
        let mut identity = Identity::starter();
        identity.name = name.to_owned();
        identity.id = format!("dev.moonframe.{name}");
        identity.title = name.to_owned();
        identity.hosted = true;
        Self {
            log: format!("error,moonframe=info,{}=info", name.replace('-', "_")),
            identity,
            app: None,
            keep: Vec::new(),
            natives: crate::natives::natives(),
            views: Registry::default(),
        }
    }

    /// The starter app: `moonframe`, on the `app/` folder of this
    /// repository or its bundle.
    pub fn starter() -> Self {
        Self {
            identity: Identity::starter(),
            ..Self::new("moonframe")
        }
    }

    /// Its name in the menu bar, the About panel and its bundle.
    pub fn title(mut self, title: &str) -> Self {
        self.identity.title = title.to_owned();
        self
    }

    /// Its id for the window system and its bundle, such as
    /// `dev.example.notes`.
    pub fn app_id(mut self, id: &str) -> Self {
        self.identity.id = id.to_owned();
        self
    }

    /// Its icon, a square PNG (512 pixels is plenty), for the Dock and the
    /// About panel; an `icon.png` in its app folder takes precedence.
    pub fn icon(mut self, png: &'static [u8]) -> Self {
        self.identity.icon = Some(png);
        self
    }

    /// Its version, for its bundle: usually `env!("CARGO_PKG_VERSION")`.
    pub fn version(mut self, version: &str) -> Self {
        version.clone_into(&mut self.identity.version);
        self
    }

    /// Its app folder, while it runs from its sources: usually
    /// `concat!(env!("CARGO_MANIFEST_DIR"), "/app")`, so edits show live. A
    /// bundle's own `Resources/app` takes precedence, and `DIR` on the
    /// command line over both.
    pub fn app_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.app = Some(AppSource::Dir(dir.into()));
        self
    }

    /// Its app folder compiled into the binary, for an app shipped as a
    /// lone executable: written to `~/.cache/<name>/app/<hash>` to run.
    ///
    /// ```ignore
    /// #[derive(moonframe::rust_embed::RustEmbed)]
    /// #[crate_path = "moonframe::rust_embed"]
    /// #[folder = "app"]
    /// struct AppFiles;
    ///
    /// moonframe::Moonframe::new("notes").embedded_app::<AppFiles>()
    /// ```
    pub fn embedded_app<E: rust_embed::RustEmbed + 'static>(mut self) -> Self {
        self.app = Some(AppSource::Embedded(Arc::new(|cache| {
            crate::embed::materialize::<E>(cache, "app")
        })));
        self
    }

    /// Keeps the starter's mods `names` (from its `app/mods`) under the
    /// app, as a profile's `keep` does: `["terminal"]`, say.
    pub fn keep<'a>(mut self, names: impl IntoIterator<Item = &'a str>) -> Self {
        self.keep.extend(names.into_iter().map(str::to_owned));
        self
    }

    /// A native provider, `name`: Rust that publishes `app.<name>` and
    /// answers the mods' messages. The app folder runs it with a
    /// `providers/<name>/provider.toml` holding `kind = "native"`. `make`
    /// is called each time the provider starts.
    pub fn native<N: Native + 'static>(
        mut self,
        name: &str,
        make: impl Fn() -> N + Send + Sync + 'static,
    ) -> Self {
        self.natives.insert(
            name.to_owned(),
            Arc::new(move || Box::new(make()) as Box<dyn Native>),
        );
        self
    }

    /// A view, `name`, that the app's Luau places with
    /// `ui.view { name = "<name>", id = "...", props = {...} }`. `make`
    /// makes it the first time a mod draws it; it is kept by the mod and
    /// `id`, and told of new props by [`HostView::set_props`].
    pub fn view<T: HostView>(
        mut self,
        name: &str,
        make: impl Fn(&ViewInit, &mut Window, &mut Context<T>) -> T + 'static,
    ) -> Self {
        self.views.insert(name, make);
        self
    }

    /// The log filter used when `RUST_LOG` is not set.
    pub fn log(mut self, filter: &str) -> Self {
        filter.clone_into(&mut self.log);
        self
    }

    /// Reads the command line and runs: the window, or `--check`, `mod`,
    /// `app`, `init` and `bundle`. Returns once the app quits.
    pub fn run(self) -> ExitCode {
        crate::cli::main(self)
    }
}
