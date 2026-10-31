//! Applying core styles to GPUI elements, as methods that chain like GPUI's
//! own: `div().look(&look, theme).placed(&frame, &look.base, theme)`.

use {
    crate::color::ThemeColor,
    gpui_kit::{component::Theme, *},
    moonframe_core::{
        Border, Direction, Frame, Resolved, Shadow, ShadowPreset, Sides, Space, Style, TextSize,
        Weight,
    },
};

/// Styles that apply to any element.
pub(super) trait StyleExt: Styled + Sized {
    /// Sizes scale from the theme's font size, which a stylesheet can set.
    fn text_scaled(self, size: TextSize, theme: &Theme) -> Self {
        let scale = match size {
            TextSize::Small => 0.85,
            TextSize::Normal => 1.0,
            TextSize::Large => 1.2,
            TextSize::Title => 1.5,
        };
        self.text_size(theme.font_size * scale)
    }

    fn weighted(self, weight: Weight) -> Self {
        self.font_weight(match weight {
            Weight::Normal => FontWeight::NORMAL,
            Weight::Medium => FontWeight::MEDIUM,
            Weight::Bold => FontWeight::BOLD,
        })
    }

    /// Text color, size, and weight, which children inherit.
    fn typography(mut self, style: &Style, theme: &Theme) -> Self {
        if let Some(color) = style.color {
            self = self.text_color(color.hsla(theme));
        }
        if let Some(size) = style.size {
            self = self.text_scaled(size, theme);
        }
        if let Some(weight) = style.weight {
            self = self.weighted(weight);
        }
        self
    }

    /// The edges `border` names, in the style's border color.
    fn edges(self, border: Option<Border>, style: &Style, theme: &Theme) -> Self {
        let element = match border {
            None => self,
            Some(Border::None) => self.border_0(),
            Some(Border::All) => self.border_1(),
            Some(Border::Top) => self.border_0().border_t_1(),
            Some(Border::Bottom) => self.border_0().border_b_1(),
            Some(Border::Left) => self.border_0().border_l_1(),
            Some(Border::Right) => self.border_0().border_r_1(),
        };
        match (style.border_color, border) {
            (Some(color), _) => element.border_color(color.hsla(theme)),
            (None, Some(_)) => element.border_color(theme.border),
            (None, None) => element,
        }
    }

    /// Padding on each edge the sides set.
    fn padded(self, sides: &Sides) -> Self {
        let edges: [(Option<f32>, fn(Self, Pixels) -> Self); 4] = [
            (sides.top, Self::pt),
            (sides.right, Self::pr),
            (sides.bottom, Self::pb),
            (sides.left, Self::pl),
        ];
        apply(self, edges)
    }

    /// Margin on each edge the sides set.
    fn margined(self, sides: &Sides) -> Self {
        let edges: [(Option<f32>, fn(Self, Pixels) -> Self); 4] = [
            (sides.top, Self::mt),
            (sides.right, Self::mr),
            (sides.bottom, Self::mb),
            (sides.left, Self::ml),
        ];
        apply(self, edges)
    }

    fn shadowed(self, shadow: &Shadow, theme: &Theme) -> Self {
        match shadow {
            Shadow::Preset(ShadowPreset::None) => self.shadow_none(),
            Shadow::Preset(ShadowPreset::Small) => self.shadow_sm(),
            Shadow::Preset(ShadowPreset::Medium) => self.shadow_md(),
            Shadow::Preset(ShadowPreset::Large) => self.shadow_lg(),
            Shadow::Layers(layers) => self.shadow(
                layers
                    .iter()
                    .map(|layer| BoxShadow {
                        color: layer.color.hsla(theme),
                        offset: point(px(layer.x), px(layer.y)),
                        blur_radius: px(layer.blur),
                        spread_radius: px(layer.spread),
                        inset: layer.inset,
                    })
                    .collect(),
            ),
        }
    }
}

impl<E: Styled> StyleExt for E {}

fn apply<E>(element: E, edges: [(Option<f32>, fn(E, Pixels) -> E); 4]) -> E {
    edges
        .into_iter()
        .fold(element, |element, (value, set)| match value {
            Some(value) => set(element, px(value)),
            None => element,
        })
}

/// A resolved look, which needs pointer events for its hover style.
pub(super) trait LookExt: StyleExt + InteractiveElement {
    /// What a stylesheet sets on any element, at rest and under the pointer.
    /// Spacing and borders are left to [`FrameExt`] and
    /// [`StyleExt::edges`], which weigh them against the element's own keys.
    fn look(mut self, look: &Resolved, theme: &Theme) -> Self {
        let style = &look.base;
        if let Some(color) = style.background {
            self = self.bg(color.hsla(theme));
        }
        self = self.typography(style, theme).margined(&style.margin);
        if let Some(radius) = style.radius {
            self = self.rounded(px(radius));
        }
        if let Some(shadow) = &style.shadow {
            self = self.shadowed(shadow, theme);
        }
        if let Some(opacity) = style.opacity {
            self = self.opacity(opacity);
        }
        // GPUI takes one hover style per element: everything goes in it.
        let hover = &look.hover;
        let background = hover.background.map(|color| color.hsla(theme));
        let color = hover.color.map(|color| color.hsla(theme));
        let border = hover.border_color.map(|color| color.hsla(theme));
        if background.is_none() && color.is_none() && border.is_none() {
            return self;
        }
        self.hover(move |mut style| {
            if let Some(background) = background {
                style = style.bg(background);
            }
            if let Some(color) = color {
                style = style.text_color(color);
            }
            if let Some(border) = border {
                style = style.border_color(border);
            }
            style
        })
    }
}

impl<E: StyleExt + InteractiveElement> LookExt for E {}

/// A container's frame: how it sits in its parent and lays out children.
pub(super) trait FrameExt: StyleExt + StatefulInteractiveElement {
    /// Size, growth, surface, and border, the frame's keys winning over the
    /// style's.
    fn placed(mut self, frame: &Frame, style: &Style, theme: &Theme) -> Self {
        self = self.min_w_0().min_h_0();
        if frame.grow {
            self = self.flex_1();
        }
        if let Some(width) = frame.width {
            self = self.w(px(width)).flex_shrink_0();
        }
        if let Some(height) = frame.height {
            self = self.h(px(height)).flex_shrink_0();
        }
        if let Some(surface) = frame.surface {
            self = self.bg(surface.hsla(theme));
        }
        self.edges(frame.border.or(style.border), style, theme)
    }

    /// Direction, gap, padding, and scrolling: the frame's own `gap` and
    /// `padding`, else the style's, else these defaults.
    fn arranged(
        self,
        frame: &Frame,
        style: &Style,
        direction: Direction,
        (gap, padding): (Space, Space),
    ) -> Self {
        let gap = frame
            .gap
            .map_or_else(|| style.gap.unwrap_or(gap.pixels()), Space::pixels);
        // The default on every edge, then the style's edges, then the
        // element's own `padding`, which sets them all.
        let mut element = self
            .gap(px(gap))
            .p(px(padding.pixels()))
            .padded(&style.padding);
        if let Some(padding) = frame.padding {
            element = element.p(px(padding.pixels()));
        }
        match (direction, frame.scroll) {
            (Direction::Row, false) => element.flex_row(),
            (Direction::Row, true) => element.flex_row().overflow_x_scroll(),
            (Direction::Column, false) => element.flex_col(),
            (Direction::Column, true) => element.flex_col().overflow_y_scroll(),
        }
    }
}

impl<E: StyleExt + StatefulInteractiveElement> FrameExt for E {}
