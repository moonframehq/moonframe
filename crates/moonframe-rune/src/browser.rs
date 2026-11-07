//! `ui.browser`: the platform's web view as a child of Rune's window, moved to
//! where the element laid out. Like GPUI's it lies over the window, so nothing
//! Rune draws can cover it. The rules are the same too: a page goes only to
//! web addresses (or among files, if the mod opened a file), saves nothing to
//! disk, and asks the mod rather than opening windows.

use {
    crate::Ui,
    moonframe_core::{BrowserCommand, Data, Markdown, is_file_url, is_web_url},
    rune::{Tx, Waker, WindowHandle},
    std::{
        cell::RefCell,
        collections::HashMap,
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{Receiver, Sender, channel},
        },
    },
    wry::{
        NewWindowResponse, PageLoadEvent, Rect, WebView, WebViewBuilder,
        dpi::{LogicalPosition, LogicalSize},
    },
};

/// What a page said, on its platform's callbacks.
enum Report {
    Title(String),
    Started(String),
    Finished(String),
}

/// A `ui.browser` as last drawn.
pub(crate) struct Wanted {
    pub(crate) owner: String,
    pub(crate) url: Option<String>,
    pub(crate) navigate: Option<Data>,
    pub(crate) loading: Option<Data>,
}

struct Page {
    view: WebView,
    asked: Option<String>,
    url: Option<String>,
    local: Arc<AtomicBool>,
    reports: Receiver<Report>,
    sender: Sender<Report>,
    shown: bool,
}

/// The pages, and the window they are children of once it exists.
#[derive(Default)]
pub(crate) struct Browsers {
    pub(crate) parent: Rc<RefCell<Option<WindowHandle>>>,
    pages: RefCell<HashMap<String, Page>>,
    pub(crate) wanted: RefCell<HashMap<String, Wanted>>,
}

fn builder(
    url: String,
    sender: &Sender<Report>,
    local: Arc<AtomicBool>,
    wake: Waker,
) -> WebViewBuilder<'static> {
    let (title, load) = (sender.clone(), sender.clone());
    let (woken, loaded) = (Arc::clone(&wake), wake);
    WebViewBuilder::new()
        .with_url(url)
        .with_visible(false)
        .with_focused(false)
        .with_bounds(Rect::default())
        .with_navigation_handler(move |url| {
            is_web_url(&url) || (local.load(Ordering::Relaxed) && is_file_url(&url))
        })
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_document_title_changed_handler(move |text| {
            if title.send(Report::Title(text)).is_ok() {
                woken();
            }
        })
        .with_on_page_load_handler(move |event, url| {
            let report = match event {
                PageLoadEvent::Started => Report::Started(url),
                PageLoadEvent::Finished => Report::Finished(url),
            };
            if load.send(report).is_ok() {
                loaded();
            }
        })
}

impl Browsers {
    /// Whether there is a window for a page to live in.
    pub(crate) fn can_show(&self) -> bool {
        self.parent.borrow().is_some()
    }

    /// Whether the page `key` has been built.
    pub(crate) fn built(&self, key: &str) -> bool {
        self.pages.borrow().contains_key(key)
    }

    /// Moves the page `key` to `rect`, in the window's points.
    pub(crate) fn place(&self, key: &str, [x, y, width, height]: [f32; 4]) {
        if let Some(page) = self.pages.borrow_mut().get_mut(key) {
            let bounds = Rect {
                position: LogicalPosition::new(f64::from(x), f64::from(y)).into(),
                size: LogicalSize::new(f64::from(width), f64::from(height)).into(),
            };
            if let Err(problem) = page.view.set_bounds(bounds) {
                tracing::warn!("cannot place a page: {problem}");
            }
            if !page.shown && page.view.set_visible(true).is_ok() {
                page.shown = true;
            }
        }
    }

    /// Starts the pages views ask for, sends those with a new address on, and
    /// hides what no view drew. True when a page was built.
    fn settle(&self, wake: &Waker) -> bool {
        let parent = self.parent.borrow();
        let Some(parent) = parent.as_ref() else {
            return false;
        };
        let wanted = self.wanted.borrow();
        let mut pages = self.pages.borrow_mut();
        let mut built = false;
        for (key, want) in wanted.iter() {
            if let Some(page) = pages.get_mut(key) {
                if page.asked != want.url {
                    page.asked.clone_from(&want.url);
                    if let Some(url) = &want.url {
                        page.local.store(is_file_url(url), Ordering::Relaxed);
                        if let Err(problem) = page.view.load_url(url) {
                            tracing::warn!("cannot load {url}: {problem}");
                        }
                    }
                }
                continue;
            }
            let (sender, reports) = channel();
            let local = Arc::new(AtomicBool::new(
                want.url.as_deref().is_some_and(is_file_url),
            ));
            let url = want.url.clone().unwrap_or_else(|| "about:blank".to_owned());
            let page = builder(url, &sender, Arc::clone(&local), Arc::clone(wake));
            match parent.with_raw_handle(|handle| page.build_as_child(&handle)) {
                Ok(Ok(view)) => {
                    built = true;
                    pages.insert(key.clone(), Page {
                        view,
                        asked: want.url.clone(),
                        url: want.url.clone(),
                        local,
                        reports,
                        sender,
                        shown: false,
                    });
                },
                Ok(Err(problem)) => tracing::warn!("cannot open a page: {problem}"),
                Err(problem) => tracing::warn!("cannot open a page: {problem}"),
            }
        }
        for (key, page) in pages.iter_mut() {
            if page.shown && !wanted.contains_key(key) && page.view.set_visible(false).is_ok() {
                page.shown = false;
            }
        }
        built
    }

    /// What pages reported, as messages for their mods.
    fn take_reports(&self) -> Vec<(String, Data)> {
        let wanted = self.wanted.borrow();
        let mut pages = self.pages.borrow_mut();
        let mut messages = Vec::new();
        for (key, page) in pages.iter_mut() {
            let Some(want) = wanted.get(key) else {
                continue;
            };
            for report in page.reports.try_iter() {
                let url = |at: &str| Data::String(at.to_owned());
                match &report {
                    Report::Title(title) => {
                        if let Some(msg) = &want.navigate {
                            let shown = page.url.clone().unwrap_or_default();
                            let note = [("url", url(&shown)), ("title", url(title))];
                            messages.push((want.owner.clone(), Markdown::note(msg, note)));
                        }
                    },
                    Report::Started(at) | Report::Finished(at) => {
                        page.url = Some(at.clone());
                        let loading = matches!(report, Report::Started(_));
                        if let Some(msg) = &want.loading {
                            let note = [("loading", Data::from(loading)), ("url", url(at))];
                            messages.push((want.owner.clone(), Markdown::note(msg, note)));
                        }
                        if let (Some(msg), false) = (&want.navigate, loading) {
                            messages.push((
                                want.owner.clone(),
                                Markdown::note(msg, [("url", url(at))]),
                            ));
                        }
                    },
                }
            }
            // Kept so a page's callbacks never see a closed channel.
            let _ = &page.sender;
        }
        messages
    }

    /// Runs a toolbar's `command` on the page `key`.
    fn command(&self, key: &str, command: BrowserCommand, tx: &mut Tx) {
        let pages = self.pages.borrow();
        let Some(page) = pages.get(key) else {
            return;
        };
        let done = match command {
            BrowserCommand::Back => page.view.evaluate_script("history.back()"),
            BrowserCommand::Forward => page.view.evaluate_script("history.forward()"),
            BrowserCommand::Reload => page.view.reload(),
            BrowserCommand::Stop => page.view.evaluate_script("window.stop()"),
            BrowserCommand::Open => {
                if let Some(url) = &page.url
                    && let Err(problem) = tx.open_url(url)
                {
                    tracing::warn!("cannot open {url}: {problem:?}");
                }
                Ok(())
            },
        };
        if let Err(problem) = done {
            tracing::warn!("a page refused {command:?}: {problem}");
        }
    }
}

impl Ui {
    /// Each frame: pages to build, and what they told their mods.
    pub(crate) fn settle_browsers(&mut self) -> bool {
        let Some(wake) = self.waker.clone() else {
            return false;
        };
        let built = self.browsers.settle(&wake);
        let messages = self.browsers.take_reports();
        let told = !messages.is_empty();
        for (owner, msg) in messages {
            self.send(&owner, &owner, msg);
        }
        built || told
    }

    /// A toolbar's command for the `ui.browser` `id` of `owner`.
    pub(crate) fn browser_command(
        &self,
        owner: &str,
        id: &str,
        command: BrowserCommand,
        tx: &mut Tx,
    ) {
        self.browsers.command(&format!("{owner}/{id}"), command, tx);
    }
}
