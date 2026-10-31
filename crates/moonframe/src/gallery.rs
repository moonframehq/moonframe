//! The component browser's own window, so browsing never blocks the main
//! one. [`MoonframeApp`] draws its content (the previews go through the same
//! painter as the main window), and it draws again whenever the app changes:
//! a reload, a new choice, a theme switch.

use {
    crate::{
        actions::{Dismiss, ShowComponent},
        app::MoonframeApp,
    },
    gpui_kit::{
        component::{ActiveTheme as _, Root},
        *,
    },
};

const WIDTH: f32 = 900.;
const HEIGHT: f32 = 620.;

pub(crate) struct Gallery {
    app: Entity<MoonframeApp>,
    focus: FocusHandle,
    _changes: Subscription,
}

impl Gallery {
    /// Opens the window; the caller keeps the handle to raise or close it.
    pub(crate) fn open(app: Entity<MoonframeApp>, cx: &mut App) -> Option<WindowHandle<Root>> {
        let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(560.), px(360.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Components".into()),
                ..Default::default()
            }),
            app_id: Some("dev.moonframe.app".into()),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| Self::new(app, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .inspect_err(|error| tracing::warn!("cannot open the components window: {error}"))
        .ok()
    }

    fn new(app: Entity<MoonframeApp>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            _changes: cx.observe(&app, |_, _, cx| cx.notify()),
            app,
            focus,
        }
    }
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self.app.update(cx, |app, cx| app.gallery_content(cx));
        let theme = cx.theme();
        div()
            .key_context("Gallery")
            .track_focus(&self.focus)
            .on_action(|_: &Dismiss, window, _| window.remove_window())
            .on_action(cx.listener(|this, action: &ShowComponent, _, cx| {
                this.app
                    .update(cx, |app, cx| app.show_component(&action.name, cx));
            }))
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(theme.font_family.clone())
            .text_size(theme.font_size)
            .child(content)
    }
}
