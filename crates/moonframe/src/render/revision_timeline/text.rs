//! Labels and optional, noninteractive preview cards inside the ruler's box.

use {
    gpui_kit::{component::ActiveTheme as _, *},
    moonframe_core::Revision,
};

fn run(text: &str, color: Hsla, cx: &App) -> TextRun {
    TextRun {
        len: text.len(),
        font: font(cx.theme().font_family.clone()),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn label(
    text: &str,
    anchor: Pixels,
    y: Pixels,
    room: f32,
    left: bool,
    color: Hsla,
    window: &mut Window,
    cx: &mut App,
) {
    if room <= 0.0 {
        return;
    }
    // Labels stay on one line even when supplied from multiline chat text.
    // Keep the original record intact for grouping and selection messages.
    let text = text.replace(['\r', '\n'], " ");
    let runs = [run(&text, color, cx)];
    let line = window
        .text_system()
        .shape_line(text.into(), px(11.0), &runs, None);
    let width = line.width.min(px(room));
    let x = if left {
        anchor
    } else {
        anchor - width
    };
    window.with_content_mask(
        Some(ContentMask {
            bounds: Bounds::new(point(x, y - px(8.0)), size(width, px(17.0))),
        }),
        |window| {
            let _ = line.paint(
                point(x, y - px(8.0)),
                px(17.0),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        },
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn card(
    entry: &Revision,
    dates: bool,
    anchor: Pixels,
    y: Pixels,
    room: f32,
    left: bool,
    area: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let width = room.min(320.0);
    if width < 48.0 {
        return;
    }
    let text_width = width - 20.0;
    let theme = cx.theme();
    let foreground = theme.foreground;
    let muted = theme.muted_foreground;
    let background = theme.popover;
    let border = theme.border;
    let day = entry.day.as_deref().filter(|_| dates);
    let summary = entry.summary.as_deref().unwrap_or("");
    let lines = window
        .text_system()
        .shape_text(
            summary.to_owned().into(),
            px(11.0),
            &[run(summary, muted, cx)],
            Some(px(text_width)),
            Some(3),
        )
        .unwrap_or_default();
    let body_height: Pixels = lines.iter().map(|line| line.size(px(17.0)).height).sum();
    let height = px(34.0
        + if day.is_some() {
            17.0
        } else {
            0.0
        })
        + body_height;
    let x = if left {
        anchor
    } else {
        anchor - px(width)
    };
    let top = (y - height / 2.0)
        .max(area.top() + px(4.0))
        .min((area.bottom() - height - px(4.0)).max(area.top()));
    let bounds = Bounds::new(point(x, top), size(px(width), height));
    window.paint_quad(
        fill(bounds, background)
            .corner_radii(px(9.0))
            .border_widths(px(1.0))
            .border_color(border),
    );
    let mut text_y = top + px(17.0);
    if let Some(day) = day {
        label(
            day,
            x + px(10.0),
            text_y,
            text_width,
            true,
            muted,
            window,
            cx,
        );
        text_y += px(17.0);
    }
    label(
        &entry.label,
        x + px(10.0),
        text_y,
        text_width,
        true,
        foreground,
        window,
        cx,
    );
    let mut origin = point(x + px(10.0), text_y + px(12.0));
    for line in lines {
        let _ = line.paint(origin, px(17.0), TextAlign::Left, None, window, cx);
        origin.y += line.size(px(17.0)).height;
    }
}
