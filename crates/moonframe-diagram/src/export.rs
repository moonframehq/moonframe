//! A laid-out diagram as SVG, drawn as the view draws it (shapes, edges
//! with rounded corners and their heads, labels, groups, a sequence's
//! lifelines and notes) in the colors given, so an export matches the
//! app's theme, light or dark. Plain text, no GUI: rasterize it for a PNG.

use {
    crate::{
        geometry::{Point, Rect},
        layout::Layout,
        model::{Diagram, EdgeKind, Head, MessageKind, NodeKind, Tone},
    },
    std::fmt::Write as _,
};

/// The colors an export is drawn in, as `#rrggbb` (or any SVG color).
#[derive(Clone, Debug, PartialEq)]
pub struct Colors {
    pub background: String,
    pub card: String,
    pub outline: String,
    pub text: String,
    pub quiet: String,
    pub edge: String,
    pub accent: String,
    pub note: String,
    pub note_text: String,
    pub success: String,
    pub warning: String,
    pub danger: String,
}

impl Colors {
    pub fn light() -> Self {
        Self {
            background: "#ffffff".into(),
            card: "#ffffff".into(),
            outline: "#d4d4d8".into(),
            text: "#18181b".into(),
            quiet: "#71717a".into(),
            edge: "#71717a".into(),
            accent: "#4f46e5".into(),
            note: "#fde68a".into(),
            note_text: "#422006".into(),
            success: "#16a34a".into(),
            warning: "#d97706".into(),
            danger: "#dc2626".into(),
        }
    }

    pub fn dark() -> Self {
        Self {
            background: "#0a0a0a".into(),
            card: "#1c1c1f".into(),
            outline: "#3f3f46".into(),
            text: "#f4f4f5".into(),
            quiet: "#a1a1aa".into(),
            edge: "#a1a1aa".into(),
            accent: "#818cf8".into(),
            note: "#6b5a17".into(),
            note_text: "#fef3c7".into(),
            success: "#4ade80".into(),
            warning: "#fbbf24".into(),
            danger: "#f87171".into(),
        }
    }

    fn tone(&self, tone: Tone) -> &str {
        match tone {
            Tone::Accent => &self.accent,
            Tone::Success => &self.success,
            Tone::Warning => &self.warning,
            Tone::Danger => &self.danger,
            Tone::Muted => &self.quiet,
        }
    }
}

/// Room around the diagram, in pixels.
const MARGIN: f32 = 24.0;
const FONT: f32 = 14.0;

/// `font` with fallbacks every renderer knows, so a name it doesn't (a
/// system UI font's private name) still draws in a sans serif.
fn families(font: &str) -> String {
    let named = if font.trim().is_empty() {
        String::new()
    } else {
        format!("'{}', ", font.replace('\'', ""))
    };
    format!("{named}-apple-system, 'Helvetica Neue', Helvetica, Arial, sans-serif")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// How text is drawn: its color, size and weight.
#[derive(Clone, Copy)]
struct Ink<'a> {
    color: &'a str,
    size: f32,
    bold: bool,
}

impl<'a> Ink<'a> {
    fn new(color: &'a str, size: f32) -> Self {
        Self {
            color,
            size,
            bold: false,
        }
    }

    fn bold(self) -> Self {
        Self { bold: true, ..self }
    }
}

/// `diagram` laid out as `layout`, as an SVG document; `highlight` ids in
/// the accent. `measure` gives a line's width at a size in `font`, so
/// labels wrap and shorten to their boxes as the view's do.
pub fn svg(
    diagram: &Diagram,
    layout: &Layout,
    colors: &Colors,
    font: &str,
    highlight: &[String],
    measure: &mut dyn FnMut(&str, f32) -> f32,
) -> String {
    let b = drawn_bounds(diagram, layout, measure);
    let (w, h) = (b.w + MARGIN * 2.0, b.h + MARGIN * 2.0);
    let (dx, dy) = (MARGIN - b.x, MARGIN - b.y);
    let at = |p: Point| Point::new(p.x + dx, p.y + dy);
    let lit = |id: &str| highlight.iter().any(|h| h == id);
    let mut out = String::new();
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.0} {h:.0}" font-family="{}" font-size="{FONT}">"#,
        escape(&families(font))
    );
    let _ = write!(
        out,
        r#"<rect width="100%" height="100%" fill="{}"/>"#,
        colors.background
    );

    for group in &diagram.groups {
        if let Some(r) = layout.groups.get(&group.id) {
            let tint = escape(group.tone.map_or(colors.quiet.as_str(), |t| colors.tone(t)));
            let _ = write!(
                out,
                r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="10" fill="{tint}" fill-opacity="0.06" stroke="{tint}" stroke-opacity="0.4" stroke-dasharray="4 3"/>"#,
                r.x + dx,
                r.y + dy,
                r.w,
                r.h
            );
            let _ = write!(
                out,
                r#"<text x="{:.1}" y="{:.1}" fill="{}" font-size="11" font-weight="500">{}</text>"#,
                r.x + dx + 10.0,
                r.y + dy + 16.0,
                colors.quiet,
                escape(&group.label)
            );
        }
    }
    for (_, top, bottom) in &layout.lifelines {
        let (a, z) = (at(*top), at(*bottom));
        let _ = write!(
            out,
            r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-dasharray="4 4"/>"#,
            a.x, a.y, z.x, z.y, colors.outline
        );
    }
    for (message, route) in diagram.messages.iter().zip(&layout.messages) {
        let points: Vec<Point> = route.points.iter().map(|p| at(*p)).collect();
        let (dash, end) = match message.kind {
            MessageKind::Sync => (None, Head::Arrow),
            MessageKind::Async => (None, Head::Open),
            MessageKind::Reply => (Some("5 4"), Head::Open),
        };
        line(
            &mut out,
            &points,
            &colors.text,
            1.5,
            dash,
            6.0,
            Head::None,
            end,
        );
        if let Some(label) = route.label {
            text(
                &mut out,
                measure,
                &message.label,
                at(label),
                f32::INFINITY,
                Ink::new(&colors.text, 12.0),
            );
        }
    }
    let mut labels = String::new();
    for edge in diagram
        .edges
        .iter()
        .filter(|e| e.kind != EdgeKind::Invisible)
    {
        let Some(route) = layout.edges.get(&edge.id) else {
            continue;
        };
        let points: Vec<Point> = route.points.iter().map(|p| at(*p)).collect();
        let color = if lit(&edge.id) {
            colors.accent.as_str()
        } else {
            edge.tone.map_or(colors.edge.as_str(), |t| colors.tone(t))
        };
        let width = if edge.kind == EdgeKind::Thick {
            3.0
        } else {
            1.5
        };
        let dash = match edge.kind {
            EdgeKind::Dashed => Some("6 4"),
            EdgeKind::Dotted => Some("1.5 3.5"),
            _ => None,
        };
        line(
            &mut out, &points, color, width, dash, 10.0, edge.start, edge.end,
        );
        if let (Some(p), Some(label)) =
            (route.label, edge.label.as_deref().filter(|l| !l.is_empty()))
        {
            let p = at(p);
            let wide = measure(label, 12.0) + 10.0;
            let _ = write!(
                labels,
                r#"<rect x="{:.1}" y="{:.1}" width="{wide:.1}" height="18" rx="9" fill="{}"/>"#,
                p.x - wide / 2.0,
                p.y - 9.0,
                colors.background
            );
            let quiet = if color == colors.edge {
                &colors.quiet
            } else {
                color
            };
            text(
                &mut labels,
                measure,
                label,
                p,
                f32::INFINITY,
                Ink::new(quiet, 12.0),
            );
        }
    }
    for node in &diagram.nodes {
        let Some(r) = layout.nodes.get(&node.id) else {
            continue;
        };
        let r = r.translate(dx, dy);
        let (fill, stroke, ink) = node_colors(node.kind, node.tone, node.fill.as_deref(), colors);
        let (stroke, width) = if lit(&node.id) {
            (colors.accent.clone(), 2.0)
        } else {
            (stroke, 1.0)
        };
        shape(&mut out, node.kind, r, &fill, &stroke, width);
        // As the view draws it: the label in the middle (a diamond's in
        // its middle half; a note's from the top), the sublabel under it.
        let room = match node.kind {
            NodeKind::Diamond => r.w * 0.6,
            NodeKind::Circle => r.w * 0.8,
            _ => r.w - 12.0,
        };
        let note = node.kind == NodeKind::Note;
        let pad = 10.0;
        let middle = if note {
            Point::new(r.center().x, r.y + pad)
        } else {
            r.center()
        };
        let tall = match node.kind {
            NodeKind::Diamond => r.h * 0.55,
            _ => r.h - pad * 2.0,
        };
        let ink = Ink::new(&ink, FONT).bold();
        match &node.sublabel {
            Some(sub) => {
                let line = FONT * 1.3;
                text(
                    &mut out,
                    measure,
                    &node.label,
                    Point::new(middle.x, middle.y - line * 0.45),
                    room,
                    ink,
                );
                text(
                    &mut out,
                    measure,
                    sub,
                    Point::new(middle.x, middle.y + line * 0.55),
                    room,
                    Ink::new(&colors.quiet, FONT * 0.86),
                );
            },
            None => paragraph(
                &mut out,
                measure,
                &node.label,
                middle,
                note,
                room,
                tall,
                ink,
            ),
        }
    }
    for (note, r) in diagram.notes.iter().zip(&layout.notes) {
        let r = r.translate(dx, dy);
        shape(&mut out, NodeKind::Note, r, &colors.note, &colors.note, 1.0);
        text(
            &mut out,
            measure,
            &note.text,
            r.center(),
            r.w - 10.0,
            Ink::new(&colors.note_text, FONT * 0.9),
        );
    }
    out.push_str(&labels);
    out.push_str("</svg>");
    out
}

/// The layout's bounds and every label drawn beside a line (a message's,
/// an edge's), which the layout does not count, so none is cut off.
fn drawn_bounds(
    diagram: &Diagram,
    layout: &Layout,
    measure: &mut dyn FnMut(&str, f32) -> f32,
) -> Rect {
    let mut b = layout.bounds;
    let mut add = |at: Point, words: &str, pad: f32| {
        if !words.is_empty() {
            let wide = measure(words, 12.0) + pad;
            b = b.union(&Rect::new(at.x - wide / 2.0, at.y - 9.0, wide, 18.0));
        }
    };
    for (message, route) in diagram.messages.iter().zip(&layout.messages) {
        if let Some(at) = route.label {
            add(at, &message.label, 0.0);
        }
    }
    for edge in &diagram.edges {
        if let (Some(at), Some(label)) = (
            layout.edges.get(&edge.id).and_then(|r| r.label),
            edge.label.as_deref(),
        ) {
            add(at, label, 10.0);
        }
    }
    b
}

fn node_colors(
    kind: NodeKind,
    tone: Option<Tone>,
    fill: Option<&str>,
    colors: &Colors,
) -> (String, String, String) {
    if let Some(fill) = fill.filter(|f| crate::model::is_hex_color(f)) {
        // As in the view: dark text on a light fill, light on a dark one.
        let ink = if lightness(fill) > 0.6 {
            "#1f1f1f"
        } else {
            "#f5f5f5"
        };
        return (fill.to_owned(), colors.outline.clone(), ink.to_owned());
    }
    match (kind, tone) {
        (NodeKind::Text, _) => ("none".into(), "none".into(), colors.text.clone()),
        (NodeKind::Note, None) => (
            colors.note.clone(),
            colors.note.clone(),
            colors.note_text.clone(),
        ),
        // As the view does: the card tinted with the tone, opaque, so a
        // group or the background under it doesn't show through.
        (_, Some(tone)) => (
            mix(&colors.card, colors.tone(tone), 0.14),
            colors.tone(tone).to_owned(),
            colors.text.clone(),
        ),
        (_, None) => (
            colors.card.clone(),
            colors.outline.clone(),
            colors.text.clone(),
        ),
    }
}

/// A `#rgb` or `#rrggbb` color's red, green and blue, 0 to 1.
fn rgb(hex: &str) -> Option<[f32; 3]> {
    if !crate::model::is_hex_color(hex) {
        return None;
    }
    let digits = &hex[1..];
    let channel = |i: usize| -> f32 {
        let v = if digits.len() == 3 {
            u8::from_str_radix(&digits[i..=i], 16).map_or(0, |v| v * 17)
        } else {
            u8::from_str_radix(&digits[i * 2..i * 2 + 2], 16).unwrap_or(0)
        };
        f32::from(v) / 255.0
    };
    Some([channel(0), channel(1), channel(2)])
}

/// The HSL lightness of a `#rgb` or `#rrggbb` color, 0 to 1.
fn lightness(hex: &str) -> f32 {
    let [r, g, b] = rgb(hex).unwrap_or_default();
    (r.max(g).max(b) + r.min(g).min(b)) / 2.0
}

/// `base` with `by` of `over` laid on it, as `#rrggbb`; `base` alone when
/// either is not a hex color.
fn mix(base: &str, over: &str, by: f32) -> String {
    let (Some(a), Some(b)) = (rgb(base), rgb(over)) else {
        return base.to_owned();
    };
    let byte = |i: usize| ((a[i] + (b[i] - a[i]) * by).clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(0), byte(1), byte(2))
}

fn shape(out: &mut String, kind: NodeKind, r: Rect, fill: &str, stroke: &str, width: f32) {
    let (fill, stroke) = (escape(fill), escape(stroke));
    let style = format!(r#"fill="{fill}" stroke="{stroke}" stroke-width="{width}""#);
    let rect = |rx: f32| {
        format!(
            r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="{rx:.1}" {style}/>"#,
            r.x, r.y, r.w, r.h
        )
    };
    let c = r.center();
    let svg = match kind {
        NodeKind::Text => String::new(),
        NodeKind::Box => rect(4.0),
        NodeKind::Rounded => rect(12.0),
        NodeKind::Pill => rect(r.h.min(r.w) / 2.0),
        NodeKind::Note => rect(3.0),
        NodeKind::Circle => format!(
            r#"<ellipse cx="{:.1}" cy="{:.1}" rx="{:.1}" ry="{:.1}" {style}/>"#,
            c.x,
            c.y,
            r.w / 2.0,
            r.h / 2.0
        ),
        NodeKind::Diamond => format!(
            r#"<polygon points="{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}" {style}/>"#,
            c.x,
            r.y,
            r.right(),
            c.y,
            c.x,
            r.bottom(),
            r.x,
            c.y
        ),
        NodeKind::Cylinder => {
            let lid = (r.h * 0.18).min(14.0);
            format!(
                r#"<path d="M{x0:.1},{t:.1} V{b:.1} A{rx:.1},{lid:.1} 0 0 0 {x1:.1},{b:.1} V{t:.1} A{rx:.1},{lid:.1} 0 0 0 {x0:.1},{t:.1} Z" {style}/><path d="M{x0:.1},{t:.1} A{rx:.1},{lid:.1} 0 0 0 {x1:.1},{t:.1}" fill="none" stroke="{stroke}" stroke-width="{width}"/>"#,
                x0 = r.x,
                x1 = r.right(),
                t = r.y + lid,
                b = r.bottom() - lid,
                rx = r.w / 2.0
            )
        },
    };
    out.push_str(&svg);
}

/// `words` on one line centred on `at`, cut with an ellipsis to fit `room`
/// pixels.
fn text(
    out: &mut String,
    measure: &mut dyn FnMut(&str, f32) -> f32,
    words: &str,
    at: Point,
    room: f32,
    ink: Ink<'_>,
) {
    if words.is_empty() {
        return;
    }
    let size = ink.size;
    let mut words = words.to_owned();
    if measure(&words, size) > room {
        // Cut at characters, by halves, to the longest start that fits.
        let chars: Vec<char> = words.chars().collect();
        let (mut lo, mut hi) = (0, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let cut: String = chars[..mid].iter().collect::<String>() + "…";
            if measure(&cut, size) <= room {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        words = chars[..lo].iter().collect::<String>() + "…";
    }
    let color = escape(ink.color);
    let weight = if ink.bold {
        r#" font-weight="500""#
    } else {
        ""
    };
    let _ = write!(
        out,
        r#"<text x="{:.1}" y="{:.1}" fill="{color}" font-size="{size}" text-anchor="middle" dominant-baseline="central"{weight}>{}</text>"#,
        at.x,
        at.y,
        escape(&words)
    );
}

/// `words` wrapped at spaces to `room` pixels wide, as many lines as
/// `height` holds (the last cut with an ellipsis), centred on `center`, or
/// hanging from it with `top`.
#[allow(clippy::too_many_arguments)]
fn paragraph(
    out: &mut String,
    measure: &mut dyn FnMut(&str, f32) -> f32,
    words: &str,
    center: Point,
    top: bool,
    room: f32,
    height: f32,
    ink: Ink<'_>,
) {
    if words.is_empty() {
        return;
    }
    let line = ink.size * 1.3;
    let most = ((height / line).floor() as usize).max(1);
    let mut lines: Vec<String> = Vec::new();
    if most == 1 || measure(words, ink.size) <= room {
        lines.push(words.to_owned());
    } else {
        let mut current = String::new();
        for word in words.split_whitespace() {
            let tried = if current.is_empty() {
                word.to_owned()
            } else {
                format!("{current} {word}")
            };
            if current.is_empty() || measure(&tried, ink.size) <= room {
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
    for (i, words) in lines.iter().enumerate() {
        let middle = Point::new(center.x, start + line * (i as f32 + 0.5));
        text(out, measure, words, middle, room, ink);
    }
}

#[allow(clippy::too_many_arguments)]
fn line(
    out: &mut String,
    points: &[Point],
    color: &str,
    width: f32,
    dash: Option<&str>,
    radius: f32,
    start: Head,
    end: Head,
) {
    if points.len() < 2 {
        return;
    }
    // The line stops short of a head, as the view's does, so it never
    // crosses a hollow one.
    let length = (width * 4.0).max(8.0);
    let n = points.len();
    let mut ends = points.to_vec();
    ends[0] = toward(points[0], points[1], length * inset(start));
    ends[n - 1] = toward(points[n - 1], points[n - 2], length * inset(end));
    let trimmed = &ends;
    let mut d = format!("M{:.1},{:.1}", trimmed[0].x, trimmed[0].y);
    for i in 1..trimmed.len() {
        let corner = trimmed[i];
        match trimmed.get(i + 1) {
            Some(&next) => {
                let room = trimmed[i - 1].distance(corner).min(corner.distance(next)) / 2.0;
                let r = radius.min(room);
                let a = toward(corner, trimmed[i - 1], r);
                let z = toward(corner, next, r);
                let _ = write!(
                    d,
                    " L{:.1},{:.1} Q{:.1},{:.1} {:.1},{:.1}",
                    a.x, a.y, corner.x, corner.y, z.x, z.y
                );
            },
            None => {
                let _ = write!(d, " L{:.1},{:.1}", corner.x, corner.y);
            },
        }
    }
    let dash = dash
        .map(|d| format!(r#" stroke-dasharray="{d}""#))
        .unwrap_or_default();
    let _ = write!(
        out,
        r#"<path d="{d}" fill="none" stroke="{color}" stroke-width="{width}"{dash}/>"#
    );
    let n = points.len();
    head(out, start, points[0], points[1], color, width);
    head(out, end, points[n - 1], points[n - 2], color, width);
}

/// How far, in head lengths, a line stops short of a head: the view's.
fn inset(head: Head) -> f32 {
    match head {
        Head::None | Head::Cross => 0.0,
        Head::Open => 0.2,
        Head::Circle => 0.9,
        Head::Diamond | Head::HollowDiamond => 1.6,
        Head::Arrow | Head::Triangle => 0.85,
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

/// `color` is already escaped by `line`.
fn head(out: &mut String, kind: Head, tip: Point, from: Point, color: &str, width: f32) {
    let span = tip.distance(from);
    if kind == Head::None || span <= f32::EPSILON {
        return;
    }
    let length = (width * 4.0).max(8.0);
    let half = length * 0.6;
    let (ux, uy) = ((tip.x - from.x) / span, (tip.y - from.y) * (1.0 / span));
    let back = |d: f32| Point::new(tip.x - ux * d, tip.y - uy * d);
    let side = |p: Point, s: f32| Point::new(p.x - uy * s, p.y + ux * s);
    let poly = |out: &mut String, pts: &[Point], filled: bool| {
        let list: Vec<String> = pts
            .iter()
            .map(|p| format!("{:.1},{:.1}", p.x, p.y))
            .collect();
        let fill = if filled {
            color
        } else {
            "none"
        };
        let _ = write!(
            out,
            r#"<polygon points="{}" fill="{fill}" stroke="{color}" stroke-width="{width}"/>"#,
            list.join(" ")
        );
    };
    match kind {
        Head::None => {},
        Head::Arrow => poly(
            out,
            &[tip, side(back(length), half), side(back(length), -half)],
            true,
        ),
        Head::Triangle => poly(
            out,
            &[tip, side(back(length), half), side(back(length), -half)],
            false,
        ),
        Head::Open => {
            let (a, z) = (side(back(length), half), side(back(length), -half));
            let _ = write!(
                out,
                r#"<polyline points="{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}" fill="none" stroke="{color}" stroke-width="{width}"/>"#,
                a.x, a.y, tip.x, tip.y, z.x, z.y
            );
        },
        Head::Diamond | Head::HollowDiamond => {
            let pts = [
                tip,
                side(back(length * 0.8), half * 0.7),
                back(length * 1.6),
                side(back(length * 0.8), -half * 0.7),
            ];
            poly(out, &pts, kind == Head::Diamond);
        },
        Head::Circle => {
            let c = back(length * 0.45);
            let _ = write!(
                out,
                r#"<circle cx="{:.1}" cy="{:.1}" r="{:.1}" fill="none" stroke="{color}" stroke-width="{width}"/>"#,
                c.x,
                c.y,
                length * 0.4
            );
        },
        Head::Cross => {
            let c = back(length * 0.6);
            let d = length * 0.35;
            for s in [d, -d] {
                let (a, z) = (
                    side(Point::new(c.x - ux * d, c.y - uy * d), s),
                    side(Point::new(c.x + ux * d, c.y + uy * d), -s),
                );
                let _ = write!(
                    out,
                    r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{color}" stroke-width="{width}"/>"#,
                    a.x, a.y, z.x, z.y
                );
            }
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use {
        super::*,
        crate::{
            layout::{Approx, Measure as _, Options, layout},
            parse,
        },
    };

    #[test]
    fn a_diagram_exports_as_svg_with_every_part() {
        let d = parse::mermaid::parse("flowchart LR\n a[\"A & B\"] -->|go| b{B?}\n b -.-> c[(C)]\n subgraph g [Group]\n c\n end").unwrap();
        let out = layout(&d, &Approx, &Options::default(), None);
        let svg = svg(
            &d,
            &out,
            &Colors::dark(),
            "Inter",
            &["b".into()],
            &mut approx,
        );
        assert!(svg.starts_with("<svg ") && svg.ends_with("</svg>"));
        assert!(svg.contains("A &amp; B"), "{svg}");
        assert!(svg.contains("<polygon"), "a diamond and an arrowhead");
        assert!(svg.contains("stroke-dasharray=\"6 4\""), "the dashed edge");
        assert!(svg.contains(">go<"), "the edge label");
        assert!(svg.contains(&Colors::dark().accent), "the highlight");
        assert!(svg.contains(">Group<"));
    }

    #[test]
    fn a_fill_that_is_not_a_color_never_reaches_the_svg() {
        let mut d = parse::mermaid::parse("flowchart LR\n a --> b").unwrap();
        d.nodes[0].fill = Some("#\" onload=\"alert(1)".into());
        d.nodes[1].fill = Some("#abc".into());
        let out = layout(&d, &Approx, &Options::default(), None);
        let svg = svg(&d, &out, &Colors::light(), "\"><script>", &[], &mut approx);
        assert!(!svg.contains("onload") && !svg.contains("<script"), "{svg}");
        assert!(svg.contains("fill=\"#abc\""));
    }

    #[test]
    fn a_long_message_to_itself_fits_in_the_export() {
        let d = parse::mermaid::parse(
            "sequenceDiagram\n A->>A: a very long message that the actor sends to itself to check",
        )
        .unwrap();
        let out = layout(&d, &Approx, &Options::default(), None);
        let label = out.messages[0].label.unwrap();
        let svg = svg(&d, &out, &Colors::light(), "Inter", &[], &mut approx);
        let width: f32 = svg
            .split("width=\"")
            .nth(1)
            .and_then(|w| w.split('"').next())
            .unwrap()
            .parse()
            .unwrap();
        let shift = MARGIN - drawn_bounds(&d, &out, &mut approx).x;
        let right = label.x + shift + Approx.width(&d.messages[0].label, 12.0) / 2.0;
        assert!(right <= width, "label ends at {right}, canvas {width}");
        assert!(
            out.bounds.right() + MARGIN * 2.0 < right,
            "the layout alone would cut it"
        );
    }

    #[test]
    fn a_toned_node_is_the_card_tinted_and_a_hollow_head_is_not_crossed() {
        let d = parse::mermaid::parse("classDiagram\n Animal <|-- Duck").unwrap();
        let out = layout(&d, &Approx, &Options::default(), None);
        let svg = svg(&d, &out, &Colors::dark(), "Inter", &[], &mut approx);
        assert_eq!(mix("#000000", "#ffffff", 0.5), "#808080");
        let mut toned = parse::mermaid::parse("flowchart LR\n a --> b").unwrap();
        toned.nodes[0].tone = Some(Tone::Accent);
        let laid = layout(&toned, &Approx, &Options::default(), None);
        let dark = Colors::dark();
        let tinted = svg_of(&toned, &laid, &dark);
        let fill = format!("fill=\"{}\"", mix(&dark.card, &dark.accent, 0.14));
        assert!(tinted.contains(&fill), "{tinted}");
        assert!(!tinted.contains(&format!("{}24", dark.accent)), "{tinted}");
        assert_eq!(mix("#1c1c1f", "not a color", 0.5), "#1c1c1f");
        // The line ends a head's length short of its tip.
        let route = out.edges.values().next().unwrap();
        let tip = route.points[0];
        let path = svg.split("<path d=\"M").nth(1).unwrap();
        let xy: Vec<f32> = path
            .split(' ')
            .next()
            .unwrap()
            .split(',')
            .map(|v| v.parse().unwrap())
            .collect();
        let shift = MARGIN - drawn_bounds(&d, &out, &mut approx).x;
        let shift_y = MARGIN - drawn_bounds(&d, &out, &mut approx).y;
        let gap = Point::new(xy[0], xy[1]).distance(Point::new(tip.x + shift, tip.y + shift_y));
        assert!(gap > 5.0, "the line stops short of the triangle: {gap}");
    }

    fn svg_of(d: &Diagram, out: &Layout, colors: &Colors) -> String {
        svg(d, out, colors, "Inter", &[], &mut approx)
    }

    fn approx(text: &str, size: f32) -> f32 {
        Approx.width(text, size)
    }

    #[test]
    fn a_dark_fill_gets_light_text_and_a_light_fill_dark() {
        let mut d = parse::mermaid::parse("flowchart LR\n a[Night] --> b[Day]").unwrap();
        d.nodes[0].fill = Some("#18181b".into());
        d.nodes[1].fill = Some("#fde68a".into());
        let out = layout(&d, &Approx, &Options::default(), None);
        let svg = svg(&d, &out, &Colors::light(), "Inter", &[], &mut approx);
        assert!(
            svg.contains(r##"fill="#f5f5f5" font-size="14""##) && svg.contains(">Night<"),
            "{svg}"
        );
        assert!(svg.contains(r##"fill="#1f1f1f" font-size="14""##), "{svg}");
    }

    #[test]
    fn a_long_label_wraps_inside_its_box_and_a_longer_one_is_cut() {
        let mut d = parse::mermaid::parse("flowchart LR\n a[x] --> b[y]").unwrap();
        d.nodes[0].label = "one two three four five six".into();
        d.nodes[0].at = Some(Rect::new(0.0, 0.0, 120.0, 80.0));
        d.nodes[1].label = "word ".repeat(40);
        d.nodes[1].at = Some(Rect::new(200.0, 0.0, 120.0, 60.0));
        let out = layout(&d, &Approx, &Options::default(), None);
        let svg = svg(&d, &out, &Colors::light(), "Inter", &[], &mut approx);
        let lines: Vec<&str> = svg
            .split("<text ")
            .skip(1)
            .map(|t| t.split('>').nth(1).unwrap().trim_end_matches("</text"))
            .collect();
        assert!(lines.len() > 3, "wrapped onto lines: {lines:?}");
        for line in &lines {
            assert!(Approx.width(line, 14.0) <= 108.0 + 0.01, "{line:?} fits");
        }
        assert!(lines.iter().any(|l| l.ends_with('…')), "{lines:?}");
    }
}
