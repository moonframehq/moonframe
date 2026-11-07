//! The app's assets: its own icons (`assets/icons`, embedded in the binary)
//! over gpui-kit's bundled Lucide set, so `ui.icon` names either and a
//! bundle ships both without copying files. A host app's assets
//! ([`crate::Moonframe::assets`]) go over both, so its views find theirs.

use {
    gpui_kit::{AssetSource, Result, SharedString},
    std::{borrow::Cow, sync::Arc},
};

/// The icons Moonframe adds; see `assets/icons/LICENSES.md`.
#[derive(rust_embed::RustEmbed)]
#[folder = "../../assets"]
#[include = "icons/*.svg"]
struct Ours;

/// The host's first, then ours, then gpui-kit's.
#[derive(Default)]
pub(crate) struct Assets {
    host: Option<Arc<dyn AssetSource>>,
}

impl Assets {
    pub(crate) fn new(host: Option<Arc<dyn AssetSource>>) -> Self {
        Self { host }
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        // A path the host lacks is a miss however it says so: some sources
        // answer `None`, others an error, as gpui-kit's does.
        if let Some(Ok(Some(data))) = self.host.as_ref().map(|host| host.load(path)) {
            return Ok(Some(data));
        }
        match Ours::get(path) {
            Some(file) => Ok(Some(file.data)),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut found = self
            .host
            .as_ref()
            .and_then(|host| host.list(path).ok())
            .unwrap_or_default();
        for name in Ours::iter().filter(|name| name.starts_with(path)) {
            let name = SharedString::from(name.into_owned());
            if !found.contains(&name) {
                found.push(name);
            }
        }
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

    /// A host with one icon of its own and one of Moonframe's names, which
    /// answers an error for any other path, as gpui-kit's source does.
    struct Host;

    impl AssetSource for Host {
        fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
            match path {
                "icons/host-only.svg" | "icons/cpu.svg" => {
                    Ok(Some(Cow::Borrowed(b"host".as_slice())))
                },
                _ => Err(std::io::Error::other(path.to_owned()).into()),
            }
        }

        fn list(&self, path: &str) -> Result<Vec<SharedString>> {
            Ok(["icons/host-only.svg", "icons/cpu.svg"]
                .into_iter()
                .filter(|name| name.starts_with(path))
                .map(SharedString::from)
                .collect())
        }
    }

    fn with_host() -> Assets {
        Assets::new(Some(Arc::new(Host)))
    }

    #[test]
    fn a_host_asset_wins_and_the_rest_fall_through() {
        let assets = with_host();
        let host = Some(Cow::Borrowed(b"host".as_slice()));
        assert_eq!(assets.load("icons/host-only.svg").unwrap(), host);
        assert_eq!(assets.load("icons/cpu.svg").unwrap(), host);
        let ours = assets.load("icons/agent-claude.svg").unwrap().unwrap();
        assert_ne!(ours.as_ref(), b"host");
        assert!(assets.load("icons/no-such-icon.svg").is_err());
    }

    #[test]
    fn a_host_asset_is_listed_once_beside_the_others() {
        let listed = with_host().list("icons/").unwrap();
        let count = |path: &str| listed.iter().filter(|name| name.as_ref() == path).count();
        assert_eq!(count("icons/host-only.svg"), 1);
        assert_eq!(count("icons/cpu.svg"), 1);
        assert_eq!(count("icons/agent-claude.svg"), 1);
    }

    #[test]
    fn every_icon_name_has_an_svg() {
        let missing: Vec<&str> = moonframe_core::ICONS
            .iter()
            .copied()
            .filter(|name| {
                let path = IconName::new(name).unwrap().path();
                !matches!(Assets::default().load(&path), Ok(Some(_)))
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
        let listed = Assets::default().list("icons/").unwrap();
        assert!(
            listed
                .iter()
                .any(|name| name.as_ref() == "icons/agent-claude.svg")
        );
        assert!(listed.iter().any(|name| name.as_ref() == "icons/cpu.svg"));
    }
}
