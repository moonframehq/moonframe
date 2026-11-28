//! A diagram drawn once, with no state of its own: fitted into its box
//! (never past 100 %), as tall as the diagram up to a cap. What markdown
//! and chat cards show; the interactive [`super::DiagramView`] is for
//! looking closer.

use {
    super::{camera::Camera, paint, scene::Scene, style::Palette},
    crate::{
        layout::{Layout, Widths},
        model::Diagram,
    },
    gpui_kit::*,
    std::{collections::HashMap, sync::Arc},
};

/// Room around the diagram inside its box, in pixels.
const MARGIN: f32 = 12.0;

/// `diagram` laid out as `layout`, filling the width it is given and as
/// tall as the diagram at 100 %, but no taller than `most_height`.
pub fn picture(diagram: Arc<Diagram>, layout: Arc<Layout>, most_height: f32) -> AnyElement {
    let natural = layout.bounds.h + MARGIN * 2.0;
    let height = natural.min(most_height).max(48.0);
    let shown = paint::Shown::still(diagram, layout.clone());
    let scene = Scene::still(&layout);
    canvas(
        |_, _, _| (),
        move |b, (), window, cx| {
            let camera = Camera::fit(
                layout.bounds,
                f32::from(b.size.width),
                f32::from(b.size.height),
                MARGIN,
            );
            paint::paint(&scene, &shown, camera, b, Palette::of(cx), window, cx);
        },
    )
    .w_full()
    .h(px(height))
    .into_any_element()
}

/// Each label of `diagram` measured in the theme's font, for its layout.
pub fn measure(diagram: &Diagram, font_size: f32, window: &mut Window, cx: &App) -> Widths {
    let mut widths = HashMap::new();
    let texts = diagram
        .nodes
        .iter()
        .flat_map(|n| std::iter::once(&n.label).chain(n.sublabel.as_ref()))
        .chain(diagram.messages.iter().map(|m| &m.label))
        .chain(diagram.notes.iter().map(|n| &n.text));
    for text in texts {
        if !widths.contains_key(text) {
            widths.insert(text.clone(), paint::text_width(text, font_size, window, cx));
        }
    }
    Widths(widths)
}
