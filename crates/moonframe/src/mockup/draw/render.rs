//! Each variant drawn headless to a PNG, in light and dark, cropped to what
//! it drew: the real components and stylesheet, no window on screen.

use {
    super::{super::source, Shot},
    moonframe_core::{
        Data, Roots,
        entries::{CheckOptions, RootPolicy},
    },
    std::path::Path,
};

/// The padding the render wrapper puts around a variant, in pixels.
const PADDING: f32 = 16.;
/// How tall the window is for a variant that names no height; the PNG is
/// cropped to what it drew.
const TALL: f32 = 1600.;

/// What the file holds, read from the wrapper's state.
struct Meta {
    ask: Option<String>,
    variants: Vec<(String, Option<String>, f32, Option<f32>)>,
}

/// Draws every variant of `file` in each of `appearances` to `out`, as
/// `a.png` and `a-dark.png`, and says what it drew.
pub(super) fn render(
    file: &str,
    roots: &Roots,
    out: &Path,
    appearances: &[&str],
) -> Result<(Option<String>, Vec<Shot>), String> {
    let work = tempfile::tempdir().map_err(|error| error.to_string())?;
    let meta = read_meta(file, roots, work.path())?;
    std::fs::create_dir_all(out).map_err(|error| format!("{}: {error}", out.display()))?;
    let mut shots = Vec::new();
    for (index, (name, note, width, height)) in meta.variants.into_iter().enumerate() {
        let letter = super::letter(index);
        let mut files = Vec::new();
        for appearance in appearances {
            let png = draw(
                file,
                roots,
                work.path(),
                index + 1,
                appearance,
                width,
                height,
            )?;
            let path = out.join(super::png_name(&letter, appearance));
            std::fs::write(&path, png).map_err(|error| format!("{}: {error}", path.display()))?;
            files.push(((*appearance).to_owned(), path));
        }
        shots.push(Shot {
            letter,
            name,
            note,
            width,
            files,
        });
    }
    Ok((meta.ask, shots))
}

/// The entry for variant `index` in `appearance`, written under `work`.
fn write_entry(
    work: &Path,
    file: &str,
    index: usize,
    appearance: &str,
) -> Result<std::path::PathBuf, String> {
    let dir = work.join(format!("v{index}-{appearance}"));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    std::fs::write(
        dir.join("main.luau"),
        source::entry(file, index, appearance),
    )
    .map_err(|error| error.to_string())?;
    Ok(dir)
}

fn options(roots: &Roots) -> CheckOptions {
    CheckOptions {
        name: Some("mockup:variant".to_owned()),
        roots: Some(roots.clone()),
        ..CheckOptions::default()
    }
}

/// Draws the first variant small, to read the names, notes and sizes.
fn read_meta(file: &str, roots: &Roots, work: &Path) -> Result<Meta, String> {
    let dir = write_entry(work, file, 1, "light")?;
    let drawn = crate::render_entry(&dir, &RootPolicy::default(), &options(roots), (240., 160.))?;
    let state = drawn
        .snapshot
        .and_then(|snapshot| snapshot.state)
        .ok_or("the mockup drew no state to read its variants from")?;
    let ask = state.get("ask").and_then(Data::as_str).map(str::to_owned);
    let Some(Data::List(list)) = state.get("variants") else {
        return Err("the mockup lists no variants".to_owned());
    };
    let text = |variant: &Data, key| variant.get(key).and_then(Data::as_str).map(str::to_owned);
    let number = |variant: &Data, key| variant.get(key).and_then(Data::as_f64).map(|n| n as f32);
    let variants = list
        .iter()
        .map(|variant| {
            (
                text(variant, "name").unwrap_or_default(),
                text(variant, "note"),
                number(variant, "width").unwrap_or(480.),
                number(variant, "height"),
            )
        })
        .collect();
    Ok(Meta { ask, variants })
}

/// One variant's PNG, cropped to its content and the wrapper's padding.
fn draw(
    file: &str,
    roots: &Roots,
    work: &Path,
    index: usize,
    appearance: &str,
    width: f32,
    height: Option<f32>,
) -> Result<Vec<u8>, String> {
    let dir = write_entry(work, file, index, appearance)?;
    let window = (width + 2. * PADDING, height.unwrap_or(TALL) + 2. * PADDING);
    let drawn = crate::render_entry(&dir, &RootPolicy::default(), &options(roots), window)?;
    if let Some(snapshot) = &drawn.snapshot {
        for problem in &snapshot.diagnostics {
            eprintln!("warning: variant {}: {problem}", super::letter(index - 1));
        }
    }
    let bottom = match height {
        Some(height) => height + 2. * PADDING,
        None => drawn
            .bounds
            .iter()
            .map(|placed| placed.y + placed.height)
            // The wrapper's own column is among them, padding and all.
            .fold(0., f32::max)
            .max(1.),
    }
    .min(window.1);
    let scale = drawn.width as f32 / window.0;
    let rows = ((bottom * scale).ceil() as u32).clamp(1, drawn.height);
    let image = image::load_from_memory(&drawn.png).map_err(|error| error.to_string())?;
    let cropped = image.crop_imm(0, 0, drawn.width, rows);
    let mut png = std::io::Cursor::new(Vec::new());
    cropped
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| format!("cannot encode the PNG: {error}"))?;
    Ok(png.into_inner())
}
