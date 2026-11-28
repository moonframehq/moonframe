//! The pieces a frame is painted from, in screen pixels: node outlines by
//! kind, lines with rounded corners and their heads, and text.

use {
    crate::{
        geometry::{Point, Rect},
        model::{Head, NodeKind},
    },
    gpui_kit::{component::ActiveTheme as _, *},
};

pub(super) fn at(origin: gpui_kit::Point<Pixels>, p: Point) -> gpui_kit::Point<Pixels> {
    point(origin.x + px(p.x), origin.y + px(p.y))
}

pub(super) fn bounds_of(origin: gpui_kit::Point<Pixels>, r: Rect) -> Bounds<Pixels> {
    Bounds::new(at(origin, Point::new(r.x, r.y)), size(px(r.w), px(r.h)))
}

/// A node's outline filled with `fill` and stroked with `outline`.
pub(super) fn node(
    kind: NodeKind,
    r: Rect,
    zoom: f32,
    fill: Hsla,
    outline: Hsla,
    width: f32,
    origin: gpui_kit::Point<Pixels>,
    window: &mut Window,
) {
    let b = bounds_of(origin, r);
    let corner = |radius: f32| quad(b, px(radius), fill, px(width), outline, BorderStyle::Solid);
    match kind {
        NodeKind::Text => {},
        NodeKind::Box => window.paint_quad(corner(4.0 * zoom)),
        NodeKind::Rounded => window.paint_quad(corner(12.0 * zoom)),
        NodeKind::Pill => window.paint_quad(corner(r.h.min(r.w) / 2.0)),
        NodeKind::Note => {
            window.paint_quad(corner(3.0 * zoom));
            // A folded corner, top right.
            let fold = (14.0 * zoom).min(r.w / 3.0).min(r.h / 3.0);
            let mut path = PathBuilder::fill();
            path.move_to(at(origin, Point::new(r.right() - fold, r.y)));
            path.line_to(at(origin, Point::new(r.right(), r.y + fold)));
            path.line_to(at(origin, Point::new(r.right() - fold, r.y + fold)));
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, outline.opacity(0.5));
            }
        },
        NodeKind::Circle => outlined(ellipse(r, origin), fill, outline, width, window),
        NodeKind::Diamond => {
            let c = r.center();
            let corners = [
                Point::new(c.x, r.y),
                Point::new(r.right(), c.y),
                Point::new(c.x, r.bottom()),
                Point::new(r.x, c.y),
            ];
            outlined(
                |path: &mut PathBuilder| {
                    path.move_to(at(origin, corners[0]));
                    for &p in &corners[1..] {
                        path.line_to(at(origin, p));
                    }
                    path.close();
                },
                fill,
                outline,
                width,
                window,
            );
        },
        NodeKind::Cylinder => {
            let lid = (r.h * 0.18).min(14.0 * zoom);
            let top = Rect::new(r.x, r.y, r.w, lid * 2.0);
            outlined(
                |path: &mut PathBuilder| {
                    let k = 0.5523;
                    let (x0, x1, cx) = (r.x, r.right(), r.center().x);
                    let (ty, by) = (r.y + lid, r.bottom() - lid);
                    path.move_to(at(origin, Point::new(x0, ty)));
                    path.line_to(at(origin, Point::new(x0, by)));
                    // The bottom's front curve.
                    path.cubic_bezier_to(
                        at(origin, Point::new(cx, r.bottom())),
                        at(origin, Point::new(x0, by + lid * k)),
                        at(origin, Point::new(cx - r.w / 2.0 * k, r.bottom())),
                    );
                    path.cubic_bezier_to(
                        at(origin, Point::new(x1, by)),
                        at(origin, Point::new(cx + r.w / 2.0 * k, r.bottom())),
                        at(origin, Point::new(x1, by + lid * k)),
                    );
                    path.line_to(at(origin, Point::new(x1, ty)));
                    path.cubic_bezier_to(
                        at(origin, Point::new(cx, r.y)),
                        at(origin, Point::new(x1, ty - lid * k)),
                        at(origin, Point::new(cx + r.w / 2.0 * k, r.y)),
                    );
                    path.cubic_bezier_to(
                        at(origin, Point::new(x0, ty)),
                        at(origin, Point::new(cx - r.w / 2.0 * k, r.y)),
                        at(origin, Point::new(x0, ty - lid * k)),
                    );
                    path.close();
                },
                fill,
                outline,
                width,
                window,
            );
            // The lid's lower rim.
            let rim = ellipse(top, origin);
            let mut path = PathBuilder::stroke(px(width));
            rim(&mut path);
            if let Ok(path) = path.build() {
                window.paint_path(path, outline);
            }
        },
    }
}

/// An ellipse filling `r`, as four cubic arcs.
fn ellipse(r: Rect, origin: gpui_kit::Point<Pixels>) -> impl Fn(&mut PathBuilder) {
    move |path: &mut PathBuilder| {
        let k = 0.5523;
        let (c, rx, ry) = (r.center(), r.w / 2.0, r.h / 2.0);
        let p = |x: f32, y: f32| at(origin, Point::new(x, y));
        path.move_to(p(c.x + rx, c.y));
        path.cubic_bezier_to(
            p(c.x, c.y + ry),
            p(c.x + rx, c.y + ry * k),
            p(c.x + rx * k, c.y + ry),
        );
        path.cubic_bezier_to(
            p(c.x - rx, c.y),
            p(c.x - rx * k, c.y + ry),
            p(c.x - rx, c.y + ry * k),
        );
        path.cubic_bezier_to(
            p(c.x, c.y - ry),
            p(c.x - rx, c.y - ry * k),
            p(c.x - rx * k, c.y - ry),
        );
        path.cubic_bezier_to(
            p(c.x + rx, c.y),
            p(c.x + rx * k, c.y - ry),
            p(c.x + rx, c.y - ry * k),
        );
        path.close();
    }
}

/// The same outline filled, then stroked.
fn outlined(
    shape: impl Fn(&mut PathBuilder),
    fill: Hsla,
    outline: Hsla,
    width: f32,
    window: &mut Window,
) {
    let mut inside = PathBuilder::fill();
    shape(&mut inside);
    if let Ok(path) = inside.build() {
        window.paint_path(path, fill);
    }
    if width > 0.0 {
        let mut edge = PathBuilder::stroke(px(width));
        shape(&mut edge);
        if let Ok(path) = edge.build() {
            window.paint_path(path, outline);
        }
    }
}

/// How a line is drawn.
#[derive(Clone, Copy)]
pub(super) struct Stroke {
    pub color: Hsla,
    pub width: f32,
    pub dash: Option<(f32, f32)>,
    pub radius: f32,
}

/// A line through `points` (screen pixels from `origin`) with rounded
/// corners and its heads; the line stops short of a head, so it doesn't
/// poke through its tip.
pub(super) fn line(
    points: &[Point],
    stroke: Stroke,
    start: Head,
    end: Head,
    origin: gpui_kit::Point<Pixels>,
    window: &mut Window,
) {
    if points.len() < 2 {
        return;
    }
    let length = (stroke.width * 4.0).max(8.0);
    let mut trimmed = points.to_vec();
    if start != Head::None {
        trimmed[0] = toward(points[0], points[1], length * inset(start));
    }
    if end != Head::None {
        let n = points.len();
        trimmed[n - 1] = toward(points[n - 1], points[n - 2], length * inset(end));
    }
    let mut path = PathBuilder::stroke(px(stroke.width));
    if let Some((on, off)) = stroke.dash {
        path = path.dash_array(&[px(on), px(off)]);
    }
    path.move_to(at(origin, trimmed[0]));
    for i in 1..trimmed.len() {
        let corner = trimmed[i];
        match trimmed.get(i + 1) {
            Some(&next) if stroke.radius > 0.0 => {
                let room = trimmed[i - 1].distance(corner).min(corner.distance(next)) / 2.0;
                let radius = stroke.radius.min(room);
                path.line_to(at(origin, toward(corner, trimmed[i - 1], radius)));
                path.curve_to(at(origin, toward(corner, next, radius)), at(origin, corner));
            },
            _ => path.line_to(at(origin, corner)),
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, stroke.color);
    }
    let n = points.len();
    head(start, points[0], points[1], length, stroke, origin, window);
    head(
        end,
        points[n - 1],
        points[n - 2],
        length,
        stroke,
        origin,
        window,
    );
}

/// How far into its length a head's base sits from its tip.
fn inset(head: Head) -> f32 {
    match head {
        Head::None => 0.0,
        Head::Open => 0.2,
        Head::Circle => 0.9,
        Head::Cross => 0.0,
        Head::Diamond | Head::HollowDiamond => 1.6,
        Head::Arrow | Head::Triangle => 0.85,
    }
}

/// A head with its tip at `tip`, pointing away from `from`.
fn head(
    kind: Head,
    tip: Point,
    from: Point,
    length: f32,
    stroke: Stroke,
    origin: gpui_kit::Point<Pixels>,
    window: &mut Window,
) {
    let span = tip.distance(from);
    if kind == Head::None || span <= f32::EPSILON {
        return;
    }
    let (ux, uy) = ((tip.x - from.x) / span, (tip.y - from.y) / span);
    let half = length * 0.6;
    let back = |d: f32| Point::new(tip.x - ux * d, tip.y - uy * d);
    let side = |p: Point, s: f32| Point::new(p.x - uy * s, p.y + ux * s);
    let polygon = |corners: &[Point], filled: bool, window: &mut Window| {
        let mut path = if filled {
            PathBuilder::fill()
        } else {
            PathBuilder::stroke(px(stroke.width))
        };
        path.move_to(at(origin, corners[0]));
        for &p in &corners[1..] {
            path.line_to(at(origin, p));
        }
        // An open arrow stays two strokes, unlike a hollow triangle.
        if filled || (corners.len() > 2 && kind != Head::Open) {
            path.close();
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, stroke.color);
        }
    };
    match kind {
        Head::None => {},
        Head::Arrow => polygon(
            &[tip, side(back(length), half), side(back(length), -half)],
            true,
            window,
        ),
        Head::Open => polygon(
            &[side(back(length), half), tip, side(back(length), -half)],
            false,
            window,
        ),
        Head::Triangle => polygon(
            &[tip, side(back(length), half), side(back(length), -half)],
            false,
            window,
        ),
        Head::Diamond | Head::HollowDiamond => {
            let corners = [
                tip,
                side(back(length * 0.8), half * 0.7),
                back(length * 1.6),
                side(back(length * 0.8), -half * 0.7),
            ];
            polygon(&corners, kind == Head::Diamond, window);
        },
        Head::Circle => {
            let c = back(length * 0.45);
            let r = length * 0.4;
            let mut path = PathBuilder::stroke(px(stroke.width));
            path.move_to(at(origin, Point::new(c.x + r, c.y)));
            path.arc_to(
                point(px(r), px(r)),
                px(0.0),
                false,
                true,
                at(origin, Point::new(c.x - r, c.y)),
            );
            path.arc_to(
                point(px(r), px(r)),
                px(0.0),
                false,
                true,
                at(origin, Point::new(c.x + r, c.y)),
            );
            if let Ok(path) = path.build() {
                window.paint_path(path, stroke.color);
            }
        },
        Head::Cross => {
            let c = back(length * 0.6);
            let d = length * 0.35;
            polygon(
                &[
                    side(Point::new(c.x - ux * d, c.y - uy * d), d),
                    side(Point::new(c.x + ux * d, c.y + uy * d), -d),
                ],
                false,
                window,
            );
            polygon(
                &[
                    side(Point::new(c.x - ux * d, c.y - uy * d), -d),
                    side(Point::new(c.x + ux * d, c.y + uy * d), d),
                ],
                false,
                window,
            );
        },
    }
}

fn toward(from: Point, to: Point, by: f32) -> Point {
    let span = from.distance(to);
    if span <= f32::EPSILON {
        return from;
    }
    let t = (by / span).min(1.0);
    Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
}

fn run(text: &str, color: Hsla, bold: bool, cx: &App) -> TextRun {
    let mut font = font(cx.theme().font_family.clone());
    if bold {
        font.weight = FontWeight::MEDIUM;
    }
    TextRun {
        len: text.len(),
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

/// The width of `text` on one line at `size` pixels in the theme's font.
pub(crate) fn text_width(text: &str, size: f32, window: &mut Window, cx: &App) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let line = window.text_system().shape_line(
        text.to_owned().into(),
        px(size),
        &[run(text, black(), true, cx)],
        None,
    );
    f32::from(line.width)
}

/// `text` on one line centred on `center`, cut with an ellipsis to fit
/// `room` pixels.
pub(super) fn text(
    text: &str,
    center: Point,
    size: f32,
    room: f32,
    color: Hsla,
    bold: bool,
    origin: gpui_kit::Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    if text.is_empty() || size < 4.0 {
        return;
    }
    let shape = |text: &str, window: &mut Window| {
        window.text_system().shape_line(
            text.to_owned().into(),
            px(size),
            &[run(text, color, bold, cx)],
            None,
        )
    };
    let mut line = shape(text, window);
    if f32::from(line.width) > room {
        // Cut at characters, by halves, to the longest start that fits.
        let chars: Vec<char> = text.chars().collect();
        let (mut lo, mut hi) = (0, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let cut: String = chars[..mid].iter().collect::<String>() + "…";
            if f32::from(shape(&cut, window).width) <= room {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let cut: String = chars[..lo].iter().collect::<String>() + "…";
        line = shape(&cut, window);
    }
    let height = size * 1.3;
    let left = center.x - f32::from(line.width) / 2.0;
    let top = center.y - height / 2.0;
    let _ = line.paint(
        at(origin, Point::new(left, top)),
        px(height),
        TextAlign::Left,
        None,
        window,
        cx,
    );
}

/// `text` wrapped at words to `room` pixels wide, as many lines as `height`
/// holds (the last cut with an ellipsis), centred on `center`, or hanging
/// from it with `top`.
#[allow(clippy::too_many_arguments)]
pub(super) fn paragraph(
    words: &str,
    center: Point,
    top: bool,
    size: f32,
    room: f32,
    height: f32,
    color: Hsla,
    bold: bool,
    origin: gpui_kit::Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    if words.is_empty() || size < 4.0 {
        return;
    }
    let line = size * 1.3;
    let most = ((height / line).floor() as usize).max(1);
    let mut lines: Vec<String> = Vec::new();
    if most == 1 || text_width(words, size, window, cx) <= room {
        lines.push(words.to_owned());
    } else {
        let mut current = String::new();
        for word in words.split_whitespace() {
            let tried = if current.is_empty() {
                word.to_owned()
            } else {
                format!("{current} {word}")
            };
            if current.is_empty() || text_width(&tried, size, window, cx) <= room {
                current = tried;
            } else {
                lines.push(std::mem::replace(&mut current, word.to_owned()));
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
        if lines.len() > most {
            lines.truncate(most);
            if let Some(last) = lines.last_mut() {
                last.push('…');
            }
        }
    }
    let total = line * lines.len() as f32;
    let start = if top {
        center.y
    } else {
        center.y - total / 2.0
    };
    for (i, text_line) in lines.iter().enumerate() {
        let middle = Point::new(center.x, start + line * (i as f32 + 0.5));
        text(
            text_line, middle, size, room, color, bold, origin, window, cx,
        );
    }
}
