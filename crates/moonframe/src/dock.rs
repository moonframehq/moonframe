//! The app's icon in the Dock and the app switcher. A bundle takes it from
//! its Info.plist; run straight from Cargo there is none, so the app sets
//! it at launch: a profile's app's `icon.png`, else Moonframe's, which
//! `scripts/draw-icon.swift` draws in `assets/icon.png`. A mod can change
//! it while the app runs (`app:icon`), bundled or not.

/// Moonframe's own icon, for an app that names none.
#[cfg(target_os = "macos")]
const ICON: &[u8] = include_bytes!("../../../assets/icon.png");

/// The icon an app gets when its folder has none: the one it was launched
/// with, else Moonframe's.
#[cfg(target_os = "macos")]
fn fallback() -> &'static [u8] {
    crate::identity().icon.unwrap_or(ICON)
}

/// `icon` with a bar along its foot, `fraction` of it filled. Drawn into the
/// picture, as the tile is a snapshot: a progress control placed in it paints
/// no more than a speck.
#[cfg(target_os = "macos")]
fn with_bar(
    icon: objc2::rc::Retained<objc2_app_kit::NSImage>,
    size: objc2_foundation::NSSize,
    fraction: f64,
) -> objc2::rc::Retained<objc2_app_kit::NSImage> {
    use {
        objc2::runtime::Bool,
        objc2_app_kit::{NSBezierPath, NSColor, NSImage},
        objc2_foundation::{NSPoint, NSRect, NSSize},
    };
    let draw = block2::RcBlock::new(move |area: NSRect| {
        icon.drawInRect(area);
        let (width, height) = (area.size.width, area.size.height);
        let pill = |left: f64, bottom: f64, long: f64, tall: f64, color: &NSColor| {
            let rect = NSRect::new(NSPoint::new(left, bottom), NSSize::new(long, tall));
            color.setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, tall / 2.0, tall / 2.0)
                .fill();
        };
        let (left, bottom) = (width * 0.14, height * 0.13);
        let (long, tall) = (width * 0.72, height * 0.1);
        let inset = tall * 0.2;
        let inner = tall - inset * 2.0;
        let track = NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, 0.7);
        let filled = NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 1.0);
        pill(left, bottom, long, tall, &track);
        pill(
            left + inset,
            bottom + inset,
            ((long - inset * 2.0) * fraction.clamp(0.0, 1.0)).max(inner),
            inner,
            &filled,
        );
        Bool::YES
    });
    NSImage::imageWithSize_flipped_drawingHandler(size, false, &draw)
}

/// Does what a mod asked with `app:dock`. Runs on the main thread, where
/// requests are carried out.
#[cfg(target_os = "macos")]
pub(crate) fn show(dock: &moonframe_core::Dock) {
    use {
        objc2::MainThreadMarker,
        objc2_app_kit::{NSApplication, NSImageView},
        objc2_foundation::{NSPoint, NSRect, NSString},
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
    match (progress, app.applicationIconImage()) {
        (Some(fraction), Some(icon)) => {
            let size = tile.size();
            let view = NSImageView::imageViewWithImage(&with_bar(icon, size, fraction), main);
            view.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), size));
            tile.setContentView(Some(&view));
        },
        _ => tile.setContentView(None),
    }
    tile.display();
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn show(_: &moonframe_core::Dock) {}

/// Shows `icon` (an image file AppKit reads), else the app's fallback, as
/// the app's icon; why not, if it cannot.
#[cfg(target_os = "macos")]
pub(crate) fn set_icon(icon: Option<&std::path::Path>) -> Result<(), String> {
    match icon {
        Some(path) => show_icon(
            &std::fs::read(path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?,
        ),
        None => show_icon(fallback()),
    }
}

/// Shows the image in `bytes` as the app's icon. Must run on the main
/// thread once the application exists: at launch, and whenever a mod asks
/// (`app:icon`).
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "one AppKit setter; see SAFETY")]
pub(crate) fn show_icon(bytes: &[u8]) -> Result<(), String> {
    use {
        objc2::{AllocAnyThread as _, MainThreadMarker},
        objc2_app_kit::{NSApplication, NSImage},
        objc2_foundation::NSData,
    };
    let Some(main) = MainThreadMarker::new() else {
        return Err("the Dock icon is set from the main thread only".to_owned());
    };
    let data = NSData::with_bytes(bytes);
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        return Err("not an image AppKit reads".to_owned());
    };
    let app = NSApplication::sharedApplication(main);
    // SAFETY: on the main thread, as the marker proves, with a valid image
    // that AppKit retains.
    unsafe { app.setApplicationIconImage(Some(&image)) };
    Ok(())
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
    renderer: &str,
) -> bool {
    use {
        objc2::{AllocAnyThread as _, MainThreadMarker, rc::Retained, runtime::AnyObject},
        objc2_app_kit::{NSApplication, NSImage},
        objc2_foundation::{NSAttributedString, NSData, NSDictionary, NSString},
    };
    let Some(main) = MainThreadMarker::new() else {
        return false;
    };
    let mut options: Vec<(&str, Retained<AnyObject>)> = vec![(
        "ApplicationName",
        Retained::into_super(Retained::into_super(NSString::from_str(name))),
    )];
    // A binary outside a bundle has no Info.plist for macOS to read it from.
    let version = shown_version(about);
    options.push((
        "ApplicationVersion",
        Retained::into_super(Retained::into_super(NSString::from_str(&version))),
    ));
    // In parentheses after the version: what the app runs on.
    let built_on = built_on();
    options.push((
        "Version",
        Retained::into_super(Retained::into_super(NSString::from_str(&built_on))),
    ));
    let text = NSString::from_str(&credits(about, renderer));
    options.push((
        "Credits",
        Retained::into_super(Retained::into_super(NSAttributedString::from_nsstring(
            &text,
        ))),
    ));
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
pub(crate) fn about_panel(
    _: &str,
    _: &moonframe_core::About,
    _: Option<&std::path::Path>,
    _: &str,
) -> bool {
    false
}

/// The version an About panel shows: the shell's, else the app's own.
pub(crate) fn shown_version(about: &moonframe_core::About) -> String {
    about
        .version
        .clone()
        .unwrap_or_else(|| crate::identity().version.clone())
}

/// What the app runs on: `Moonframe 0.2.0`.
pub(crate) fn built_on() -> String {
    format!("Moonframe {}", env!("CARGO_PKG_VERSION"))
}

/// The text under the name in an About panel: what the shell says of the
/// app, and what draws this build.
pub(crate) fn credits(about: &moonframe_core::About, renderer: &str) -> String {
    let backend = backend_line(renderer);
    [about.description.as_deref(), about.website.as_deref()]
        .into_iter()
        .flatten()
        .chain([backend.as_str()])
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// What draws the app, as its About says: a build is one or the other.
pub(crate) fn backend_line(renderer: &str) -> String {
    format!("Backend: {renderer}")
}

/// Elsewhere the window system takes the icon from the app's desktop
/// entry, so it stays as installed.
#[cfg(not(target_os = "macos"))]
pub(crate) fn set_icon(_: Option<&std::path::Path>) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn show_icon(_: &[u8]) -> Result<(), String> {
    Ok(())
}
