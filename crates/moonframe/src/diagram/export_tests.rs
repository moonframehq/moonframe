//! Exports drawn for real: a diagram as SVG, written as a file and as a
//! PNG through GPUI's SVG renderer, with no window (CI has no GPU).

use {
    gpui_kit::HeadlessAppContext,
    moonframe_diagram::{
        Options,
        export::Colors,
        layout::{Approx, Measure as _, layout},
        parse,
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
    let renderer = cx.update(|cx| cx.svg_renderer());
    let diagram =
        parse::mermaid::parse("flowchart LR\n a[Export] --> b{Works?} -->|yes| c((Done))")
            .expect("parsed");
    let laid = layout(&diagram, &Approx, &Options::default(), None);
    let mut approx = |text: &str, size: f32| Approx.width(text, size);
    let text = moonframe_diagram::export::svg(
        &diagram,
        &laid,
        &Colors::light(),
        "Helvetica",
        &[],
        &mut approx,
    );
    let name = format!("test-{}", std::process::id());
    let svg = super::export::write(&text, "svg", &name, &renderer).expect("svg written");
    let png = super::export::write(&text, "png", &name, &renderer).expect("png written");
    assert!(super::export::write(&text, "gif", &name, &renderer).is_err());
    let written = std::fs::read_to_string(&svg).expect("svg read");
    assert!(written.contains(">Works?<"), "{written}");
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
        for (colors, file) in [(Colors::light(), "light.svg"), (Colors::dark(), "dark.svg")] {
            let _ = std::fs::write(
                keep.join(file),
                moonframe_diagram::export::svg(
                    &diagram,
                    &laid,
                    &colors,
                    "Helvetica",
                    &["b".into()],
                    &mut approx,
                ),
            );
        }
    }
    let _ = std::fs::remove_file(svg);
    let _ = std::fs::remove_file(png);
}
