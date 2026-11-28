//! Exports drawn for real: a diagram laid out in a headless window, then
//! written as SVG and as PNG through GPUI's SVG renderer.

use {
    gpui_kit::{AppContext as _, HeadlessAppContext, px, size},
    moonframe_diagram::{
        Options,
        layout::layout,
        parse,
        view::{DiagramView, measure},
    },
    std::sync::Arc,
};

#[test]
fn a_diagram_exports_as_svg_and_png() {
    let mut cx = HeadlessAppContext::with_platform(
        crate::render_entry::window::text_system(),
        Arc::new(crate::assets::Assets::new(None)),
        gpui_platform_capture::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    let handle = cx
        .open_window(size(px(400.0), px(300.0)), |_, cx| cx.new(DiagramView::new))
        .expect("a window");
    let diagram =
        parse::mermaid::parse("flowchart LR\n a[Export] --> b{Works?} -->|yes| c((Done))")
            .expect("parsed");
    let name = format!("test-{}", std::process::id());
    let (svg, png) = cx
        .update_window(handle.into(), |_, window, cx| {
            let widths = measure(&diagram, 14.0, window, cx);
            let laid = layout(&diagram, &widths, &Options::default(), None);
            let svg = super::export::file(&diagram, &laid, &[], "svg", &name, cx);
            let png = super::export::file(&diagram, &laid, &[], "png", &name, cx);
            (svg, png)
        })
        .expect("drawn");
    let (svg, png) = (svg.expect("svg written"), png.expect("png written"));
    let text = std::fs::read_to_string(&svg).expect("svg read");
    assert!(text.contains(">Works?<"), "{text}");
    let picture = image::open(&png).expect("png decodes");
    assert!(
        picture.width() > 200 && picture.height() > 60,
        "{}x{}",
        picture.width(),
        picture.height()
    );
    // `KEEP_EXPORTS=<folder>` keeps them, to look at.
    if let Some(keep) = std::env::var_os("KEEP_EXPORTS") {
        let keep = std::path::PathBuf::from(keep);
        let _ = std::fs::copy(&svg, keep.join("export.svg"));
        let _ = std::fs::copy(&png, keep.join("export.png"));
        for (colors, file) in [
            (moonframe_diagram::export::Colors::light(), "light.svg"),
            (moonframe_diagram::export::Colors::dark(), "dark.svg"),
        ] {
            let widths = moonframe_diagram::layout::Approx;
            let laid = layout(&diagram, &widths, &Options::default(), None);
            let _ =
                std::fs::write(
                    keep.join(file),
                    moonframe_diagram::export::svg(&diagram, &laid, &colors, "Helvetica", &[
                        "b".into()
                    ]),
                );
        }
    }
    let _ = std::fs::remove_file(svg);
    let _ = std::fs::remove_file(png);
}
