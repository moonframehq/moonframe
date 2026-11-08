//! Optional developer export of our GPU scene, not the user's desktop.
//! `MOONFRAME_CAPTURE=/absolute/file.png` saves once, then exits this process;
//! `MOONFRAME_CAPTURE_DELAY=<ms>` waits that long first (750 by default).
use {
    gpui_kit::{App, Window},
    std::{
        path::PathBuf,
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    },
};

static SCHEDULED: AtomicBool = AtomicBool::new(false);

/// How often the window is drawn while waiting to capture it.
const STEP: Duration = Duration::from_millis(250);

fn draw(window: &mut Window, cx: &mut App) {
    window.refresh();
    window.draw(cx).clear(cx);
}

pub(super) fn schedule(window: &Window, cx: &App) {
    let Some(path) = std::env::var_os("MOONFRAME_CAPTURE").map(PathBuf::from) else {
        return;
    };
    if !path.is_absolute() || SCHEDULED.swap(true, Ordering::Relaxed) {
        return;
    }
    // `MOONFRAME_CAPTURE_DELAY` (milliseconds) waits longer, for providers
    // and network content such as map tiles to arrive first.
    let delay = std::env::var("MOONFRAME_CAPTURE_DELAY")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map_or(Duration::from_millis(750), Duration::from_millis);
    // A window behind others or on a locked screen gets no frame callbacks,
    // so nothing it was told since would show, and terminals, which start as
    // they are drawn, would not start. A timer still runs: draw from it.
    window
        .spawn(cx, async move |cx| {
            let end = Instant::now() + delay;
            while Instant::now() < end {
                cx.background_executor().timer(STEP).await;
                let _ = cx.update(draw);
            }
            let _ = cx.update(|window, cx| {
                draw(window, cx);
                match window.render_to_image() {
                    Ok(image) => {
                        if let Err(error) = image.save(&path) {
                            eprintln!("cannot save {}: {error}", path.display());
                        }
                    },
                    Err(error) => eprintln!("cannot render {}: {error}", path.display()),
                }
                cx.quit();
            });
        })
        .detach();
}
