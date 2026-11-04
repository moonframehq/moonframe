//! Measurements in Inspect, as designers' tools draw them: redlines to
//! the nearest neighbours, rulers and guides, a devtools-style box model,
//! or the distance between the selection and another element. The style is
//! picked on the banner while picking (or with 1 to 5) and kept between runs.

use {
    crate::{app::MoonframeApp, inspect::Placed},
    gpui_kit::{
        component::{
            ActiveTheme as _, Selectable as _, Sizable as _,
            button::{Button, ButtonGroup},
        },
        *,
    },
    std::{cell::RefCell, rc::Rc},
};

mod geometry;
mod paint;

/// The redline red designers know, the box model's gap orange and padding
/// green, and the pink of alignment guides: fixed, as in design tools, so
/// they read the same in every theme.
const RED: u32 = 0xf43f5e;
const ORANGE: u32 = 0xf97316;
const GREEN: u32 = 0x22c55e;
const PINK: u32 = 0xec4899;

/// How the inspector measures. Its order is the switch's, and 1 to 5's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Measure {
    Off,
    /// Lines to the nearest neighbour or container edge on each side.
    #[default]
    Redlines,
    /// Rulers along the window with the element's extent, and guides.
    Rulers,
    /// The space around the element and its own padding, shaded.
    Box,
    /// From the selection to the element under the pointer.
    Between,
}

impl Measure {
    /// Each style with its saved name and its label, in the switch's order.
    pub(crate) const ALL: &[(Self, &str, &str)] = &[
        (Self::Off, "off", "Off"),
        (Self::Redlines, "redlines", "Redlines"),
        (Self::Rulers, "rulers", "Rulers"),
        (Self::Box, "box", "Box"),
        (Self::Between, "between", "Between"),
    ];

    pub(crate) fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(style, ..)| *style == self)
            .map_or("off", |(_, name, _)| name)
    }

    pub(crate) fn named(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(_, saved, _)| *saved == name)
            .map(|(style, ..)| *style)
    }

    /// The style a digit key picks: 1 for the first.
    pub(crate) fn numbered(key: &str) -> Option<Self> {
        let index = key.parse::<usize>().ok()?.checked_sub(1)?;
        Self::ALL.get(index).map(|(style, ..)| *style)
    }
}

/// The chosen style, and what the pointer is over while a selection
/// shows, for `Between`; the layer's own listener keeps it.
#[derive(Default)]
pub(crate) struct Measuring {
    pub(crate) style: Measure,
    pointed: Rc<RefCell<Option<String>>>,
}

/// The theme's colours the layer paints with.
#[derive(Clone, Copy)]
struct Colors {
    accent: Hsla,
    accent_text: Hsla,
    surface: Hsla,
    border: Hsla,
    muted: Hsla,
}

impl MoonframeApp {
    /// The measurements over the hovered element while picking, else over
    /// the selection; nothing while the style is off.
    pub(super) fn measure_layer(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let inspector = &self.inspector;
        let style = inspector.measuring.style;
        let subject = if inspector.picking {
            inspector.hovered.as_ref().map(|target| &target.placed)
        } else {
            inspector.selected.as_ref()
        };
        let subject = subject.map(|placed| placed.path.clone());
        if style == Measure::Off || subject.is_none() {
            return None;
        }
        // Between measures from the selection, once there is one.
        let between = style == Measure::Between && !inspector.picking;
        let theme = cx.theme();
        let colors = Colors {
            accent: theme.primary,
            accent_text: theme.primary_foreground,
            surface: theme.popover,
            border: theme.border,
            muted: theme.muted_foreground.opacity(0.6),
        };
        let recorded = inspector.recorded();
        let pointed = inspector.measuring.pointed.clone();
        Some(
            deferred(
                canvas(
                    |_, _, _| (),
                    move |area, _, window, cx| {
                        if between {
                            follow_pointer(&recorded, &pointed, window);
                        }
                        let recorded = recorded.borrow();
                        let Some(subject) =
                            subject.as_deref().and_then(|path| find(&recorded, path))
                        else {
                            return;
                        };
                        let others: Vec<Bounds<Pixels>> =
                            recorded.iter().map(|placed| placed.bounds).collect();
                        let pointed = pointed.borrow();
                        let other = pointed
                            .as_deref()
                            .filter(|path| *path != subject.path)
                            .and_then(|path| find(&recorded, path));
                        draw(style, subject, other, &others, area, colors, window, cx);
                    },
                )
                .absolute()
                .inset_0(),
            )
            // Over the highlight, under the picker, label and card.
            .with_priority(crate::render::INSPECTOR_LAYER)
            .into_any_element(),
        )
    }

    /// The banner's switch between the styles.
    pub(super) fn measure_switch(&self, cx: &Context<Self>) -> AnyElement {
        let chosen = self.inspector.measuring.style;
        let app = cx.entity().downgrade();
        div()
            .id("inspect-measure")
            // Its clicks are its own, not picks of what is under it.
            .occlude()
            .child(
                ButtonGroup::new("inspect-measure-styles")
                    .xsmall()
                    .compact()
                    .outline()
                    .children(
                        Measure::ALL
                            .iter()
                            .enumerate()
                            .map(|(index, (style, _, label))| {
                                Button::new(("inspect-measure", index))
                                    .label(*label)
                                    .selected(*style == chosen)
                                    .tooltip(format!("Measure: {label} ({})", index + 1))
                            }),
                    )
                    .on_click(move |clicked: &Vec<usize>, _, cx| {
                        let Some(style) =
                            clicked.first().and_then(|index| Measure::ALL.get(*index))
                        else {
                            return;
                        };
                        let _ = app.update(cx, |this, cx| this.set_measure(style.0, cx));
                    }),
            )
            .into_any_element()
    }
}

fn find<'a>(recorded: &'a [Placed], path: &str) -> Option<&'a Placed> {
    recorded.iter().find(|placed| placed.path == path)
}

/// Keeps `pointed` on the deepest element under the pointer as it moves,
/// without taking the pointer from the window: clicks still reach the app.
fn follow_pointer(
    recorded: &crate::inspect::Sink,
    pointed: &Rc<RefCell<Option<String>>>,
    window: &mut Window,
) {
    let recorded = recorded.clone();
    let pointed = pointed.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
        if phase != DispatchPhase::Bubble {
            return;
        }
        let under = recorded
            .borrow()
            .iter()
            .rev()
            .find(|placed| placed.bounds.contains(&event.position))
            .map(|placed| placed.path.clone());
        if *pointed.borrow() != under {
            *pointed.borrow_mut() = under;
            window.refresh();
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn draw(
    style: Measure,
    subject: &Placed,
    other: Option<&Placed>,
    others: &[Bounds<Pixels>],
    area: Bounds<Pixels>,
    colors: Colors,
    window: &mut Window,
    cx: &mut App,
) {
    let red: Hsla = rgb(RED).into();
    let white: Hsla = rgb(0xffffff).into();
    let bounds = subject.bounds;
    match style {
        Measure::Off => {},
        Measure::Redlines => {
            for span in geometry::gaps(bounds, others, area) {
                paint::measured(&span, red, white, window, cx);
            }
        },
        Measure::Rulers => {
            let ruler = paint::RulerColors {
                surface: colors.surface,
                border: colors.border,
                ticks: colors.muted,
                accent: colors.accent,
                accent_text: colors.accent_text,
            };
            paint::rulers(bounds, area, &ruler, window, cx);
        },
        Measure::Box => {
            for span in geometry::gaps(bounds, others, area) {
                paint::shaded(&span, rgb(ORANGE).into(), window, cx);
            }
            for span in geometry::padding(bounds, others) {
                paint::shaded(&span, rgb(GREEN).into(), window, cx);
            }
        },
        Measure::Between => {
            let Some(other) = other else {
                return;
            };
            for guide in geometry::aligned(bounds, other.bounds) {
                paint::guide(guide.vertical, guide.at, area, rgb(PINK).into(), window);
            }
            paint::outline(other.bounds, other.corners, red, window);
            for span in geometry::between(bounds, other.bounds) {
                paint::measured(&span, red, white, window, cx);
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::Measure;

    #[test]
    fn styles_round_trip_through_their_saved_names() {
        for (style, name, _) in Measure::ALL {
            assert_eq!(style.name(), *name);
            assert_eq!(Measure::named(name), Some(*style));
        }
        assert_eq!(Measure::named("nope"), None);
    }

    #[test]
    fn digits_pick_styles_in_the_switch_order() {
        assert_eq!(Measure::numbered("1"), Some(Measure::ALL[0].0));
        assert_eq!(Measure::numbered("0"), None);
        assert_eq!(Measure::numbered("x"), None);
        let past = (Measure::ALL.len() + 1).to_string();
        assert_eq!(Measure::numbered(&past), None);
    }
}
