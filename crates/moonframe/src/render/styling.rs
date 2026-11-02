//! Applying core styles to GPUI elements, as methods that chain like GPUI's
//! own: `div().look(&look, theme).placed(&frame, &look.base, theme)`.

use {
    crate::color::ThemeColor,
    gpui_kit::{
        component::{
            Theme,
            scroll::{Scrollbar, ScrollbarAxis},
        },
        *,
    },
    moonframe_core::{
        Align, Border, Corners, Direction, Font, Frame, Gradient, Resolved, Scroll, Shadow,
        ShadowPreset, Sides, Space, Style, TextSize, Weight,
    },
};

/// A text size in pixels: sizes scale from the theme's font size, which a
/// stylesheet can set. No size is the theme's.
pub(super) fn text_pixels(size: Option<TextSize>, theme: &Theme) -> Pixels {
    let scale = match size {
        Some(TextSize::Small) => 0.85,
        None | Some(TextSize::Normal) => 1.0,
        Some(TextSize::Large) => 1.2,
        Some(TextSize::Title) => 1.5,
        Some(TextSize::Headline) => 1.75,
        Some(TextSize::Display) => 2.25,
        Some(TextSize::Hero) => 3.0,
    };
    theme.font_size * scale
}

/// A gradient as GPUI paints it.
fn paint(gradient: Gradient, theme: &Theme) -> Background {
    linear_gradient(
        gradient.angle,
        linear_color_stop(gradient.from.hsla(theme), 0.0),
        linear_color_stop(gradient.to.hsla(theme), 1.0),
    )
}

/// Styles that apply to any element.
pub(super) trait StyleExt: Styled + Sized {
    fn text_scaled(self, size: TextSize, theme: &Theme) -> Self {
        self.text_size(text_pixels(Some(size), theme))
    }

    fn weighted(self, weight: Weight) -> Self {
        self.font_weight(match weight {
            Weight::Light => FontWeight::LIGHT,
            Weight::Normal => FontWeight::NORMAL,
            Weight::Medium => FontWeight::MEDIUM,
            Weight::Semibold => FontWeight::SEMIBOLD,
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
        match style.font {
            Some(Font::Mono) => self.font_family(theme.mono_font_family.clone()),
            Some(Font::Sans) => self.font_family(theme.font_family.clone()),
            None => self,
        }
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

    /// Rounds each corner by its radius.
    fn cornered(self, corners: Corners) -> Self {
        match corners.uniform() {
            Some(radius) => self.rounded(px(radius)),
            None => self
                .rounded_tl(px(corners.top_left))
                .rounded_tr(px(corners.top_right))
                .rounded_br(px(corners.bottom_right))
                .rounded_bl(px(corners.bottom_left)),
        }
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
        if let Some(gradient) = style.gradient {
            self = self.bg(paint(gradient, theme));
        }
        self = self.typography(style, theme).margined(&style.margin);
        if let Some(radius) = style.radius {
            self = self.cornered(radius);
        }
        if let Some(shadow) = &style.shadow {
            self = self.shadowed(shadow, theme);
        }
        if let Some(opacity) = style.opacity {
            self = self.opacity(opacity);
        }
        if style.clip == Some(true) {
            self = self.overflow_hidden();
        }
        // GPUI takes one hover style per element: everything goes in it.
        let hover = &look.hover;
        let background = hover.background.map(|color| color.hsla(theme));
        let gradient = hover.gradient.map(|gradient| paint(gradient, theme));
        let color = hover.color.map(|color| color.hsla(theme));
        let border = hover.border_color.map(|color| color.hsla(theme));
        if background.is_none() && gradient.is_none() && color.is_none() && border.is_none() {
            return self;
        }
        self.hover(move |mut style| {
            if let Some(background) = background {
                style = style.bg(background);
            }
            if let Some(gradient) = gradient {
                style = style.bg(gradient);
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
        self = if frame.shrink {
            self.min_w_0().min_h_0()
        } else {
            self.flex_shrink_0()
        };
        self = match frame.self_align {
            None => self,
            Some(Align::Stretch) => self.self_stretch(),
            Some(Align::Start) => self.self_start(),
            Some(Align::Center) => self.self_center(),
            Some(Align::End) => self.self_end(),
        };
        // A fixed size is the basis to grow from, not nothing.
        let sized =
            frame.width.or(style.width).is_some() || frame.height.or(style.height).is_some();
        if frame.grow && !sized {
            self = self.flex_1();
        } else if frame.grow {
            self = self.flex_grow(1.);
        }
        if let Some(width) = frame.max_width {
            self = self.max_w(px(width));
        }
        if let Some(height) = frame.max_height {
            self = self.max_h(px(height));
        }
        if let Some(offsets) = frame.float {
            self = self.absolute();
            if let Some(top) = offsets.top {
                self = self.top(px(top));
            }
            if let Some(right) = offsets.right {
                self = self.right(px(right));
            }
            if let Some(bottom) = offsets.bottom {
                self = self.bottom(px(bottom));
            }
            if let Some(left) = offsets.left {
                self = self.left(px(left));
            }
        }
        if let Some(area) = frame.area {
            self = self.absolute();
            if let Some(left) = area.left {
                self = self.left(relative(left));
            }
            if let Some(top) = area.top {
                self = self.top(relative(top));
            }
            if let Some(width) = area.width {
                self = self.w(relative(width));
            }
            if let Some(height) = area.height {
                self = self.h(relative(height));
            }
        }
        // The element's own size, else a rule's, such as a named state's.
        if let Some(width) = frame.width.or(style.width) {
            self = self.w(px(width)).flex_shrink_0();
        }
        if let Some(height) = frame.height.or(style.height) {
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
        let element = match direction {
            Direction::Row => element.flex_row(),
            Direction::Column => element.flex_col(),
        };
        match (direction, frame.scroll) {
            (_, Scroll::No) => element,
            (Direction::Row, Scroll::Along) | (_, Scroll::X) => element.overflow_x_scroll(),
            (Direction::Column, Scroll::Along) | (_, Scroll::Y) => element.overflow_y_scroll(),
            (_, Scroll::Both) => element.overflow_scroll(),
        }
    }

    /// A scrolling frame's scrollbars, shown while it scrolls and faded out
    /// after, kept at `handle`. Add them after the children, to draw over
    /// them. They hang from a box of no size where the content starts, so
    /// they add nothing to what scrolls, and take the frame's bounds from
    /// `handle`.
    fn scrollbars(
        self,
        frame: &Frame,
        direction: Direction,
        handle: &ScrollHandle,
        id: ElementId,
    ) -> Self
    where
        Self: ParentElement,
    {
        if frame.follow && frame.scroll != Scroll::No {
            follow(handle);
        }
        let axis = match (direction, frame.scroll) {
            (_, Scroll::No) => return self,
            (Direction::Row, Scroll::Along) | (_, Scroll::X) => ScrollbarAxis::Horizontal,
            (Direction::Column, Scroll::Along) | (_, Scroll::Y) => ScrollbarAxis::Vertical,
            (_, Scroll::Both) => ScrollbarAxis::Both,
        };
        self.track_scroll(handle).child(
            div()
                .absolute()
                .size_0()
                .child(Scrollbar::new(handle).id(id).axis(axis)),
        )
    }
}

impl<E: StyleExt + StatefulInteractiveElement> FrameExt for E {}

/// Keeps a frame with `follow` at its end while it is there: as the content
/// grows, it scrolls along. Where the handle last laid out says whether the
/// user is at the end; a frame laid out for the first time starts there.
fn follow(handle: &ScrollHandle) {
    /// How near the end still counts as there, for the last line's rounding.
    const SLACK: f32 = 24.;
    let offset = handle.offset();
    let max = handle.max_offset();
    let at_end = -offset.y >= max.y - px(SLACK) && -offset.x >= max.x - px(SLACK);
    if at_end {
        handle.scroll_to_bottom();
    }
}
