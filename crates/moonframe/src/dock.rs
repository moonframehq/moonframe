//! The app's icon in the Dock and the app switcher. A bundle takes it from
//! its Info.plist; run straight from Cargo there is none, so the app sets
//! it at launch. `scripts/draw-icon.swift` draws `assets/icon.png`.

#[cfg(target_os = "macos")]
const ICON: &[u8] = include_bytes!("../../../assets/icon.png");

/// Must run on the main thread once the application exists.
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "one AppKit setter; see SAFETY")]
pub(crate) fn set_icon() {
    use {
        objc2::{AllocAnyThread as _, MainThreadMarker},
        objc2_app_kit::{NSApplication, NSImage},
        objc2_foundation::NSData,
    };
    let Some(main) = MainThreadMarker::new() else {
        tracing::warn!("the Dock icon is set from the main thread only");
        return;
    };
    let data = NSData::with_bytes(ICON);
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        tracing::warn!("cannot read the app icon");
        return;
    };
    let app = NSApplication::sharedApplication(main);
    // SAFETY: on the main thread, as the marker proves, with a valid image
    // that AppKit retains.
    unsafe { app.setApplicationIconImage(Some(&image)) };
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn set_icon() {}
