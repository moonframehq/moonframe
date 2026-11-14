//! PDFs with WebKit: a `WKWebView` in an offscreen window, a session of its
//! own (`nonPersistentDataStore`), content rules that keep it off the
//! network (HTML) or off this machine and its network (a web page), then
//! its print operation, saving to a file without a panel.

#![allow(
    unsafe_code,
    reason = "WebKit and AppKit calls the bindings mark unsafe; each says why it is sound"
)]

use {
    super::{Job, Source},
    block2::RcBlock,
    futures::channel::oneshot,
    moonframe_core::Paper,
    objc2::{
        DefinedClass as _, MainThreadMarker, MainThreadOnly as _, Message as _, define_class,
        msg_send,
        rc::Retained,
        runtime::{AnyObject, Bool, NSObject, NSObjectProtocol, ProtocolObject},
        sel,
    },
    objc2_app_kit::{
        NSBackingStoreType, NSPrintInfo, NSPrintJobSavingURL, NSPrintOperation, NSPrintSaveJob,
        NSPrintingPaginationMode, NSWindow, NSWindowStyleMask,
    },
    objc2_foundation::{NSError, NSPoint, NSRect, NSSize, NSString, NSURL, NSURLRequest},
    objc2_web_kit::{
        WKContentRuleList, WKContentRuleListStore, WKNavigation, WKNavigationDelegate, WKWebView,
        WKWebViewConfiguration, WKWebsiteDataStore,
    },
    std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        ffi::c_void,
        path::PathBuf,
    },
};

/// Most PDFs made at once.
const MAX_RUNNING: usize = 3;
/// Each page's margins, in points: half an inch.
const MARGIN: f64 = 36.0;

/// For HTML: nothing from anywhere but the page itself and `data:`.
const OFFLINE: &str = r#"[
    {"trigger": {"url-filter": ".*"}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^data:"}, "action": {"type": "ignore-previous-rules"}},
    {"trigger": {"url-filter": "^about:"}, "action": {"type": "ignore-previous-rules"}}
]"#;

/// For an `.html` file: the same, and the file itself, the only one its
/// view may read.
const OFFLINE_FILE: &str = r#"[
    {"trigger": {"url-filter": ".*"}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^data:"}, "action": {"type": "ignore-previous-rules"}},
    {"trigger": {"url-filter": "^file:"}, "action": {"type": "ignore-previous-rules"}}
]"#;

/// For a web page: the web, never this machine or its network by address
/// (rules have no alternation, so one each), nor files.
const ONLINE: &str = r#"[
    {"trigger": {"url-filter": "^file:"}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://localhost[:/]"}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://127\\."}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://0\\."}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://10\\."}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://169\\.254\\."}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://192\\.168\\."}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://172\\.1[6-9]\\."}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://172\\.2[0-9]\\."}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://172\\.3[01]\\."}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://\\["}, "action": {"type": "block"}}
]"#;

/// A PDF being made: the window its view lays out in, and the delegate
/// that prints it once loaded.
struct Running {
    window: Retained<NSWindow>,
    view: Retained<WKWebView>,
    _delegate: Retained<Delegate>,
}

thread_local! {
    static RUNNING: RefCell<HashMap<u64, Running>> = RefCell::new(HashMap::new());
    static NEXT: Cell<u64> = const { Cell::new(1) };
    /// The compiled rules for each kind of page, once compiled.
    static RULES: RefCell<[Option<Retained<WKContentRuleList>>; 3]> =
        const { RefCell::new([None, None, None]) };
}

pub(super) fn render(source: Source, paper: Option<Paper>, out: PathBuf) -> Result<Job, String> {
    let main = MainThreadMarker::new().ok_or("PDFs are made on the main thread only")?;
    if RUNNING.with_borrow(HashMap::len) >= MAX_RUNNING {
        return Err(format!(
            "{MAX_RUNNING} PDFs are being made already; ask again once one is done"
        ));
    }
    let id = NEXT.get();
    NEXT.set(id + 1);
    let (answer, done) = oneshot::channel();
    let kind = match source {
        Source::Html(_) => 0,
        Source::HtmlFile(_) => 1,
        Source::Url(_) => 2,
    };
    let job = Pending {
        id,
        source,
        paper,
        out,
        answer,
    };
    with_rules(kind, main, move |rules| match rules {
        Ok(rules) => start(job, &rules, main),
        Err(problem) => {
            let _ = job.answer.send(Err(problem));
        },
    });
    Ok(Job { id, done })
}

/// Lets the view of the PDF `id` go, done or not.
pub(super) fn finish(id: u64) {
    let running = RUNNING.with_borrow_mut(|running| running.remove(&id));
    if let Some(running) = running {
        unsafe { running.view.setNavigationDelegate(None) };
        running.window.close();
    }
}

/// What a PDF needs until its view starts.
struct Pending {
    id: u64,
    source: Source,
    paper: Option<Paper>,
    out: PathBuf,
    answer: oneshot::Sender<Result<(), String>>,
}

/// Runs `then` with the rules for HTML (`0`), an `.html` file (`1`) or a
/// web page (`2`), compiling them the first time.
fn with_rules(
    kind: usize,
    main: MainThreadMarker,
    then: impl FnOnce(Result<Retained<WKContentRuleList>, String>) + 'static,
) {
    let slot = kind;
    if let Some(rules) = RULES.with_borrow(|rules| rules[slot].clone()) {
        return then(Ok(rules));
    }
    // SAFETY: the main thread's default store.
    let Some(store) = (unsafe { WKContentRuleListStore::defaultStore(main) }) else {
        return then(Err("WebKit has no content rule store".to_owned()));
    };
    let then = RefCell::new(Some(then));
    let compiled = RcBlock::new(move |list: *mut WKContentRuleList, error: *mut NSError| {
        let Some(then) = then.borrow_mut().take() else {
            return;
        };
        // SAFETY: WebKit passes a valid list or error, or null.
        let list = unsafe { list.as_ref() }.map(|list| list.retain());
        match list {
            Some(list) => {
                RULES.with_borrow_mut(|rules| rules[slot] = Some(list.clone()));
                then(Ok(list));
            },
            None => {
                let why = unsafe { error.as_ref() }.map_or_else(
                    || "unknown".to_owned(),
                    |error| error.localizedDescription().to_string(),
                );
                then(Err(format!(
                    "WebKit would not take the page's rules: {why}"
                )));
            },
        }
    });
    let (identifier, rules) = [
        ("moonframe-pdf-html", OFFLINE),
        ("moonframe-pdf-file", OFFLINE_FILE),
        ("moonframe-pdf-web", ONLINE),
    ][slot];
    // SAFETY: plain strings, and a block WebKit calls once on the main thread.
    unsafe {
        store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(
            Some(&NSString::from_str(identifier)),
            Some(&NSString::from_str(rules)),
            Some(&compiled),
        );
    }
}

/// The paper the PDF's pages are, in points.
fn paper_size(paper: Option<Paper>) -> NSSize {
    match paper {
        Some(Paper::A4) => NSSize::new(595.28, 841.89),
        Some(Paper::Letter) => NSSize::new(612.0, 792.0),
        None => NSPrintInfo::sharedPrintInfo().paperSize(),
    }
}

/// Loads the page in a view of its own; its delegate prints it.
fn start(job: Pending, rules: &WKContentRuleList, main: MainThreadMarker) {
    let paper = paper_size(job.paper);
    let width = (paper.width - 2.0 * MARGIN).max(100.0);
    let online = matches!(job.source, Source::Url(_));
    // SAFETY: a fresh configuration on the main thread, given its own
    // store, script setting and rules before any view takes it.
    let configuration = unsafe {
        let configuration = WKWebViewConfiguration::new(main);
        configuration.setWebsiteDataStore(&WKWebsiteDataStore::nonPersistentDataStore(main));
        configuration
            .defaultWebpagePreferences()
            .setAllowsContentJavaScript(online);
        configuration
            .userContentController()
            .addContentRuleList(rules);
        // Colours and pictures behind the text print, as the page shows them.
        configuration.preferences().setShouldPrintBackgrounds(true);
        configuration
    };
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, 1000.0));
    // SAFETY: a fresh view with the configuration above.
    let view = unsafe {
        WKWebView::initWithFrame_configuration(WKWebView::alloc(main), frame, &configuration)
    };
    // Far off any screen, never shown: the view lays out in it.
    let place = NSRect::new(NSPoint::new(-30_000.0, -30_000.0), frame.size);
    // SAFETY: a fresh borderless window, kept (not released) when closed,
    // as `RUNNING` holds it.
    let window = unsafe {
        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(main),
            place,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        );
        window.setReleasedWhenClosed(false);
        window
    };
    window.setContentView(Some(&view));
    let id = job.id;
    let delegate = Delegate::new(main, Printing {
        id,
        paper,
        out: job.out,
        answer: RefCell::new(Some(job.answer)),
    });
    // SAFETY: the delegate lives in `RUNNING` as long as the view does.
    unsafe { view.setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate))) };
    let loading = match &job.source {
        // SAFETY: a string and no base, so the page has no origin to reach.
        Source::Html(html) => unsafe {
            view.loadHTMLString_baseURL(&NSString::from_str(html), None)
        },
        Source::HtmlFile(file) => {
            let url = NSURL::fileURLWithPath(&NSString::from_str(&file.to_string_lossy()));
            // SAFETY: the file alone may be read, nothing beside it.
            unsafe { view.loadFileURL_allowingReadAccessToURL(&url, &url) }
        },
        Source::Url(address) => {
            NSURL::URLWithString(&NSString::from_str(address)).and_then(|url| {
                // SAFETY: a request for a checked web address.
                unsafe { view.loadRequest(&NSURLRequest::requestWithURL(&url)) }
            })
        },
    };
    if loading.is_none() {
        delegate.answer(Err("the page did not start loading".to_owned()));
    }
    RUNNING.with_borrow_mut(|running| {
        running.insert(id, Running {
            window,
            view,
            _delegate: delegate,
        })
    });
}

/// What the delegate prints, and to whom it answers.
struct Printing {
    id: u64,
    paper: NSSize,
    out: PathBuf,
    answer: RefCell<Option<oneshot::Sender<Result<(), String>>>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the delegate
    // implements no `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = objc2::MainThreadOnly]
    #[name = "MoonframePdfDelegate"]
    #[ivars = Printing]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl WKNavigationDelegate for Delegate {
        #[unsafe(method(webView:didFinishNavigation:))]
        fn did_finish(&self, view: &WKWebView, _navigation: Option<&WKNavigation>) {
            if let Err(problem) = self.print(view) {
                self.answer(Err(problem));
            }
        }

        #[unsafe(method(webView:didFailNavigation:withError:))]
        fn did_fail(&self, _view: &WKWebView, _navigation: Option<&WKNavigation>, error: &NSError) {
            self.answer(Err(format!("the page did not load: {}", error.localizedDescription())));
        }

        #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
        fn did_fail_early(
            &self,
            _view: &WKWebView,
            _navigation: Option<&WKNavigation>,
            error: &NSError,
        ) {
            self.answer(Err(format!("the page did not load: {}", error.localizedDescription())));
        }
    }

    impl Delegate {
        /// The print operation is over.
        #[unsafe(method(printOperationDidRun:success:contextInfo:))]
        fn printed(&self, _operation: &NSPrintOperation, success: Bool, _context: *mut c_void) {
            let out = &self.ivars().out;
            self.answer(if success.as_bool() && out.is_file() {
                Ok(())
            } else {
                Err("WebKit did not print the page".to_owned())
            });
        }
    }
);

impl Delegate {
    fn new(main: MainThreadMarker, printing: Printing) -> Retained<Self> {
        let this = Self::alloc(main).set_ivars(printing);
        // SAFETY: NSObject's `init`, on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// Answers once; later answers (a failure after a print) are dropped.
    fn answer(&self, result: Result<(), String>) {
        if let Some(answer) = self.ivars().answer.borrow_mut().take() {
            let _ = answer.send(result);
        }
    }

    /// Prints the loaded page to the PDF file, in its window, without a
    /// panel; `printed` hears how it went.
    fn print(&self, view: &WKWebView) -> Result<(), String> {
        let printing = self.ivars();
        let window = RUNNING
            .with_borrow(|running| {
                running
                    .get(&printing.id)
                    .map(|running| running.window.clone())
            })
            .ok_or("the PDF was given up")?;
        // SAFETY: `copy` of the shared print info gives a print info of
        // our own, its settings changed below before any operation runs.
        let info: Retained<NSPrintInfo> =
            unsafe { msg_send![&*NSPrintInfo::sharedPrintInfo(), copy] };
        info.setPaperSize(printing.paper);
        info.setTopMargin(MARGIN);
        info.setBottomMargin(MARGIN);
        info.setLeftMargin(MARGIN);
        info.setRightMargin(MARGIN);
        info.setHorizontalPagination(NSPrintingPaginationMode::Fit);
        info.setVerticalPagination(NSPrintingPaginationMode::Automatic);
        info.setHorizontallyCentered(false);
        info.setVerticallyCentered(false);
        let url = NSURL::fileURLWithPath(&NSString::from_str(&printing.out.to_string_lossy()));
        // SAFETY: AppKit's own keys and values, a URL where it wants one.
        unsafe {
            info.setJobDisposition(NSPrintSaveJob);
            info.dictionary()
                .setObject_forKey(&url, ProtocolObject::from_ref(NSPrintJobSavingURL));
        }
        // SAFETY: WKWebView's print operation (macOS 11), for its page.
        let operation: Option<Retained<NSPrintOperation>> =
            unsafe { msg_send![view, printOperationWithPrintInfo: &*info] };
        let operation = operation.ok_or("WebKit has no print operation")?;
        operation.setShowsPrintPanel(false);
        operation.setShowsProgressPanel(false);
        // WebKit's print view has no size until given one.
        if let Some(printed) = operation.view() {
            printed.setFrame(view.bounds());
        }
        let delegate: &AnyObject = self;
        // SAFETY: the window holds the view; the delegate answers the
        // selector, with the operation, a flag and the null context.
        unsafe {
            operation.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
                &window,
                Some(delegate),
                Some(sel!(printOperationDidRun:success:contextInfo:)),
                std::ptr::null_mut(),
            );
        }
        Ok(())
    }
}
