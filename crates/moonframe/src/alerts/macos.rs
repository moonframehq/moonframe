//! Sounds with `NSSound`, notifications with `UNUserNotificationCenter`.

use {
    super::Permission,
    block2::RcBlock,
    moonframe_core::Notification,
    objc2::{
        AnyThread as _, define_class, msg_send,
        rc::Retained,
        runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject},
    },
    objc2_app_kit::NSSound,
    objc2_foundation::{NSArray, NSBundle, NSData, NSError, NSString},
    objc2_user_notifications::{
        UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent,
        UNNotification, UNNotificationPresentationOptions, UNNotificationRequest,
        UNNotificationResponse, UNNotificationSettings, UNUserNotificationCenter,
        UNUserNotificationCenterDelegate,
    },
    std::{cell::Cell, path::Path, ptr::NonNull, sync::OnceLock},
};

/// The sounds playing: `NSSound` stops when its last reference goes.
#[derive(Default)]
pub(super) struct Sounds {
    playing: Vec<Retained<NSSound>>,
}

impl Sounds {
    pub(super) fn play_file(&mut self, file: &Path) -> Result<(), String> {
        let path = NSString::from_str(&file.to_string_lossy());
        let sound = NSSound::initWithContentsOfFile_byReference(NSSound::alloc(), &path, true)
            .ok_or_else(|| format!("{} is not a sound macOS can play", file.display()))?;
        self.start(sound)
    }

    pub(super) fn play_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        let data = NSData::with_bytes(bytes);
        let sound = NSSound::initWithData(NSSound::alloc(), &data)
            .ok_or_else(|| "the chime is not a sound macOS can play".to_owned())?;
        self.start(sound)
    }

    fn start(&mut self, sound: Retained<NSSound>) -> Result<(), String> {
        self.playing.retain(|sound| sound.isPlaying());
        if !sound.play() {
            return Err("macOS did not play the sound".to_owned());
        }
        self.playing.push(sound);
        Ok(())
    }
}

/// Whether this run is an app bundle with an identifier: the notification
/// centre throws for a bare binary (`cargo run`), so nothing is posted
/// from one.
pub(super) fn bundled() -> bool {
    static BUNDLED: OnceLock<bool> = OnceLock::new();
    *BUNDLED.get_or_init(|| {
        let bundle = NSBundle::mainBundle();
        bundle.bundleIdentifier().is_some() && bundle.bundlePath().to_string().ends_with(".app")
    })
}

/// What is told of each click, with the notification's identifier.
type Clicked = Box<dyn Fn(&str) + Send + Sync>;
static CLICKED: OnceLock<Clicked> = OnceLock::new();
/// What is told of whether the user lets the app post notifications.
type Changed = Box<dyn Fn(Permission) + Send + Sync>;
static CHANGED: OnceLock<Changed> = OnceLock::new();

thread_local! {
    /// The centre's delegate, which it holds weakly; the main thread's.
    static DELEGATE: Cell<Option<Retained<Delegate>>> = const { Cell::new(None) };
    /// Whether the user was asked to allow notifications this run.
    static ASKED: Cell<bool> = const { Cell::new(false) };
}

/// Hears clicks from now on, and whether notifications are allowed, now
/// and after each answer: in a bundle, as the caller checks.
pub(super) fn listen(clicked: Clicked, changed: Changed) {
    if CLICKED.set(clicked).is_err() || CHANGED.set(changed).is_err() {
        return;
    }
    let delegate = Delegate::new();
    let center = UNUserNotificationCenter::currentNotificationCenter();
    center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    DELEGATE.set(Some(delegate));
    read_permission();
}

/// Tells [`listen`]'s `changed` whether notifications are allowed, once the
/// centre says.
#[allow(
    unsafe_code,
    reason = "reading the settings the centre passes; see SAFETY"
)]
pub(super) fn read_permission() {
    let Some(changed) = CHANGED.get() else {
        return;
    };
    let read = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
        // SAFETY: the centre passes valid settings.
        let status = unsafe { settings.as_ref() }.authorizationStatus();
        changed(match status {
            UNAuthorizationStatus::NotDetermined => Permission::Undecided,
            UNAuthorizationStatus::Denied => Permission::Denied,
            _ => Permission::Allowed,
        });
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .getNotificationSettingsWithCompletionHandler(&read);
}

/// Asks the user to allow notifications; once answered, or at once if
/// they have before, says how it stands.
#[allow(
    unsafe_code,
    reason = "reading the errors the centre passes; see SAFETY"
)]
pub(super) fn ask() {
    ASKED.set(true);
    let answered = RcBlock::new(|granted: Bool, error: *mut NSError| {
        if !granted.as_bool() {
            // SAFETY: the centre passes a valid error or null.
            let reason =
                unsafe { error.as_ref() }.map(|error| error.localizedDescription().to_string());
            tracing::info!(
                "notifications are not allowed{}",
                reason
                    .map(|reason| format!(": {reason}"))
                    .unwrap_or_default()
            );
        }
        read_permission();
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &answered,
        );
}

/// Posts `notification` as `identifier`; false when this run cannot (a
/// bare binary), for the caller to show it otherwise. The first one asks
/// the user to allow notifications, unless a mod asked before; until they
/// do, macOS drops them. One that replaces takes the place of those
/// delivered with its mod and `id`.
#[allow(unsafe_code, reason = "reading what the centre passes; see SAFETY")]
pub(super) fn post(identifier: &str, notification: &Notification) -> bool {
    if !bundled() {
        return false;
    }
    if !ASKED.get() {
        ask();
    }
    let center = UNUserNotificationCenter::currentNotificationCenter();
    if notification.replace {
        let posted = identifier.to_owned();
        let delivered = RcBlock::new(move |delivered: NonNull<NSArray<UNNotification>>| {
            // SAFETY: the centre passes a valid array.
            let delivered = unsafe { delivered.as_ref() };
            let replaced: Vec<Retained<NSString>> = delivered
                .iter()
                .map(|notification| notification.request().identifier())
                .filter(|other| super::replaced_by(&posted, &other.to_string()))
                .collect();
            if !replaced.is_empty() {
                UNUserNotificationCenter::currentNotificationCenter()
                    .removeDeliveredNotificationsWithIdentifiers(&NSArray::from_retained_slice(
                        &replaced,
                    ));
            }
        });
        center.getDeliveredNotificationsWithCompletionHandler(&delivered);
    }
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(&notification.title));
    content.setBody(&NSString::from_str(&notification.body));
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(identifier),
        &content,
        None,
    );
    let added = RcBlock::new(|error: *mut NSError| {
        // SAFETY: the centre passes a valid error or null.
        if let Some(error) = unsafe { error.as_ref() } {
            tracing::warn!(
                "the notification was not posted: {}",
                error.localizedDescription()
            );
        }
    });
    center.addNotificationRequest_withCompletionHandler(&request, Some(&added));
    true
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the delegate
    // implements no `Drop`.
    #[unsafe(super(NSObject))]
    #[name = "MoonframeNotificationDelegate"]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        /// Shows it even while the app is in front: the mod chose to post.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            handler
                .call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            handler: &block2::DynBlock<dyn Fn()>,
        ) {
            let identifier = response.notification().request().identifier().to_string();
            if let Some(clicked) = CLICKED.get() {
                clicked(&identifier);
            }
            handler.call(());
        }
    }
);

impl Delegate {
    #[allow(unsafe_code, reason = "NSObject's init; see SAFETY")]
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        // SAFETY: NSObject's `init`, on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}
