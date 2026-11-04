//! The command line every app on Moonframe takes: the window, `--check`,
//! `--catalogue`, and the `mod`, `app`, `init` and `bundle` commands.

use {
    crate::{
        app::MoonframeApp,
        backends::Backends,
        launch::{AppSource, Moonframe},
        packages,
        paths::{self, Folders},
        profiles,
    },
    gpui_kit::{
        AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions,
        component::{ActiveTheme as _, Root},
        point, px, size,
    },
    moonframe_core::{AppLayer, Chrome, Roots, Titlebar},
    std::{
        fs,
        path::PathBuf,
        process::ExitCode,
        sync::{Arc, atomic::Ordering},
    },
};

const USAGE: &str = "\
moonframe: a window laid out by Luau modules

Usage: moonframe [OPTIONS] [DIR]
       moonframe [--user DIR] [--profile NAME] mod <COMMAND>
                                         install and share mods; see `mod help`
       moonframe [--user DIR] app <COMMAND>
                                         install whole apps as profiles; see `app help`
       moonframe [--user DIR] [--profile NAME] history [COMMAND]
                                         undo and redo changes to your folder;
                                         see `history help`
       moonframe init [DIR] [--name NAME] [--path CHECKOUT]
                                         start an app of your own on the Moonframe
                                         library, or add one to a crate; see `init help`
       moonframe [--profile NAME] pane new [--agent NAME] [--cwd DIR] [PROMPT...]
                                         open a terminal tab in the running window,
                                         with an agent in it; see `pane help`
       moonframe bundle [--dist DIR] [OUT]
                                         build this app's bundle (for apps on the
                                         library; the starter's is `just bundle`)

  DIR           the built-in app folder: main.luau, style.luau, mods/, providers/
                (default: Contents/Resources/app in the bundle, else app
                beside the executable, else ./app)

The user's folder goes over it: a mod or provider there replaces the
built-in one of the same name, a main.luau replaces the shell, and a
style.luau is merged over the built-in one. Packages installed with
`moonframe mod install` go between the two.

Options:
  --user DIR    the user's folder (default: $XDG_CONFIG_HOME/moonframe,
                or ~/.config/moonframe)
  --no-user     the built-in app alone
  --profile NAME
                run the profile NAME: another app, kept in profiles/NAME in
                the user's folder, often one installed with `moonframe app
                install`. Its packages and mods are its own; only your
                style.luau and theme follow you into it
  --check       load every module, report problems, and exit without a window
  --release     Developer Mode off for this run: no Components…, Inspect
                Element, or Reload Modules (Developer Mode in the app menu
                switches it, and is kept)
  --dev         Developer Mode on for this run
  --background  open the window without bringing the app forward (for
                screenshots and scripts while someone is working)
  --component NAME
                open the components window on NAME as well
  --preview-only
                with --component, show only NAME's preview, filling the
                components window (for screenshots of it)
  --catalogue   print the component catalogue (markdown) and exit; fails if a
                component is broken or says nothing of when to use it
  -h, --help    show this help";

struct Args {
    folders: Folders,
    check: bool,
    /// Open without activating the app or focusing its window.
    background: bool,
    /// The components window shows the chosen preview alone.
    preview_only: bool,
    /// `--release`, or `--dev` (`Some(false)`); else as the bundle says.
    release: Option<bool>,
    catalogue: bool,
    component: Option<String>,
    /// The words after `mod`, for the package manager.
    packages: Option<Vec<String>>,
    /// The words after `app`, for profiles.
    apps: Option<Vec<String>>,
    /// The words after `history`, to undo changes to the folder.
    history: Option<Vec<String>>,
    /// The words after `init`, to start an app on the library.
    init: Option<Vec<String>>,
    /// The words after `bundle`, to bundle an app on the library.
    bundle: Option<Vec<String>>,
    /// The words after `pane`, for the running window.
    panes: Option<Vec<String>>,
}

fn usage() -> &'static str {
    if cfg!(feature = "sealed") {
        "Usage: moonframe [--check] [--no-user | --user DIR] [--background]\n\n\
         This sealed executable runs only its bundled app. Settings and state\n\
         persist, but user source overlays and development tools are unavailable.\n\
         Rebuild the app to add capabilities removed from this executable."
    } else {
        USAGE
    }
}

fn parse_args(host: Option<&AppSource>, keep: &[String]) -> Result<Option<Args>, String> {
    let mut builtin = None;
    let mut user = paths::user();
    let bundled = paths::bundled();
    let mut check = false;
    let mut catalogue = false;
    let mut background = false;
    let mut preview_only = false;
    let mut release = None;
    let mut component = None;
    let mut packages = None;
    let mut apps = None;
    let mut init = None;
    let mut history = None;
    let mut bundle = None;
    let mut panes = None;
    let mut profile = None;
    let mut args = std::env::args().skip(1).collect::<Vec<_>>().into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--catalogue" => catalogue = true,
            "--background" => background = true,
            "--preview-only" => preview_only = true,
            "--release" => release = Some(true),
            "--dev" => release = Some(false),
            "--component" => component = Some(args.next().ok_or("--component needs a name")?),
            "--user" => {
                user = Some(PathBuf::from(args.next().ok_or("--user needs a folder")?));
            },
            "--no-user" => user = None,
            "--profile" => {
                let name = args.next().ok_or("--profile needs a name")?;
                if !packages::valid_name(&name) {
                    return Err(format!(
                        "`{name}` is not a profile name: use a-z, 0-9, - and _"
                    ));
                }
                profile = Some(name);
            },
            "-h" | "--help" => return Ok(None),
            flag if flag.starts_with('-') => return Err(format!("unknown option: {flag}")),
            "mod" | "mods" if builtin.is_none() => {
                packages = Some(args.by_ref().collect());
            },
            "history" if builtin.is_none() => history = Some(args.by_ref().collect()),
            "init" if builtin.is_none() => init = Some(args.by_ref().collect()),
            "bundle" if builtin.is_none() => bundle = Some(args.by_ref().collect()),
            "pane" | "panes" if builtin.is_none() => panes = Some(args.by_ref().collect()),
            // `app` is also the app folder's usual name (`moonframe app`),
            // so it is the command only before one of its words.
            "app" | "apps" if builtin.is_none() => {
                let rest: Vec<String> = args.by_ref().collect();
                if rest
                    .first()
                    .is_some_and(|word| profiles::COMMANDS.contains(&word.as_str()))
                {
                    apps = Some(rest);
                } else {
                    builtin = Some(PathBuf::from(&arg));
                    args = rest.into_iter();
                }
            },
            path if builtin.is_none() => builtin = Some(PathBuf::from(path)),
            extra => return Err(format!("unexpected argument: {extra}")),
        }
    }
    if cfg!(feature = "sealed") {
        if bundle.is_some() {
            return Err("a sealed executable cannot build app bundles".to_owned());
        }
        let app = bundled
            .as_ref()
            .ok_or("a sealed executable must run inside its app bundle")?;
        if builtin.is_some() || profile.as_ref().is_some_and(|name| name != &app.name) {
            return Err("a sealed executable only runs its bundled app".to_owned());
        }
    }
    if !cfg!(feature = "studio")
        && (init.is_some()
            || apps.is_some()
            || packages.is_some()
            || history.is_some()
            || catalogue
            || component.is_some()
            || preview_only
            || release == Some(false))
    {
        return Err(
            "this build does not include development tools or package installation".to_owned(),
        );
    }
    // An app on the library: DIR is its own folder, over the toolbox.
    let (builtin, host) = match host {
        Some(source) => {
            let app = builtin.unwrap_or_else(|| crate::embed::host_app(source));
            (
                crate::embed::toolbox_folder(),
                Some(AppLayer {
                    path: app,
                    keep: keep.to_vec(),
                }),
            )
        },
        None => (builtin.unwrap_or_else(crate::embed::builtin), None),
    };
    Ok(Some(Args {
        folders: Folders {
            builtin,
            host,
            user,
            profile: profile.or_else(|| bundled.as_ref().map(|bundle| bundle.name.clone())),
            bundled,
        },
        check,
        catalogue,
        background,
        preview_only,
        release,
        component,
        packages,
        apps,
        history,
        init,
        bundle,
        panes,
    }))
}

/// Runs `run` with the stack views are read on: 8 MiB, the main thread's
/// on macOS and Linux. Windows gives its main thread 1 MiB, which a debug
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

/// Loads every module headlessly: what an agent runs after an edit.
fn check(roots: Roots) -> ExitCode {
    let workspace = crate::capabilities::open(roots.clone(), false, None);
    let mut problems: Vec<(String, String)> = workspace
        .problems()
        .into_iter()
        .map(|(name, error)| (name.to_owned(), error.to_string()))
        .collect();
    problems.extend(
        moonframe_providers::check(&roots)
            .into_iter()
            .map(|(name, error)| (format!("provider {name}"), error.to_string())),
    );
    if problems.is_empty() {
        let mods = workspace.mod_names().collect::<Vec<_>>();
        let providers = moonframe_providers::names(&roots);
        let components: Vec<&str> = workspace
            .components()
            .iter()
            .map(|component| component.name.as_str())
            .collect();
        println!(
            "ok: {} (shell + {} mods: {}; {} providers: {}; {} components: {})",
            workspace.roots(),
            mods.len(),
            mods.join(", "),
            providers.len(),
            providers.join(", "),
            components.len(),
            components.join(", ")
        );
        return ExitCode::SUCCESS;
    }
    for (name, error) in problems {
        eprintln!("{name}: {error}");
    }
    ExitCode::FAILURE
}

/// Prints the catalogue the `moonframe-components` skill is made of.
fn catalogue(roots: Roots) -> ExitCode {
    let workspace = crate::capabilities::open(roots, false, None);
    let components = workspace.components();
    let mut failed = false;
    for component in components {
        if let Err(error) = &component.preview {
            eprintln!("components/{}: {error}", component.path_name());
            failed = true;
        }
        if component.when.is_empty() {
            eprintln!(
                "components/{}: say `when` to use it, for the catalogue",
                component.path_name()
            );
            failed = true;
        }
    }
    if failed {
        return ExitCode::FAILURE;
    }
    print!("{}", moonframe_core::catalogue(components));
    ExitCode::SUCCESS
}

/// The window's frame for what the shell asks. GPUI takes it only when it
/// opens a window, so a change to the shell's `window` waits for a restart.
///
/// A hidden title bar puts the layout under the transparent title bar with
/// the traffic lights over it on macOS, and drops the system frame on
/// Windows and Linux (client-side decorations, where the compositor allows:
/// gpui-component's `Root` then draws the resize edges and shadow). The
/// layout moves the window from its `drag = true` stacks, so AppKit must
/// not take the title bar's clicks for itself.
fn framed(chrome: Chrome) -> WindowOptions {
    let hidden = chrome.titlebar == Titlebar::Hidden;
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: hidden,
            traffic_light_position: chrome
                .traffic_lights
                .filter(|_| hidden)
                .map(|(x, y)| point(px(x), px(y))),
        }),
        window_decorations: hidden.then_some(WindowDecorations::Client),
        app_owns_titlebar_drag: hidden,
        ..Default::default()
    }
}

/// What `Moonframe::run` does.
pub(crate) fn main(launch: Moonframe) -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&launch.log)),
        )
        .with_writer(std::io::stderr)
        .init();
    // The app's tokio runtime, else the one `run` is called in
    // (`#[tokio::main]`), for async providers and views.
    #[cfg(feature = "tokio")]
    if let Some(runtime) = launch
        .runtime
        .clone()
        .or_else(|| moonframe_providers::tokio::runtime::Handle::try_current().ok())
        && !moonframe_providers::set_runtime(runtime)
    {
        tracing::warn!(
            "a current-thread tokio runtime would never run beside the window; using one of Moonframe's own"
        );
    }
    let name = &launch.identity.name;
    if !packages::valid_name(name) {
        eprintln!("`{name}` cannot name an app: use a-z, 0-9, - and _");
        return ExitCode::from(2);
    }
    let _ = crate::IDENTITY.set(launch.identity.clone());
    if let Ok(dir) = std::env::current_dir() {
        // Terminals start here, and the explorer shows its project.
        moonframe_providers::set_launch_dir(&dir);
    }
    if let Some(resources) = paths::resources() {
        moonframe_providers::set_payload(&resources.join("payload"));
    }
    let host = launch.app.as_ref().filter(|_| launch.identity.hosted);
    if launch.identity.hosted && host.is_none() {
        eprintln!("{name}: the app names no folder: call `Moonframe::app_dir` or `embedded_app`");
        return ExitCode::from(2);
    }
    let args = match parse_args(host, &launch.keep) {
        Ok(Some(args)) => args,
        Ok(None) => {
            println!("{}", branded(usage()));
            return ExitCode::SUCCESS;
        },
        Err(message) => {
            eprintln!("{message}\n\n{}", branded(usage()));
            return ExitCode::from(2);
        },
    };
    if cfg!(feature = "studio")
        && let Some(words) = &args.init
    {
        return crate::init::run(words);
    }
    // Packaging copies this executable, so hosts without developer tools
    // must be able to bundle themselves too.
    if !cfg!(feature = "sealed")
        && let Some(words) = &args.bundle
    {
        return crate::bundle::run(&args.folders, words);
    }
    let folders = args.folders;
    paths::set_user_folder(folders.user.clone());
    let _ =
        crate::CRASH_DIR.set(paths::state(folders.profile.as_deref()).with_extension("crashes"));
    crate::crash::install(
        crate::crash_dir(),
        format!("{} {}", launch.identity.title, launch.identity.version),
    );
    if cfg!(feature = "studio")
        && let Some(words) = &args.apps
    {
        let Some(user) = &folders.user else {
            eprintln!(
                "{} app: profiles go in the user's folder; drop --no-user",
                paths::name()
            );
            return ExitCode::from(2);
        };
        return profiles::run(user, words);
    }
    if let (Some(name), Some(user), None) = (&folders.profile, &folders.user, &folders.bundled)
        && !paths::profile(user, name).is_dir()
    {
        eprintln!(
            "{app}: no profile `{name}` in {}; `{app} app list` shows them, \
             `{app} app install` adds one",
            user.display(),
            app = paths::name(),
        );
        return ExitCode::from(2);
    }
    if let Some(words) = &args.panes {
        // A window without the user's folder takes no requests.
        let Some(user) = &folders.user else {
            eprintln!(
                "{} pane: panes open in a window with the user's folder; drop --no-user",
                paths::name()
            );
            return ExitCode::from(2);
        };
        return crate::panes::run(user, folders.profile.as_deref(), &folders.title(), words);
    }
    if cfg!(feature = "studio")
        && let Some(words) = &args.packages
    {
        let Some(own) = folders.own() else {
            eprintln!(
                "{} mod: packages go in the user's folder; drop --no-user",
                paths::name()
            );
            return ExitCode::from(2);
        };
        return packages::run(&own, words);
    }
    if cfg!(feature = "studio")
        && let Some(words) = &args.history
    {
        return crate::history::run(&folders, words);
    }
    if args.check {
        let roots = folders.roots();
        return on_a_view_stack(move || check(roots));
    }
    if cfg!(feature = "studio") && args.catalogue {
        let roots = folders.roots();
        return on_a_view_stack(move || catalogue(roots));
    }

    // The user's folder, or the bundled app's profile in it, exists from
    // the first run, so it is watched and easy to find.
    if let Some(own) = folders.own()
        && let Err(error) = fs::create_dir_all(&own)
    {
        tracing::warn!("cannot create {}: {error}", own.display());
    }
    // The developer tools (Components, Inspect Element, Reload Modules)
    // are for whoever builds the app: an app on the library shows them in
    // a debug build (`cargo run`), not in a release one (`cargo run
    // --release`, an install, a bundle). The starter always shows them
    // outside a bundle, as building apps is what it is for.
    let hosted_release = folders.host.is_some() && (paths::in_bundle() || !cfg!(debug_assertions));
    // The user's own choice (Developer Mode in the app menu) wins over
    // those defaults; `--release` and `--dev` over both, for this run.
    let developer_file =
        paths::state(folders.profile.as_deref()).with_file_name(match &folders.profile {
            Some(profile) => format!("developer-{profile}"),
            None => "developer".to_owned(),
        });
    let chosen = crate::developer_choice(&developer_file).map(|on| !on);
    let _ = crate::DEVELOPER_FILE.set(developer_file);
    crate::RELEASE.store(
        args.release
            .or(chosen)
            .unwrap_or(folders.bundled.is_some() || hosted_release),
        Ordering::Relaxed,
    );
    let _ = crate::TITLE.set(folders.title());
    #[cfg(feature = "updates")]
    {
        let _ = crate::update::SOURCE.set(folders.updates());
        crate::update::load_beta(folders.profile.as_deref());
    }
    crate::dock::set_name(&crate::title());
    tracing::info!(
        "starting {} ({}{}) on {}",
        crate::title(),
        folders.app_id(),
        if crate::release() {
            ", release"
        } else {
            ""
        },
        folders.roots()
    );
    let icon = folders.icon();
    let background = args.background;
    crate::BACKGROUND.store(background, Ordering::Relaxed);
    crate::PREVIEW_ONLY.store(args.preview_only, Ordering::Relaxed);
    let _ = crate::RUNNING_ID.set(folders.app_id());
    if let Some(profile) = &folders.profile {
        moonframe_providers::set_profile(profile);
    }
    let component = args.component.filter(|_| !crate::release());
    let (natives, views) = (launch.natives, launch.views);
    gpui_kit::application()
        .with_assets(crate::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            crate::dock::set_icon(icon.as_deref());
            crate::actions::bind_keys(cx);
            cx.set_menus(crate::actions::menus(&[]));
            cx.on_action(|_: &crate::actions::Quit, cx| crate::app::guard::quit(cx));
            cx.on_window_closed(|cx, id| {
                crate::video::window_closed(id);
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            // Each mod's state and the topics come back from the last run:
            // from herdr-core when it runs here, else SQLite, each profile
            // its own. Only with the user's folder: `--no-user` runs
            // (checks, screenshots) start fresh and keep nothing.
            let backends = Arc::new(Backends::for_app(&paths::namespace(
                folders.profile.as_deref(),
            )));
            let persist = folders
                .user
                .as_ref()
                .and_then(|_| crate::persist::open(&backends, folders.profile.as_deref()));
            // The shell says what window it wants, so it loads first.
            let workspace =
                crate::capabilities::open(folders.roots(), cx.theme().is_dark(), persist);
            let chrome = workspace.chrome();
            // The shell's size if it asks, kept within the screen.
            let (width, height) = chrome.size.unwrap_or((1280., 820.));
            let screen = cx.primary_display().map(|display| display.bounds().size);
            let (width, height) = match screen {
                Some(screen) => (
                    width.min(f32::from(screen.width) - 40.),
                    height.min(f32::from(screen.height) - 60.),
                ),
                None => (width, height),
            };
            let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(640.), px(400.))),
                app_id: Some(crate::app_id()),
                focus: !background,
                ..framed(chrome)
            };
            let opened = cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| {
                    let mut app = MoonframeApp::new(
                        workspace,
                        chrome,
                        folders.clone(),
                        backends,
                        crate::app::Parts { natives, views },
                        window,
                        cx,
                    );
                    if let Some(name) = &component {
                        app.show_component(name, cx);
                    }
                    app
                });
                cx.new(|cx| Root::new(view, window, cx))
            });
            if let Err(error) = opened {
                eprintln!("{}: unable to open a window: {error}", paths::name());
                cx.quit();
            }
            if !background {
                cx.activate(true);
            }
        });
    ExitCode::SUCCESS
}

/// `text` for this app: its command and folders by its own name.
pub(crate) fn branded(text: &str) -> String {
    let name = paths::name();
    if name == "moonframe" {
        return text.to_owned();
    }
    text.replace("Usage: moonframe", &format!("Usage: {name}"))
        .replace("moonframe: a window", &format!("{name}: a window"))
        .replace("       moonframe ", &format!("       {name} "))
        .replace("`moonframe ", &format!("`{name} "))
        .replace("config/moonframe", &format!("config/{name}"))
        .replace("cache/moonframe", &format!("cache/{name}"))
}
