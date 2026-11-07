//! [`Moonframe`]: how an app built on this library says who it is and what
//! it adds, then runs.

use {
    crate::{
        Identity,
        hosted::{HostView, Registry, ViewInit},
    },
    gpui_kit::{AssetSource, Context, Window},
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
    pub(crate) assets: Option<Arc<dyn AssetSource>>,
    pub(crate) project: Option<PathBuf>,
    pub(crate) keep: Vec<String>,
    pub(crate) natives: Natives,
    pub(crate) views: Registry,
    pub(crate) log: String,
    /// The app's own tokio runtime, from [`Moonframe::tokio`].
    #[cfg(feature = "tokio")]
    pub(crate) runtime: Option<moonframe_providers::tokio::runtime::Handle>,
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
        identity.updates = None;
        Self {
            log: format!("error,moonframe=info,{}=info", name.replace('-', "_")),
            identity,
            app: None,
            project: None,
            keep: Vec::new(),
            natives: crate::natives::natives(),
            views: Registry::default(),
            assets: None,
            #[cfg(feature = "tokio")]
            runtime: None,
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

    /// The GitHub repository whose releases update it, `owner/repo`: the
    /// app menu gets **Check for Updates…**, which compares the newest
    /// release's tag (`v1.2.3`) with [`version`](Self::version), and on
    /// macOS installs its `.dmg` over the running `.app` and starts again.
    /// An installed or release build also checks by itself once a day.
    /// Requires the `updates` feature to check or install; without it this
    /// only records the repository in the app's metadata.
    pub fn updates(mut self, repo: &str) -> Self {
        if crate::update::valid_repo(repo) {
            self.identity.updates = Some(repo.to_owned());
        } else {
            eprintln!("`{repo}` is no GitHub repository (owner/repo): no updates");
        }
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

    /// The source checkout for an embedded app's debug branch bar, usually
    /// `env!("CARGO_MANIFEST_DIR")` in the host crate. This works even when
    /// Cargo's target folder is outside the checkout. An app folder selected
    /// with `app_dir`, on the command line, or from a bundle takes precedence.
    pub fn project_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.project = Some(dir.into());
        self
    }

    /// Its app folder compiled into the binary, for an app shipped as a
    /// lone executable: written to `~/.cache/<name>/app/<hash>` to run.
    /// Set [`project_dir`](Self::project_dir) to name its source checkout in
    /// the debug branch bar; without it, embedded files show no branch bar.
    ///
    /// ```ignore
    /// #[derive(moonframe::rust_embed::RustEmbed)]
    /// #[crate_path = "moonframe::rust_embed"]
    /// #[folder = "app"]
    /// struct AppFiles;
    ///
    /// moonframe::Moonframe::new("notes")
    ///     .project_dir(env!("CARGO_MANIFEST_DIR"))
    ///     .embedded_app::<AppFiles>()
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

    /// A native provider written with `async fn`, as [`Moonframe::native`]
    /// takes one: each call is awaited in turn on the provider's thread,
    /// with sockets and timers on the app's tokio runtime.
    #[cfg(feature = "tokio")]
    pub fn native_async<N: moonframe_providers::AsyncNative>(
        mut self,
        name: &str,
        make: impl Fn() -> N + Send + Sync + 'static,
    ) -> Self {
        self.natives.insert(
            name.to_owned(),
            Arc::new(move || moonframe_providers::async_native(make())),
        );
        self
    }

    /// The tokio runtime async providers and [`crate::on_runtime`] use: the
    /// app's own, so they share its clients and pools. Without one, `run`
    /// takes the multi-thread runtime it is called in (`#[tokio::main]`),
    /// else one is made when first needed. A current-thread runtime is
    /// passed over: the window holds the main thread, so it would never run.
    #[cfg(feature = "tokio")]
    pub fn tokio(mut self, runtime: moonframe_providers::tokio::runtime::Handle) -> Self {
        self.runtime = Some(runtime);
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

    /// The app's own assets, for its views: GPUI asks them first, so a path
    /// they hold wins over Moonframe's, and the rest fall through to it.
    pub fn assets(mut self, assets: impl AssetSource) -> Self {
        self.assets = Some(Arc::new(assets));
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
