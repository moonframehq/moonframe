//! The main window's view: the app, and the host views drawn over it (see
//! `crate::hosted::layers`).
//!
//! GPUI draws again every view a notified view was drawn inside. The app is
//! drawn here as a cached view, beside the host views rather than around
//! them, so a host view drawing again by itself draws this view and GPUI's
//! root, and the app is replayed as it was painted. Anything else the app
//! does that needs drawing notifies it, which draws this view too.
//!
//! The window's keys and actions are here, around both, so that a key typed
//! in a host view reaches them as it did when the view was inside the app.

use super::*;

/// The app, and the host views it shows.
pub(crate) struct MainWindow {
    app: Entity<MoonframeApp>,
    /// Keep the window's place for the next launch, while it is open.
    _placement: Option<[Subscription; 2]>,
}

impl MainWindow {
    /// The window showing `app`, saving its place to `placement` as it
    /// changes when that is given.
    pub(crate) fn new(
        app: Entity<MoonframeApp>,
        placement: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            app,
            _placement: placement.map(|path| crate::placement::remember(path, window, cx)),
        }
    }

    /// Runs `f` on the app, from one of the window's actions.
    fn with_app(
        &self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut MoonframeApp, &mut Context<MoonframeApp>),
    ) {
        self.app.update(cx, f);
    }
}

impl Render for MainWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.app.read(cx);
        let layers = app.views.layers().clone();
        let focus = app.focus.clone();
        // While inspecting, every node notes where it lands in each frame.
        let replayed = layers.any() && app.inspector.sink().is_none();
        layers.set_replayed(replayed);
        let view = AnyView::from(self.app.clone());
        let content = if replayed {
            let mut style = StyleRefinement::default();
            style.size.width = Some(relative(1.).into());
            style.size.height = Some(relative(1.).into());
            view.cached(style).into_any_element()
        } else {
            view.into_any_element()
        };
        div()
            .id("moonframe-window")
            .key_context(if crate::release() {
                "MoonframeApp"
            } else {
                "MoonframeApp developer"
            })
            .track_focus(&focus)
            .on_action(cx.listener(|this, _: &ToggleTheme, window, cx| {
                this.with_app(cx, |app, cx| app.run(Command::ToggleTheme, window, cx));
            }))
            .on_action(cx.listener(|this, _: &Inspect, _, cx| {
                this.with_app(cx, |app, cx| app.toggle_inspect(cx));
            }))
            .on_action(cx.listener(|this, action: &QuickInspect, _, cx| {
                this.with_app(cx, |app, cx| {
                    if !app.quick_inspect_key(&action.keystroke, cx) {
                        cx.propagate();
                    }
                });
            }))
            .on_action(cx.listener(|this, action: &MenuSend, window, cx| {
                this.with_app(cx, |app, cx| app.run_menu_item(&action.id, window, cx));
            }))
            .on_action(cx.listener(|this, action: &MenuKey, window, cx| {
                this.with_app(cx, |app, cx| {
                    if !app.run_menu_key(&action.keystroke, window, cx) {
                        cx.propagate();
                    }
                });
            }))
            .on_action(cx.listener(|this, _: &Dismiss, _, cx| {
                this.with_app(cx, |app, cx| app.dismiss(cx));
            }))
            .relative()
            .size_full()
            .child(content)
            .child(layers.element())
    }
}
