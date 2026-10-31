//! A window whose whole layout comes from Luau modules: the app's own
//! folder with the user's `~/.config/moonframe` over it, reloaded as
//! they change.

mod actions;
mod app;
mod assets;
mod branch_bar;
mod browser;
mod color;
mod dock;
mod gallery;
mod input;
mod inspect;
mod keys;
mod motion;
mod overlay;
mod packages;
mod paths;
mod persist;
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
    std::{fs, path::PathBuf, process::ExitCode, sync::Arc},
};

const USAGE: &str = "\
moonframe: a window laid out by Luau modules

Usage: moonframe [OPTIONS] [DIR]
       moonframe [--user DIR] mod <COMMAND>   install and share mods; see `mod help`

  DIR           the built-in app folder: main.luau, style.luau, mods/, providers/
                (default: Contents/Resources/app in the bundle, else ./app)

The user's folder goes over it: a mod or provider there replaces the
built-in one of the same name, a main.luau replaces the shell, and a
style.luau is merged over the built-in one. Packages installed with
`moonframe mod install` go between the two.

Options:
  --user DIR    the user's folder (default: $XDG_CONFIG_HOME/moonframe,
                or ~/.config/moonframe)
  --no-user     the built-in app alone
  --check       load every module, report problems, and exit without a window
  --background  open the window without bringing the app forward (for
                screenshots and scripts while someone is working)
  --component NAME
                open the components window on NAME as well
  --catalogue   print the component catalogue (markdown) and exit; fails if a
                component is broken or says nothing of when to use it
  -h, --help    show this help";

/// The app's id for the window system: every window's, and the bundle's
/// (scripts/bundle-app.sh). Change it with the name when you fork.
pub(crate) const APP_ID: &str = "dev.moonframe.app";

/// Whether this run opens its windows behind others (`--background`): set
/// once at launch, read by every window the app opens.
static BACKGROUND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether windows open without taking focus; see [`BACKGROUND`].
pub(crate) fn background() -> bool {
    BACKGROUND.load(std::sync::atomic::Ordering::Relaxed)
}

struct Args {
    builtin: PathBuf,
    user: Option<PathBuf>,
    check: bool,
    /// Open without activating the app or focusing its window.
    background: bool,
    catalogue: bool,
    component: Option<String>,
    /// The words after `mod`, for the package manager.
    packages: Option<Vec<String>>,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut builtin = None;
    let mut user = paths::user();
    let mut check = false;
    let mut catalogue = false;
    let mut background = false;
    let mut component = None;
    let mut packages = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--catalogue" => catalogue = true,
            "--background" => background = true,
            "--component" => component = Some(args.next().ok_or("--component needs a name")?),
            "--user" => {
                user = Some(PathBuf::from(args.next().ok_or("--user needs a folder")?));
            },
            "--no-user" => user = None,
            "-h" | "--help" => return Ok(None),
            flag if flag.starts_with('-') => return Err(format!("unknown option: {flag}")),
            "mod" | "mods" if builtin.is_none() => {
                packages = Some(args.by_ref().collect());
            },
            path if builtin.is_none() => builtin = Some(PathBuf::from(path)),
            extra => return Err(format!("unexpected argument: {extra}")),
        }
    }
    Ok(Some(Args {
        builtin: builtin.unwrap_or_else(paths::builtin),
        user,
        check,
        catalogue,
        background,
        component,
        packages,
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
    if let Some(words) = &args.packages {
        let Some(user) = &args.user else {
            eprintln!("moonframe mod: packages go in the user's folder; drop --no-user");
            return ExitCode::from(2);
        };
        return packages::run(user, words);
    }
    if args.check {
        return check(paths::roots(args.builtin, args.user));
    }
    if args.catalogue {
        return catalogue(paths::roots(args.builtin, args.user));
    }

    // The user's folder exists from the first run, so it is watched and
    // easy to find.
    if let Some(user) = &args.user
        && let Err(error) = fs::create_dir_all(user)
    {
        tracing::warn!("cannot create {}: {error}", user.display());
    }
    let (builtin, user, background) = (args.builtin, args.user, args.background);
    BACKGROUND.store(background, std::sync::atomic::Ordering::Relaxed);
    let component = args.component;
    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            dock::set_icon();
            actions::bind_keys(cx);
            cx.set_menus(actions::menus(&[]));
            cx.on_action(|_: &actions::Quit, cx| cx.quit());
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            // What mods kept last time, from herdr-core when it runs here,
            // else a file. Only with the user's folder: `--no-user` runs
            // (checks, screenshots) start fresh and keep nothing.
            let backends = Arc::new(Backends::new());
            let (kept, persistence) = match &user {
                Some(_) => {
                    let store = backends.store(paths::NAME, paths::document());
                    let (kept, writable) = persist::load(&*store);
                    let persistence =
                        writable.then(|| persist::Persistence::start(store, kept.clone()));
                    (kept, persistence)
                },
                None => (persist::Kept::new(), None),
            };
            // The shell says what window it wants, so it loads first.
            let workspace = Workspace::open_saved(
                paths::roots(builtin.clone(), user.clone()),
                cx.theme().is_dark(),
                kept,
            );
            let chrome = workspace.chrome();
            let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(640.), px(400.))),
                app_id: Some(APP_ID.into()),
                focus: !background,
                ..framed(chrome)
            };
            let opened = cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| {
                    let mut app = MoonframeApp::new(
                        workspace,
                        chrome,
                        builtin,
                        user,
                        backends,
                        persistence,
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
                eprintln!("moonframe: unable to open a window: {error}");
                cx.quit();
            }
            if !background {
                cx.activate(true);
            }
        });
    ExitCode::SUCCESS
}
