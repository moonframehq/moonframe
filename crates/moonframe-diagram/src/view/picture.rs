//! A diagram drawn once, with no state of its own: fitted into its box
//! (never past 100 %), as tall as the diagram up to a cap. What markdown
//! and chat cards show; the interactive [`super::DiagramView`] is for
//! looking closer.

use {
    super::{camera::Camera, paint, scene::Scene, style::Palette},
    crate::{
        geometry::Rect,
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
            let camera = fit(
                layout.bounds,
                f32::from(b.size.width),
                f32::from(b.size.height),
            );
            paint::paint(&scene, &shown, camera, b, Palette::of(cx), window, cx);
        },
    )
    .w_full()
    .h(px(height))
    .into_any_element()
}

/// All of `content` in a box of `w` by `h`, never past 100 %. Unlike
/// [`Camera::fit`] it shrinks as far as it takes: a picture has no
/// controls to reach what a least zoom would cut off.
fn fit(content: Rect, w: f32, h: f32) -> Camera {
    let mut camera = Camera::fit(content, w, h, MARGIN);
    if content.w <= 0.0 || content.h <= 0.0 || w <= 0.0 || h <= 0.0 {
        return camera;
    }
    let zoom = ((w - MARGIN * 2.0) / content.w)
        .min((h - MARGIN * 2.0) / content.h)
        .clamp(f32::EPSILON, 1.0);
    let center = content.center();
    camera.x = center.x - w / 2.0 / zoom;
    camera.y = center.y - h / 2.0 / zoom;
    camera.zoom = zoom;
    camera
}

/// Each label of `diagram` measured in the theme's font, for its layout.
pub fn measure(diagram: &Diagram, font_size: f32, window: &mut Window, cx: &App) -> Widths {
    let mut widths = HashMap::new();
    let texts = diagram
        .nodes
        .iter()
        .flat_map(|n| std::iter::once(&n.label).chain(n.sublabel.as_ref()))
        .chain(diagram.messages.iter().map(|m| &m.label))
        .chain(diagram.edges.iter().filter_map(|e| e.label.as_ref()))
        .chain(diagram.notes.iter().map(|n| &n.text));
    for text in texts {
        if !widths.contains_key(text) {
            widths.insert(text.clone(), paint::text_width(text, font_size, window, cx));
        }
    }
    Widths(widths)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use {super::fit, crate::geometry::Rect};

    #[test]
    fn a_tall_diagram_shrinks_past_the_views_least_zoom_to_show_both_ends() {
        let content = Rect::new(0.0, 0.0, 120.0, 6_000.0);
        let camera = fit(content, 600.0, 480.0);
        let shown = camera.visible(600.0, 480.0);
        assert!(camera.zoom < super::super::camera::MIN_ZOOM, "{camera:?}");
        assert!(shown.y <= 0.0 && shown.bottom() >= 6_000.0, "{shown:?}");
        assert_eq!(fit(Rect::new(0.0, 0.0, 50.0, 50.0), 600.0, 480.0).zoom, 1.0);
    }
}
