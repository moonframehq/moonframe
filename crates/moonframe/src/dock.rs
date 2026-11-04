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

/// Shows `icon` (an image file AppKit reads), else the app's fallback, as
/// the app's icon; why not, if it cannot. Must run on the main thread
/// once the application exists; at launch, and again whenever a mod asks
/// (`app:icon`).
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "one AppKit setter; see SAFETY")]
pub(crate) fn set_icon(icon: Option<&std::path::Path>) -> Result<(), String> {
    use {
        objc2::{AllocAnyThread as _, MainThreadMarker},
        objc2_app_kit::{NSApplication, NSImage},
        objc2_foundation::NSData,
    };
    let Some(main) = MainThreadMarker::new() else {
        return Err("the Dock icon is set from the main thread only".to_owned());
    };
    let own = match icon {
        Some(path) => Some(
            std::fs::read(path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?,
        ),
        None => None,
    };
    let data = NSData::with_bytes(own.as_deref().unwrap_or_else(|| fallback()));
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        return Err(match icon {
            Some(path) => format!("{} is not an image AppKit reads", path.display()),
            None => "cannot read the app icon".to_owned(),
        });
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

/// Elsewhere the window system takes the icon from the app's desktop
/// entry, so it stays as installed.
#[cfg(not(target_os = "macos"))]
pub(crate) fn set_icon(_: Option<&std::path::Path>) -> Result<(), String> {
    Ok(())
}
