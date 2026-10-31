//! The app's assets: its own icons (`assets/icons`, embedded in the binary)
//! over gpui-kit's bundled Lucide set, so `ui.icon` names either and a
//! bundle ships both without copying files.

use {
    gpui_kit::{AssetSource, Result, SharedString},
    std::borrow::Cow,
};

/// The icons Moonframe adds; see `assets/icons/LICENSES.md`.
#[derive(rust_embed::RustEmbed)]
#[folder = "../../assets"]
#[include = "icons/*.svg"]
struct Ours;

/// Ours first, then gpui-kit's.
pub(crate) struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match Ours::get(path) {
            Some(file) => Ok(Some(file.data)),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut found: Vec<SharedString> = Ours::iter()
            .filter(|name| name.starts_with(path))
            .map(|name| SharedString::from(name.into_owned()))
            .collect();
        for name in gpui_kit::assets::Assets.list(path)? {
            if !found.contains(&name) {
                found.push(name);
            }
        }
        Ok(found)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use {super::*, moonframe_core::IconName};

    #[test]
    fn every_icon_name_has_an_svg() {
        let missing: Vec<&str> = moonframe_core::ICONS
            .iter()
            .copied()
            .filter(|name| {
                let path = IconName::new(name).unwrap().path();
                !matches!(Assets.load(&path), Ok(Some(_)))
            })
            .collect();
        assert!(missing.is_empty(), "no SVG for {missing:?}");
    }

    #[test]
    fn our_icons_are_listed_and_tinted_by_the_text_color() {
        let ours: Vec<String> = Ours::iter().map(|name| name.into_owned()).collect();
        assert!(ours.len() > 50, "{ours:?}");
        for path in &ours {
            let name = path
                .strip_prefix("icons/")
                .and_then(|name| name.strip_suffix(".svg"))
                .unwrap();
            assert!(IconName::new(name).is_some(), "{name} is not in ICONS");
            let svg = String::from_utf8(Ours::get(path).unwrap().data.into_owned()).unwrap();
            // A square box, and no color of its own: GPUI draws it as a mask
            // in the text color.
            assert!(svg.contains("viewBox=\"0 0 24 24\""), "{name}: not 24×24");
            assert!(!svg.contains("fill=\"#"), "{name}: a fixed fill");
            assert!(!svg.contains("stroke=\"#"), "{name}: a fixed stroke");
            assert!(svg.contains("currentColor"), "{name}: not currentColor");
        }
        let listed = Assets.list("icons/").unwrap();
        assert!(
            listed
                .iter()
                .any(|name| name.as_ref() == "icons/agent-claude.svg")
        );
        assert!(listed.iter().any(|name| name.as_ref() == "icons/cpu.svg"));
    }
}
