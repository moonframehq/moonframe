//! The view's colors, from the app's theme: nodes on the card color with
//! the border's outline, edges in the muted text color, the selection in
//! the accent. A tone tints a node; a fill of its own replaces the card.

use {
    crate::model::{NodeKind, Tone},
    gpui_kit::{component::ActiveTheme as _, *},
};

/// Every color the view paints with, read from the theme once a frame.
#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Hsla,
    pub card: Hsla,
    pub outline: Hsla,
    pub text: Hsla,
    pub quiet: Hsla,
    pub edge: Hsla,
    pub accent: Hsla,
    pub note: Hsla,
    pub note_text: Hsla,
    pub dark: bool,
}

impl Palette {
    pub fn of(cx: &App) -> Self {
        let theme = cx.theme();
        let dark = theme.is_dark();
        Self {
            background: theme.background,
            card: theme.popover,
            outline: theme.border,
            text: theme.foreground,
            quiet: theme.muted_foreground,
            edge: theme.muted_foreground,
            accent: theme.primary,
            note: if dark { hsla(48.0 / 360.0, 0.55, 0.30, 1.0) } else { hsla(48.0 / 360.0, 0.95, 0.80, 1.0) },
            note_text: if dark { hsla(48.0 / 360.0, 0.4, 0.92, 1.0) } else { hsla(30.0 / 360.0, 0.5, 0.18, 1.0) },
            dark,
        }
    }

    pub fn tone(&self, tone: Tone, cx: &App) -> Hsla {
        let theme = cx.theme();
        match tone {
            Tone::Accent => theme.primary,
            Tone::Success => theme.success,
            Tone::Warning => theme.warning,
            Tone::Danger => theme.danger,
            Tone::Muted => theme.muted_foreground,
        }
    }

    /// A node's fill, outline and text color.
    pub fn node(&self, kind: NodeKind, tone: Option<Tone>, fill: Option<&str>, cx: &App) -> (Hsla, Hsla, Hsla) {
        if let Some(color) = fill.and_then(hex) {
            // A light fill of its own keeps dark text in either theme.
            let text = if color.l > 0.6 { hsla(0.0, 0.0, 0.12, 1.0) } else { hsla(0.0, 0.0, 0.96, 1.0) };
            return (color, color.blend(self.outline.opacity(0.9)).opacity(1.0).darken(0.15), text);
        }
        match (kind, tone) {
            (NodeKind::Text, _) => (transparent_black(), transparent_black(), self.text),
            (NodeKind::Note, None) => (self.note, self.note.darken(0.1), self.note_text),
            (_, Some(tone)) => {
                let color = self.tone(tone, cx);
                (self.card.blend(color.opacity(0.14)), color.opacity(0.8), self.text)
            },
            (_, None) => (self.card, self.outline, self.text),
        }
    }
}

trait Darken {
    fn darken(self, by: f32) -> Self;
}

impl Darken for Hsla {
    fn darken(self, by: f32) -> Self {
        Hsla { l: (self.l - by).max(0.0), ..self }
    }
}

/// `#rgb` or `#rrggbb` as a color.
pub fn hex(text: &str) -> Option<Hsla> {
    let digits = text.strip_prefix('#')?;
    let value = match digits.len() {
        3 => {
            let v = u32::from_str_radix(digits, 16).ok()?;
            let (r, g, b) = ((v >> 8) & 0xf, (v >> 4) & 0xf, v & 0xf);
            ((r * 17) << 16) | ((g * 17) << 8) | (b * 17)
        },
        6 => u32::from_str_radix(digits, 16).ok()?,
        _ => return None,
    };
    Some(rgb(value).into())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::hex;

    #[test]
    fn hex_reads_both_lengths() {
        assert_eq!(hex("#fff"), hex("#ffffff"));
        assert!(hex("#12345").is_none());
        assert!(hex("blue").is_none());
    }
}
