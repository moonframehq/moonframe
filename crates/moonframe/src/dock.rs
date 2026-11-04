//! The app's icon in the Dock and the app switcher. A bundle takes it from
//! its Info.plist; run straight from Cargo there is none, so the app sets
//! it at launch: a profile's app's `icon.png`, else Moonframe's, which
//! `scripts/draw-icon.swift` draws in `assets/icon.png`.

/// Moonframe's own icon, for an app that names none.
#[cfg(target_os = "macos")]
const ICON: &[u8] = include_bytes!("../../../assets/icon.png");

/// The icon an app gets when its folder has none: the one it was launched
/// with, else Moonframe's.
#[cfg(target_os = "macos")]
fn fallback() -> &'static [u8] {
    crate::identity().icon.unwrap_or(ICON)
}

/// The icon with a bar over it, kept while the app runs.
#[cfg(target_os = "macos")]
struct ProgressTile {
    view: objc2::rc::Retained<objc2_app_kit::NSImageView>,
    bar: objc2::rc::Retained<objc2_app_kit::NSProgressIndicator>,
}

#[cfg(target_os = "macos")]
thread_local! {
    static PROGRESS: std::cell::RefCell<Option<ProgressTile>> = const { std::cell::RefCell::new(None) };
}

/// Does what a mod asked with `app:dock`. Runs on the main thread, where
/// requests are carried out.
#[cfg(target_os = "macos")]
pub(crate) fn show(dock: &moonframe_core::Dock) {
    use {
        objc2::{MainThreadMarker, MainThreadOnly as _},
        objc2_app_kit::{
            NSApplication, NSImageView, NSProgressIndicator, NSProgressIndicatorStyle,
        },
        objc2_foundation::{NSPoint, NSRect, NSSize, NSString},
    };
    let Some(main) = MainThreadMarker::new() else {
        tracing::warn!("the Dock tile is set from the main thread only");
        return;
    };
    let app = NSApplication::sharedApplication(main);
    let tile = app.dockTile();
    if let Some(badge) = &dock.badge {
        let badge = badge.as_deref().map(NSString::from_str);
        tile.setBadgeLabel(badge.as_deref());
    }
    let Some(progress) = dock.progress else {
        return;
    };
    match progress {
        None => {
            tile.setContentView(None);
        },
        Some(fraction) => PROGRESS.with_borrow_mut(|progress| {
            let shown = progress.get_or_insert_with(|| {
                let size = tile.size();
                let image = app.applicationIconImage();
                let view = match image {
                    Some(image) => NSImageView::imageViewWithImage(&image, main),
                    None => NSImageView::new(main),
                };
                view.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), size));
                let bar = NSProgressIndicator::initWithFrame(
                    NSProgressIndicator::alloc(main),
                    NSRect::new(
                        NSPoint::new(size.width * 0.1, size.height * 0.06),
                        NSSize::new(size.width * 0.8, size.height * 0.12),
                    ),
                );
                bar.setStyle(NSProgressIndicatorStyle::Bar);
                bar.setIndeterminate(false);
                bar.setMinValue(0.0);
                bar.setMaxValue(1.0);
                view.addSubview(&bar);
                ProgressTile { view, bar }
            });
            shown.bar.setDoubleValue(fraction);
            tile.setContentView(Some(&shown.view));
        }),
    }
    tile.display();
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn show(_: &moonframe_core::Dock) {}

/// Must run on the main thread once the application exists.
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "one AppKit setter; see SAFETY")]
pub(crate) fn set_icon(icon: Option<&std::path::Path>) {
    use {
        objc2::{AllocAnyThread as _, MainThreadMarker},
        objc2_app_kit::{NSApplication, NSImage},
        objc2_foundation::NSData,
    };
    let Some(main) = MainThreadMarker::new() else {
        tracing::warn!("the Dock icon is set from the main thread only");
        return;
    };
    let own = icon.and_then(|path| {
        std::fs::read(path)
            .inspect_err(|error| tracing::warn!("cannot read {}: {error}", path.display()))
            .ok()
    });
    let data = NSData::with_bytes(own.as_deref().unwrap_or_else(|| fallback()));
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        tracing::warn!("cannot read the app icon");
        return;
    };
    let app = NSApplication::sharedApplication(main);
    // SAFETY: on the main thread, as the marker proves, with a valid image
    // that AppKit retains.
    unsafe { app.setApplicationIconImage(Some(&image)) };
}

/// Names the process `name`, before the app starts: macOS titles the app
/// menu, and lists the app, by the process's name, which outside a bundle
/// is the binary's (`moonframe`). A bundle's Info.plist names it anyway.
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "one Foundation message; see SAFETY")]
pub(crate) fn set_name(name: &str) {
    use {
        objc2::{msg_send, runtime::AnyObject},
        objc2_foundation::{NSBundle, NSProcessInfo, NSString},
    };
    let name = NSString::from_str(name);
    NSProcessInfo::processInfo().setProcessName(&name);
    // The menu bar takes the app's name from the main bundle's
    // CFBundleName, which an unbundled binary lacks.
    let Some(info) = NSBundle::mainBundle().infoDictionary() else {
        return;
    };
    let key = NSString::from_str("CFBundleName");
    let info: &AnyObject = &info;
    // SAFETY: the main bundle's info dictionary is mutable (AppKit and
    // Foundation keep it as an NSMutableDictionary); checked with
    // respondsToSelector before setting a string under a string key.
    unsafe {
        let mutable: bool = msg_send![info, respondsToSelector: objc2::sel!(setObject:forKey:)];
        if mutable {
            let _: () = msg_send![info, setObject: &*name, forKey: &*key];
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn set_name(_: &str) {}

/// Opens macOS's About panel for the app `name`, with the shell's `about`
/// and its `icon`; false where there is none, for the caller to say it
/// another way.
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "one AppKit call; see SAFETY")]
pub(crate) fn about_panel(
    name: &str,
    about: &moonframe_core::About,
    icon: Option<&std::path::Path>,
) -> bool {
    use {
        objc2::{AllocAnyThread as _, MainThreadMarker, rc::Retained, runtime::AnyObject},
        objc2_app_kit::{NSApplication, NSImage},
        objc2_foundation::{NSAttributedString, NSData, NSDictionary, NSString},
    };
    let Some(main) = MainThreadMarker::new() else {
        return false;
    };
    let credits: Vec<&str> = [&about.description, &about.website]
        .into_iter()
        .filter_map(|text| text.as_deref())
        .collect();
    let mut options: Vec<(&str, Retained<AnyObject>)> = vec![(
        "ApplicationName",
        Retained::into_super(Retained::into_super(NSString::from_str(name))),
    )];
    if let Some(version) = &about.version {
        options.push((
            "ApplicationVersion",
            Retained::into_super(Retained::into_super(NSString::from_str(version))),
        ));
    }
    // In parentheses after the version: what the app runs on.
    let built_on = format!("Moonframe {}", env!("CARGO_PKG_VERSION"));
    options.push((
        "Version",
        Retained::into_super(Retained::into_super(NSString::from_str(&built_on))),
    ));
    if !credits.is_empty() {
        let text = NSString::from_str(&credits.join("\n\n"));
        let credits = NSAttributedString::from_nsstring(&text);
        options.push((
            "Credits",
            Retained::into_super(Retained::into_super(credits)),
        ));
    }
    if let Some(copyright) = &about.copyright {
        options.push((
            "Copyright",
            Retained::into_super(Retained::into_super(NSString::from_str(copyright))),
        ));
    }
    let image = icon
        .and_then(|path| std::fs::read(path).ok())
        .map(|bytes| NSData::with_bytes(&bytes))
        .or_else(|| Some(NSData::with_bytes(fallback())))
        .and_then(|data| NSImage::initWithData(NSImage::alloc(), &data));
    if let Some(image) = image {
        options.push((
            "ApplicationIcon",
            Retained::into_super(Retained::into_super(image)),
        ));
    }
    let keys: Vec<Retained<NSString>> = options
        .iter()
        .map(|(key, _)| NSString::from_str(key))
        .collect();
    let keys: Vec<&NSString> = keys.iter().map(|key| &**key).collect();
    let values: Vec<Retained<AnyObject>> = options.into_iter().map(|(_, value)| value).collect();
    let dictionary = NSDictionary::from_retained_objects(&keys, &values);
    let app = NSApplication::sharedApplication(main);
    // SAFETY: on the main thread, as the marker proves, with a dictionary
    // whose keys are the panel's documented option names and whose values
    // are of the types AppKit documents for them.
    unsafe { app.orderFrontStandardAboutPanelWithOptions(&dictionary) };
    true
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn about_panel(_: &str, _: &moonframe_core::About, _: Option<&std::path::Path>) -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn set_icon(_: Option<&std::path::Path>) {}
