//! A window whose whole layout comes from Luau modules: the app's own
//! folder with the user's `~/.config/moonframe` over it, reloaded as
//! they change.

mod actions;
mod app;
mod assets;
mod branch_bar;
mod browser;
mod build;
mod color;
mod dock;
mod gallery;
mod input;
mod inspect;
mod keys;
mod mod_window;
mod motion;
mod natives;
mod overlay;
mod packages;
mod paths;
mod persist;
mod profiles;
mod render;
mod settings;
mod terminal;
mod theme_picker;
mod themes;

use {
    app::MoonframeApp,
    gpui_kit::{
        AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions,
        component::{ActiveTheme as _, Root},
        point, px, size,
    },
    moonframe_core::{Chrome, Roots, Titlebar, Workspace},
    moonframe_mux::Backends,
    paths::Folders,
    std::{
        fs,
        path::PathBuf,
        process::ExitCode,
        sync::{Arc, OnceLock},
    },
};

const USAGE: &str = "\
moonframe: a window laid out by Luau modules

Usage: moonframe [OPTIONS] [DIR]
       moonframe [--user DIR] [--profile NAME] mod <COMMAND>
                                         install and share mods; see `mod help`
       moonframe [--user DIR] app <COMMAND>
                                         install whole apps as profiles; see `app help`

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
  --release     run as the app's users see it: no Components menu, Inspect
                Element, or Reload Modules (an app bundle always does)
  --dev         the tools again, in an app bundle
  --background  open the window without bringing the app forward (for
                screenshots and scripts while someone is working)
  --component NAME
                open the components window on NAME as well
  --catalogue   print the component catalogue (markdown) and exit; fails if a
                component is broken or says nothing of when to use it
  -h, --help    show this help";

/// The app's id for the window system: every window's, and the bundle's
/// (scripts/bundle-app.sh). Change it with the name when you fork. A
/// profile adds its name, so each app shows as its own.
pub(crate) const APP_ID: &str = "dev.moonframe.app";

/// This run's app id, set once at launch; see [`APP_ID`].
static RUNNING_ID: OnceLock<String> = OnceLock::new();

/// The app id every window of this run opens with.
pub(crate) fn app_id() -> String {
    RUNNING_ID
        .get()
        .cloned()
        .unwrap_or_else(|| APP_ID.to_owned())
}

/// Whether this run is a release (`--release`, or an app bundle): set once
/// at launch, read by the menus and key bindings.
static RELEASE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether developer tools are left out; see [`RELEASE`].
pub(crate) fn release() -> bool {
    RELEASE.load(std::sync::atomic::Ordering::Relaxed)
}

/// This run's name for the menu bar: the app's title in a profile.
static TITLE: OnceLock<String> = OnceLock::new();

/// The app's name in the menu bar; see [`TITLE`].
pub(crate) fn title() -> String {
    TITLE
        .get()
        .cloned()
        .unwrap_or_else(|| "Moonframe".to_owned())
}

/// Whether this run opens its windows behind others (`--background`): set
/// once at launch, read by every window the app opens.
static BACKGROUND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether windows open without taking focus; see [`BACKGROUND`].
pub(crate) fn background() -> bool {
    BACKGROUND.load(std::sync::atomic::Ordering::Relaxed)
}

struct Args {
    folders: Folders,
    check: bool,
    /// Open without activating the app or focusing its window.
    background: bool,
    /// `--release`, or `--dev` (`Some(false)`); else as the bundle says.
    release: Option<bool>,
    catalogue: bool,
    component: Option<String>,
    /// The words after `mod`, for the package manager.
    packages: Option<Vec<String>>,
    /// The words after `app`, for profiles.
    apps: Option<Vec<String>>,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut builtin = None;
    let mut user = paths::user();
    let bundled = paths::bundled();
    let mut check = false;
    let mut catalogue = false;
    let mut background = false;
    let mut release = None;
    let mut component = None;
    let mut packages = None;
    let mut apps = None;
    let mut profile = None;
    let mut args = std::env::args().skip(1).collect::<Vec<_>>().into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--catalogue" => catalogue = true,
            "--background" => background = true,
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
    Ok(Some(Args {
        folders: Folders {
            builtin: builtin.unwrap_or_else(paths::builtin),
            user,
            profile: profile.or_else(|| bundled.as_ref().map(|bundle| bundle.name.clone())),
            bundled,
        },
        check,
        catalogue,
        background,
        release,
        component,
        packages,
        apps,
    }))
}

/// Loads every module headlessly: what an agent runs after an edit.
fn check(roots: Roots) -> ExitCode {
    let workspace = Workspace::open(roots.clone(), false);
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
    let workspace = Workspace::open(roots, false);
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

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("error,moonframe=info")),
        )
        .with_writer(std::io::stderr)
        .init();
    let args = match parse_args() {
        Ok(Some(args)) => args,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        },
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            return ExitCode::from(2);
        },
    };
    let folders = args.folders;
    if let Some(words) = &args.apps {
        let Some(user) = &folders.user else {
            eprintln!("moonframe app: profiles go in the user's folder; drop --no-user");
            return ExitCode::from(2);
        };
        return profiles::run(user, words);
    }
    if let (Some(name), Some(user), None) = (&folders.profile, &folders.user, &folders.bundled)
        && !paths::profile(user, name).is_dir()
    {
        eprintln!(
            "moonframe: no profile `{name}` in {}; `moonframe app list` shows them, \
             `moonframe app install` adds one",
            user.display()
        );
        return ExitCode::from(2);
    }
    if let Some(words) = &args.packages {
        let Some(own) = folders.own() else {
            eprintln!("moonframe mod: packages go in the user's folder; drop --no-user");
            return ExitCode::from(2);
        };
        return packages::run(&own, words);
    }
    if args.check {
        return check(folders.roots());
    }
    if args.catalogue {
        return catalogue(folders.roots());
    }

    // The user's folder, or the bundled app's profile in it, exists from
    // the first run, so it is watched and easy to find.
    if let Some(own) = folders.own()
        && let Err(error) = fs::create_dir_all(&own)
    {
        tracing::warn!("cannot create {}: {error}", own.display());
    }
    RELEASE.store(
        args.release.unwrap_or(folders.bundled.is_some()),
        std::sync::atomic::Ordering::Relaxed,
    );
    let _ = TITLE.set(folders.title());
    dock::set_name(&title());
    tracing::info!(
        "starting {} ({}{}) on {}",
        title(),
        folders.app_id(),
        if release() {
            ", release"
        } else {
            ""
        },
        folders.roots()
    );
    let icon = folders.icon();
    let background = args.background;
    BACKGROUND.store(background, std::sync::atomic::Ordering::Relaxed);
    let _ = RUNNING_ID.set(folders.app_id());
    if let Some(profile) = &folders.profile {
        moonframe_providers::set_profile(profile);
    }
    let component = args.component.filter(|_| !release());
    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            dock::set_icon(icon.as_deref());
            actions::bind_keys(cx);
            cx.set_menus(actions::menus(&[], &[]));
            cx.on_action(|_: &actions::Quit, cx| cx.quit());
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            // Each mod's state and the topics come back from the last run:
            // from herdr-core when it runs here, else SQLite, each profile
            // its own. Only with the user's folder: `--no-user` runs
            // (checks, screenshots) start fresh and keep nothing.
            let backends = Arc::new(Backends::new());
            let persist = folders
                .user
                .as_ref()
                .and_then(|_| persist::open(&backends, folders.profile.as_deref()));
            // The shell says what window it wants, so it loads first.
            let workspace = Workspace::open_with(folders.roots(), cx.theme().is_dark(), persist);
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
                app_id: Some(app_id()),
                focus: !background,
                ..framed(chrome)
            };
            let opened = cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| {
                    let mut app =
                        MoonframeApp::new(workspace, chrome, folders.clone(), backends, window, cx);
                    if let Some(name) = &component {
                        app.show_component(name, cx);
                    }
                    app
                });
                cx.new(|cx| Root::new(view, window, cx))
            });
            if let Err(error) = opened {
                eprintln!("moonframe: unable to open a window: {error}");
                cx.quit();
            }
            if !background {
                cx.activate(true);
            }
        });
    ExitCode::SUCCESS
}
