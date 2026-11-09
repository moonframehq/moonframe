use {
    super::{geometry::Tick, text},
    gpui_kit::{component::ActiveTheme as _, *},
    moonframe_core::{RevisionTimeline, TimelineLabels, TimelineSide},
};

#[allow(clippy::too_many_arguments)]
pub(super) fn ruler(
    model: &RevisionTimeline,
    ticks: &[Tick],
    selected: Option<usize>,
    hovered: Option<usize>,
    area: Bounds<Pixels>,
    accent: Hsla,
    window: &mut Window,
    cx: &mut App,
) {
    let muted = cx.theme().muted_foreground;
    let foreground = cx.theme().foreground;
    let alpha = if model.disabled {
        0.4
    } else {
        1.0
    };
    let left = model.side == TimelineSide::Left;
    let edge = if left {
        area.left() + px(8.0)
    } else {
        area.right() - px(8.0)
    };
    let available = f32::from(area.size.width).max(0.0);
    let detail = match model.labels {
        TimelineLabels::None => None,
        TimelineLabels::Hover => hovered,
        TimelineLabels::Active | TimelineLabels::All => hovered.or(selected),
    };
    let detail_y = ticks
        .iter()
        .find(|tick| Some(tick.index) == detail)
        .map(|tick| tick.y);
    if ticks.is_empty() {
        text::label(
            "No revisions",
            edge,
            area.center().y,
            available - 16.0,
            left,
            muted,
            window,
            cx,
        );
        return;
    }
    let mut last_label = -100.0;
    let mut preview = None;
    for tick in ticks {
        let entry = &model.entries[tick.index];
        let major = major(model, tick.index);
        let chosen = selected == Some(tick.index);
        let hot = hovered == Some(tick.index);
        let width = tick_width(model, tick, selected).min((available - 16.0).max(0.0));
        let thickness = model.metrics.tick_height
            * (if chosen {
                2.0
            } else {
                1.0
            } + (model.metrics.magnification - 1.0) / 1.4 * tick.influence);
        let thickness = thickness.min(tick.slot * 0.8);
        let y = area.top() + px(tick.y);
        let color = if chosen {
            accent
        } else if hot {
            foreground
        } else {
            muted.opacity(0.35 + 0.55 * tick.influence)
        };
        let x = if left {
            edge
        } else {
            edge - px(width)
        };
        window.paint_quad(
            fill(
                Bounds::new(
                    point(x, y - px(thickness / 2.0)),
                    size(px(width), px(thickness)),
                ),
                color.opacity(alpha),
            )
            .corner_radii(px(1.0)),
        );
        let is_detail = detail == Some(tick.index);
        let clear = detail_y.is_none_or(|detail| (detail - tick.y).abs() > 36.0);
        let day_label =
            major && model.labels == TimelineLabels::All && clear && tick.y - last_label > 28.0;
        if !is_detail && !day_label {
            continue;
        }
        // Both detail lines must clear the longest neighbouring tick.
        let label_width = if is_detail && model.preview {
            rail_width(model, ticks, selected, available)
        } else if is_detail {
            ticks
                .iter()
                .filter(|near| (near.y - tick.y).abs() < 26.0)
                .map(|near| tick_width(model, near, selected))
                .fold(width, f32::max)
        } else {
            width
        };
        let anchor = if left {
            edge + px(label_width + 12.0)
        } else {
            edge - px(label_width + 12.0)
        };
        let room = f32::from(if left {
            area.right() - anchor
        } else {
            anchor - area.left()
        }) - 6.0;
        let color = if chosen {
            accent
        } else {
            foreground
        }
        .opacity(alpha);
        if is_detail && model.preview {
            preview = Some((tick.index, anchor, y, room));
        } else {
            let day = entry.day.as_deref().filter(|_| model.dates);
            if let Some(day) = day {
                text::label(
                    day,
                    anchor,
                    y - px(if is_detail {
                        8.0
                    } else {
                        0.0
                    }),
                    room,
                    left,
                    color,
                    window,
                    cx,
                );
            }
            if is_detail {
                text::label(
                    &entry.label,
                    anchor,
                    y + px(if day.is_some() {
                        9.0
                    } else {
                        0.0
                    }),
                    room,
                    left,
                    muted.opacity(alpha),
                    window,
                    cx,
                );
            }
        }
        last_label = tick.y;
    }
    // Cards cover nearby labels, but never the rail or a later-painted tick.
    if let Some((index, anchor, y, room)) = preview {
        text::card(
            &model.entries[index],
            model.dates,
            anchor,
            y,
            room,
            left,
            area,
            window,
            cx,
        );
    }
}

/// The longest painted tick: shared by preview placement and rail hit testing.
pub(super) fn rail_width(
    model: &RevisionTimeline,
    ticks: &[Tick],
    selected: Option<usize>,
    available: f32,
) -> f32 {
    ticks
        .iter()
        .map(|tick| tick_width(model, tick, selected))
        .fold(0.0, f32::max)
        .min((available - 16.0).max(0.0))
}

fn major(model: &RevisionTimeline, index: usize) -> bool {
    model.dates
        && model.entries[index].day.is_some()
        && (index == 0 || model.entries[index - 1].day != model.entries[index].day)
}

fn tick_width(model: &RevisionTimeline, tick: &Tick, selected: Option<usize>) -> f32 {
    let scale = if selected == Some(tick.index) {
        8.0 / 3.0
    } else if major(model, tick.index) {
        2.0
    } else {
        1.0
    };
    model.metrics.tick_width * (scale + (model.metrics.magnification - 1.0) * tick.influence)
}
