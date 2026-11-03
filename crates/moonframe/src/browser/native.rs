//! The platform's web view for a page: built as a child of the window, then
//! moved and shown where the element laid out.
//!
//! - **macOS:** a WKWebView inside a view of its own that clips it, so a
//!   page in a scrolled box or a sliding pane shows only what the layout
//!   shows of it. Both lie above GPUI's view, which cannot draw over them.
//! - **Windows:** a WebView2 child window. WebView2 builds through callbacks
//!   that wry waits for in a nested message pump, which must not run while
//!   GPUI holds the App, so the page is built in a foreground task.
//! - **Elsewhere:** nothing. wry has no working GPUI child-view backend on
//!   Linux yet, so the element shows its address and opens it in the system
//!   browser instead.

#[cfg(all(feature = "browser", any(target_os = "macos", windows)))]
pub(crate) use platform::*;
#[cfg(not(all(feature = "browser", any(target_os = "macos", windows))))]
pub(crate) use unsupported::*;

#[cfg(all(feature = "browser", any(target_os = "macos", windows)))]
mod platform {
    use {
        super::super::{Event, Outbox, Page},
        gpui_kit::{App, Bounds, Pixels, Window},
        moonframe_core::is_web_url,
        std::{
            cell::Cell,
            rc::Rc,
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
        },
        wry::{
            NewWindowResponse, PageLoadEvent, Rect, WebViewBuilder,
            dpi::{LogicalPosition, LogicalSize},
        },
    };

    /// This platform shows pages.
    pub(crate) const SUPPORTED: bool = true;

    pub(crate) struct Native {
        view: wry::WebView,
        /// The view that clips the page to what the layout shows of it.
        #[cfg(target_os = "macos")]
        clip: objc2::rc::Retained<objc2_app_kit::NSView>,
        shown: Cell<bool>,
        /// Where it was last put, whole and as much as shows.
        placed: Cell<Option<(Bounds<Pixels>, Bounds<Pixels>)>>,
        zoom: Cell<f32>,
    }

    /// The page as the user may take it: web addresses, file addresses
    /// while the page shows a file (`local`, so a web page cannot reach the
    /// machine's files), and the blank and inline documents pages make.
    /// Anything else, such as `mailto:` or an app's own scheme, stays
    /// unopened rather than reaching the OS.
    fn navigable(url: &str, local: bool) -> bool {
        if moonframe_core::is_file_url(url) {
            return local && is_web_url(url);
        }
        url == "about:blank"
            || url.starts_with("about:srcdoc")
            || url.starts_with("blob:")
            || is_web_url(url)
    }

    /// The builder for a page going to `url`, reporting to `outbox`; `local`
    /// says whether the page shows a file.
    fn builder(url: String, outbox: Outbox, local: Arc<AtomicBool>) -> WebViewBuilder<'static> {
        let (title, load, open) = (outbox.clone(), outbox.clone(), outbox);
        WebViewBuilder::new()
            .with_url(url)
            // Built hidden and unfocused: it shows where the layout puts it,
            // and takes keys once clicked.
            .with_visible(false)
            .with_focused(false)
            .with_bounds(Rect::default())
            .with_navigation_handler(move |url| navigable(&url, local.load(Ordering::Relaxed)))
            .with_new_window_req_handler(move |url, _| {
                open.report(Event::Open(url));
                NewWindowResponse::Deny
            })
            // Nothing a page offers is saved to disk.
            .with_download_started_handler(|_, _| false)
            .with_document_title_changed_handler(move |text| title.report(Event::Title(text)))
            .with_on_page_load_handler(move |event, url| {
                load.report(match event {
                    PageLoadEvent::Started => Event::Started(url),
                    PageLoadEvent::Finished => Event::Finished(url),
                });
            })
    }

    /// Builds `page`'s view on a later turn, outside this frame, and hands it
    /// to the page; the window draws again to place it.
    pub(crate) fn start(
        page: Rc<Page>,
        url: String,
        outbox: Outbox,
        local: Arc<AtomicBool>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let builder = builder(url, outbox, local);
        window
            .spawn(cx, async move |cx| {
                #[cfg(target_os = "macos")]
                let _ = cx.update(|window, _| {
                    page.built(macos::build(builder, window));
                    window.refresh();
                });
                #[cfg(windows)]
                {
                    let parent = match cx.update(|window, _| windows::parent(window)) {
                        Ok(parent) => parent,
                        Err(_) => return,
                    };
                    let built = match parent {
                        Ok(parent) => builder
                            .build_as_child_async(&parent)
                            .await
                            .map_err(|error| error.to_string())
                            .map(Native::new),
                        Err(error) => Err(error),
                    };
                    let _ = cx.update(|window, _| {
                        page.built(built);
                        window.refresh();
                    });
                }
            })
            .detach();
    }

    impl Native {
        pub(crate) fn load(&self, url: &str) {
            if let Err(error) = self.view.load_url(url) {
                tracing::debug!(%error, "browser navigation failed");
            }
        }

        pub(crate) fn reload(&self) {
            if let Err(error) = self.view.reload() {
                tracing::debug!(%error, "browser reload failed");
            }
        }

        /// Runs one of the host's own scripts, never one a mod wrote.
        pub(crate) fn script(&self, script: &'static str) {
            if let Err(error) = self.view.evaluate_script(script) {
                tracing::debug!(%error, "browser script failed");
            }
        }

        /// Where the page is now.
        pub(crate) fn url(&self) -> Option<String> {
            self.view.url().ok()
        }

        /// Whether the page can go back, and forward; None where the
        /// platform can't say.
        pub(crate) fn history(&self) -> Option<(bool, bool)> {
            #[cfg(target_os = "macos")]
            return Some(macos::history(&self.view));
            #[cfg(windows)]
            None
        }

        /// Draws the page at `factor`; only a change reaches the platform.
        pub(crate) fn zoom(&self, factor: f32) {
            if self.zoom.get() == factor {
                return;
            }
            match self.view.zoom(f64::from(factor)) {
                Ok(()) => self.zoom.set(factor),
                Err(error) => tracing::debug!(%error, "browser zoom failed"),
            }
        }

        pub(crate) fn show(&self) {
            if !self.shown.replace(true) {
                let _ = self.view.set_visible(true);
                #[cfg(target_os = "macos")]
                self.clip.setHidden(false);
            }
        }

        /// Hides the page, first taking the keyboard back from it: a hidden
        /// view would go on taking keys.
        pub(crate) fn hide(&self) {
            if self.shown.replace(false) {
                self.blur();
                let _ = self.view.set_visible(false);
                #[cfg(target_os = "macos")]
                macos::park(&self.clip);
                self.placed.set(None);
            }
        }

        /// Hands the keyboard back to the window, if the page holds it.
        pub(crate) fn blur(&self) {
            if self.holds_keyboard() {
                let _ = self.view.focus_parent();
            }
        }

        fn holds_keyboard(&self) -> bool {
            #[cfg(target_os = "macos")]
            return macos::holds_keyboard(&self.view);
            // Windows moves its focus to the parent itself when asked.
            #[cfg(windows)]
            true
        }

        /// Puts the page at `bounds`, showing only `visible` of it, both in
        /// the window's points.
        pub(crate) fn place(&self, bounds: Bounds<Pixels>, visible: Bounds<Pixels>) {
            if self.placed.replace(Some((bounds, visible))) == Some((bounds, visible)) {
                return;
            }
            #[cfg(target_os = "macos")]
            let bounds = {
                macos::frame(&self.clip, visible);
                // Inside the clip, from its top left.
                Bounds::new(bounds.origin - visible.origin, bounds.size)
            };
            let _ = self.view.set_bounds(Rect {
                position: LogicalPosition::new(
                    f32::from(bounds.origin.x),
                    f32::from(bounds.origin.y),
                )
                .into(),
                size: LogicalSize::new(f32::from(bounds.size.width), f32::from(bounds.size.height))
                    .into(),
            });
        }
    }

    impl Drop for Native {
        fn drop(&mut self) {
            self.hide();
            // wry takes the page's own view out as it drops.
            #[cfg(target_os = "macos")]
            self.clip.removeFromSuperview();
        }
    }

    #[cfg(target_os = "macos")]
    #[allow(unsafe_code, reason = "reading a view's superview; see SAFETY")]
    mod macos {
        use {
            super::Native,
            gpui_kit::{Bounds, Pixels, Window},
            objc2::{MainThreadMarker, MainThreadOnly as _, rc::Retained},
            objc2_app_kit::{NSApplication, NSAutoresizingMaskOptions, NSView},
            objc2_foundation::{NSPoint, NSRect, NSSize},
            std::cell::Cell,
            wry::{WebViewExtMacOS as _, raw_window_handle::HasWindowHandle},
        };

        /// Far outside any window, where a hidden page takes no drags.
        const AWAY: NSPoint = NSPoint::new(-100_000., -100_000.);

        /// Builds the page as a child of GPUI's view, then moves it into a
        /// clipping view of its own there.
        pub(super) fn build(
            builder: wry::WebViewBuilder<'static>,
            window: &Window,
        ) -> Result<Native, String> {
            let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
            // Window has an inherent `window_handle` of its own.
            let handle =
                HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?;
            // wry brings the app to the front as it builds a page; one
            // running in the background (`--background`) stays there.
            let app = NSApplication::sharedApplication(mtm);
            let active = app.isActive();
            let built = builder.build_as_child(&handle);
            if !active {
                app.deactivate();
            }
            let view = built.map_err(|error| error.to_string())?;
            let page: Retained<NSView> = Retained::into_super(Retained::into_super(view.webview()));
            // SAFETY: a property read on a view wry just added to GPUI's, on
            // the main thread.
            let gpui = unsafe { page.superview() }.ok_or("the page has no parent view")?;
            let clip = NSView::initWithFrame(NSView::alloc(mtm), NSRect::ZERO);
            clip.setClipsToBounds(true);
            clip.setHidden(true);
            // Pinned to the window's top as it resizes, as wry pins pages.
            clip.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinYMargin);
            gpui.addSubview(&clip);
            page.removeFromSuperview();
            clip.addSubview(&page);
            super::super::super::keys::install();
            Ok(Native {
                view,
                clip,
                shown: Cell::new(false),
                placed: Cell::new(None),
                zoom: Cell::new(1.0),
            })
        }

        /// Puts `clip` at `visible`, in the window's points from its top left.
        pub(super) fn frame(clip: &NSView, visible: Bounds<Pixels>) {
            // SAFETY: a property read on this view, on the main thread.
            let Some(parent) = (unsafe { clip.superview() }) else {
                return;
            };
            let top = f64::from(visible.origin.y);
            let size = NSSize::new(
                f64::from(visible.size.width),
                f64::from(visible.size.height),
            );
            let y = if parent.isFlipped() {
                top
            } else {
                parent.bounds().size.height - top - size.height
            };
            let frame = NSRect::new(NSPoint::new(f64::from(visible.origin.x), y), size);
            if clip.frame() != frame {
                clip.setFrame(frame);
            }
        }

        /// Hides `clip` and moves it out of the window: AppKit goes on
        /// finding a hidden view under the pointer during a drag.
        pub(super) fn park(clip: &NSView) {
            clip.setHidden(true);
            clip.setFrameOrigin(AWAY);
        }

        /// Whether the page can go back, and forward.
        pub(super) fn history(view: &wry::WebView) -> (bool, bool) {
            let page = view.webview();
            // SAFETY: property reads on the page's view, on the main thread.
            unsafe { (page.canGoBack(), page.canGoForward()) }
        }

        /// Whether the page is its window's first responder, which takes keys.
        pub(super) fn holds_keyboard(view: &wry::WebView) -> bool {
            let page: Retained<NSView> = Retained::into_super(Retained::into_super(view.webview()));
            page.window()
                .and_then(|window| window.firstResponder())
                .is_some_and(|responder| std::ptr::addr_eq(&*responder, &*page))
        }
    }

    #[cfg(windows)]
    #[allow(unsafe_code, reason = "a window handle a future can hold; see SAFETY")]
    mod windows {
        use {
            super::Native,
            gpui_kit::Window,
            std::cell::Cell,
            wry::raw_window_handle::{
                HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
            },
        };

        impl Native {
            pub(super) fn new(view: wry::WebView) -> Self {
                Self {
                    view,
                    shown: Cell::new(false),
                    placed: Cell::new(None),
                    zoom: Cell::new(1.0),
                }
            }
        }

        pub(super) fn parent(window: &Window) -> Result<Parent, String> {
            // Window has an inherent `window_handle` of its own.
            match HasWindowHandle::window_handle(window)
                .map_err(|error| error.to_string())?
                .as_raw()
            {
                RawWindowHandle::Win32(handle) => Ok(Parent(handle)),
                _ => Err("not a Win32 window".to_owned()),
            }
        }

        /// The window's HWND, as the handle wry builds a child of: a GPUI
        /// window's own handle borrows the window, which an awaiting future
        /// cannot hold.
        pub(super) struct Parent(Win32WindowHandle);

        impl HasWindowHandle for Parent {
            fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
                // SAFETY: the HWND was read from the open window in the poll
                // in which wry makes its child of it, before the build first
                // waits. Windows checks an HWND on every use, so a window
                // that closes meanwhile fails the build's later calls.
                Ok(unsafe { WindowHandle::borrow_raw(self.0.into()) })
            }
        }
    }
}

#[cfg(not(all(feature = "browser", any(target_os = "macos", windows))))]
mod unsupported {
    use {
        super::super::{Outbox, Page},
        gpui_kit::{App, Bounds, Pixels, Window},
        std::{
            rc::Rc,
            sync::{Arc, atomic::AtomicBool},
        },
    };

    /// This platform has no web view; the element offers the system browser.
    pub(crate) const SUPPORTED: bool = false;

    /// Never built.
    pub(crate) enum Native {}

    pub(crate) fn start(
        page: Rc<Page>,
        _: String,
        _: Outbox,
        _: Arc<AtomicBool>,
        _: &mut Window,
        _: &mut App,
    ) {
        page.built(Err("this platform has no web view".to_owned()));
    }

    impl Native {
        pub(crate) fn load(&self, _: &str) {}

        pub(crate) fn reload(&self) {}

        pub(crate) fn script(&self, _: &'static str) {}

        pub(crate) fn url(&self) -> Option<String> {
            None
        }

        pub(crate) fn history(&self) -> Option<(bool, bool)> {
            None
        }

        pub(crate) fn zoom(&self, _: f32) {}

        pub(crate) fn show(&self) {}

        pub(crate) fn hide(&self) {}

        pub(crate) fn blur(&self) {}

        pub(crate) fn place(&self, _: Bounds<Pixels>, _: Bounds<Pixels>) {}
    }
}
