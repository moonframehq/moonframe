//! Applying core styles to Rune elements, as `crates/moonframe`'s
//! `render/styling.rs` does to GPUI's.

use {
    crate::theme::Palette,
    moonframe_core::{
        Align, Area, Border, Font, Frame, Resolved, Scroll, Shadow, ShadowPreset, Sides, Style,
        TextSize, Token, Weight,
    },
    rune::{Color, El, Family, ScrollSpec, TextStyle, col, el, row},
};

/// What children inherit: text color, size, weight and typeface.
#[derive(Clone, Copy)]
pub(crate) struct Ink {
    pub(crate) color: Color,
    pub(crate) base: f32,
    pub(crate) size: f32,
    pub(crate) weight: f32,
    pub(crate) mono: bool,
}

pub(crate) fn pixels(size: TextSize, base: f32) -> f32 {
    base * match size {
        TextSize::Small => 0.85,
        TextSize::Normal => 1.0,
        TextSize::Large => 1.2,
        TextSize::Title => 1.5,
        TextSize::Headline => 1.75,
        TextSize::Display => 2.25,
        TextSize::Hero => 3.0,
    }
}

pub(crate) fn weight(weight: Weight) -> f32 {
    match weight {
        Weight::Light => 300.,
        Weight::Normal => 400.,
        Weight::Medium => 500.,
        Weight::Semibold => 600.,
        Weight::Bold => 700.,
    }
}

/// GPUI's default line height, the golden ratio, in sizes of the text.
const LINE_HEIGHT: f32 = 1.618;

impl Ink {
    /// This ink under what `style` sets.
    pub(crate) fn under(self, style: &Style, palette: Palette<'_>) -> Self {
        Self {
            color: style
                .color
                .as_ref()
                .map_or(self.color, |color| palette.color(color)),
            size: style.size.map_or(self.size, |size| pixels(size, self.base)),
            weight: style.weight.map_or(self.weight, weight),
            mono: style.font.map_or(self.mono, |font| font == Font::Mono),
            ..self
        }
    }

    /// As GPUI sets text, so a layout has the same rhythm on both: its line
    /// height, and no tracking, which Rune otherwise adds to system fonts.
    pub(crate) fn text_style(self) -> TextStyle {
        // GPUI's system font has no medium: it draws regular.
        let weight = if self.weight < 600. {
            400.
        } else {
            self.weight
        };
        let mut style = TextStyle::new(self.size, weight);
        style.line_height = self.size * LINE_HEIGHT;
        style.features = "'trak' 0";
        if self.mono {
            style.family = Family::SystemMono;
        }
        style
    }
}

fn padded(mut element: El, sides: &Sides) -> El {
    if let Some(top) = sides.top {
        element = element.pt(top);
    }
    if let Some(right) = sides.right {
        element = element.pr(right);
    }
    if let Some(bottom) = sides.bottom {
        element = element.pb(bottom);
    }
    if let Some(left) = sides.left {
        element = element.pl(left);
    }
    element
}

fn margined(mut element: El, sides: &Sides) -> El {
    if let Some(top) = sides.top {
        element = element.mt(top);
    }
    if let Some(right) = sides.right {
        element = element.mr(right);
    }
    if let Some(bottom) = sides.bottom {
        element = element.mb(bottom);
    }
    if let Some(left) = sides.left {
        element = element.ml(left);
    }
    element
}

fn shadowed(element: El, shadow: &Shadow, palette: Palette<'_>) -> El {
    let black = |alpha| Color::BLACK.alpha(alpha);
    match shadow {
        Shadow::Preset(ShadowPreset::None) => element,
        Shadow::Preset(ShadowPreset::Small) => element.shadow(1., 3., black(0.2)),
        Shadow::Preset(ShadowPreset::Medium) => element.shadow(4., 12., black(0.25)),
        Shadow::Preset(ShadowPreset::Large) => element.shadow(10., 30., black(0.3)),
        // Rune draws one outer shadow from these; the first layer stands in.
        Shadow::Layers(layers) => match layers.iter().find(|layer| !layer.inset) {
            Some(layer) => element.shadow(layer.y, layer.blur, palette.color(&layer.color)),
            None => element,
        },
    }
}

/// A hairline along one edge: Rune's border goes all the way round.
fn edge(side: Border, color: Color) -> Option<El> {
    let line = el().fill(color);
    Some(match side {
        Border::Top => line.pin(Some(0.), Some(0.), Some(0.), None).h(1.),
        Border::Bottom => line.pin(Some(0.), None, Some(0.), Some(0.)).h(1.),
        Border::Left => line.pin(Some(0.), Some(0.), None, Some(0.)).w(1.),
        Border::Right => line.pin(None, Some(0.), Some(0.), Some(0.)).w(1.),
        Border::None | Border::All => return None,
    })
}

/// A resolved look: everything but what the element's frame says.
pub(crate) fn looked(
    mut element: El,
    look: &Resolved,
    border: Option<Border>,
    palette: Palette,
) -> El {
    let style = &look.base;
    if let Some(gradient) = style.gradient {
        element = element.linear_gradient(gradient.angle, [
            (0., palette.color(&gradient.from)),
            (1., palette.color(&gradient.to)),
        ]);
    } else if let Some(background) = &style.background {
        element = element.fill(palette.color(background));
    }
    if let Some(hover) = &look.hover.background {
        element = element.hover(palette.color(hover));
    }
    if let Some(radius) = style.radius {
        element = match radius.uniform() {
            Some(radius) => element.radius(radius),
            None => element.corners([
                radius.top_left,
                radius.top_right,
                radius.bottom_right,
                radius.bottom_left,
            ]),
        };
    }
    element = margined(padded(element, &style.padding), &style.margin);
    if let Some(shadow) = &style.shadow {
        element = shadowed(element, shadow, palette);
    }
    if let Some(opacity) = style.opacity {
        element = element.opacity(opacity);
    }
    if style.clip == Some(true) {
        element = element.clip_rounded(style.radius.map_or(0., |radius| radius.top_left));
    }
    if let Some(width) = style.width {
        element = element.w(width);
    }
    if let Some(height) = style.height {
        element = element.h(height);
    }
    let color = style
        .border_color
        .as_ref()
        .map_or(palette.token(Token::Border), |color| palette.color(color));
    match border.or(style.border) {
        Some(Border::All) => element.border(1., color),
        Some(side) => match edge(side, color) {
            Some(line) => element.child(line),
            None => element,
        },
        None => element,
    }
}

/// What a frame places: size, growth, scrolling and where it floats.
pub(crate) fn framed(mut element: El, frame: &Frame, column: bool, palette: Palette<'_>) -> El {
    if let Some(surface) = frame.surface {
        element = element.fill(palette.surface(surface));
    }
    if let Some(padding) = frame.padding {
        element = element.padding(padding.pixels());
    }
    // Shrinking past its content is what lets anything inside it scroll.
    if frame.shrink {
        element = element.min_w(0.).min_h(0.);
    }
    if frame.grow {
        element = element.grow(1.);
    }
    if let Some(width) = frame.width {
        element = element.w(width);
    }
    if let Some(height) = frame.height {
        element = element.h(height);
    }
    if let Some(width) = frame.max_width {
        element = element.max_w(width);
    }
    if let Some(height) = frame.max_height {
        element = element.max_h(height);
    }
    element = match frame.self_align {
        Some(Align::Start) => element.self_start(),
        Some(Align::Center) => element.self_center(),
        Some(Align::End) => element.self_end(),
        Some(Align::Stretch) => element.self_stretch(),
        None => element,
    };
    if let Some(float) = frame.float {
        element = element.pin(float.left, float.top, float.right, float.bottom);
    }
    let spec = ScrollSpec {
        stick_bottom: frame.follow,
    };
    let element = match (frame.scroll, column) {
        (Scroll::No, _) => element,
        (Scroll::Along, true) | (Scroll::Y, _) => element.min_h(0.).scroll(spec),
        (Scroll::Along, false) | (Scroll::X, _) => element.min_w(0.).scroll_x(),
        (Scroll::Both, _) => element.min_w(0.).min_h(0.).scroll_xy(),
    };
    match frame.area {
        Some(area) => in_area(element, area),
        None => element,
    }
}

/// `element` over its parent at fractions of its size. Rune pins in points
/// only, so a row and a column of flexible gaps put it there.
fn in_area(element: El, area: Area) -> El {
    let gap = |share: f32| el().grow(share.max(0.));
    // Without a size it keeps its own, starting there.
    let span = |start: Option<f32>, size: Option<f32>| {
        let start = start.unwrap_or(0.);
        (start, 1. - start - size.unwrap_or(0.))
    };
    let sized = |host: El, size: Option<f32>| match size {
        Some(size) => host.grow(size.max(f32::EPSILON)),
        None => host,
    };
    let (left, right) = span(area.left, area.width);
    let (top, bottom) = span(area.top, area.height);
    let element = if area.height.is_some() {
        element.grow(1.)
    } else {
        element
    };
    let across = sized(row(), area.height)
        .child(gap(left))
        .child(sized(col(), area.width).child(element))
        .child(gap(right));
    col()
        .pin(Some(0.), Some(0.), Some(0.), Some(0.))
        .child(gap(top))
        .child(across)
        .child(gap(bottom))
}
