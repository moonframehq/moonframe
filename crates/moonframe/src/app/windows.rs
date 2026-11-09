//! The windows besides the main one: the components browser and mods'
//! own, and what each frame leaves to do.

use super::*;

impl MoonframeApp {
    /// Publishes a mod window's state, or forgets it when the window closes.
    pub(crate) fn sync_mod_window(
        &mut self,
        name: &str,
        state: Option<WindowState>,
        cx: &mut Context<Self>,
    ) {
        let closed = state.is_none();
        self.workspace.set_mod_window(name, state);
        self.flush("window", cx);
        if closed && name == moonframe_core::DEVTOOLS {
            self.devtools_window_closed(cx);
        }
    }

    /// Whether `window` is the main window, not a mod's, the components
    /// browser or the theme picker.
    pub(crate) fn is_main(&self, window: &Window) -> bool {
        window.window_handle().window_id() == self.main_window
    }

    /// Shows the component `name` (or the last one, when empty) in the
    /// browser's window, opening it if it is closed.
    pub(crate) fn show_component(&mut self, name: &str, cx: &mut Context<Self>) {
        if !cfg!(feature = "studio") {
            return;
        }
        if !name.is_empty() {
            self.chosen = name.to_owned();
        }
        // The demos run only while their window is open.
        self.workspace.show_demos(true);
        cx.notify();
        let open = self.gallery.filter(|gallery| {
            cx.windows()
                .iter()
                .any(|window| window.window_id() == gallery.window_id())
        });
        // Both after this update: the action may have come from the
        // browser's own window, which cannot be updated while it is busy,
        // and a new window draws by asking the app.
        match open {
            Some(_) if crate::background() => {},
            Some(gallery) => cx.defer(move |cx| {
                let _ = gallery.update(cx, |_, window, _| window.activate_window());
            }),
            None => {
                let app = cx.entity();
                cx.defer(move |cx| {
                    let gallery = Gallery::open(app.clone(), cx);
                    app.update(cx, |app, _| app.gallery = gallery);
                });
            },
        }
    }

    /// Opens the mod `name` in a window of its own, or raises it; `place`
    /// outranks where the mod's `window` key puts it.
    pub(super) fn show_mod_window(
        &mut self,
        name: &str,
        place: Option<moonframe_core::WindowPlace>,
        cx: &mut Context<Self>,
    ) {
        let Some(slot) = self.workspace.slot(name) else {
            tracing::warn!("no mod `{name}` to open in a window");
            return;
        };
        let title = slot
            .meta()
            .and_then(|meta| meta.title.clone())
            .unwrap_or_else(|| name.to_owned());
        let mut sizing = slot.meta().and_then(|meta| meta.window).unwrap_or_default();
        if let Some(place) = place {
            sizing.place = place;
        }
        if sizing.place != moonframe_core::WindowPlace::default()
            && !self.built_with(name, cfg!(feature = "displays"), "displays")
        {
            sizing.place = moonframe_core::WindowPlace::default();
        }
        // After this update, as for the components window: the click may
        // have come from that window, which is busy until it returns.
        // Look up the handle then too: an earlier request in this batch
        // may be opening or closing the same mod's window.
        let app = cx.entity();
        let name = name.to_owned();
        cx.defer(move |cx| {
            let open = app
                .read(cx)
                .mod_windows
                .get(&name)
                .copied()
                .filter(|handle| {
                    cx.windows()
                        .iter()
                        .any(|window| window.window_id() == handle.window_id())
                });
            match open {
                Some(_) if crate::background() => {},
                Some(handle) => {
                    let _ = handle.update(cx, |_, window, _| window.activate_window());
                },
                None => {
                    if let Some(handle) =
                        ModWindow::open(app.clone(), name.clone(), title, sizing, cx)
                    {
                        app.update(cx, |app, _| app.mod_windows.insert(name, handle));
                    }
                },
            }
        });
    }

    /// Does what the mod `from` asked with `app:window`: opens a mod's
    /// window (what it was sent is already delivered) or closes it. A
    /// preview asks nothing.
    pub(super) fn window_request(
        &mut self,
        from: &str,
        request: WindowRequest,
        cx: &mut Context<Self>,
    ) {
        if from.starts_with(moonframe_core::DEMO) {
            return;
        }
        match request {
            WindowRequest::Open { name, .. } if self.workspace.slot(&name).is_none() => {
                self.workspace.refuse(
                    from,
                    Error::new(format!("`app:window`: no mod named `{name}`")),
                );
            },
            WindowRequest::Open { name, place, .. } => self.show_mod_window(&name, place, cx),
            WindowRequest::Close(name) => {
                // After this update: the mod may have asked from its own
                // window, which is busy until it returns. Resolve its
                // handle after any earlier deferred open has run.
                let app = cx.entity();
                cx.defer(move |cx| {
                    let handle = app.update(cx, |app, _| app.mod_windows.remove(&name));
                    if let Some(handle) = handle {
                        let _ = handle.update(cx, |_, window, _| window.remove_window());
                    }
                });
            },
        }
    }

    /// A mod window's content, and whether it is still moving.
    pub(crate) fn mod_window_content(
        &mut self,
        name: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> (AnyElement, bool) {
        let mut painter = Painter::new(
            &self.workspace,
            &mut self.motion,
            #[cfg(feature = "terminal")]
            &mut self.terminals,
            &self.inputs,
            None,
            cx.theme(),
        )
        .with_views(&mut self.views)
        .in_window(window.window_handle().window_id());
        let content = painter.window(name, cx);
        let frame = painter.finish();
        let animating = self.after_frame(frame, window, cx);
        let content = div()
            .relative()
            .size_full()
            .child(content)
            .children(self.problems_layer(cx))
            .into_any_element();
        (content, animating)
    }

    /// Does what a frame drawn in `window` left to do: keeps the keys of
    /// its open popovers and overlays, asks for the rows its lists want,
    /// and makes its views. Returns whether it still moves.
    pub(super) fn after_frame(
        &mut self,
        frame: Finished,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.key_layers
            .insert(window.window_handle().window_id(), frame.key_layers);
        self.request_list_rows(frame.list_requests, cx);
        self.make_views(window, cx);
        frame.animating
    }

    /// Makes the views the frame drew that are not made yet, and gives the
    /// others their new props, once the frame is done; draws again to show
    /// what was made.
    fn make_views(&mut self, window: &Window, cx: &mut Context<Self>) {
        let requests = self.views.take_requests();
        if requests.is_empty() {
            return;
        }
        // In the window that drew them, which a view may want at its start.
        let this = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let _ = this.update(cx, |this, cx| {
                if this.views.apply(requests, window, cx) {
                    cx.notify();
                }
            });
        });
    }

    /// The browser window's content, and whether it is still moving.
    pub(crate) fn gallery_content(
        &mut self,
        query: &str,
        search: AnyElement,
        cx: &mut Context<Self>,
    ) -> (AnyElement, bool) {
        let mut painter = Painter::new(
            &self.workspace,
            &mut self.motion,
            #[cfg(feature = "terminal")]
            &mut self.terminals,
            &self.inputs,
            None,
            cx.theme(),
        )
        .previewing();
        let content = painter.gallery(&self.chosen, query, search, cx);
        let frame = painter.finish();
        self.request_list_rows(frame.list_requests, cx);
        (content, frame.animating)
    }

    /// Asks an agent to add the component `name`, at the selection if any.
    pub(crate) fn copy_component_request(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(component) = self
            .workspace
            .components()
            .iter()
            .find(|component| component.name == name)
        else {
            return;
        };
        let place = match &self.inspector.selected {
            Some(selected) => format!(
                "at the selected element:\n\n{}",
                inspect::report(selected, &self.workspace)
            ),
            None => "where it fits (no element is selected; use View > Inspect Element to pick a place).".to_owned(),
        };
        let request = format!(
            "Add the `{name}` component ({}) {place}\nIt is used like this:\n\n{}\n",
            component.path.display(),
            component.usage
        );
        self.copy(request, cx);
    }
}
