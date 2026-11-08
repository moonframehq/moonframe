//! `ui.browser`: the platform's web view as a child of Rune's window, moved to
//! where the element laid out. Like GPUI's it lies over the window, so nothing
//! Rune draws can cover it. The rules are the same too: a page goes only to
//! web addresses (or among files, if the mod opened a file), saves nothing to
//! disk, and asks the mod rather than opening windows.

use {
    crate::Ui,
    moonframe_core::{
        BrowserCommand, Data, Markdown, is_file_url, is_remote_url, is_web_url, typed_address,
    },
    rune::{Tx, Waker, WindowHandle},
    std::{
        cell::{Cell, RefCell},
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
    pub(crate) zoom: f32,
}

struct Page {
    view: WebView,
    owner: String,
    zoom: f32,
    /// The last settle that had it drawn, so the longest gone goes first.
    seen: u64,
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
    settles: Cell<u64>,
    pub(crate) wanted: RefCell<HashMap<String, Wanted>>,
}

/// Pages one mod can have, and the whole app, as on GPUI: each runs web
/// content processes, so a mod drawing ids in a loop must not start them
/// without end.
const PER_MOD: usize = 4;
const TOTAL: usize = 16;

/// What a page is to the limits: its key, its mod, whether a view drew it
/// now, and the last settle that did.
type Held<'a> = (&'a str, &'a str, bool, u64);

/// The key of the page that must go for `owner` to open one: the longest
/// gone of those no view draws, its own when it is at its limit. `Ok(None)`
/// when there is room; `Err` says why there is none to give up.
fn to_reclaim(held: &[Held<'_>], owner: &str) -> Result<Option<String>, String> {
    let mine = held.iter().filter(|page| page.1 == owner).count();
    let over_mine = mine >= PER_MOD;
    if !over_mine && held.len() < TOTAL {
        return Ok(None);
    }
    held.iter()
        .filter(|page| !page.2 && (!over_mine || page.1 == owner))
        .min_by_key(|page| page.3)
        .map(|page| Some(page.0.to_owned()))
        .ok_or_else(|| {
            if over_mine {
                format!("`{owner}` already has {PER_MOD} browsers")
            } else {
                format!("the app already has {TOTAL} browsers")
            }
        })
}

/// Whether a page may go to `url`, as on GPUI. `is_web_url` passes `file`
/// addresses too, so those are for `local` pages alone: a web page must not
/// read the disk. The blank and inline documents are ones pages make.
fn may_navigate(url: &str, local: bool) -> bool {
    if is_file_url(url) {
        return local && is_web_url(url);
    }
    url == "about:blank"
        || url.starts_with("about:srcdoc")
        || url.starts_with("blob:")
        || is_web_url(url)
}

/// Notes that a page began loading `url`: a local page that went to the web
/// is a web page now, and must not reach back to the disk. On the page's own
/// callback, as an idle window would read a report too late.
fn started(local: &AtomicBool, url: &str) {
    local.store(is_file_url(url), Ordering::Relaxed);
}

fn builder(
    url: String,
    sender: &Sender<Report>,
    local: Arc<AtomicBool>,
    wake: Waker,
) -> WebViewBuilder<'static> {
    let (title, load) = (sender.clone(), sender.clone());
    let (woken, loaded) = (Arc::clone(&wake), wake);
    let loading = Arc::clone(&local);
    WebViewBuilder::new()
        .with_url(url)
        .with_visible(false)
        .with_focused(false)
        .with_bounds(Rect::default())
        .with_navigation_handler(move |url| may_navigate(&url, local.load(Ordering::Relaxed)))
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_document_title_changed_handler(move |text| {
            if title.send(Report::Title(text)).is_ok() {
                woken();
            }
        })
        .with_on_page_load_handler(move |event, url| {
            let report = match event {
                PageLoadEvent::Started => {
                    started(&loading, &url);
                    Report::Started(url)
                },
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
        let settle = self.settles.get() + 1;
        self.settles.set(settle);
        for (key, want) in wanted.iter() {
            if let Some(page) = pages.get_mut(key) {
                page.seen = settle;
                if (page.zoom - want.zoom).abs() > f32::EPSILON {
                    page.zoom = want.zoom;
                    if let Err(problem) = page.view.zoom(f64::from(want.zoom)) {
                        tracing::warn!("cannot zoom a page: {problem}");
                    }
                }
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
            let held: Vec<Held<'_>> = pages
                .iter()
                .map(|(key, page)| {
                    (
                        key.as_str(),
                        page.owner.as_str(),
                        wanted.contains_key(key),
                        page.seen,
                    )
                })
                .collect();
            match to_reclaim(&held, &want.owner) {
                Ok(None) => {},
                Ok(Some(gone)) => {
                    pages.remove(&gone);
                },
                Err(problem) => {
                    tracing::warn!("cannot open a page: {problem}");
                    continue;
                },
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
                    if let Err(problem) = view.zoom(f64::from(want.zoom)) {
                        tracing::warn!("cannot zoom a page: {problem}");
                    }
                    pages.insert(key.clone(), Page {
                        view,
                        owner: want.owner.clone(),
                        zoom: want.zoom,
                        seen: settle,
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
            // Only web addresses reach the system, as on GPUI: a `file` one
            // would open whatever program the file is.
            BrowserCommand::Open => {
                if let Some(url) = page.url.as_deref().filter(|url| is_remote_url(url))
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

    /// Goes where `text`, typed in the address field of the `ui.browser`
    /// `id` of `owner`, says, even to the address it shows. Returns whether
    /// the text named somewhere to go.
    pub(crate) fn visit(&self, owner: &str, id: &str, text: &str) -> bool {
        let Some(url) = typed_address(text) else {
            return false;
        };
        let pages = self.browsers.pages.borrow();
        let Some(page) = pages.get(&format!("{owner}/{id}")) else {
            return false;
        };
        page.local.store(is_file_url(&url), Ordering::Relaxed);
        if let Err(problem) = page.view.load_url(&url) {
            tracing::warn!("cannot load {url}: {problem}");
        }
        true
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

#[cfg(test)]
mod tests {
    use {
        super::{Held, PER_MOD, TOTAL, may_navigate, started, to_reclaim},
        std::sync::atomic::{AtomicBool, Ordering},
    };

    /// `count` pages of `owner`, named from `first`, gone longest first.
    fn pages<'a>(names: &'a [String], owner: &'a str, drawn: bool) -> Vec<Held<'a>> {
        names
            .iter()
            .zip(0..)
            .map(|(name, seen)| (name.as_str(), owner, drawn, seen))
            .collect()
    }

    fn names(count: usize) -> Vec<String> {
        (0..count).map(|index| format!("page{index}")).collect()
    }

    #[test]
    fn a_mod_under_its_limit_gives_nothing_up() {
        let names = names(PER_MOD - 1);
        assert_eq!(to_reclaim(&pages(&names, "a", false), "a"), Ok(None));
    }

    #[test]
    fn a_mod_at_its_limit_gives_up_the_page_gone_longest() {
        let names = names(PER_MOD);
        let held = pages(&names, "a", false);
        assert_eq!(to_reclaim(&held, "a"), Ok(Some("page0".to_owned())));
    }

    #[test]
    fn a_page_on_show_is_never_given_up() {
        let names = names(PER_MOD);
        let held = pages(&names, "a", true);
        assert!(to_reclaim(&held, "a").is_err());
    }

    #[test]
    fn a_mod_at_its_limit_takes_nothing_from_another() {
        let (mine, theirs) = (names(PER_MOD), vec!["other".to_owned()]);
        let mut held = pages(&mine, "a", true);
        held.extend(pages(&theirs, "b", false));
        assert!(to_reclaim(&held, "a").is_err());
    }

    #[test]
    fn the_app_at_its_limit_gives_up_any_mods_hidden_page() {
        let names = names(TOTAL);
        let held = pages(&names, "b", false);
        assert_eq!(to_reclaim(&held, "a"), Ok(Some("page0".to_owned())));
    }

    #[test]
    fn a_web_page_cannot_reach_the_disk() {
        assert!(may_navigate("https://example.com/", false));
        assert!(!may_navigate("file:///etc/passwd", false));
        assert!(!may_navigate("FILE:///etc/passwd", false));
        assert!(!may_navigate("file://localhost/etc/passwd", false));
    }

    #[test]
    fn a_local_page_reaches_files_and_the_web() {
        assert!(may_navigate("file:///tmp/page.html", true));
        assert!(may_navigate("https://example.com/", true));
    }

    #[test]
    fn a_local_page_that_goes_to_the_web_loses_the_disk() {
        let local = AtomicBool::new(true);
        started(&local, "file:///tmp/next.html");
        assert!(
            local.load(Ordering::Relaxed),
            "links among files stay local"
        );
        started(&local, "https://example.com/");
        assert!(!may_navigate(
            "file:///etc/passwd",
            local.load(Ordering::Relaxed)
        ));
    }

    #[test]
    fn pages_keep_the_documents_they_make() {
        for url in ["about:blank", "about:srcdoc", "blob:https://example.com/1"] {
            assert!(may_navigate(url, false), "{url}");
        }
    }

    #[test]
    fn neither_goes_anywhere_else() {
        for local in [false, true] {
            assert!(!may_navigate("javascript:alert(1)", local));
            assert!(!may_navigate("ftp://example.com/", local));
        }
    }
}
