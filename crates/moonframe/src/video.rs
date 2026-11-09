//! `ui.video`: frames from another process on the screen. Each one is a
//! [`Live`]: a native surface over the window, and a thread that takes the
//! newest frame from a ring in shared memory and draws it there, so a
//! frame never passes through Luau or the window's drawing, and video and
//! the rest of the window can't hold each other up.
//!
//! They live on the main thread, by window, mod and id, so that a mod in a
//! window of its own (a second screen) can show one as the main window
//! does.

mod geometry;
mod render;
mod stream;
mod surface;

use {
    futures::channel::mpsc::UnboundedSender,
    gpui_kit::*,
    moonframe_core::{Data, Video},
    std::{cell::RefCell, collections::HashMap, sync::Arc, thread},
    stream::Shared,
    surface::Surface,
};

/// The most videos the app keeps.
const MAX_VIDEOS: usize = 32;

thread_local! {
    /// Where videos in each main window send the mod that drew them
    /// messages: that window's app.
    static MESSAGES: RefCell<Vec<(WindowId, UnboundedSender<(String, Data)>)>> =
        const { RefCell::new(Vec::new()) };
}

/// Sets where videos in the main window `window` send messages, as it
/// opens.
pub(crate) fn deliver_to(window: WindowId, sender: UnboundedSender<(String, Data)>) {
    MESSAGES.with_borrow_mut(|senders| senders.push((window, sender)));
}

/// Where a video in `window` sends messages: that main window's app, else
/// (a mod's window) the app that opened last and is still open.
fn messages_for(window: WindowId) -> Option<UnboundedSender<(String, Data)>> {
    MESSAGES.with_borrow_mut(|senders| {
        senders.retain(|(_, sender)| !sender.is_closed());
        senders
            .iter()
            .find(|(id, _)| *id == window)
            .or_else(|| senders.last())
            .map(|(_, sender)| sender.clone())
    })
}

/// A video on the screen.
struct Live {
    window: WindowId,
    surface: Surface,
    shared: Arc<Shared>,
    /// Painted since the window last swept.
    painted: bool,
}

impl Drop for Live {
    fn drop(&mut self) {
        self.shared.stop();
    }
}

thread_local! {
    static LIVE: RefCell<HashMap<String, Live>> = RefCell::new(HashMap::new());
}

/// Shows `video` at `bounds`: makes what it needs the first time, then
/// hands it what the mod's view says now. An answer is a problem to show.
pub(crate) fn paint(
    owner: &str,
    video: &Video,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) -> Result<(), String> {
    let id = window.window_handle().window_id();
    let key = format!("{id:?}/{owner}/{}", video.id);
    LIVE.with(|live| {
        let mut live = live.borrow_mut();
        if !live.contains_key(&key) {
            if live.len() >= MAX_VIDEOS {
                return Err(format!("the app already has {MAX_VIDEOS} videos"));
            }
            live.insert(key.clone(), start(owner, video, id, window)?);
        }
        let Some(this) = live.get_mut(&key) else {
            return Ok(());
        };
        this.painted = true;
        this.shared.update(video);
        this.surface
            .set_color(video.view.color, video.view.extended);
        let covered = crate::browser::covers(id)
            .iter()
            .any(|cover| cover.intersects(&bounds));
        let visible = bounds.intersect(&window.content_mask().bounds);
        let shows = !covered && visible.size.width > px(0.) && visible.size.height > px(0.);
        if shows && this.surface.place(bounds, visible, window.scale_factor()) {
            this.shared.set_density(window.scale_factor());
            this.shared.touch();
        }
        this.surface.show(shows);
        this.shared.pause(!shows);
        Ok(())
    })
}

fn start(owner: &str, video: &Video, id: WindowId, window: &Window) -> Result<Live, String> {
    let device = metal::Device::system_default().ok_or("this Mac has no Metal device")?;
    let surface = Surface::new(window, &device)?;
    let renderer = render::Renderer::new(device, surface.layer())?;
    let shared = Shared::new(video);
    let messages = messages_for(id).ok_or("videos aren't ready")?;
    let (thread_shared, owner) = (shared.clone(), owner.to_owned());
    thread::Builder::new()
        .name(format!("video-{}", video.id))
        .spawn(move || stream::run(&thread_shared, renderer, &owner, &messages))
        .map_err(|error| format!("cannot start a thread: {error}"))?;
    Ok(Live {
        window: id,
        surface,
        shared,
        painted: false,
    })
}

/// Draws nothing; hides the window's videos that this frame didn't paint
/// (a tab that closed, a pane that went). Goes last in the window.
pub(crate) fn sweep() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |_, _, window, _| {
            let id = window.window_handle().window_id();
            LIVE.with(|live| {
                for this in live.borrow_mut().values_mut() {
                    if this.window == id {
                        if !this.painted {
                            this.surface.show(false);
                            this.shared.pause(true);
                        }
                        this.painted = false;
                    }
                }
            });
        },
    )
    .absolute()
}

/// Lets go of the videos of a window that closed.
pub(crate) fn window_closed(id: WindowId) {
    LIVE.with(|live| live.borrow_mut().retain(|_, this| this.window != id));
}
