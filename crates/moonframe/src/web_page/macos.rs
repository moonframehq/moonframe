//! A page in WebKit for `moonframe.page`: a `WKWebView` in an offscreen
//! window with the web session a PDF of a web page has (its own, going out
//! through the PDF proxy alone), read once it has loaded or run out of
//! time: a picture first, when asked for, then its text, by a script in a
//! world of its own, so the page's script cannot change what reads it.

#![allow(
    unsafe_code,
    reason = "WebKit and AppKit calls the bindings mark unsafe; each says why it is sound"
)]

use {
    super::{MAX_PICTURE, READING, Rendered, Request, Screenshot},
    crate::pdf::macos::{WEB, load, offscreen, web_configuration, with_rules},
    block2::RcBlock,
    dispatch2::DispatchQueue,
    objc2::{
        AnyThread as _, DefinedClass as _, MainThreadMarker, MainThreadOnly as _, define_class,
        msg_send,
        rc::Retained,
        runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject},
    },
    objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSDeviceRGBColorSpace, NSGraphicsContext, NSImage,
        NSWindow,
    },
    objc2_foundation::{
        NSDataBase64EncodingOptions, NSDictionary, NSError, NSNumber, NSPoint, NSRect, NSSize,
        NSString,
    },
    objc2_web_kit::{
        WKContentRuleList, WKContentWorld, WKNavigation, WKNavigationDelegate,
        WKSnapshotConfiguration, WKWebView,
    },
    std::{
        cell::{Cell, RefCell},
        collections::{HashMap, HashSet},
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64, Ordering},
            mpsc::{self, RecvTimeoutError},
        },
    },
};

/// Most pages rendered at once.
const MAX_RUNNING: usize = 4;

/// Reads the page's title and text, leaving out what `DROP` (a JSON list
/// of selectors) names; run in a world of its own, so the page's script
/// cannot redefine what it calls. Answers JSON text.
const READ: &str = r#"(() => {
  for (const selector of DROP) {
    try { for (const node of document.querySelectorAll(selector)) node.remove(); } catch (error) {}
  }
  const text = document.body === null ? "" : document.body.innerText;
  return JSON.stringify({ title: document.title, text: text.replace(/\n{3,}/g, "\n\n").trim() });
})()"#;

static NEXT: AtomicU64 = AtomicU64::new(1);

type Answer = mpsc::Sender<Result<Rendered, String>>;

/// A page being rendered: the window its view lays out in, and who waits.
struct Running {
    window: Retained<NSWindow>,
    view: Retained<WKWebView>,
    _delegate: Retained<Delegate>,
    request: Request,
    answer: Answer,
    /// It is being read: it loaded, or ran out of time.
    reading: bool,
}

thread_local! {
    static RUNNING: RefCell<HashMap<u64, Running>> = RefCell::new(HashMap::new());
    /// Pages waiting for their rules to compile, counted with `RUNNING`.
    static STARTING: Cell<usize> = const { Cell::new(0) };
    /// Pages whose time ran out while their rules compiled: they answer
    /// as they would have shown then, with nothing, rather than load.
    static LATE: RefCell<HashSet<u64>> = RefCell::new(HashSet::new());
}

/// Renders `request` on the main thread, waiting here (a provider's call)
/// for its answer: once it has loaded, or as it shows when its time is up.
pub(super) fn render(request: Request) -> Result<Rendered, String> {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let wanted = Arc::new(AtomicBool::new(true));
    let (answer, answered) = mpsc::channel();
    let timeout = request.timeout;
    let starting = Arc::clone(&wanted);
    DispatchQueue::main().exec_async(move || start(id, request, answer, &starting));
    let result = match answered.recv_timeout(timeout) {
        Err(RecvTimeoutError::Timeout) => {
            DispatchQueue::main().exec_async(move || read(id, true));
            answered.recv_timeout(READING)
        },
        result => result,
    };
    wanted.store(false, Ordering::Release);
    DispatchQueue::main().exec_async(move || finish(id));
    match result {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err("the page could not be read in time".to_owned()),
        Err(RecvTimeoutError::Disconnected) => Err("the page was given up".to_owned()),
    }
}

/// Starts loading the page, once the rules for a web page are compiled.
fn start(id: u64, request: Request, answer: Answer, wanted: &Arc<AtomicBool>) {
    let Some(main) = MainThreadMarker::new() else {
        let _ = answer.send(Err("pages are rendered on the main thread only".to_owned()));
        return;
    };
    if RUNNING.with_borrow(HashMap::len) + STARTING.get() >= MAX_RUNNING {
        let _ = answer.send(Err(format!(
            "{MAX_RUNNING} pages are being rendered already; ask again once one is done"
        )));
        return;
    }
    STARTING.set(STARTING.get() + 1);
    let wanted = Arc::clone(wanted);
    with_rules(WEB, main, move |rules| {
        STARTING.set(STARTING.get().saturating_sub(1));
        // Given up while its rules compiled.
        if !wanted.load(Ordering::Acquire) {
            return;
        }
        // Out of time while they compiled: nothing showed in its time.
        if LATE.with_borrow_mut(|late| late.remove(&id)) {
            let _ = answer.send(Ok(Rendered::page(&request.url, "", "", true)));
            return;
        }
        match rules.and_then(|rules| open(id, &request, &rules, main)) {
            Ok((window, view, delegate)) => RUNNING.with_borrow_mut(|running| {
                running.insert(id, Running {
                    window,
                    view,
                    _delegate: delegate,
                    request,
                    answer,
                    reading: false,
                });
            }),
            Err(problem) => {
                let _ = answer.send(Err(problem));
            },
        }
    });
}

/// The page's view, loading, and the delegate that reads it once loaded.
fn open(
    id: u64,
    request: &Request,
    rules: &WKContentRuleList,
    main: MainThreadMarker,
) -> Result<(Retained<NSWindow>, Retained<WKWebView>, Retained<Delegate>), String> {
    let configuration = web_configuration(rules, main)?;
    let size = NSSize::new(f64::from(request.width), f64::from(request.height));
    let (window, view) = offscreen(&configuration, size, main);
    let delegate = Delegate::new(main, Page { id });
    // SAFETY: the delegate lives in `RUNNING` as long as the view does.
    unsafe { view.setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate))) };
    load(&view, &request.url).ok_or("the page did not start loading")?;
    Ok((window, view, delegate))
}

/// Reads the page `id` once: a picture, when asked for, then its text.
/// `timed_out` when its time ran out first: it stops loading and is read
/// as it shows.
fn read(id: u64, timed_out: bool) {
    let Some(main) = MainThreadMarker::new() else {
        return;
    };
    let opened = RUNNING.with_borrow_mut(|running| {
        let running = running.get_mut(&id)?;
        Some((!running.reading).then(|| {
            running.reading = true;
            (running.view.clone(), running.request.clone())
        }))
    });
    let Some((view, request)) = (match opened {
        Some(reading) => reading,
        // Not open yet: its rules are still compiling.
        None => {
            if timed_out {
                LATE.with_borrow_mut(|late| late.insert(id));
            }
            None
        },
    }) else {
        return;
    };
    if timed_out {
        // SAFETY: the page's own view, on the main thread.
        unsafe { view.stopLoading() };
    }
    if !request.screenshot {
        return text(id, timed_out, None, main);
    }
    let (width, height) = (request.width, request.height);
    let shot = RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
        // SAFETY: WebKit passes a valid picture or error, or null.
        let picture = match unsafe { image.as_ref() } {
            Some(image) => picture(image, width, height),
            None => Err(format!("WebKit took no picture: {}", described(error))),
        };
        if let Some(main) = MainThreadMarker::new() {
            text(id, timed_out, Some(picture), main);
        }
    });
    // SAFETY: a fresh configuration, and a block WebKit calls once on the
    // main thread.
    unsafe {
        let configuration = WKSnapshotConfiguration::new(main);
        configuration.setSnapshotWidth(Some(&NSNumber::new_f64(f64::from(width))));
        view.takeSnapshotWithConfiguration_completionHandler(Some(&configuration), &shot);
    }
}

/// Reads the page's text, then answers with it and `picture`.
fn text(
    id: u64,
    timed_out: bool,
    picture: Option<Result<Screenshot, String>>,
    main: MainThreadMarker,
) {
    let Some((view, request)) = RUNNING.with_borrow(|running| {
        running
            .get(&id)
            .map(|running| (running.view.clone(), running.request.clone()))
    }) else {
        return;
    };
    let drop = serde_json::to_string(&request.drop).unwrap_or_else(|_| "[]".to_owned());
    let script = READ.replace("DROP", &drop);
    let picture = RefCell::new(picture);
    let page = view.clone();
    let done = RcBlock::new(move |result: *mut AnyObject, error: *mut NSError| {
        // SAFETY: WebKit passes the script's value or an error, or null.
        let read =
            match unsafe { result.as_ref() }.and_then(|value| value.downcast_ref::<NSString>()) {
                Some(json) => title_and_text(&json.to_string()),
                None => Err(format!("the page could not be read: {}", described(error))),
            };
        // SAFETY: the page's own view, on the main thread.
        let url = unsafe { page.URL() }
            .and_then(|url| url.absoluteString())
            .map_or_else(|| request.url.clone(), |url| url.to_string());
        let rendered = read.map(|(title, text)| {
            let mut rendered = Rendered::page(&url, &title, &text, timed_out);
            match picture.borrow_mut().take() {
                Some(Ok(screenshot)) => rendered.screenshot = Some(screenshot),
                Some(Err(problem)) => rendered.screenshot_error = Some(problem),
                None => {},
            }
            rendered
        });
        answer(id, rendered);
    });
    // SAFETY: a script string, the main frame, a world of WebKit's own for
    // scripts the app runs, and a block WebKit calls once on the main thread.
    unsafe {
        view.evaluateJavaScript_inFrame_inContentWorld_completionHandler(
            &NSString::from_str(&script),
            None,
            &WKContentWorld::defaultClientWorld(main),
            Some(&done),
        );
    }
}

/// The title and text in what [`READ`] answered.
fn title_and_text(json: &str) -> Result<(String, String), String> {
    #[derive(serde::Deserialize)]
    struct Read {
        title: String,
        text: String,
    }
    let read: Read = serde_json::from_str(json)
        .map_err(|error| format!("the page's text did not come back: {error}"))?;
    Ok((read.title, read.text))
}

/// `image` as a PNG `width` pixels wide, whatever the screen's scale: a
/// picture for reading, not for a high-density screen.
fn picture(image: &NSImage, width: u32, height: u32) -> Result<Screenshot, String> {
    let size = image.size();
    let high = if size.width > 0.0 {
        (size.height * f64::from(width) / size.width).round()
    } else {
        f64::from(height)
    };
    let (wide, high) = (i64::from(width), (high as i64).clamp(1, 16_384));
    // SAFETY: a bitmap that allocates its own pixels, 8 bits for each of
    // red, green, blue and alpha, in the device's RGB space.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            wide as isize,
            high as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )
    }
    .ok_or("no bitmap for the picture")?;
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)
        .ok_or("no context to draw in")?;
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    image.drawInRect(NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(wide as f64, high as f64),
    ));
    context.flushGraphics();
    NSGraphicsContext::restoreGraphicsState_class();
    // SAFETY: AppKit's PNG type, with no properties.
    let png = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .ok_or("the picture could not be made a PNG")?;
    if png.length() > MAX_PICTURE {
        return Err(format!(
            "the picture is over {} MiB",
            MAX_PICTURE / 1024 / 1024
        ));
    }
    Ok(Screenshot {
        mime: "image/png",
        base64: png
            .base64EncodedStringWithOptions(NSDataBase64EncodingOptions::empty())
            .to_string(),
        width,
        height: high as u32,
    })
}

/// Tells who waits for the page `id` how it went.
fn answer(id: u64, result: Result<Rendered, String>) {
    RUNNING.with_borrow(|running| {
        if let Some(running) = running.get(&id) {
            let _ = running.answer.send(result);
        }
    });
}

/// Lets the page `id` go, answered or given up.
fn finish(id: u64) {
    LATE.with_borrow_mut(|late| late.remove(&id));
    if let Some(running) = RUNNING.with_borrow_mut(|running| running.remove(&id)) {
        // SAFETY: the page's own view, on the main thread; it loads no more.
        unsafe {
            running.view.setNavigationDelegate(None);
            running.view.stopLoading();
        }
        running.window.close();
    }
}

/// What `error` says, when there is one.
fn described(error: *mut NSError) -> String {
    // SAFETY: WebKit passes a valid error or null.
    unsafe { error.as_ref() }.map_or_else(
        || "no reason given".to_owned(),
        |error| error.localizedDescription().to_string(),
    )
}

/// Which page a delegate reads.
struct Page {
    id: u64,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the delegate
    // implements no `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = objc2::MainThreadOnly]
    #[name = "MoonframePageDelegate"]
    #[ivars = Page]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl WKNavigationDelegate for Delegate {
        #[unsafe(method(webView:didFinishNavigation:))]
        fn did_finish(&self, _view: &WKWebView, _navigation: Option<&WKNavigation>) {
            // A page that goes on to another is read as it first finished.
            read(self.ivars().id, false);
        }

        #[unsafe(method(webView:didFailNavigation:withError:))]
        fn did_fail(&self, _view: &WKWebView, _navigation: Option<&WKNavigation>, error: &NSError) {
            self.failed(error);
        }

        #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
        fn did_fail_early(
            &self,
            _view: &WKWebView,
            _navigation: Option<&WKNavigation>,
            error: &NSError,
        ) {
            self.failed(error);
        }
    }
);

impl Delegate {
    fn new(main: MainThreadMarker, page: Page) -> Retained<Self> {
        let this = Self::alloc(main).set_ivars(page);
        // SAFETY: NSObject's `init`, on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// The page did not load; one already being read goes on.
    fn failed(&self, error: &NSError) {
        let id = self.ivars().id;
        let reading = RUNNING.with_borrow_mut(|running| {
            running
                .get_mut(&id)
                .is_none_or(|page| std::mem::replace(&mut page.reading, true))
        });
        if !reading {
            answer(
                id,
                Err(format!(
                    "the page did not load: {}",
                    error.localizedDescription()
                )),
            );
        }
    }
}
