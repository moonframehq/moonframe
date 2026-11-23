//! A paste of files or a picture: what a drop hands over, from the
//! clipboard. ⌘V (Ctrl-V elsewhere) in a field with `paste`, or anywhere
//! while no field or terminal has focus for a mod with `paste`, sends that
//! message with `files` (paths) added; a picture is saved to the image
//! cache first and also comes as `image = { path, width, height }`. Text
//! pastes as it always does. What is pasted, the mod may show (`ui.image`)
//! and share, as a dropped file.

use {
    crate::{app::MoonframeApp, paths},
    gpui_kit::*,
    moonframe_core::{Action, Data, with_fields},
    std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
    },
};

/// Where pasted pictures go, in the image cache.
const FOLDER: &str = "pasted";
/// The largest picture a paste saves.
const MAX_BYTES: usize = 64 * 1024 * 1024;

/// What the clipboard holds besides text.
#[derive(Debug, PartialEq)]
pub(crate) enum Pasted {
    Files(Vec<PathBuf>),
    /// A picture, as its format's extension and its bytes.
    Image(&'static str, Vec<u8>),
}

/// What `item` holds that a paste of text would not take: copied files
/// (Finder puts their names beside them as text), else a picture with no
/// text beside it (a browser's copied picture often carries its address,
/// which pastes as the text).
pub(crate) fn pasted(item: &ClipboardItem) -> Option<Pasted> {
    let entries = item.entries();
    if let Some(paths) = entries.iter().find_map(|entry| match entry {
        ClipboardEntry::ExternalPaths(paths) if !paths.paths().is_empty() => {
            Some(paths.paths().to_vec())
        },
        _ => None,
    }) {
        return Some(Pasted::Files(paths));
    }
    if entries
        .iter()
        .any(|entry| matches!(entry, ClipboardEntry::String(_)))
    {
        return None;
    }
    entries.iter().find_map(|entry| match entry {
        ClipboardEntry::Image(image) if image.bytes.len() <= MAX_BYTES => {
            Some(Pasted::Image(extension(image.format), image.bytes.clone()))
        },
        _ => None,
    })
}

fn extension(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Webp => "webp",
        ImageFormat::Gif => "gif",
        ImageFormat::Svg => "svg",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Ico => "ico",
        _ => "png",
    }
}

/// The fields a paste adds to a message, the picture saved in `folder`
/// first, and the files they name. `None` when the picture can't be saved.
pub(crate) fn payload(
    pasted: Pasted,
    folder: &Path,
) -> Option<(BTreeMap<String, Data>, Vec<PathBuf>)> {
    let path_of = |path: &PathBuf| Data::String(path.to_string_lossy().into_owned());
    let mut fields = BTreeMap::new();
    let files = match pasted {
        Pasted::Files(files) => files,
        Pasted::Image(extension, bytes) => {
            let path = folder.join(format!("{:016x}.{extension}", fnv(&bytes)));
            std::fs::create_dir_all(folder).ok()?;
            if !path.exists() {
                std::fs::write(&path, &bytes).ok()?;
            }
            let mut image = BTreeMap::from([("path".to_owned(), path_of(&path))]);
            if let Ok(size) = imagesize::blob_size(&bytes) {
                image.insert("width".to_owned(), Data::Integer(size.width as i64));
                image.insert("height".to_owned(), Data::Integer(size.height as i64));
            }
            fields.insert("image".to_owned(), Data::Map(image));
            vec![path]
        },
    };
    fields.insert(
        "files".to_owned(),
        Data::List(files.iter().map(path_of).collect()),
    );
    Some((fields, files))
}

/// A stable name for a picture's bytes, so pasting it twice keeps one file.
fn fnv(bytes: &[u8]) -> u64 {
    // FNV-1a: no dependency, and stable across runs, unlike std's hasher.
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// ⌘V on macOS, Ctrl-V elsewhere, with nothing else held.
fn is_paste(keystroke: &Keystroke) -> bool {
    let held = keystroke.modifiers;
    let secondary = if cfg!(target_os = "macos") {
        held.platform && !held.control
    } else {
        held.control && !held.platform
    };
    keystroke.key == "v" && secondary && !held.alt && !held.shift && !held.function
}

impl MoonframeApp {
    /// Hands a paste of files or a picture to the field with focus that
    /// takes `paste`, or, while no field or terminal has focus, to every
    /// mod with `paste`. Whether one took it; text goes on to paste.
    pub(crate) fn paste_keystroke(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !is_paste(keystroke) {
            return false;
        }
        let hearers: Vec<(String, Option<String>, Data)> = match self.inputs.focused(window, cx) {
            Some((wiring, _)) => match (&wiring.paste, wiring.disabled) {
                (Some(msg), false) => vec![(wiring.owner.clone(), wiring.to.clone(), msg.clone())],
                _ => return false,
            },
            None => {
                #[cfg(feature = "terminal")]
                if self.terminals.any_focused(window) {
                    return false;
                }
                self.workspace
                    .mod_pastes()
                    .map(|(owner, msg)| (owner.to_owned(), None, msg.clone()))
                    .collect()
            },
        };
        if hearers.is_empty() {
            return false;
        }
        let Some(pasted) = cx.read_from_clipboard().as_ref().and_then(pasted) else {
            return false;
        };
        let Some((fields, files)) = payload(pasted, &paths::images().join(FOLDER)) else {
            return true;
        };
        for (owner, to, msg) in hearers {
            crate::render::images::grant(&owner, &files);
            let msg = with_fields(&msg, fields.clone());
            self.dispatch(&owner, &Action::Send { to, msg }, window, cx);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{Pasted, pasted, payload},
        gpui_kit::{ClipboardEntry, ClipboardItem, ExternalPaths, Image, ImageFormat},
        moonframe_core::Data,
        std::path::PathBuf,
    };

    #[test]
    fn copied_files_paste_as_files_and_text_as_text() {
        let files = ClipboardItem {
            entries: vec![
                ClipboardEntry::ExternalPaths(ExternalPaths(
                    vec![PathBuf::from("/tmp/a.pdf")].into(),
                )),
                ClipboardEntry::String("/tmp/a.pdf".to_owned().into()),
            ],
        };
        assert_eq!(
            pasted(&files),
            Some(Pasted::Files(vec![PathBuf::from("/tmp/a.pdf")]))
        );
        assert_eq!(pasted(&ClipboardItem::new_string("hello".into())), None);
    }

    #[test]
    fn a_picture_is_saved_once_with_its_size() {
        // A 1×1 PNG.
        let png: Vec<u8> = vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52,
            0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89,
        ];
        let item = ClipboardItem {
            entries: vec![ClipboardEntry::Image(Image::from_bytes(
                ImageFormat::Png,
                png.clone(),
            ))],
        };
        let Some(picture) = pasted(&item) else {
            panic!("a picture")
        };
        let folder = tempfile::tempdir().unwrap();
        let (fields, files) = payload(picture, folder.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(std::fs::read(&files[0]).unwrap(), png);
        let Some(Data::Map(image)) = fields.get("image") else {
            panic!("{fields:?}")
        };
        assert_eq!(image.get("width"), Some(&Data::Integer(1)));
        assert!(matches!(fields.get("files"), Some(Data::List(paths)) if paths.len() == 1));
        // Pasted again: the same file.
        let again = pasted(&item).unwrap();
        assert_eq!(payload(again, folder.path()).unwrap().1, files);
    }
}
