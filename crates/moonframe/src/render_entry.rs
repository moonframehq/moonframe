//! A headless render of one miniapp entry to PNG: the entry mounted alone,
//! full-size, in a window no one sees, over Moonframe's components and
//! stylesheet (or the host app's), drawn by the GPU and read back. A host
//! shows it to an agent or a vision model without the user's window.

mod window;

use {
    moonframe_core::{
        Data, Roots,
        entries::{CheckOptions, Entries, RootPolicy, ViewSnapshot, module_name},
    },
    std::path::{Path, PathBuf},
};

/// What [`render_entry`] drew.
#[derive(Clone, Debug)]
pub struct RenderedEntry {
    /// The window as a PNG.
    pub png: Vec<u8>,
    /// Its size in device pixels: the window's size times its scale.
    pub width: u32,
    pub height: u32,
    /// The entry's view as data, as it was drawn.
    pub snapshot: Option<ViewSnapshot>,
    /// Where each of the entry's elements was laid out, parents before
    /// their children.
    pub bounds: Vec<ElementBounds>,
}

/// One element's laid-out box, in window pixels (not device pixels), the
/// window's top left at 0, 0.
#[derive(Clone, Debug, PartialEq)]
pub struct ElementBounds {
    /// Its element path, as the view snapshot's `path` names it.
    pub path: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Draws the entry at `path` alone in a headless window `size` pixels
/// big, with `options`' name, app layers, first `app` table (else the
/// entry's `sample.json` or `provider/sample.json`, under its name), first
/// state and messages; `size` stands for its sizes. Fails where the platform
/// has no headless renderer.
pub fn render_entry(
    path: &Path,
    policy: &RootPolicy,
    options: &CheckOptions,
    size: (f32, f32),
) -> Result<RenderedEntry, String> {
    let path = std::path::absolute(path).map_err(|error| error.to_string())?;
    if !path.join("main.luau").is_file() {
        return Err(format!("{}: no main.luau to draw", path.display()));
    }
    if !(size.0 > 0. && size.1 > 0.) {
        return Err(format!("cannot draw at {} by {}", size.0, size.1));
    }
    let name = options.name.as_deref().unwrap_or("check:entry");
    let Some((namespace, id)) = name.split_once(':').filter(|_| module_name(name)) else {
        return Err(format!("`{name}` is no entry name: use <namespace>:<id>"));
    };
    let shell = shell(name)?;
    let entries = Entries::default();
    let root = path.parent().unwrap_or(&path);
    entries.add_root(namespace, root, policy.clone())?;
    entries.add_entry(namespace, id, &path)?;
    let roots = entries.apply(layers(options.roots.as_ref(), shell.path().to_owned()));
    let app = match options.app.first() {
        Some(Data::Map(app)) => app.clone().into_iter().collect(),
        Some(_) => return Err("an `app` case is a table of keys".to_owned()),
        None => sample(&path)?
            .map(|sample| vec![(name.to_owned(), sample)])
            .unwrap_or_default(),
    };
    window::draw(window::Job {
        name,
        roots,
        entries,
        folder: shell.path(),
        app,
        messages: &options.messages,
        state: options.state.first(),
        size,
    })
}

/// A folder whose `main.luau` mounts the entry `name`, filling the window.
fn shell(name: &str) -> Result<tempfile::TempDir, String> {
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let main = format!(
        "return mod {{ view = function() return ui.column {{ grow = true, ui.mount {name:?} }} end }}\n"
    );
    std::fs::write(dir.path().join("main.luau"), main).map_err(|error| error.to_string())?;
    Ok(dir)
}

/// The app's layers with the shell over them, or Moonframe's toolbox
/// under the shell. The host's layers all count as app layers here: the
/// shell, highest, is the one that loads, and mounts the entry alone.
fn layers(host: Option<&Roots>, shell: PathBuf) -> Roots {
    match host {
        Some(host) => Roots::new(host.layers().map(Path::to_path_buf).chain([shell])),
        None => Roots::hosted(
            crate::embed::toolbox_folder(),
            moonframe_core::AppLayer {
                path: shell,
                keep: Vec::new(),
            },
            Vec::new(),
            None,
        ),
    }
}

/// The entry's sample of what its provider publishes, if it has one.
fn sample(entry: &Path) -> Result<Option<Data>, String> {
    let Some(file) = ["sample.json", "provider/sample.json"]
        .into_iter()
        .map(|name| entry.join(name))
        .find(|file| file.is_file())
    else {
        return Ok(None);
    };
    let read = |error: &dyn std::fmt::Display| format!("{}: {error}", file.display());
    let text = std::fs::read_to_string(&file).map_err(|error| read(&error))?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|error| read(&error))?;
    moonframe_providers::from_json(&json)
        .map(Some)
        .map_err(|error| read(&error))
}

#[cfg(all(test, target_os = "macos"))]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn render_entry_draws_a_tiny_entry_to_png() {
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("hello");
        std::fs::create_dir(&entry).unwrap();
        std::fs::write(
            entry.join("main.luau"),
            r#"return mod { view = function() return ui.column { ui.text "hello" } end }"#,
        )
        .unwrap();
        let options = CheckOptions {
            name: Some("user:hello".to_owned()),
            ..CheckOptions::default()
        };
        let rendered =
            render_entry(&entry, &RootPolicy::default(), &options, (320., 200.)).unwrap();
        assert!(rendered.png.starts_with(b"\x89PNG"));
        assert!(rendered.width > 0 && rendered.height > 0);
        let snapshot = rendered.snapshot.expect("a snapshot of the entry's view");
        let Data::Map(tree) = &snapshot.tree else {
            panic!("the tree is a table: {:?}", snapshot.tree);
        };
        assert_eq!(tree.get("kind"), Some(&Data::from("column")));
        assert!(
            rendered
                .bounds
                .iter()
                .any(|placed| placed.width > 0. && placed.height > 0.),
            "{:?}",
            rendered.bounds
        );
    }

    #[test]
    fn render_entry_refuses_what_it_cannot_draw() {
        let dir = tempfile::tempdir().unwrap();
        let options = CheckOptions::default();
        let policy = RootPolicy::default();
        assert!(render_entry(dir.path(), &policy, &options, (320., 200.)).is_err());
        std::fs::write(dir.path().join("main.luau"), "return mod {}").unwrap();
        let named = CheckOptions {
            name: Some("no name".to_owned()),
            ..CheckOptions::default()
        };
        assert!(render_entry(dir.path(), &policy, &named, (320., 200.)).is_err());
        assert!(render_entry(dir.path(), &policy, &options, (0., 200.)).is_err());
    }
}
