//! What `ui.diagram` hands out: the diagram as SVG or PNG, written to the
//! app's share folder (where `app:share` may take it from, and nowhere a
//! mod names), and as Mermaid on the clipboard.

use {
    gpui_kit::{App, ClipboardItem},
    moonframe_core::Data,
    moonframe_diagram::{Diagram, Layout, export, view, write},
    std::path::PathBuf,
};

/// A file of `diagram` in `format` (`svg` or `png`), drawn in the app's
/// colors, written to the share folder as `diagram-<name>.<format>`.
pub(super) fn file(
    diagram: &Diagram,
    layout: &Layout,
    highlight: &[String],
    format: &str,
    name: &str,
    cx: &App,
) -> Result<PathBuf, String> {
    let font = gpui_kit::component::ActiveTheme::theme(cx)
        .font_family
        .to_string();
    let svg = export::svg(diagram, layout, &view::export_colors(cx), &font, highlight);
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
    match format {
        "svg" => std::fs::write(&path, svg).map_err(|e| e.to_string())?,
        "png" => png(&svg, &path, cx)?,
        other => return Err(format!("no export as `{other}`: svg or png")),
    }
    Ok(path)
}

/// `svg` rasterized at twice its size by GPUI's SVG renderer, as a PNG.
fn png(svg: &str, path: &std::path::Path, cx: &App) -> Result<(), String> {
    let picture = cx
        .svg_renderer()
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

/// The diagram as Mermaid, on the clipboard.
pub(super) fn copy(diagram: &Diagram, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(write::mermaid(diagram)));
}

/// What the mod hears of an export.
pub(super) fn told(result: Result<PathBuf, String>, format: &str) -> Data {
    let body = match result {
        Ok(path) => Data::map([
            ("path", Data::String(path.to_string_lossy().into_owned())),
            ("format", Data::String(format.to_owned())),
        ]),
        Err(error) => Data::map([
            ("error", Data::String(error)),
            ("format", Data::String(format.to_owned())),
        ]),
    };
    Data::map([("exported", body)])
}
