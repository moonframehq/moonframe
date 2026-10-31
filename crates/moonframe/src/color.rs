//! Turning the core's colors and color roles into GPUI colors for the
//! current theme.

use {
    gpui_kit::{Hsla, component::Theme, rgba},
    moonframe_core::{Color, Paint, Shade, Surface, Token, Tone},
};

/// A color that depends on the theme: a role, a palette entry, or a mix.
pub(crate) trait ThemeColor {
    fn hsla(&self, theme: &Theme) -> Hsla;
}

impl ThemeColor for Token {
    fn hsla(&self, theme: &Theme) -> Hsla {
        match self {
            Token::Background => theme.background,
            Token::Foreground => theme.foreground,
            Token::Muted => theme.muted,
            Token::MutedForeground => theme.muted_foreground,
            Token::Sidebar => theme.sidebar,
            Token::Card => theme.popover,
            Token::Secondary => theme.secondary,
            Token::Border => theme.border,
            Token::Accent => theme.primary,
            Token::AccentForeground => theme.primary_foreground,
            Token::Success => theme.success,
            Token::Warning => theme.warning,
            Token::Danger => theme.danger,
            Token::ListHover => theme.list_hover,
            Token::ListActive => theme.list_active,
        }
    }
}

impl ThemeColor for Shade {
    fn hsla(&self, theme: &Theme) -> Hsla {
        rgba_color(if theme.is_dark() {
            self.dark
        } else {
            self.light
        })
    }
}

impl ThemeColor for Color {
    fn hsla(&self, theme: &Theme) -> Hsla {
        let base = match &self.paint {
            Paint::Token(token) => token.hsla(theme),
            Paint::Fixed(shade) => shade.hsla(theme),
        };
        base.opacity(self.alpha)
    }
}

impl ThemeColor for Tone {
    fn hsla(&self, theme: &Theme) -> Hsla {
        match self {
            Tone::Normal => theme.foreground,
            Tone::Muted => theme.muted_foreground,
            Tone::Accent => theme.primary,
            Tone::Success => theme.success,
            Tone::Warning => theme.warning,
            Tone::Danger => theme.danger,
        }
    }
}

impl ThemeColor for Surface {
    fn hsla(&self, theme: &Theme) -> Hsla {
        match self {
            Surface::Background => theme.background,
            Surface::Sidebar => theme.sidebar,
            Surface::Muted => theme.muted,
            Surface::Secondary => theme.secondary,
        }
    }
}

/// An `0xRRGGBBAA` color.
pub(crate) fn rgba_color(value: u32) -> Hsla {
    rgba(value).into()
}
