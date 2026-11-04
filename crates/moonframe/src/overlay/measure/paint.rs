//! How the measurements are drawn: thin lines with end ticks, small
//! numbered labels, shaded spaces, and rulers along the window's edges.

use {super::geometry::Span, gpui_kit::*};

/// Labels' text size and the font they share with the inspector's label.
const TEXT: f32 = 10.;
const FONT: &str = "Menlo";
/// A label's height and the room around its text.
const PILL_HEIGHT: f32 = 15.;
const PILL_PADDING: f32 = 4.;
/// How long a measured line's end ticks are.
const TICK: f32 = 7.;
/// How thick the rulers are.
pub(super) const RULER: f32 = 16.;

/// Paints a filled rectangle.
pub(super) fn fill(rect: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    window.paint_quad(fill_quad(rect, color));
}

fn fill_quad(rect: Bounds<Pixels>, color: Hsla) -> PaintQuad {
    quad(
        rect,
        Corners::default(),
        color,
        px(0.),
        transparent_black(),
        BorderStyle::Solid,
    )
}

/// A one-pixel outline around `rect`.
pub(super) fn outline(
    rect: Bounds<Pixels>,
    corners: Corners<Pixels>,
    color: Hsla,
    window: &mut Window,
) {
    window.paint_quad(quad(
        rect,
        corners,
        transparent_black(),
        px(1.),
        color,
        BorderStyle::Solid,
    ));
}

/// A full-length guide across `window`.
pub(super) fn guide(
    vertical: bool,
    at: Pixels,
    area: Bounds<Pixels>,
    color: Hsla,
    window: &mut Window,
) {
    let rect = if vertical {
        Bounds::new(point(at, area.top()), size(px(1.), area.size.height))
    } else {
        Bounds::new(point(area.left(), at), size(area.size.width, px(1.)))
    };
    fill(rect, color, window);
}

/// A span as a line through its middle, with a tick at each end, and its
/// length on a label beside it.
pub(super) fn measured(span: &Span, color: Hsla, text: Hsla, window: &mut Window, cx: &mut App) {
    let rect = span.rect;
    let center = rect.center();
    if span.horizontal {
        let y = center.y.round();
        fill(
            Bounds::new(point(rect.left(), y), size(rect.size.width, px(1.))),
            color,
            window,
        );
        for x in [rect.left(), rect.right() - px(1.)] {
            fill(
                Bounds::new(point(x, y - px(TICK / 2.).floor()), size(px(1.), px(TICK))),
                color,
                window,
            );
        }
        pill(
            &number(span.length()),
            point(center.x, y - px(PILL_HEIGHT / 2. + 3.)),
            color,
            text,
            window,
            cx,
        );
    } else {
        let x = center.x.round();
        fill(
            Bounds::new(point(x, rect.top()), size(px(1.), rect.size.height)),
            color,
            window,
        );
        for y in [rect.top(), rect.bottom() - px(1.)] {
            fill(
                Bounds::new(point(x - px(TICK / 2.).floor(), y), size(px(TICK), px(1.))),
                color,
                window,
            );
        }
        let label = number(span.length());
        let half = width(&label, window) / 2. + px(PILL_PADDING + 4.);
        pill(&label, point(x + half, center.y), color, text, window, cx);
    }
}

/// A span as shaded space with its length written in the middle.
pub(super) fn shaded(span: &Span, color: Hsla, window: &mut Window, cx: &mut App) {
    fill(span.rect, color.opacity(0.25), window);
    write(
        &number(span.length()),
        span.rect.center(),
        color,
        window,
        cx,
    );
}

/// A label centred on `center`: `text` on a rounded `background`.
pub(super) fn pill(
    label: &str,
    center: Point<Pixels>,
    background: Hsla,
    text: Hsla,
    window: &mut Window,
    cx: &mut App,
) {
    let wide = width(label, window) + px(PILL_PADDING * 2.);
    let rect = Bounds::new(
        point(center.x - wide / 2., center.y - px(PILL_HEIGHT / 2.)),
        size(wide, px(PILL_HEIGHT)),
    );
    window.paint_quad(quad(
        rect,
        Corners::all(px(4.)),
        background,
        px(0.),
        transparent_black(),
        BorderStyle::Solid,
    ));
    write(label, center, text, window, cx);
}

/// `label` centred on `center`.
pub(super) fn write(
    label: &str,
    center: Point<Pixels>,
    color: Hsla,
    window: &mut Window,
    cx: &mut App,
) {
    let line = shape(label, color, window);
    let origin = point(center.x - line.width / 2., center.y - px(PILL_HEIGHT / 2.));
    let _ = line.paint(origin, px(PILL_HEIGHT), TextAlign::Left, None, window, cx);
}

/// The colours rulers are drawn in.
pub(super) struct RulerColors {
    pub(super) surface: Hsla,
    pub(super) border: Hsla,
    pub(super) ticks: Hsla,
    pub(super) accent: Hsla,
    pub(super) accent_text: Hsla,
}

/// Rulers along the top and left of `area`, with `subject`'s extent on
/// each, its edges' positions on labels, and guides from its edges.
pub(super) fn rulers(
    subject: Bounds<Pixels>,
    area: Bounds<Pixels>,
    colors: &RulerColors,
    window: &mut Window,
    cx: &mut App,
) {
    for (vertical, at) in [
        (true, subject.left()),
        (true, subject.right() - px(1.)),
        (false, subject.top()),
        (false, subject.bottom() - px(1.)),
    ] {
        guide(vertical, at, area, colors.accent.opacity(0.45), window);
    }
    let thick = px(RULER);
    let top = Bounds::new(area.origin, size(area.size.width, thick));
    let left = Bounds::new(area.origin, size(thick, area.size.height));
    for (ruler, border) in [
        (
            top,
            Bounds::new(
                point(area.left(), thick - px(1.)),
                size(area.size.width, px(1.)),
            ),
        ),
        (
            left,
            Bounds::new(
                point(thick - px(1.), area.top()),
                size(px(1.), area.size.height),
            ),
        ),
    ] {
        fill(ruler, colors.surface, window);
        fill(border, colors.border, window);
    }
    // Ticks every 10 pixels, longer every 50, numbered every 100.
    let mut x = 0.;
    while x < f32::from(area.size.width) {
        let long = x % 50. == 0.;
        let tall = if long {
            8.
        } else {
            4.
        };
        fill(
            Bounds::new(point(px(x), thick - px(tall)), size(px(1.), px(tall))),
            colors.ticks,
            window,
        );
        if x % 100. == 0. && x > 0. {
            small(
                &number(x),
                point(px(x + 2.), px(1.)),
                colors.ticks,
                window,
                cx,
            );
        }
        x += 10.;
    }
    let mut y = 0.;
    while y < f32::from(area.size.height) {
        let long = y % 50. == 0.;
        let wide = if long {
            8.
        } else {
            4.
        };
        fill(
            Bounds::new(point(thick - px(wide), px(y)), size(px(wide), px(1.))),
            colors.ticks,
            window,
        );
        y += 10.;
    }
    fill(
        Bounds::new(area.origin, size(thick, thick)),
        colors.surface,
        window,
    );
    // The subject's extent, and where it starts and ends.
    let span_x = Bounds::from_corners(
        point(subject.left(), area.top()),
        point(subject.right(), thick - px(1.)),
    );
    let span_y = Bounds::from_corners(
        point(area.left(), subject.top()),
        point(thick - px(1.), subject.bottom()),
    );
    fill(span_x, colors.accent.opacity(0.2), window);
    fill(span_y, colors.accent.opacity(0.2), window);
    let at = |pixels: Pixels| number(f32::from(pixels).round());
    let mid = thick / 2.;
    for edge in [subject.left(), subject.right()] {
        pill(
            &at(edge),
            point(edge, mid),
            colors.accent,
            colors.accent_text,
            window,
            cx,
        );
    }
    for edge in [subject.top(), subject.bottom()] {
        let label = at(edge);
        let half = width(&label, window) / 2. + px(PILL_PADDING);
        pill(
            &label,
            point(half.max(mid), edge),
            colors.accent,
            colors.accent_text,
            window,
            cx,
        );
    }
}

/// A ruler's number, from its top left corner.
fn small(label: &str, origin: Point<Pixels>, color: Hsla, window: &mut Window, cx: &mut App) {
    let line = window.text_system().shape_line(
        label.to_owned().into(),
        px(TEXT - 2.),
        &[run(label, color)],
        None,
    );
    let _ = line.paint(origin, px(TEXT), TextAlign::Left, None, window, cx);
}

fn shape(label: &str, color: Hsla, window: &mut Window) -> ShapedLine {
    window.text_system().shape_line(
        label.to_owned().into(),
        px(TEXT),
        &[run(label, color)],
        None,
    )
}

fn width(label: &str, window: &mut Window) -> Pixels {
    shape(label, black(), window).width
}

fn run(label: &str, color: Hsla) -> TextRun {
    TextRun {
        len: label.len(),
        font: font(FONT),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

/// A length or position as a label shows it: whole pixels.
fn number(value: f32) -> String {
    format!("{}", value.round() as i64)
}
