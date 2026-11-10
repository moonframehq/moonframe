//! The headless window an entry is drawn in: the app as `e2e.rs` opens it,
//! on GPUI's test platform with the system's text and a GPU renderer.

use {
    super::{ElementBounds, RenderedEntry},
    crate::{
        app::{Hub, Launch, MoonframeApp},
        backends::Backends,
        hosted::Registry,
    },
    gpui_kit::{
        AnyWindowHandle, AppContext as _, Entity, HeadlessAppContext, PlatformTextSystem,
        component::Root, px, size,
    },
    moonframe_core::{Data, Roots, SHELL, entries::Entries},
    moonframe_packages::paths::Folders,
    moonframe_providers::Natives,
    std::{path::Path, sync::Arc},
};

/// Frames drawn before the capture: parses land, messages arrive and
/// what a frame asks for shows in the next.
const SETTLE: usize = 4;

/// What to draw.
pub(super) struct Job<'a> {
    /// The entry, `<namespace>:<id>`.
    pub(super) name: &'a str,
    pub(super) roots: Roots,
    pub(super) entries: Entries,
    /// The shell's folder.
    pub(super) folder: &'a Path,
    /// The `app` keys the view reads.
    pub(super) app: Vec<(String, Data)>,
    /// Sent to the entry, in turn, before the capture.
    pub(super) messages: &'a [Data],
    /// The entry's starting state, as `init(previous)` gets it.
    pub(super) state: Option<&'a Data>,
    pub(super) size: (f32, f32),
}

pub(super) fn draw(job: Job<'_>) -> Result<RenderedEntry, String> {
    let renderer = gpui_platform_capture::current_headless_renderer()
        .map_err(|error| format!("no headless renderer: {error}"))?;
    if renderer.is_none() {
        return Err("this platform has no headless renderer to draw an entry with".to_owned());
    }
    drop(renderer);
    let mut cx = HeadlessAppContext::with_platform(
        text_system(),
        Arc::new(crate::assets::Assets::new(None)),
        gpui_platform_capture::current_headless_renderer,
    );
    let (handle, app) = open(&mut cx, &job)?;
    cx.update_entity(&app, |app, cx| {
        for (key, value) in job.app {
            app.workspace.set_host(&key, value);
        }
        if let Some(state) = job.state {
            app.workspace.restart(job.name, Some(state.clone()));
        }
        cx.notify();
    });
    settle(&mut cx, handle)?;
    for message in job.messages {
        cx.update_entity(&app, |app, cx| {
            app.workspace.send(SHELL, job.name, message.clone());
            cx.notify();
        });
        settle(&mut cx, handle)?;
    }
    let image = cx
        .capture_screenshot(handle)
        .map_err(|error| format!("cannot capture the window: {error}"))?;
    let (width, height) = image.dimensions();
    let mut png = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| format!("cannot encode the PNG: {error}"))?;
    let snapshot = cx.read_entity(&app, |app, _| app.workspace.view_snapshot(job.name));
    let bounds = bounds(&mut cx, handle, &app, job.name)?;
    Ok(RenderedEntry {
        png: png.into_inner(),
        width,
        height,
        snapshot,
        bounds,
    })
}

/// The system's text shaping. On macOS, without the platform, which only
/// the main thread may create (and a host already has).
#[cfg(target_os = "macos")]
fn text_system() -> Arc<dyn PlatformTextSystem> {
    Arc::new(gpui_apple_capture::AppleTextSystem::new())
}

#[cfg(not(target_os = "macos"))]
fn text_system() -> Arc<dyn PlatformTextSystem> {
    gpui_platform_capture::current_platform(true).text_system()
}

/// The app's window on `job`'s roots, at its size.
fn open(
    cx: &mut HeadlessAppContext,
    job: &Job<'_>,
) -> Result<(AnyWindowHandle, Entity<MoonframeApp>), String> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::actions::bind_keys(cx);
    });
    let workspace = crate::capabilities::open(job.roots.clone(), false, None);
    let chrome = workspace.chrome();
    let folders = Folders {
        builtin: job.folder.to_owned(),
        host: None,
        user: None,
        profile: None,
        bundled: None,
    };
    let backends = Arc::new(Backends::for_app("render-entry"));
    let views = Registry::default();
    cx.update(|cx| {
        Hub::install(
            &workspace,
            Natives::default(),
            Launch {
                view_diagnostics: None,
                entries: job.entries.clone(),
                providers: Default::default(),
                folders: folders.clone(),
                backends: backends.clone(),
                views: views.clone(),
                chrome,
            },
            cx,
        );
    });
    let mut app = None;
    let (width, height) = job.size;
    let handle = cx
        .open_window(size(px(width), px(height)), |window, cx| {
            let view = cx.new(|cx| {
                MoonframeApp::new(workspace, chrome, folders, backends, views, window, cx)
            });
            app = Some(view.clone());
            let view = cx.new(|cx| crate::app::MainWindow::new(view, None, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .map_err(|error| format!("cannot open a headless window: {error}"))?;
    let app = app.ok_or("the window opened without the app")?;
    Ok((handle.into(), app))
}

/// Lets parses land, messages arrive and frames draw.
fn settle(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) -> Result<(), String> {
    for _ in 0..SETTLE {
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.simulate_next_frame(cx);
            window.refresh();
            window.draw(cx).clear(cx);
        })
        .map_err(|error| format!("cannot draw the window: {error}"))?;
        cx.run_until_parked();
    }
    Ok(())
}

/// Where the entry's elements were laid out: one more frame drawn with
/// the inspector's tracking on (after the capture, so it shows nothing),
/// keeping the nodes a mod of the entry drew.
fn bounds(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    app: &Entity<MoonframeApp>,
    name: &str,
) -> Result<Vec<ElementBounds>, String> {
    cx.update_entity(app, |app, cx| {
        app.inspector.picking = true;
        cx.notify();
    });
    settle(cx, handle)?;
    let instance = format!("{name}#");
    let ours = |owner: &str| owner == name || owner.starts_with(&instance);
    Ok(cx.read_entity(app, |app, _| {
        app.inspector
            .recorded()
            .borrow()
            .iter()
            .filter(|placed| {
                placed
                    .crumb
                    .trail()
                    .iter()
                    .any(|crumb| crumb.owner.as_deref().is_some_and(ours))
            })
            .map(|placed| ElementBounds {
                path: placed.path.clone(),
                x: f32::from(placed.bounds.origin.x),
                y: f32::from(placed.bounds.origin.y),
                width: f32::from(placed.bounds.size.width),
                height: f32::from(placed.bounds.size.height),
            })
            .collect()
    }))
}
