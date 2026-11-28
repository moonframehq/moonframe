//! What `ui.diagram` hands out: the diagram as SVG or PNG, written to the
//! app's share folder (where `app:share` may take it from, and nowhere a
//! mod names).

use {
    gpui_kit::SvgRenderer,
    moonframe_core::Data,
    std::path::{Path, PathBuf},
};

/// `svg` as a file in `format` (`svg` or `png`), written to the share
/// folder as `diagram-<name>.<format>`. Off the UI thread: a PNG is drawn,
/// encoded and written here.
pub(super) fn write(
    svg: &str,
    format: &str,
    name: &str,
    renderer: &SvgRenderer,
) -> Result<PathBuf, String> {
    if !matches!(format, "svg" | "png") {
        return Err(format!("no export as `{format}`: svg or png"));
    }
    let dir = crate::paths::shared();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let path = dir.join(format!("diagram-{name}.{format}"));
    if format == "svg" {
        std::fs::write(&path, svg).map_err(|e| e.to_string())?;
    } else {
        png(svg, &path, renderer)?;
    }
    Ok(path)
}

/// `svg` rasterized at twice its size by GPUI's SVG renderer, as a PNG.
fn png(svg: &str, path: &Path, renderer: &SvgRenderer) -> Result<(), String> {
    let picture = renderer
        .render_single_frame(svg.as_bytes(), 2.0)
        .map_err(|e| e.to_string())?;
    let size = picture.size(0);
    let mut pixels = picture.as_bytes(0).ok_or("nothing drawn")?.to_vec();
    // GPUI keeps BGRA for the GPU; a PNG wants RGBA.
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let (w, h) = (size.width.0 as u32, size.height.0 as u32);
    let image =
        image::RgbaImage::from_raw(w, h, pixels).ok_or("the picture's size and pixels disagree")?;
    image
        .save_with_format(path, image::ImageFormat::Png)
        .map_err(|e| e.to_string())
}

/// What the mod hears of an export, with the `seq` it was asked with.
pub(super) fn told(result: Result<PathBuf, String>, format: &str, seq: Data) -> Data {
    let outcome = match result {
        Ok(path) => ("path", Data::String(path.to_string_lossy().into_owned())),
        Err(error) => ("error", Data::String(error)),
    };
    let body = Data::map([
        outcome,
        ("format", Data::String(format.to_owned())),
        ("seq", seq),
    ]);
    Data::map([("exported", body)])
}
