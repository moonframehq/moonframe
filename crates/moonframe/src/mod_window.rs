//! A mod in a window of its own (`window = "<mod>"` on a control): a tool
//! that stays open beside the main window, such as the layout picker.
//! [`MoonframeApp`] draws its content through the same painter as the main
//! window, and it draws again whenever the app changes, so what it sends
//! shows in both at once.

use {
    crate::{
        actions::Dismiss,
        app::{MoonframeApp, window_state},
    },
    gpui_kit::{
        component::{ActiveTheme as _, Root},
        *,
    },
    moonframe_core::{ModWindowSize, Titlebar},
};

/// The size a mod's window opens at, and its smallest, when its `window`
/// key does not say.
const SIZE: (f32, f32) = (420., 560.);
const MIN_SIZE: (f32, f32) = (280., 240.);

pub(crate) struct ModWindow {
    app: Entity<MoonframeApp>,
    /// The mod it shows.
    name: String,
    titlebar: Titlebar,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl ModWindow {
    /// Opens the window titled `title`, at the size the mod asks for;
    /// the caller keeps the handle to raise or close it.
    pub(crate) fn open(
        app: Entity<MoonframeApp>,
        name: String,
        title: String,
        sizing: ModWindowSize,
        cx: &mut App,
    ) -> Option<WindowHandle<Root>> {
        let screen = cx
            .primary_display()
            .map(|display| display.bounds().size)
            .map(|screen| (f32::from(screen.width), f32::from(screen.height)));
        let hidden = sizing.titlebar == Titlebar::Hidden;
        let (opens, smallest) = fitted(sizing, screen);
        let bounds = Bounds::centered(None, size(px(opens.0), px(opens.1)), cx);
        let options = WindowOptions {
            focus: !crate::background(),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(smallest.0), px(smallest.1))),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                appears_transparent: hidden,
                traffic_light_position: sizing
                    .traffic_lights
                    .filter(|_| hidden)
                    .map(|(x, y)| point(px(x), px(y))),
            }),
            window_decorations: hidden.then_some(WindowDecorations::Client),
            app_owns_titlebar_drag: hidden,
            app_id: Some(crate::APP_ID.into()),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            let view = cx.new(|cx: &mut Context<Self>| {
                let focus = cx.focus_handle();
                focus.focus(window, cx);
                let subscriptions = vec![
                    cx.observe(&app, |_, _, cx| cx.notify()),
                    cx.observe_window_bounds(window, |this, window, cx| {
                        this.sync_window(window, cx);
                    }),
                    cx.observe_window_activation(window, |this, window, cx| {
                        this.sync_window(window, cx);
                    }),
                    cx.on_release(|this, cx| {
                        this.app.update(cx, |app, cx| {
                            app.sync_mod_window(&this.name, None, cx);
                        });
                    }),
                ];
                let this = Self {
                    _subscriptions: subscriptions,
                    app,
                    name,
                    titlebar: sizing.titlebar,
                    focus,
                };
                this.sync_window(window, cx);
                this
            });
            cx.new(|cx| Root::new(view, window, cx))
        })
        .inspect_err(|error| tracing::warn!("cannot open a mod's window: {error}"))
        .ok()
    }

    fn sync_window(&self, window: &Window, cx: &mut Context<Self>) {
        let state = window_state(self.titlebar, window);
        self.app.update(cx, |app, cx| {
            app.sync_mod_window(&self.name, Some(state), cx);
        });
    }
}

impl Render for ModWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name = self.name.clone();
        let (content, animating) = self
            .app
            .update(cx, |app, cx| app.mod_window_content(&name, window, cx));
        if animating {
            window.request_animation_frame();
        }
        let theme = cx.theme();
        div()
            .key_context("ModWindow")
            .track_focus(&self.focus)
            .on_action(|_: &Dismiss, window, _| window.remove_window())
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(theme.font_family.clone())
            .text_size(theme.font_size)
            .child(content)
    }
}

/// The size a mod's window opens at and its smallest, as `sizing` asks,
/// kept within a `screen` of that size as the main window is.
fn fitted(sizing: ModWindowSize, screen: Option<(f32, f32)>) -> ((f32, f32), (f32, f32)) {
    let (mut width, mut height) = sizing.size.unwrap_or(SIZE);
    let (mut min_width, mut min_height) = sizing.min_size.unwrap_or(MIN_SIZE);
    // An explicit minimum can outgrow the default opening size. Only the
    // screen may shrink it; an explicit size was checked against it.
    if sizing.size.is_none() {
        width = width.max(min_width);
        height = height.max(min_height);
    }
    if let Some((screen_width, screen_height)) = screen {
        width = width.min(screen_width - 40.);
        height = height.min(screen_height - 60.);
    }
    // Never smaller than its smallest, which the screen may have shrunk.
    min_width = min_width.min(width);
    min_height = min_height.min(height);
    ((width, height), (min_width, min_height))
}

#[cfg(test)]
mod tests {
    use super::{MIN_SIZE, ModWindowSize, SIZE, fitted};

    #[test]
    fn an_explicit_minimum_grows_the_default_opening_size() {
        let asked = ModWindowSize {
            size: None,
            min_size: Some((640., 480.)),
            ..ModWindowSize::default()
        };
        assert_eq!(fitted(asked, None), ((640., 560.), (640., 480.)));
        assert_eq!(
            fitted(asked, Some((1920., 1080.))),
            ((640., 560.), (640., 480.))
        );
        let tall = ModWindowSize {
            size: None,
            min_size: Some((300., 700.)),
            ..ModWindowSize::default()
        };
        assert_eq!(fitted(tall, None), ((420., 700.), (300., 700.)));
        // A screen smaller than the minimum still limits both sizes.
        assert_eq!(
            fitted(asked, Some((600., 500.))),
            ((560., 440.), (560., 440.))
        );
    }

    #[test]
    fn a_window_opens_at_the_mods_size_within_the_screen() {
        let unset = ModWindowSize::default();
        assert_eq!(fitted(unset, None), (SIZE, MIN_SIZE));
        let asked = ModWindowSize {
            size: Some((860., 600.)),
            min_size: Some((640., 480.)),
            ..ModWindowSize::default()
        };
        assert_eq!(
            fitted(asked, Some((1920., 1080.))),
            ((860., 600.), (640., 480.))
        );
        // A small screen shrinks it, and its smallest with it.
        assert_eq!(
            fitted(asked, Some((700., 500.))),
            ((660., 440.), (640., 440.))
        );
        // The default smallest is smaller than what a mod may ask.
        let small = ModWindowSize {
            size: Some((240., 200.)),
            min_size: None,
            ..ModWindowSize::default()
        };
        assert_eq!(fitted(small, None), ((240., 200.), (240., 200.)));
    }
}
