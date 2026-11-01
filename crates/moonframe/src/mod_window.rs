//! A mod in a window of its own (`window = "<mod>"` on a control): a tool
//! that stays open beside the main window, such as the layout picker.
//! [`MoonframeApp`] draws its content through the same painter as the main
//! window, and it draws again whenever the app changes, so what it sends
//! shows in both at once.

use {
    crate::{actions::Dismiss, app::MoonframeApp},
    gpui_kit::{
        component::{ActiveTheme as _, Root},
        *,
    },
};

const WIDTH: f32 = 420.;
const HEIGHT: f32 = 560.;

pub(crate) struct ModWindow {
    app: Entity<MoonframeApp>,
    /// The mod it shows.
    name: String,
    focus: FocusHandle,
    _changes: Subscription,
}

impl ModWindow {
    /// Opens the window titled `title`; the caller keeps the handle to raise
    /// or close it.
    pub(crate) fn open(
        app: Entity<MoonframeApp>,
        name: String,
        title: String,
        cx: &mut App,
    ) -> Option<WindowHandle<Root>> {
        let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
        let options = WindowOptions {
            focus: !crate::background(),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(280.), px(240.))),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            }),
            app_id: Some(crate::APP_ID.into()),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| {
                let focus = cx.focus_handle();
                focus.focus(window, cx);
                Self {
                    _changes: cx.observe(&app, |_, _, cx| cx.notify()),
                    app,
                    name,
                    focus,
                }
            });
            cx.new(|cx| Root::new(view, window, cx))
        })
        .inspect_err(|error| tracing::warn!("cannot open a mod's window: {error}"))
        .ok()
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
