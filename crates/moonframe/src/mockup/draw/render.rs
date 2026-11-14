//! Each variant drawn headless to a PNG, in light and dark, cropped to what
//! it drew: the real components and stylesheet, no window on screen.

use {
    super::{super::source, Shot},
    moonframe_core::{
        Data, Roots, Workspace,
        entries::{CheckOptions, Entries, RootPolicy},
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
    clear_old(out)?;
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
            // The PNG holds the wrapper's padding on each side too.
            width: width + 2. * PADDING,
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

/// Removes the PNGs an earlier render left in `out` (`a.png`, `b-dark.png`,
/// `v27.png`, ...), so a variant taken out of the file goes with them.
fn clear_old(out: &Path) -> Result<(), String> {
    let listing = std::fs::read_dir(out).map_err(|error| format!("{}: {error}", out.display()))?;
    for item in listing.flatten() {
        let name = item.file_name();
        if is_shot_name(&name.to_string_lossy()) {
            let path = item.path();
            std::fs::remove_file(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    Ok(())
}

/// Whether `name` is one [`super::png_name`] makes.
fn is_shot_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".png") else {
        return false;
    };
    let letter = stem.strip_suffix("-dark").unwrap_or(stem);
    let mut chars = letter.chars();
    match (chars.next(), chars.as_str()) {
        (Some('v'), digits) if !digits.is_empty() => digits.chars().all(|c| c.is_ascii_digit()),
        (Some(c), "") => c.is_ascii_lowercase(),
        _ => false,
    }
}

/// Reads the names, notes and sizes from the wrapper's state: its mod
/// loaded in a workspace of its own, nothing drawn.
fn read_meta(file: &str, roots: &Roots, work: &Path) -> Result<Meta, String> {
    let dir = write_entry(work, file, 0, "light")?;
    let name = "mockup:meta";
    let entries = Entries::default();
    entries.add_root("mockup", work, RootPolicy::default())?;
    entries.add_entry("mockup", "meta", &dir)?;
    let mut workspace = Workspace::open(entries.apply(roots.clone()), false);
    workspace.restart(name, None);
    let slot = workspace
        .slot(name)
        .ok_or("the mockup did not load to read its variants from")?;
    if let Some(error) = slot.error() {
        return Err(format!("the mockup did not load: {error}"));
    }
    let state = slot
        .state()
        .ok_or("the mockup has no state to read its variants from")?
        .map_err(|error| error.to_string())?;
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

/// The bottom of what a variant with no height drew. A box that reaches the
/// window's bottom stretched to fill it (the wrapper's column, say) and
/// says nothing of the content, so only the boxes inside it count.
fn content_bottom(bounds: &[crate::ElementBounds], window: f32) -> f32 {
    bounds
        .iter()
        .map(|placed| placed.y + placed.height)
        .filter(|bottom| *bottom < window - 0.5)
        .fold(PADDING, f32::max)
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
        None => content_bottom(&drawn.bounds, window.1) + PADDING,
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

#[cfg(test)]
mod tests {
    use {super::*, crate::ElementBounds};

    fn placed(y: f32, height: f32) -> ElementBounds {
        ElementBounds {
            path: String::new(),
            x: 0.,
            y,
            width: 100.,
            height,
        }
    }

    #[test]
    fn a_stretched_column_does_not_set_the_crop() {
        let window = TALL + 2. * PADDING;
        let bounds = [placed(0., window), placed(16., 40.), placed(20., 30.)];
        assert_eq!(content_bottom(&bounds, window), 56.);
        assert_eq!(content_bottom(&[placed(0., window)], window), PADDING);
    }

    #[test]
    fn only_the_shots_names_are_cleared() {
        for name in ["a.png", "b-dark.png", "v.png", "v27.png", "v27-dark.png"] {
            assert!(is_shot_name(name), "{name}");
        }
        for name in [
            "index.html",
            "logo.png",
            "A.png",
            "ab.png",
            "vx.png",
            "a-dark.jpg",
            "-dark.png",
        ] {
            assert!(!is_shot_name(name), "{name}");
        }
    }
}
