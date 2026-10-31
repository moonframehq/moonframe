//! A window whose whole layout comes from Luau modules: the app's own
//! folder with the user's `~/.config/moonframe` over it, reloaded as
//! they change.

mod actions;
mod app;
mod branch_bar;
mod color;
mod dock;
mod gallery;
mod inspect;
mod motion;
mod overlay;
mod packages;
mod paths;
mod render;
mod settings;
mod terminal;
mod theme_picker;
mod themes;

use {
    app::MoonframeApp,
    gpui_kit::{AppContext as _, Bounds, WindowBounds, WindowOptions, component::Root, px, size},
    moonframe_core::{Roots, Workspace},
    std::{fs, path::PathBuf, process::ExitCode},
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
  -h, --help    show this help";

struct Args {
    builtin: PathBuf,
    user: Option<PathBuf>,
    check: bool,
    /// The words after `mod`, for the package manager.
    packages: Option<Vec<String>>,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut builtin = None;
    let mut user = paths::user();
    let mut check = false;
    let mut packages = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
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

    // The user's folder exists from the first run, so it is watched and
    // easy to find.
    if let Some(user) = &args.user
        && let Err(error) = fs::create_dir_all(user)
    {
        tracing::warn!("cannot create {}: {error}", user.display());
    }
    let (builtin, user) = (args.builtin, args.user);
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
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

            let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(640.), px(400.))),
                app_id: Some("dev.moonframe.app".into()),
                ..Default::default()
            };
            let opened = cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| MoonframeApp::new(builtin, user, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            });
            if let Err(error) = opened {
                eprintln!("moonframe: unable to open a window: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
    ExitCode::SUCCESS
}
