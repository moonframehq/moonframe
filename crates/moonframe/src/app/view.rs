//! Drawing the main window.

use {
    super::{toasts::toast, *},
    crate::render::cached::{Kept, Source},
    std::rc::Rc,
};

impl Render for MoonframeApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(feature = "capture")]
        crate::capture::schedule(window, cx);
        let started = std::time::Instant::now();
        let gallery_open = self.gallery.is_some_and(|gallery| {
            cx.windows()
                .iter()
                .any(|window| window.window_id() == gallery.window_id())
        });
        if !gallery_open {
            self.workspace.show_demos(false);
        }
        self.cached.prepare(cx);
        let inspecting = self.inspector.sink().is_some();
        self.cached.begin(window.viewport_size(), inspecting);
        self.inputs.begin_frame();
        let last_keyed = self.motion.begin_frame();
        #[cfg(feature = "terminal")]
        self.terminals.begin_frame();
        let mut painter = Painter::new(
            &self.workspace,
            &mut self.motion,
            #[cfg(feature = "terminal")]
            &mut self.terminals,
            &self.inputs,
            self.inspector.sink(),
            cx.theme(),
        )
        .with_browsers(&mut self.browsers)
        .with_views(&mut self.views)
        .with_keyed(&last_keyed)
        .with_cached(&mut self.cached)
        .in_window(window.window_handle().window_id());
        let body = painter.shell(cx);
        let mut frame = painter.finish();
        self.cached.finish();
        let animating = frame.animating;
        if animating {
            window.request_animation_frame();
        }
        let live_again = std::mem::take(&mut frame.live_again);
        if !live_again.is_empty() {
            window.on_next_frame(move |_, cx| {
                for view in live_again {
                    cx.notify(view);
                }
            });
        }
        // Building the element tree from the views; GPUI lays it out and
        // paints it after. Slow frames are worth seeing in the log; fast
        // ones while animating would flood even `debug`.
        let built = started.elapsed();
        if built >= moonframe_core::perf::SLOW || !animating {
            moonframe_core::perf!(built, "frame: built in {built:.1?}");
        }
        // Closed windows' layers go: their ids are not reused while open.
        let open: Vec<WindowId> = cx
            .windows()
            .iter()
            .map(AnyWindowHandle::window_id)
            .collect();
        self.key_layers.retain(|id, _| open.contains(id));
        self.after_frame(frame, window, cx);
        // A terminal that just appeared (a new tab, or one selected) takes
        // the keyboard, once it is drawn.
        #[cfg(feature = "terminal")]
        if let Some(focus) = self.terminals.end_frame() {
            window.defer(cx, move |window, cx| focus.focus(window, cx));
        }
        // Shells that exited tell their mods (a tab closes), once drawn.
        #[cfg(feature = "terminal")]
        let exits = self.terminals.take_exits();
        #[cfg(feature = "terminal")]
        if !exits.is_empty() {
            cx.defer_in(window, move |app, _, cx| {
                for (owner, msg) in exits {
                    app.deliver(&owner, msg, cx);
                }
            });
        }
        // The panels follow the project of the terminal last typed in.
        #[cfg(feature = "terminal")]
        if let Some(folder) = self.terminals.focused_folder(window) {
            self.project.follow(folder);
        }
        let problems = self.problems_layer(cx);
        let layers = self.inspector_layers(window.viewport_size(), cx);
        let toast = self.toast.as_ref().map(|(id, message)| {
            let notice = self
                .toast_notice
                .as_ref()
                .is_some_and(|(shown, ..)| shown == id);
            toast(message.clone(), notice, cx)
        });
        let theme = cx.theme();
        div()
            .id("moonframe-app")
            .key_context(if crate::release() {
                "MoonframeApp"
            } else {
                "MoonframeApp developer"
            })
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ToggleTheme, window, cx| {
                this.run(Command::ToggleTheme, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Inspect, _, cx| this.toggle_inspect(cx)))
            .on_action(cx.listener(|this, action: &QuickInspect, _, cx| {
                if !this.quick_inspect_key(&action.keystroke, cx) {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, action: &MenuSend, window, cx| {
                this.run_menu_item(&action.id, window, cx);
            }))
            .on_action(cx.listener(|this, action: &MenuKey, window, cx| {
                if !this.run_menu_key(&action.keystroke, window, cx) {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &Dismiss, _, cx| this.dismiss(cx)))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(theme.font_family.clone())
            .text_size(theme.font_size)
            .relative()
            // The shell takes what the branch bar leaves.
            .child(div().flex_1().min_h_0().child(body))
            .children(crate::branch_bar::label().map(crate::branch_bar::render))
            .children(problems)
            .children(layers)
            .children(toast)
            .child(crate::browser::presenter(self.browsers.pages()))
            .child(crate::video::sweep())
    }
}

impl MoonframeApp {
    /// A kept view again, for GPUI drawing it without a frame having asked.
    pub(crate) fn redraw_kept(
        &mut self,
        source: &Source,
        size: Option<Size<Pixels>>,
        kept: &Rc<Kept>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        Painter::new(
            &self.workspace,
            &mut self.motion,
            #[cfg(feature = "terminal")]
            &mut self.terminals,
            &self.inputs,
            None,
            cx.theme(),
        )
        .kept_in(window.window_handle().window_id())
        .redraw(source, size, kept, cx)
    }
}
