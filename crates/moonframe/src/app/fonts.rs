//! Choose a fixed-width Nerd Font when installed, otherwise keep the
//! toolkit's installed, platform-specific monospace default.

use gpui_kit::{App, SharedString};

pub(super) fn default_mono(cx: &App, fallback: &SharedString) -> SharedString {
    let mut families = cx.text_system().all_font_names();
    choose_mono(&mut families, fallback)
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
