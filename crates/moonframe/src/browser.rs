//! The web pages `ui.browser` elements show. Each is a native web view
//! (WebKit on macOS, WebView2 on Windows), kept by its mod and `id`, so it
//! outlives reloads and hidden panes and ends with the app; Linux has none
//! yet (see `native`).
//!
//! A page lies above the window's own drawing, as a child view at the
//! element's bounds. Each frame the element records where it laid out, and
//! [`presenter`], painted after every element has laid out, shows each page
//! drawn this frame where it drew and hides every other one. Anything that
//! floats over the layout (a popover, a toast, the inspector's card) calls
//! [`cover`], and a page it reaches is hidden while it shows, since the
//! window cannot draw over a native view.
//!
//! What a page reports (loads, titles) arrives on its platform's callbacks;
//! they queue it, and the app hands it to the mod as the `navigate` and
//! `loading` messages it asked for.

mod native;

#[cfg(target_os = "macos")]
mod keys;

use {
    crate::app::MoonframeApp,
    futures::StreamExt as _,
    gpui_kit::*,
    moonframe_core::{Browser, BrowserCommand, Data, is_web_url, typed_address},
    std::{
        cell::{Cell, RefCell},
        collections::{BTreeMap, HashMap},
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver, Sender},
        },
    },
};

pub(crate) use native::SUPPORTED;

/// Pages one mod can have, and the whole app: each runs web content
/// processes, so a mod drawing ids in a loop must not start them without end.
const PER_MOD: usize = 4;
const TOTAL: usize = 16;
/// Reports kept between two drains; later ones are dropped.
const BACKLOG: usize = 256;
/// What a page with no address shows.
const BLANK: &str = "about:blank";

/// What a page reported, on its platform's callbacks.
// Only the native web views' callbacks report; Linux has none yet.
#[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
#[derive(Debug)]
pub(crate) enum Event {
    /// A load started, at this address.
    Started(String),
    Finished(String),
    Title(String),
    /// The page asked for a new window; it opens in the same page.
    Open(String),
}

/// Where a page's callbacks queue what it reports, and how they wake the
/// window to read it.
// Only the native web views' callbacks report; Linux has none yet.
#[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
#[derive(Clone)]
pub(crate) struct Outbox {
    key: String,
    sender: Sender<(String, Event)>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Outbox {
    #[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
    pub(crate) fn report(&self, event: Event) {
        if self.sender.send((self.key.clone(), event)).is_ok() {
            (self.wake)();
        }
    }
}

/// What a page is doing, and what its mod asked of it.
#[derive(Default)]
struct State {
    /// The last address the mod gave; a new one navigates.
    asked: Option<String>,
    /// Where to go once the view is built.
    pending: Option<String>,
    building: bool,
    failed: Option<String>,
    url: String,
    title: String,
    /// A load is under way.
    busy: bool,
    /// What was last sent as `navigate`, so each change is sent once.
    sent: Option<Report>,
    zoom: f32,
    navigate: Option<Data>,
    loading: Option<Data>,
}

/// What `navigate` tells the mod: where the page is, its title, whether
/// it is loading, and whether it can go back and forward (None where the
/// platform can't say).
#[derive(Clone, Debug, Default, PartialEq)]
struct Report {
    url: String,
    title: String,
    loading: bool,
    history: Option<(bool, bool)>,
}

/// One `ui.browser`'s page.
pub(crate) struct Page {
    owner: String,
    outbox: Outbox,
    native: RefCell<Option<native::Native>>,
    /// Whether the page shows a `file` address, which is what lets it go
    /// to another; shared with the view's navigation handler.
    local: Arc<AtomicBool>,
    state: RefCell<State>,
    /// Where it laid out this frame, whole and as much as shows, in the
    /// window's points; [`presenter`] takes it.
    drawn: Cell<Option<(Bounds<Pixels>, Bounds<Pixels>)>>,
}

impl Page {
    /// Records where the page laid out this frame, and starts building its
    /// view the first time. Called as the element lays out.
    pub(crate) fn laid_out(
        self: &Rc<Self>,
        bounds: Bounds<Pixels>,
        visible: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.drawn.set(Some((bounds, visible)));
        let url = {
            let mut state = self.state.borrow_mut();
            if self.native.borrow().is_some() || state.building || state.failed.is_some() {
                return;
            }
            state.building = true;
            state.pending.take().unwrap_or_else(|| BLANK.to_owned())
        };
        self.local
            .store(moonframe_core::is_file_url(&url), Ordering::Relaxed);
        native::start(
            self.clone(),
            url,
            self.outbox.clone(),
            self.local.clone(),
            window,
            cx,
        );
    }

    /// Takes the view the platform built, or why it could not.
    fn built(&self, built: Result<native::Native, String>) {
        let mut state = self.state.borrow_mut();
        state.building = false;
        match built {
            Ok(view) => {
                view.zoom(state.zoom);
                // Asked for while it was built.
                if let Some(url) = state.pending.take() {
                    view.load(&url);
                }
                *self.native.borrow_mut() = Some(view);
            },
            Err(error) => {
                tracing::warn!("browser {}: {error}", self.outbox.key);
                state.failed = Some(error);
            },
        }
    }

    /// Why the page has no view, if building it failed.
    pub(crate) fn failed(&self) -> Option<String> {
        self.state.borrow().failed.clone()
    }

    /// Takes what the element says this frame: a new `url` navigates.
    fn update(&self, element: &Browser) {
        let mut state = self.state.borrow_mut();
        state.navigate = element.navigate.clone();
        state.loading = element.loading.clone();
        if state.zoom != element.zoom {
            state.zoom = element.zoom;
            if let Some(view) = &*self.native.borrow() {
                view.zoom(element.zoom);
            }
        }
        if state.asked != element.url {
            state.asked = element.url.clone();
            if let Some(url) = &element.url {
                self.go(&mut state, url);
            }
        }
    }

    fn go(&self, state: &mut State, url: &str) {
        // The mod's own address may be a file; a page may then follow links
        // among files, which a web page may not.
        self.local
            .store(moonframe_core::is_file_url(url), Ordering::Relaxed);
        match &*self.native.borrow() {
            Some(view) => view.load(url),
            None => state.pending = Some(url.to_owned()),
        }
    }

    /// Goes where `text`, typed in its address field, says (see
    /// [`typed_address`]), even to the address it shows. Returns whether
    /// the text named somewhere to go.
    fn visit(&self, text: &str) -> bool {
        let Some(url) = typed_address(text) else {
            return false;
        };
        let mut state = self.state.borrow_mut();
        self.go(&mut state, &url);
        true
    }

    /// Where the page is: its view's address, else the last one it
    /// reported, else the one its mod gave.
    fn address(&self) -> Option<String> {
        let state = self.state.borrow();
        self.native
            .borrow()
            .as_ref()
            .and_then(native::Native::url)
            .or_else(|| Some(state.url.clone()).filter(|url| !url.is_empty()))
            .or_else(|| state.asked.clone())
    }

    /// Runs `command`; returns the address to open in the system browser
    /// for `browser-open`.
    fn command(&self, command: BrowserCommand) -> Option<String> {
        if command == BrowserCommand::Open {
            return self.address().filter(|url| is_web_url(url));
        }
        let native = self.native.borrow();
        let view = native.as_ref()?;
        match command {
            BrowserCommand::Back => view.script("history.back()"),
            BrowserCommand::Forward => view.script("history.forward()"),
            BrowserCommand::Reload => view.reload(),
            BrowserCommand::Stop => view.script("window.stop()"),
            BrowserCommand::Open => {},
        }
        None
    }

    /// The messages `event` makes for the page's mod.
    fn report(&self, event: Event) -> Vec<Data> {
        let mut state = self.state.borrow_mut();
        let mut sends = Vec::new();
        let loading = match event {
            Event::Started(url) => {
                self.local
                    .store(moonframe_core::is_file_url(&url), Ordering::Relaxed);
                state.url = url;
                state.busy = true;
                Some(true)
            },
            Event::Finished(url) => {
                state.url = url;
                state.busy = false;
                Some(false)
            },
            Event::Title(title) => {
                state.title = title;
                // A script's own navigation (`history.pushState`) reports no
                // load, but its title change reads the address it went to.
                if let Some(url) = self.native.borrow().as_ref().and_then(native::Native::url) {
                    state.url = url;
                }
                None
            },
            // A page's popup opens here only if it is a web address: a page
            // cannot reach the machine's files that way.
            Event::Open(url) => {
                if moonframe_core::is_remote_url(&url) {
                    self.go(&mut state, &url);
                }
                return sends;
            },
        };
        if let (Some(loading), Some(msg)) = (loading, &state.loading) {
            sends.push(with(msg, [
                ("loading", Data::Bool(loading)),
                ("url", Data::String(state.url.clone())),
            ]));
        }
        let now = Report {
            url: state.url.clone(),
            title: state.title.clone(),
            loading: state.busy,
            history: self
                .native
                .borrow()
                .as_ref()
                .and_then(native::Native::history),
        };
        if state.sent.as_ref() != Some(&now)
            && let Some(msg) = &state.navigate
        {
            let mut msg = with(msg, [
                ("url", Data::String(now.url.clone())),
                ("title", Data::String(now.title.clone())),
                ("loading", Data::Bool(now.loading)),
            ]);
            if let Some((back, forward)) = now.history {
                msg = with(&msg, [
                    ("back", Data::Bool(back)),
                    ("forward", Data::Bool(forward)),
                ]);
            }
            sends.push(msg);
            state.sent = Some(now);
        }
        sends
    }
}

/// `msg` with `fields` added: into it when it is a table with string keys,
/// else as `{ msg = msg, ... }`.
pub(crate) fn with<const N: usize>(msg: &Data, fields: [(&str, Data); N]) -> Data {
    let mut map = match msg {
        Data::Map(map) => map.clone(),
        other => BTreeMap::from([("msg".to_owned(), other.clone())]),
    };
    for (key, value) in fields {
        map.insert(key.to_owned(), value);
    }
    Data::Map(map)
}

pub(crate) struct Browsers {
    pages: HashMap<String, Rc<Page>>,
    sender: Sender<(String, Event)>,
    events: Receiver<(String, Event)>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Browsers {
    pub(crate) fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (sender, events) = mpsc::channel();
        Self {
            pages: HashMap::new(),
            sender,
            events,
            wake,
        }
    }

    /// The page `element` of the mod `owner` shows, made now if it is new,
    /// with what the element asks this frame applied. Its view is built as
    /// it first lays out. Err says why there is none.
    pub(crate) fn open(&mut self, owner: &str, element: &Browser) -> Result<Rc<Page>, String> {
        let key = format!("{owner}/{}", element.id);
        if let Some(page) = self.pages.get(&key) {
            page.update(element);
            return Ok(page.clone());
        }
        let mine = self
            .pages
            .values()
            .filter(|page| page.owner == owner)
            .count();
        if mine >= PER_MOD {
            return Err(format!("`{owner}` already has {PER_MOD} browsers"));
        }
        if self.pages.len() >= TOTAL {
            return Err(format!("the app already has {TOTAL} browsers"));
        }
        let page = Rc::new(Page {
            owner: owner.to_owned(),
            outbox: Outbox {
                key: key.clone(),
                sender: self.sender.clone(),
                wake: self.wake.clone(),
            },
            native: RefCell::new(None),
            local: Arc::new(AtomicBool::new(false)),
            state: RefCell::new(State {
                asked: element.url.clone(),
                pending: element.url.clone(),
                zoom: element.zoom,
                navigate: element.navigate.clone(),
                loading: element.loading.clone(),
                ..State::default()
            }),
            drawn: Cell::new(None),
        });
        self.pages.insert(key, page.clone());
        Ok(page)
    }

    /// Every page, for [`presenter`].
    pub(crate) fn pages(&self) -> Vec<Rc<Page>> {
        self.pages.values().cloned().collect()
    }

    /// Runs `command` on the browser `id` of the mod `owner`, if it has one.
    /// Returns the address `browser-open` asks the system browser to show.
    pub(crate) fn command(&self, owner: &str, id: &str, command: BrowserCommand) -> Option<String> {
        match self.pages.get(&format!("{owner}/{id}")) {
            Some(page) => page.command(command),
            None => {
                tracing::debug!("{owner} has no browser `{id}` for {}", command.name());
                None
            },
        }
    }

    /// Loads what was typed in the address field of the browser `id` of the
    /// mod `owner`. Returns whether it went anywhere.
    pub(crate) fn visit(&self, owner: &str, id: &str, text: &str) -> bool {
        self.pages
            .get(&format!("{owner}/{id}"))
            .is_some_and(|page| page.visit(text))
    }

    /// Where the browser `id` of the mod `owner` is, if it has one.
    pub(crate) fn address(&self, owner: &str, id: &str) -> Option<String> {
        self.pages.get(&format!("{owner}/{id}"))?.address()
    }

    /// What the pages reported since the last call, as messages for their
    /// mods: `(mod, message)`.
    fn take(&self) -> Vec<(String, Data)> {
        let mut sends = Vec::new();
        for (key, event) in self.events.try_iter().take(BACKLOG) {
            if let Some(page) = self.pages.get(&key) {
                for msg in page.report(event) {
                    sends.push((page.owner.clone(), msg));
                }
            }
        }
        // Whatever is left past the backlog is stale by now.
        while self.events.try_recv().is_ok() {}
        sends
    }
}

impl MoonframeApp {
    /// Hands what the pages reported to their mods.
    fn browser_reports(&mut self, cx: &mut Context<Self>) {
        for (owner, msg) in self.browsers.take() {
            self.deliver(&owner, msg, cx);
        }
    }
}

/// A callback for pages, whose reports arrive on platform callbacks: each
/// call reads the reports on the next turn, and a burst of calls reads once.
pub(crate) fn wake_on_report(cx: &mut Context<MoonframeApp>) -> Arc<dyn Fn() + Send + Sync> {
    let (sender, mut woken) = futures::channel::mpsc::unbounded::<()>();
    cx.spawn(async move |this, cx| {
        while woken.next().await.is_some() {
            while woken.try_recv().is_ok() {}
            if this.update(cx, |app, cx| app.browser_reports(cx)).is_err() {
                break;
            }
        }
    })
    .detach();
    Arc::new(move || {
        let _ = sender.unbounded_send(());
    })
}

thread_local! {
    /// What floats over the layout this frame, by window: pages it reaches
    /// hide. [`presenter`] takes it.
    static COVERS: RefCell<Vec<(WindowId, Bounds<Pixels>)>> = const { RefCell::new(Vec::new()) };
}

/// Marks its parent as floating over the layout, so that a page under it
/// hides while it shows: put it in a popover, a toast or a dialog, as an
/// extra child. It takes no space and draws nothing.
pub(crate) fn cover() -> impl IntoElement {
    canvas(
        |bounds, window, _| {
            let id = window.window_handle().window_id();
            COVERS.with(|covers| covers.borrow_mut().push((id, bounds)));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// Shows each of `pages` drawn this frame where it drew, unless something
/// covers it, and hides the rest. It draws nothing; put it anywhere in the
/// window that has the pages, which it reads once every element has laid
/// out. A press anywhere in the window takes the keyboard back from the
/// pages, as the press reached the window, not a page.
pub(crate) fn presenter(pages: Vec<Rc<Page>>) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            let id = window.window_handle().window_id();
            let covers: Vec<Bounds<Pixels>> = COVERS.with(|covers| {
                covers
                    .take()
                    .into_iter()
                    .filter(|(window, _)| *window == id)
                    .map(|(_, bounds)| bounds)
                    .collect()
            });
            for page in &pages {
                let native = page.native.borrow();
                let drawn = page.drawn.take();
                let Some(view) = &*native else {
                    continue;
                };
                match drawn.filter(|(_, visible)| shows(*visible, &covers)) {
                    Some((bounds, visible)) => {
                        view.place(bounds, visible);
                        view.show();
                    },
                    None => view.hide(),
                }
            }
            window.on_mouse_event(move |_: &MouseDownEvent, phase, _, _| {
                if phase == DispatchPhase::Capture {
                    for page in &pages {
                        if let Some(view) = &*page.native.borrow() {
                            view.blur();
                        }
                    }
                }
            });
        },
    )
    .absolute()
    .size_0()
}

/// Whether a page showing `visible` is on screen and nothing covers it.
fn shows(visible: Bounds<Pixels>, covers: &[Bounds<Pixels>]) -> bool {
    let empty =
        |bounds: Bounds<Pixels>| bounds.size.width <= px(0.) || bounds.size.height <= px(0.);
    !empty(visible) && covers.iter().all(|cover| empty(visible.intersect(cover)))
}

/// `bounds` with each edge on the nearest whole device pixel at `scale`, so
/// the page's edges are sharp and meet the window's drawing exactly.
pub(crate) fn snap(bounds: Bounds<Pixels>, scale: f32) -> Bounds<Pixels> {
    if !scale.is_finite() || scale <= 0. {
        return bounds;
    }
    let edge = |value: Pixels| px((f32::from(value) * scale).round() / scale);
    Bounds::from_corners(
        point(edge(bounds.left()), edge(bounds.top())),
        point(edge(bounds.right()), edge(bounds.bottom())),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {
        super::{Browser, Browsers, Data, Event, shows},
        gpui_kit::{Bounds, point, px, size},
        moonframe_core::Look,
        std::{collections::BTreeMap, rc::Rc, sync::Arc},
    };

    fn element(id: &str, url: Option<&str>) -> Browser {
        Browser {
            look: Look::default(),
            id: id.to_owned(),
            url: url.map(str::to_owned),
            zoom: 1.0,
            navigate: Some(Data::Map(BTreeMap::from([(
                "go".to_owned(),
                Data::Bool(true),
            )]))),
            loading: Some(Data::String("loading".to_owned())),
        }
    }

    fn browsers() -> Browsers {
        Browsers::new(Arc::new(|| {}))
    }

    #[test]
    fn pages_are_kept_by_mod_and_id_and_bounded() {
        let mut browsers = browsers();
        let first = browsers.open("docs", &element("a", None)).unwrap();
        let again = browsers.open("docs", &element("a", None)).unwrap();
        assert!(Rc::ptr_eq(&first, &again));
        assert!(!Rc::ptr_eq(
            &first,
            &browsers.open("other", &element("a", None)).unwrap()
        ));
        for id in ["b", "c", "d"] {
            browsers.open("docs", &element(id, None)).unwrap();
        }
        let refused = browsers.open("docs", &element("e", None));
        assert!(refused.is_err_and(|error| error.contains("already has")));
    }

    #[test]
    fn a_new_url_from_the_mod_navigates_and_the_same_one_does_not() {
        let mut browsers = browsers();
        let page = browsers
            .open("docs", &element("a", Some("https://a.test/")))
            .unwrap();
        // Not built yet: the first address waits for the view.
        assert_eq!(
            page.state.borrow().pending.as_deref(),
            Some("https://a.test/")
        );
        page.state.borrow_mut().pending = None;
        browsers
            .open("docs", &element("a", Some("https://a.test/")))
            .unwrap();
        assert_eq!(page.state.borrow().pending, None);
        browsers
            .open("docs", &element("a", Some("https://b.test/")))
            .unwrap();
        assert_eq!(
            page.state.borrow().pending.as_deref(),
            Some("https://b.test/")
        );
    }

    #[test]
    fn reports_become_the_messages_the_mod_asked_for() {
        let mut browsers = browsers();
        let page = browsers.open("docs", &element("a", None)).unwrap();
        let started = page.report(Event::Started("https://a.test/".to_owned()));
        let field = |msg: &Data, key: &str| match msg {
            Data::Map(map) => map.get(key).cloned(),
            _ => None,
        };
        assert_eq!(started.len(), 2);
        // A string message is wrapped; a table one gets the fields added.
        assert_eq!(
            field(&started[0], "msg"),
            Some(Data::String("loading".to_owned()))
        );
        assert_eq!(field(&started[0], "loading"), Some(Data::Bool(true)));
        assert_eq!(field(&started[1], "go"), Some(Data::Bool(true)));
        assert_eq!(
            field(&started[1], "url"),
            Some(Data::String("https://a.test/".to_owned()))
        );
        assert_eq!(field(&started[1], "loading"), Some(Data::Bool(true)));
        // The load ending is news; the same report again is not.
        let finished = page.report(Event::Finished("https://a.test/".to_owned()));
        assert_eq!(finished.len(), 2);
        assert_eq!(field(&finished[1], "loading"), Some(Data::Bool(false)));
        assert_eq!(
            page.report(Event::Finished("https://a.test/".to_owned()))
                .len(),
            1
        );
        let titled = page.report(Event::Title("A".to_owned()));
        assert_eq!(
            field(&titled[0], "title"),
            Some(Data::String("A".to_owned()))
        );
        // A popup's address opens in the page; anything else is ignored.
        assert!(page.report(Event::Open("vscode://x".to_owned())).is_empty());
        assert_eq!(page.state.borrow().pending, None);
        // Nor a file, which a web page must not reach.
        assert!(
            page.report(Event::Open("file:///etc/passwd".to_owned()))
                .is_empty()
        );
        assert_eq!(page.state.borrow().pending, None);
        page.report(Event::Open("https://c.test/".to_owned()));
        assert_eq!(
            page.state.borrow().pending.as_deref(),
            Some("https://c.test/")
        );
    }

    #[test]
    fn typed_addresses_load_even_the_one_shown() {
        let mut browsers = browsers();
        let page = browsers
            .open("docs", &element("a", Some("https://a.test/")))
            .unwrap();
        page.state.borrow_mut().pending = None;
        assert!(browsers.visit("docs", "a", "a.test/"));
        assert_eq!(
            page.state.borrow().pending.as_deref(),
            Some("https://a.test/")
        );
        assert!(browsers.visit("docs", "a", "two words"));
        assert_eq!(
            page.state.borrow().pending.as_deref(),
            Some("https://duckduckgo.com/?q=two+words")
        );
        assert!(!browsers.visit("docs", "a", "  "));
        assert!(!browsers.visit("docs", "b", "a.test"));
        // The mod's address still stands: a new render does not undo it.
        assert_eq!(page.address().as_deref(), Some("https://a.test/"));
    }

    #[test]
    fn covered_or_clipped_pages_hide() {
        let at =
            |x: f32, y: f32, w: f32, h: f32| Bounds::new(point(px(x), px(y)), size(px(w), px(h)));
        let page = at(0., 0., 100., 100.);
        assert!(shows(page, &[]));
        assert!(shows(page, &[at(100., 0., 20., 20.)]));
        assert!(!shows(page, &[at(90., 90., 20., 20.)]));
        assert!(!shows(at(0., 0., 0., 100.), &[]));
    }
}
