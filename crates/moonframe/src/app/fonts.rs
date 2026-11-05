//! Choose a fixed-width Nerd Font when installed, otherwise keep the
//! toolkit's installed, platform-specific monospace default.

use {
    gpui_kit::{App, SharedString, TextSystem},
    std::{
        sync::Mutex,
        thread::{self, JoinHandle},
    },
};

/// The font list under way, for the first window to take.
static LISTING: Mutex<Option<JoinHandle<Vec<String>>>> = Mutex::new(None);

/// Starts listing the installed font families on the side, for the first
/// window, while the toolkit starts up.
pub(crate) fn list_ahead(cx: &App) {
    let text = cx.text_system().clone();
    let listing = thread::Builder::new()
        .name("list-fonts".to_owned())
        .spawn(move || families(&text));
    match listing {
        Ok(listing) => {
            *LISTING
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(listing);
        },
        Err(error) => tracing::debug!("no thread to list fonts on: {error}"),
    }
}

pub(super) fn default_mono(cx: &App, fallback: &SharedString) -> SharedString {
    let listing = LISTING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    let mut names = listing
        .and_then(|listing| listing.join().ok())
        .unwrap_or_else(|| families(cx.text_system()));
    choose_mono(&mut names, fallback)
}

/// The installed font families. GPUI's list weeds out duplicate faces, which
/// on a Mac with many fonts takes the better part of a second, under a lock
/// its toolkit is holding for its own list at launch; the family names alone
/// take a tenth of that.
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "one CoreText call; see SAFETY")]
fn families(text: &TextSystem) -> Vec<String> {
    use {
        objc2::rc::Retained,
        objc2_foundation::{NSArray, NSString},
    };
    #[link(name = "CoreText", kind = "framework")]
    unsafe extern "C" {
        fn CTFontManagerCopyAvailableFontFamilyNames() -> *mut NSArray<NSString>;
    }
    // SAFETY: the function takes nothing and may be called from any thread.
    // It returns a CFArray of CFStrings, toll-free bridged to these types,
    // which by the Create Rule is ours to release, as `Retained` does.
    let names = unsafe { Retained::from_raw(CTFontManagerCopyAvailableFontFamilyNames()) };
    match names {
        Some(names) => names.iter().map(|name| name.to_string()).collect(),
        None => text.all_font_names(),
    }
}

#[cfg(not(target_os = "macos"))]
fn families(text: &TextSystem) -> Vec<String> {
    text.all_font_names()
}

fn choose_mono(families: &mut [String], fallback: &SharedString) -> SharedString {
    // Font enumeration order varies by platform. Prefer JetBrains Mono,
    // then another patched, fixed-width family (not the symbols-only font).
    families.sort();
    let family = families
        .iter()
        .find(|name| name.as_str() == "JetBrainsMono Nerd Font Mono")
        .or_else(|| {
            families
                .iter()
                .find(|name| name.ends_with(" Nerd Font Mono") && !name.starts_with("Symbols "))
        });
    family.map_or_else(|| fallback.clone(), |name| name.clone().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choose(installed: &[&str], fallback: &'static str) -> SharedString {
        let mut families: Vec<String> = installed.iter().map(|name| (*name).to_owned()).collect();
        choose_mono(&mut families, &fallback.into())
    }

    #[test]
    fn keeps_platform_monospace_without_a_patched_text_font() {
        // Symbols are a glyph fallback, not a primary text face. In
        // particular, a stock Mac must continue to use Menlo.
        for family in ["Menlo", "DejaVu Sans Mono", "Consolas"] {
            assert_eq!(
                choose(&[family, "Symbols Nerd Font Mono", "Noto Sans"], family).as_ref(),
                family
            );
        }
    }

    #[test]
    fn prefers_jetbrains_nerd_mono_on_every_platform() {
        let installed = [
            "FiraCode Nerd Font Mono",
            "Menlo",
            "JetBrainsMono Nerd Font Propo",
            "JetBrainsMono Nerd Font Mono",
        ];
        for fallback in ["Menlo", "DejaVu Sans Mono", "Consolas"] {
            assert_eq!(
                choose(&installed, fallback).as_ref(),
                "JetBrainsMono Nerd Font Mono"
            );
        }
    }

    #[test]
    fn another_nerd_mono_is_stable_across_font_enumeration_orders() {
        let mut installed = [
            "Symbols Nerd Font Mono",
            "JetBrainsMono Nerd Font Propo",
            "Hack Nerd Font Mono",
            "FiraCode Nerd Font Mono",
        ];
        assert_eq!(
            choose(&installed, "Menlo").as_ref(),
            "FiraCode Nerd Font Mono"
        );
        installed.reverse();
        assert_eq!(
            choose(&installed, "Menlo").as_ref(),
            "FiraCode Nerd Font Mono"
        );
    }
}
