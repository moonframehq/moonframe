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
    let shape = |text: &str, window: &mut Window| {
        window.text_system().shape_line(
            text.to_owned().into(),
            px(11.0),
            &[run(text, color, cx)],
            None,
        )
    };
    // Cut between words with an ellipsis rather than mid-letter at the edge.
    let text = clamp_words(&text, |text| shape(text, window).width <= px(room));
    let line = shape(&text, window);
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

/// Lines of summary a card shows.
const SUMMARY_LINES: usize = 3;

/// `text`, or its longest start that `fits` once an ellipsis ends it, cut
/// between words; just the ellipsis when not even one word does. Searches
/// the word ends by halves, so a long summary costs a few checks.
pub(super) fn clamp_words(text: &str, mut fits: impl FnMut(&str) -> bool) -> String {
    if fits(text) {
        return text.to_owned();
    }
    let ends: Vec<usize> = text
        .char_indices()
        .filter(|(_, c)| c.is_whitespace())
        .map(|(index, _)| index)
        .filter(|&index| index > 0 && !text[..index].trim_end().is_empty())
        .collect();
    let cut = |end: usize| format!("{}\u{2026}", text[..end].trim_end());
    let (mut low, mut high) = (0, ends.len());
    while low < high {
        let middle = (low + high).div_ceil(2);
        if fits(&cut(ends[middle - 1])) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    match low {
        0 => "\u{2026}".to_owned(),
        count => cut(ends[count - 1]),
    }
}

/// How many lines `lines` take once wrapped.
fn wrapped_lines(lines: &[WrappedLine]) -> usize {
    lines
        .iter()
        .map(|line| line.wrap_boundaries().len() + 1)
        .sum()
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
    let shape = |text: &str, window: &mut Window| {
        window
            .text_system()
            .shape_text(
                text.to_owned().into(),
                px(11.0),
                &[run(text, muted, cx)],
                Some(px(text_width)),
                None,
            )
            .unwrap_or_default()
    };
    // A clamp would leave the rest of the text unwrapped on its last line,
    // past the card's edge: the summary is cut to the lines instead.
    let summary = clamp_words(entry.summary.as_deref().unwrap_or(""), |text| {
        wrapped_lines(&shape(text, window)) <= SUMMARY_LINES
    });
    let lines = shape(&summary, window);
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
    window.with_content_mask(Some(ContentMask { bounds }), |window| {
        for line in lines {
            let _ = line.paint(origin, px(17.0), TextAlign::Left, None, window, cx);
            origin.y += line.size(px(17.0)).height;
        }
    });
}

#[cfg(test)]
mod tests;
