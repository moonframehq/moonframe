//! Optional developer export of our GPU scene, not the user's desktop.
//! `MOONFRAME_CAPTURE=/absolute/file.png` saves once, then exits this process.
use {
    gpui_kit::Window,
    std::{
        path::PathBuf,
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    },
};

static SCHEDULED: AtomicBool = AtomicBool::new(false);

pub(super) fn schedule(window: &Window) {
    let Some(path) = std::env::var_os("MOONFRAME_CAPTURE").map(PathBuf::from) else {
        return;
    };
    if !path.is_absolute() || SCHEDULED.swap(true, Ordering::Relaxed) {
        return;
    }
    after_frame(window, path, Instant::now());
}

fn after_frame(window: &Window, path: PathBuf, started: Instant) {
    window.on_next_frame(move |window, cx| {
        // Let entry transitions and initial layout settle before exporting.
        if started.elapsed() < Duration::from_millis(500) {
            after_frame(window, path, started);
            return;
        }
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
}
