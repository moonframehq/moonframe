//! `ui.line`, `ui.paths` and `ui.hatch`, as Rune shapes. Rune has no dashes
//! or arrowheads of its own, so they are drawn here as segments.

use {
    super::{Painter, Place},
    crate::style::looked,
    moonframe_core::{Arrow, Dots, Hatch, Line, Part, Paths, Polyline, States, Token},
    rune::{Color, El, LineCap, Shape, canvas, el},
};

/// Room kept around a line for its stroke and dots, in pixels.
const DOT: f32 = 3.;
/// The box a line is plotted in when it fills what its parent gives it:
/// Rune stretches it to fit, and stroke widths stay in points.
const STRETCHED: f32 = 200.;
/// What stripes cover when a hatch's size is left to its parent; the box
/// clips what is past its edges.
const FAR: f32 = 2400.;
/// An arrowhead's length and half its width, per pixel of stroke, and the
/// least of each, as the GPUI renderer draws them.
const HEAD: (f32, f32) = (4., 2.6);
const LEAST_HEAD: (f32, f32) = (7., 4.);

/// `points` as dashes of `on` with gaps of `off`.
fn dashes(
    mut shape: Shape,
    points: &[[f32; 2]],
    (on, off): (f32, f32),
    width: f32,
    color: Color,
) -> Shape {
    let (mut drawing, mut left) = (true, on);
    for pair in points.windows(2) {
        let (mut from, to) = (pair[0], pair[1]);
        let mut length = (to[0] - from[0]).hypot(to[1] - from[1]);
        while length > 0. {
            let step = left.min(length);
            let t = step / length;
            let at = [
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
            ];
            if drawing {
                shape = shape.line(from, at, width, LineCap::Butt, color);
            }
            from = at;
            length -= step;
            left -= step;
            if left <= 0. {
                drawing = !drawing;
                left = if drawing {
                    on
                } else {
                    off
                };
            }
        }
    }
    shape
}

/// A head at `tip`, pointing away from `from`.
fn arrowhead(shape: Shape, from: [f32; 2], tip: [f32; 2], stroke: f32, color: Color) -> Shape {
    let length = (stroke * HEAD.0).max(LEAST_HEAD.0);
    let half = (stroke * HEAD.1).max(LEAST_HEAD.1);
    let (dx, dy) = (tip[0] - from[0], tip[1] - from[1]);
    let norm = dx.hypot(dy).max(f32::EPSILON);
    let (ux, uy) = (dx / norm, dy / norm);
    let base = [tip[0] - ux * length, tip[1] - uy * length];
    shape.fill(
        vec![tip, [base[0] - uy * half, base[1] + ux * half], [
            base[0] + uy * half,
            base[1] - ux * half,
        ]],
        color,
    )
}

/// `line`'s corners with each rounded by its `radius`, as short chords.
fn rounded(line: &Polyline) -> Vec<[f32; 2]> {
    if line.radius <= 0. || line.points.len() < 3 {
        return line.points.clone();
    }
    let mut corners = vec![line.points[0]];
    for trio in line.points.windows(3) {
        let (a, b, c) = (trio[0], trio[1], trio[2]);
        let toward = |to: [f32; 2]| {
            let length = (to[0] - b[0]).hypot(to[1] - b[1]).max(f32::EPSILON);
            let cut = line.radius.min(length / 2.);
            [
                b[0] + (to[0] - b[0]) / length * cut,
                b[1] + (to[1] - b[1]) / length * cut,
            ]
        };
        let (start, end) = (toward(a), toward(c));
        for step in 0..=6 {
            let t = step as f32 / 6.;
            let curve = |axis: usize| {
                (1. - t).powi(2) * start[axis] + 2. * (1. - t) * t * b[axis] + t * t * end[axis]
            };
            corners.push([curve(0), curve(1)]);
        }
    }
    corners.extend(line.points.last());
    corners
}

impl<'w> Painter<'w> {
    pub(super) fn line(&self, line: &'w Line, place: &Place<'w>) -> El {
        let mut look = self.look(Part::Line, &line.look, States::default(), place);
        let stroke = look
            .base
            .color
            .as_ref()
            .map_or(self.palette.token(Token::Accent), |color| {
                self.palette.color(color)
            });
        // The background fills the area under the line, not the box.
        let area = look
            .base
            .background
            .take()
            .map_or(stroke.alpha(0.12), |color| self.palette.color(&color));
        let rule = look
            .base
            .border_color
            .as_ref()
            .map_or(self.palette.token(Token::Border), |color| {
                self.palette.color(color)
            });
        let (width, height) = (
            line.width.unwrap_or(STRETCHED),
            line.height.unwrap_or(STRETCHED),
        );
        let shape = plotted(line, (width, height), (stroke, area, rule));
        let shape = shape.view_box(width, height);
        let element = canvas(shape).key(place.path.clone());
        let element = match line.width {
            Some(width) => element.w(width),
            None => element.w_full(),
        };
        let element = match line.height {
            Some(height) => element.h(height),
            None => element.h_full(),
        };
        looked(element, &look, None, self.palette)
    }

    pub(super) fn paths(&self, paths: &'w Paths, place: &Place<'w>) -> El {
        let look = self.look(Part::Paths, &paths.look, States::default(), place);
        let fallback = look
            .base
            .color
            .as_ref()
            .map_or(self.palette.token(Token::Border), |color| {
                self.palette.color(color)
            });
        let mut shape = Shape::new();
        for line in &paths.paths {
            let color = line
                .color
                .as_ref()
                .map_or(fallback, |color| self.palette.color(color));
            let corners = rounded(line);
            shape = if line.dashed {
                dashes(shape, &corners, (4., 3.), line.stroke, color)
            } else {
                shape.stroke(corners, line.stroke, color)
            };
            if let (Some(arrow), [first, second, ..]) = (line.arrow, line.points.as_slice()) {
                if matches!(arrow, Arrow::Start | Arrow::Both) {
                    shape = arrowhead(shape, *second, *first, line.stroke, color);
                }
                if matches!(arrow, Arrow::End | Arrow::Both)
                    && let [.., before, last] = line.points.as_slice()
                {
                    shape = arrowhead(shape, *before, *last, line.stroke, color);
                }
            }
        }
        let element = canvas(shape).key(place.path.clone());
        let element = match paths.width {
            Some(width) => element.w(width),
            None => element.w_full(),
        };
        let element = match paths.height {
            Some(height) => element.h(height),
            None => element.h_full(),
        };
        looked(element, &look, None, self.palette)
    }

    pub(super) fn hatch(&self, hatch: &'w Hatch, place: &Place<'w>) -> El {
        let look = self.look(Part::Hatch, &hatch.look, States::default(), place);
        let stripe = look.base.color.as_ref().map_or(
            self.palette.token(Token::MutedForeground).alpha(0.35),
            |color| self.palette.color(color),
        );
        let (width, height) = (hatch.width.unwrap_or(FAR), hatch.height.unwrap_or(FAR));
        let mut shape = Shape::new();
        let mut offset = -height;
        while offset < width {
            shape = shape.line(
                [offset, height],
                [offset + height, 0.],
                1.,
                LineCap::Butt,
                stripe,
            );
            offset += hatch.gap.max(1.);
        }
        let stripes = canvas(shape)
            .pin(Some(0.), Some(0.), None, None)
            .size(width, height);
        let radius = look.base.radius.map_or(0., |radius| radius.top_left);
        let element = el()
            .key(place.path.clone())
            .clip_rounded(radius)
            .child(stripes);
        let element = match hatch.width {
            Some(width) => element.w(width),
            None => element,
        };
        let element = match hatch.height {
            Some(height) => element.h(height),
            None => element,
        };
        let element = if hatch.grow {
            element.grow(1.).min_w(0.).min_h(0.)
        } else {
            element
        };
        looked(element, &look, None, self.palette)
    }
}

/// `line` in a box of `size`: the area to the baseline, the baseline dashed,
/// the line, then its dots.
fn plotted(
    line: &Line,
    (width, height): (f32, f32),
    (stroke, area, rule): (Color, Color, Color),
) -> Shape {
    let mut shape = Shape::new();
    if line.points.is_empty() {
        return shape;
    }
    let fold = |start: f32, pick: fn(f32, f32) -> f32| {
        line.points
            .iter()
            .copied()
            .chain(line.baseline)
            .fold(start, pick)
    };
    let min = line.min.unwrap_or_else(|| fold(f32::INFINITY, f32::min));
    let max = line
        .max
        .unwrap_or_else(|| fold(f32::NEG_INFINITY, f32::max));
    let span = if (max - min).abs() < f32::EPSILON {
        1.
    } else {
        max - min
    };
    let pad = line.stroke + DOT;
    let (inner_w, inner_h) = ((width - 2. * pad).max(1.), (height - 2. * pad).max(1.));
    let y = |value: f32| pad + (1. - ((value - min) / span).clamp(0., 1.)) * inner_h;
    let steps = (line.points.len().max(2) - 1) as f32;
    let points: Vec<[f32; 2]> = line
        .points
        .iter()
        .enumerate()
        .map(|(index, value)| [pad + index as f32 / steps * inner_w, y(*value)])
        .collect();
    let base = y(line.baseline.unwrap_or(min));
    if let ([first, ..], [.., last], true) = (points.as_slice(), points.as_slice(), line.area) {
        let mut outline = vec![[first[0], base]];
        outline.extend(&points);
        outline.push([last[0], base]);
        shape = shape.fill(outline, area);
    }
    if line.baseline.is_some() {
        shape = dashes(shape, &[[1., base], [width - 1., base]], (3., 3.), 1., rule);
    }
    if points.len() > 1 {
        shape = if line.dashed {
            dashes(shape, &points, (4., 3.), line.stroke, stroke)
        } else {
            shape.stroke(points.clone(), line.stroke, stroke)
        };
    }
    let dotted: &[[f32; 2]] = match line.dots {
        Some(Dots::All) => &points,
        Some(Dots::Last) => &points[points.len() - 1..],
        None => &[],
    };
    for at in dotted {
        // A round cap on a line of no length is a dot, whatever the stretch.
        shape = shape.line(*at, *at, DOT * 2., LineCap::Round, stroke);
    }
    shape
}
