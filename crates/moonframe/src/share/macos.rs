//! The share menu with AppKit's `NSSharingServicePicker`, whose delegate
//! hears which service the user picked.

use {
    super::Items,
    futures::channel::oneshot,
    gpui_kit::Window,
    objc2::{
        AnyThread as _, DefinedClass as _, MainThreadMarker, MainThreadOnly as _, Message as _,
        define_class, msg_send,
        rc::Retained,
        runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject},
    },
    objc2_app_kit::{
        NSSharingService, NSSharingServicePicker, NSSharingServicePickerDelegate, NSView,
    },
    objc2_foundation::{NSArray, NSPoint, NSRect, NSRectEdge, NSSize, NSString, NSURL},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    std::cell::RefCell,
};

/// The window's view, which the menu shows over.
pub(crate) struct Anchor {
    view: Retained<NSView>,
}

/// Who hears the answer: the delegate's one channel, taken when answered.
type Answer = RefCell<Option<oneshot::Sender<Option<String>>>>;

thread_local! {
    /// The picker showing, and its delegate, which it holds weakly.
    static SHOWN: RefCell<Option<(Retained<NSSharingServicePicker>, Retained<Delegate>)>> =
        const { RefCell::new(None) };
}

#[allow(unsafe_code, reason = "the window handle's view; see SAFETY")]
pub(super) fn anchor(window: &Window) -> Result<Anchor, String> {
    // `Window` has an inherent `window_handle` of its own.
    let handle = HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return Err("not an AppKit window".to_owned());
    };
    // SAFETY: the handle's view is the window's, alive as `window` is; it
    // is retained here for as long as the menu needs it.
    let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    Ok(Anchor {
        view: view.retain(),
    })
}

#[allow(unsafe_code, reason = "AppKit's picker initializer; see SAFETY")]
pub(super) fn show(anchor: &Anchor, items: Items) -> oneshot::Receiver<Option<String>> {
    let (answer, answered) = oneshot::channel();
    let Some(main) = MainThreadMarker::new() else {
        tracing::warn!("the share menu shows from the main thread only");
        let _ = answer.send(None);
        return answered;
    };
    // One at a time: closing the last one answers it, cancelled.
    if let Some((picker, delegate)) = SHOWN.take() {
        picker.close();
        drop(delegate);
    }
    let mut objects: Vec<Retained<AnyObject>> = Vec::new();
    if let Some(text) = &items.text {
        objects.push(NSString::from_str(text).into());
    }
    if let Some(url) = items
        .url
        .as_deref()
        .and_then(|url| NSURL::URLWithString(&NSString::from_str(url)))
    {
        objects.push(url.into());
    }
    for file in &items.files {
        objects.push(NSURL::fileURLWithPath(&NSString::from_str(&file.to_string_lossy())).into());
    }
    let objects = NSArray::from_retained_slice(&objects);
    // SAFETY: strings and URLs, which the pasteboard writes, as the
    // picker wants its items.
    let picker =
        unsafe { NSSharingServicePicker::initWithItems(NSSharingServicePicker::alloc(), &objects) };
    let delegate = Delegate::new(main, answer);
    picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    let at = NSRect::new(pointer(&anchor.view), NSSize::new(1.0, 1.0));
    SHOWN.set(Some((picker.clone(), delegate)));
    picker.showRelativeToRect_ofView_preferredEdge(at, &anchor.view, NSRectEdge::MinY);
    answered
}

/// Where the pointer is in `view`, kept inside it: where the click that
/// asked was, or the view's middle when it is not over the window.
fn pointer(view: &NSView) -> NSPoint {
    let bounds = view.bounds();
    let middle = NSPoint::new(
        bounds.origin.x + bounds.size.width / 2.0,
        bounds.origin.y + bounds.size.height / 2.0,
    );
    let Some(window) = view.window() else {
        return middle;
    };
    let at = view.convertPoint_fromView(window.mouseLocationOutsideOfEventStream(), None);
    let inside = (bounds.origin.x..=bounds.origin.x + bounds.size.width).contains(&at.x)
        && (bounds.origin.y..=bounds.origin.y + bounds.size.height).contains(&at.y);
    if inside {
        at
    } else {
        middle
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the delegate
    // implements no `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = objc2::MainThreadOnly]
    #[name = "MoonframeSharePickerDelegate"]
    #[ivars = Answer]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl NSSharingServicePickerDelegate for Delegate {
        /// The user picked a service, or none (closed the menu).
        #[unsafe(method(sharingServicePicker:didChooseSharingService:))]
        fn did_choose(&self, _picker: &NSSharingServicePicker, service: Option<&NSSharingService>) {
            if let Some(answer) = self.ivars().borrow_mut().take() {
                let _ = answer.send(service.map(|service| service.title().to_string()));
            }
        }
    }
);

impl Delegate {
    #[allow(unsafe_code, reason = "NSObject's init; see SAFETY")]
    fn new(main: MainThreadMarker, answer: oneshot::Sender<Option<String>>) -> Retained<Self> {
        let this = Self::alloc(main).set_ivars(RefCell::new(Some(answer)));
        // SAFETY: NSObject's `init`, on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}
