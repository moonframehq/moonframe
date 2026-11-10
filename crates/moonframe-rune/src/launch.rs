//! The same builder as `moonframe::Moonframe`, so an app chooses its renderer
//! with one line (`extern crate moonframe_rune as moonframe;`) and keeps its
//! code. It finds the same folders, keeps state in the same place, and runs
//! the same providers; what it lacks are the commands (`mod`, `app`, `init`,
//! `bundle`), which `moonframe` keeps.

use {
    crate::{Ui, persist, profiles},
    moonframe_core::{AppLayer, Appearance, Data, Roots, Workspace},
    moonframe_mux::Backends,
    moonframe_packages::{
        IDENTITY, Identity, packages,
        paths::{self, Folders},
    },
    moonframe_providers::{Native, Natives},
    rune::{App, AppSpec, El, WindowHandle},
    std::{
        cell::RefCell,
        collections::HashMap,
        fs, io,
        path::{Path, PathBuf},
        process::ExitCode,
        rc::Rc,
        sync::Arc,
    },
};

/// A view an app writes in Rust: its element for `props`, drawn each frame.
pub type View = Rc<dyn Fn(&Data) -> El>;

/// What the window opens at when the shell names no size.
const SIZE: (f32, f32) = (1200., 800.);

/// What the command line asks of Moonframe; the rest of it is Rune's.
#[derive(Debug, Default, PartialEq)]
struct Given {
    /// `--user DIR`, or `None` inside for `--no-user`.
    user: Option<Option<PathBuf>>,
    profile: Option<String>,
    /// The words after `app`.
    apps: Option<Vec<String>>,
    check: bool,
    rest: Vec<String>,
}

impl Given {
    fn read(arguments: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut given = Self::default();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--user" => {
                    let folder = arguments.next().ok_or("--user needs a folder")?;
                    given.user = Some(Some(PathBuf::from(folder)));
                },
                "--no-user" => given.user = Some(None),
                "--profile" => {
                    let profile = arguments.next().ok_or("--profile needs a name")?;
                    if !packages::valid_name(&profile) {
                        return Err(format!(
                            "`{profile}` is not a profile name: use a-z, 0-9, - and _"
                        ));
                    }
                    given.profile = Some(profile);
                },
                "--check" => given.check = true,
                "app" | "apps" if given.rest.is_empty() => {
                    given.apps = Some(arguments.by_ref().collect());
                },
                _ => given.rest.push(argument),
            }
        }
        Ok(given)
    }
}

/// Where the host's own app folder comes from.
#[derive(Clone)]
pub(crate) enum AppSource {
    /// A folder on disk: the app's sources while it is developed.
    Dir(PathBuf),
    /// Files compiled into the binary, written to the cache to run.
    Embedded(Arc<dyn Fn(&Path) -> io::Result<PathBuf> + Send + Sync>),
}

/// An app on Moonframe, drawn with Rune.
pub struct Moonframe {
    identity: Identity,
    app: Option<AppSource>,
    keep: Vec<String>,
    natives: Natives,
    views: HashMap<String, View>,
    log: String,
}

impl Moonframe {
    /// The app `name`, with the folders `moonframe::Moonframe::new` gives it.
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
            keep: Vec::new(),
            natives: crate::natives::natives(),
            views: HashMap::new(),
        }
    }

    /// The starter app: `moonframe`, on the `app/` folder of this checkout or
    /// of the bundle it runs from.
    pub fn starter() -> Self {
        Self {
            identity: Identity::starter(),
            ..Self::new("moonframe")
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        title.clone_into(&mut self.identity.title);
        self
    }

    pub fn app_id(mut self, id: &str) -> Self {
        id.clone_into(&mut self.identity.id);
        self
    }

    pub fn icon(mut self, png: &'static [u8]) -> Self {
        self.identity.icon = Some(png);
        self
    }

    pub fn version(mut self, version: &str) -> Self {
        version.clone_into(&mut self.identity.version);
        self
    }

    /// Recorded in its metadata; a Rune build has no Check for Updates yet.
    pub fn updates(mut self, repo: &str) -> Self {
        self.identity.updates = Some(repo.to_owned());
        self
    }

    pub fn app_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.app = Some(AppSource::Dir(dir.into()));
        self
    }

    /// Only the branch bar reads this on GPUI, and a Rune build has none.
    #[must_use]
    pub fn project_dir(self, _: impl Into<PathBuf>) -> Self {
        self
    }

    pub fn embedded_app<E: rust_embed::RustEmbed + 'static>(mut self) -> Self {
        self.app = Some(AppSource::Embedded(Arc::new(|cache| {
            crate::embed::materialize::<E>(cache, "app")
        })));
        self
    }

    pub fn keep<'a>(mut self, names: impl IntoIterator<Item = &'a str>) -> Self {
        self.keep.extend(names.into_iter().map(str::to_owned));
        self
    }

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

    /// A view, `name`, that the app's Luau places with `ui.view`. Unlike
    /// GPUI's, it holds no state of its own: `draw` gives its element for the
    /// `props` of the moment.
    pub fn view(mut self, name: &str, draw: impl Fn(&Data) -> El + 'static) -> Self {
        self.views.insert(name.to_owned(), Rc::new(draw));
        self
    }

    pub fn log(mut self, filter: &str) -> Self {
        filter.clone_into(&mut self.log);
        self
    }

    /// The folders this run uses, as `moonframe`'s command line works them
    /// out with no arguments.
    fn folders(&self) -> Folders {
        let bundled = paths::bundled();
        let (builtin, host) = match self.app.as_ref().filter(|_| self.identity.hosted) {
            Some(source) => {
                let selected = paths::resources().map(|dir| dir.join("app"));
                (
                    crate::embed::toolbox_folder(),
                    Some(AppLayer {
                        path: selected.unwrap_or_else(|| crate::embed::host_app(source)),
                        keep: self.keep.clone(),
                    }),
                )
            },
            None => (crate::embed::builtin(), None),
        };
        Folders {
            builtin,
            host,
            user: paths::user(),
            profile: bundled.as_ref().map(|bundle| bundle.name.clone()),
            bundled,
        }
    }

    /// Runs the window, or with `--check` loads every module and says what is
    /// wrong. Rune reads its own flags, such as `--snap out.png`.
    pub fn run(self) -> ExitCode {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&self.log)),
            )
            .with_writer(io::stderr)
            .init();
        let name = self.identity.name.clone();
        if !packages::valid_name(&name) {
            eprintln!("`{name}` cannot name an app: use a-z, 0-9, - and _");
            return ExitCode::from(2);
        }
        if self.identity.hosted && self.app.is_none() {
            eprintln!(
                "{name}: the app names no folder: call `Moonframe::app_dir` or `embedded_app`"
            );
            return ExitCode::from(2);
        }
        // Only ever set here, and `run` is called once.
        let _ = IDENTITY.set(self.identity.clone());
        if let Ok(here) = std::env::current_dir() {
            moonframe_providers::set_launch_dir(&here);
        }
        if let Some(resources) = paths::resources() {
            moonframe_providers::set_payload(&resources.join("payload"));
        }
        let given = match Given::read(std::env::args().skip(1)) {
            Ok(given) => given,
            Err(problem) => {
                eprintln!("{name}: {problem}");
                return ExitCode::from(2);
            },
        };
        let mut folders = self.folders();
        if let Some(user) = given.user {
            folders.user = user;
        }
        folders.profile = given.profile.or(folders.profile);
        paths::set_user_folder(folders.user.clone());
        if let Some(words) = &given.apps {
            let Some(user) = &folders.user else {
                eprintln!("{name} app: profiles go in the user's folder; drop --no-user");
                return ExitCode::from(2);
            };
            return profiles::run(user, words);
        }
        if let (Some(profile), Some(user), None) =
            (&folders.profile, &folders.user, &folders.bundled)
            && !paths::profile(user, profile).is_dir()
        {
            eprintln!("{name}: no profile `{profile}`; `{name} app link DIR` adds one");
            return ExitCode::from(2);
        }
        if let Some(profile) = &folders.profile {
            moonframe_providers::set_profile(profile);
        }
        if given.check {
            let roots = folders.roots();
            return on_a_view_stack(move || checked(roots));
        }
        if let Some(own) = folders.own()
            && let Err(error) = fs::create_dir_all(&own)
        {
            tracing::warn!("cannot create {}: {error}", own.display());
        }
        let title: &'static str = Box::leak(folders.title().into_boxed_str());
        let namespace = paths::namespace(folders.profile.as_deref());
        let backends = Arc::new(Backends::for_app(&namespace));
        // `--no-user` is a fresh run: it neither reads nor writes saved state.
        let store = folders
            .user
            .as_ref()
            .and_then(|_| persist::open(&backends, folders.profile.as_deref()));
        let mut ui = Ui::open(folders.roots(), store);
        ui.natives = self.natives;
        ui.views = self.views;
        ui.terminals.backends = backends;
        ui.icon = folders.icon();
        let files = folders.settings();
        ui.settings = files.first().cloned();
        let saved = files
            .iter()
            .find_map(|file| crate::settings::get(file, crate::settings::APPEARANCE));
        if let Some(appearance) = saved.as_deref().and_then(Appearance::named) {
            ui.show_appearance(appearance);
        }
        crate::dock::set_name(title);
        if let Err(problem) = crate::dock::set_icon(ui.icon.as_deref()) {
            tracing::warn!("no Dock icon: {problem}");
        }
        let size = ui.workspace.chrome().size.unwrap_or(SIZE);
        let app = App::new(AppSpec {
            setup: Some(crate::setup),
            poll: Some(crate::poll),
            on_key: Some(crate::on_key),
            ..AppSpec::new(title, size, crate::unused_init, crate::view)
        });
        ui.app = Some(app.handle());
        let ready = app.handle();
        let parent: Rc<RefCell<Option<WindowHandle>>> = Rc::default();
        ui.browsers.parent = Rc::clone(&parent);
        app.args(given.rest)
            .init(move || ui)
            .on_ready(move |window| {
                *parent.borrow_mut() = Some(window.clone());
                // An idle window polls no more: what waited for it has to hear.
                ready.update(|_, _| {});
            })
            .run();
        ExitCode::SUCCESS
    }
}

/// Loads every module and reports, as `moonframe --check` does.
/// Runs `run` with the stack views are read on: 8 MiB, the main thread's on
/// macOS and Linux. Windows gives its main thread 1 MiB, which a debug
/// build's frames reading a deep view (an example app, in CI) outgrow.
fn on_a_view_stack(run: impl FnOnce() -> ExitCode + Send + 'static) -> ExitCode {
    std::thread::Builder::new()
        .name("check".to_owned())
        .stack_size(8 * 1024 * 1024)
        .spawn(run)
        .ok()
        .and_then(|thread| thread.join().ok())
        .unwrap_or(ExitCode::FAILURE)
}

fn checked(roots: Roots) -> ExitCode {
    let workspace = Workspace::open_with_capabilities(roots, false, None, crate::compiled());
    let problems = workspace.problems();
    if problems.is_empty() {
        let mods: Vec<&str> = workspace.mod_names().collect();
        println!("ok: shell + {} mods: {}", mods.len(), mods.join(", "));
        return ExitCode::SUCCESS;
    }
    for (name, error) in problems {
        eprintln!("{name}: {error}");
    }
    ExitCode::FAILURE
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn read(words: &[&str]) -> Result<Given, String> {
        Given::read(words.iter().map(|word| (*word).to_owned()))
    }

    #[test]
    fn moonframes_flags_stay_and_runes_go_on_to_it() {
        let given = read(&["--user", "/u", "--profile", "notes", "--snap", "a.png"]).unwrap();
        assert_eq!(given.user, Some(Some(PathBuf::from("/u"))));
        assert_eq!(given.profile.as_deref(), Some("notes"));
        assert_eq!(given.rest, ["--snap", "a.png"]);
    }

    #[test]
    fn the_app_command_takes_all_that_follows() {
        let given = read(&["--user", "/u", "app", "link", "examples/demo", "--yes"]).unwrap();
        assert_eq!(given.apps.unwrap(), ["link", "examples/demo", "--yes"]);
    }

    #[test]
    fn a_profile_name_is_checked_before_a_path_is_made_of_it() {
        assert!(read(&["--profile", "../up"]).is_err());
        assert!(read(&["--user"]).is_err());
    }
}
