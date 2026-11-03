//! `ui.line`, `ui.paths` and `ui.hatch`: what views cannot build from
//! boxes. Each paints
//! with GPUI paths into a canvas the size of their box; the box carries the
//! look (corners, border, margin), and its colors say how to paint.

use {
    super::{
        Cx, Draw, Painter, Place,
        styling::{LookExt as _, StyleExt as _},
    },
    crate::color::ThemeColor,
    gpui_kit::{component::ActiveTheme, *},
    moonframe_core::{Arrow, Dots, Hatch, Line, Part, Paths, Polyline},
};

/// Room kept around a line for its stroke and dots, in pixels.
const DOT: f32 = 3.0;

impl<'w> Draw<'w> for Line {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let mut look = painter.look(Part::Line, &self.look, false, place);
        let theme = cx.theme();
        let stroke = look
            .base
            .color
            .map_or(theme.primary, |color| color.hsla(theme));
        // The background fills the area under the line, not the box.
        let area = look
            .base
            .background
            .take()
            .map_or_else(|| stroke.opacity(0.12), |color| color.hsla(theme));
        let baseline_color = look
            .base
            .border_color
            .map_or(theme.border, |color| color.hsla(theme));
        let dot_fill = theme.background;
        let line = self.clone();
        let plot = canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                paint_line(
                    &line,
                    bounds,
                    stroke,
                    area,
                    baseline_color,
                    dot_fill,
                    window,
                )
            },
        )
        .size_full();
        let mut element = div()
            .id(place.id())
            .relative()
            .flex_shrink_0()
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .child(plot);
        element = match self.width {
            Some(width) => element.w(px(width)),
            None => element.w_full(),
        };
        element = match self.height {
            Some(height) => element.h(px(height)),
            None => element.h_full(),
        };
        element.into_any_element()
    }
}

/// The line in `bounds`: the area to the baseline, the baseline dashed,
/// the line, then its dots.
fn paint_line(
    line: &Line,
    bounds: Bounds<Pixels>,
    stroke: Hsla,
    area: Hsla,
    baseline_color: Hsla,
    dot_fill: Hsla,
    window: &mut Window,
) {
    if line.points.is_empty() {
        return;
    }
    let mut least = line.points.iter().copied().fold(f32::INFINITY, f32::min);
    let mut most = line
        .points
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    if let Some(baseline) = line.baseline {
        least = least.min(baseline);
        most = most.max(baseline);
    }
    let min = line.min.unwrap_or(least);
    let max = line.max.unwrap_or(most);
    let span = if (max - min).abs() < f32::EPSILON {
        1.0
    } else {
        max - min
    };
    let pad = line.stroke + DOT;
    let left = f32::from(bounds.origin.x) + pad;
    let top = f32::from(bounds.origin.y) + pad;
    let width = (f32::from(bounds.size.width) - 2.0 * pad).max(1.0);
    let height = (f32::from(bounds.size.height) - 2.0 * pad).max(1.0);
    let y = |value: f32| px(top + (1.0 - (value - min) / span).clamp(0.0, 1.0) * height);
    let steps = (line.points.len().max(2) - 1) as f32;
    let points: Vec<Point<Pixels>> = line
        .points
        .iter()
        .enumerate()
        .map(|(index, value)| point(px(left + index as f32 / steps * width), y(*value)))
        .collect();
    let base = y(line.baseline.unwrap_or(min));
    let (first, last) = (points[0], points[points.len() - 1]);

    if line.area && points.len() > 1 {
        let mut fill = PathBuilder::fill();
        fill.move_to(point(first.x, base));
        for at in &points {
            fill.line_to(*at);
        }
        fill.line_to(point(last.x, base));
        fill.close();
        if let Ok(path) = fill.build() {
            window.paint_path(path, area);
        }
    }
    if line.baseline.is_some() {
        let mut dashes = PathBuilder::stroke(px(1.0)).dash_array(&[px(3.0), px(3.0)]);
        dashes.move_to(point(px(left - pad + 1.0), base));
        dashes.line_to(point(px(left + width + pad - 1.0), base));
        if let Ok(path) = dashes.build() {
            window.paint_path(path, baseline_color);
        }
    }
    if points.len() > 1 {
        let mut path = PathBuilder::stroke(px(line.stroke));
        if line.dashed {
            path = path.dash_array(&[px(4.0), px(3.0)]);
        }
        path.move_to(first);
        for at in &points[1..] {
            path.line_to(*at);
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, stroke);
        }
    }
    let dotted: &[Point<Pixels>] = match line.dots {
        Some(Dots::All) => &points,
        Some(Dots::Last) => &points[points.len() - 1..],
        None => &[],
    };
    for at in dotted {
        let size = px(DOT * 2.0);
        let dot = Bounds::new(
            point(at.x - px(DOT), at.y - px(DOT)),
            gpui_kit::size(size, size),
        );
        window.paint_quad(
            fill(dot, dot_fill)
                .corner_radii(px(DOT))
                .border_widths(px(1.5))
                .border_color(stroke),
        );
    }
}

/// An arrowhead's length and half its width, per pixel of stroke, and the
/// least of each.
const HEAD: (f32, f32) = (4.0, 2.6);
const LEAST_HEAD: (f32, f32) = (7.0, 4.0);

impl<'w> Draw<'w> for Paths {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Paths, &self.look, false, place);
        let theme = cx.theme();
        let fallback = look
            .base
            .color
            .map_or(theme.border, |color| color.hsla(theme));
        let lines: Vec<(Polyline, Hsla)> = self
            .paths
            .iter()
            .map(|line| {
                let color = line.color.map_or(fallback, |color| color.hsla(theme));
                (line.clone(), color)
            })
            .collect();
        let plot = canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                for (line, color) in &lines {
                    paint_polyline(line, bounds.origin, *color, window);
                }
            },
        )
        .absolute()
        .size_full();
        let mut element = div()
            .id(place.id())
            .relative()
            .flex_shrink_0()
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .child(plot);
        element = match self.width {
            Some(width) => element.w(px(width)),
            None => element.w_full(),
        };
        element = match self.height {
            Some(height) => element.h(px(height)),
            None => element.h_full(),
        };
        element.into_any_element()
    }
}

/// One line of a `ui.paths` from `origin`: its corners rounded by its
/// radius, then its arrowheads.
fn paint_polyline(line: &Polyline, origin: Point<Pixels>, color: Hsla, window: &mut Window) {
    let at = |[x, y]: [f32; 2]| point(origin.x + px(x), origin.y + px(y));
    let corners: Vec<Point<Pixels>> = line.points.iter().copied().map(at).collect();
    if corners.len() < 2 {
        return;
    }
    let (length, half) = (
        (line.stroke * HEAD.0).max(LEAST_HEAD.0),
        (line.stroke * HEAD.1).max(LEAST_HEAD.1),
    );
    let (start_head, end_head) = match line.arrow {
        Some(Arrow::Start) => (true, false),
        Some(Arrow::End) => (false, true),
        Some(Arrow::Both) => (true, true),
        None => (false, false),
    };
    // The stroke stops at an arrowhead's base, so its end doesn't poke
    // through the tip.
    let mut trimmed = corners.clone();
    if start_head && let [a, b, ..] = corners[..] {
        trimmed[0] = toward(a, b, length * 0.8);
    }
    if end_head && let [.., a, b] = corners[..] {
        let end = trimmed.len() - 1;
        trimmed[end] = toward(b, a, length * 0.8);
    }
    let mut path = PathBuilder::stroke(px(line.stroke));
    if line.dashed {
        path = path.dash_array(&[px(4.0), px(3.0)]);
    }
    path.move_to(trimmed[0]);
    for index in 1..trimmed.len() {
        let corner = trimmed[index];
        match trimmed.get(index + 1) {
            Some(&next) if line.radius > 0.0 => {
                let room = distance(trimmed[index - 1], corner).min(distance(corner, next)) / 2.0;
                let radius = line.radius.min(room);
                path.line_to(toward(corner, trimmed[index - 1], radius));
                path.curve_to(toward(corner, next, radius), corner);
            },
            _ => path.line_to(corner),
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
    if start_head && let [a, b, ..] = corners[..] {
        paint_head(a, b, length, half, color, window);
    }
    if end_head && let [.., a, b] = corners[..] {
        paint_head(b, a, length, half, color, window);
    }
}

/// A filled arrowhead with its tip at `tip`, pointing away from `from`.
fn paint_head(
    tip: Point<Pixels>,
    from: Point<Pixels>,
    length: f32,
    half: f32,
    color: Hsla,
    window: &mut Window,
) {
    let span = distance(from, tip);
    if span <= f32::EPSILON {
        return;
    }
    let (ux, uy) = (
        f32::from(tip.x - from.x) / span,
        f32::from(tip.y - from.y) / span,
    );
    let base = point(tip.x - px(ux * length), tip.y - px(uy * length));
    let mut head = PathBuilder::fill();
    head.move_to(tip);
    head.line_to(point(base.x - px(uy * half), base.y + px(ux * half)));
    head.line_to(point(base.x + px(uy * half), base.y - px(ux * half)));
    head.close();
    if let Ok(head) = head.build() {
        window.paint_path(head, color);
    }
}

fn distance(a: Point<Pixels>, b: Point<Pixels>) -> f32 {
    f32::from(b.x - a.x).hypot(f32::from(b.y - a.y))
}

/// The point `by` pixels from `from` toward `to`.
fn toward(from: Point<Pixels>, to: Point<Pixels>, by: f32) -> Point<Pixels> {
    let span = distance(from, to);
    if span <= f32::EPSILON {
        return from;
    }
    let t = (by / span).min(1.0);
    point(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
}

impl<'w> Draw<'w> for Hatch {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Hatch, &self.look, false, place);
        let theme = cx.theme();
        let stripe = look.base.color.map_or_else(
            || theme.muted_foreground.opacity(0.35),
            |color| color.hsla(theme),
        );
        let gap = self.gap;
        let stripes = canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    paint_stripes(bounds, gap, stripe, window);
                });
            },
        )
        .absolute()
        .size_full();
        let mut element = div()
            .id(place.id())
            .relative()
            .overflow_hidden()
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .child(stripes);
        if let Some(width) = self.width {
            element = element.w(px(width)).flex_shrink_0();
        }
        if let Some(height) = self.height {
            element = element.h(px(height));
        }
        if self.grow {
            element = element.flex_1();
        }
        element.into_any_element()
    }
}

/// Lines rising at 45° every `gap` pixels, across `bounds`.
fn paint_stripes(bounds: Bounds<Pixels>, gap: f32, color: Hsla, window: &mut Window) {
    let (x, y) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let mut path = PathBuilder::stroke(px(1.0));
    let mut offset = -height;
    while offset < width {
        path.move_to(point(px(x + offset), px(y + height)));
        path.line_to(point(px(x + offset + height), px(y)));
        offset += gap;
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
