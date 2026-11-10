//! The theme's colors, as the stylesheet's roles name them. The values are
//! the neutral palette the GPUI renderer starts from.

use {
    moonframe_core::{Color, Paint, Shade, Surface, Token, Tone},
    rune::{Color as RuneColor, color::hex},
};

/// An `0xRRGGBBAA` color.
fn rgba(value: u32) -> RuneColor {
    hex(&format!("#{value:08x}")).unwrap_or(RuneColor::WHITE)
}

/// Each role's color, light then dark: what gpui-component's stock themes
/// resolve to, so an app looks the same on either backend.
fn role(token: Token) -> Shade {
    let (light, dark) = match token {
        Token::Background => (0xffff_ffff, 0x0a0a_0aff),
        Token::Foreground => (0x0a0a_0aff, 0xfafa_faff),
        Token::Muted => (0xf5f5_f5ff, 0x2626_26ff),
        Token::MutedForeground => (0x7373_73ff, 0xa3a3_a3ff),
        Token::Sidebar => (0xfafa_faff, 0x0a0a_0aff),
        Token::Card => (0xffff_ffff, 0x0a0a_0aff),
        Token::Secondary => (0xe5e5_e5ff, 0x2626_26ff),
        Token::Border => (0xe5e5_e5ff, 0x2626_26ff),
        Token::Accent => (0x1717_17ff, 0xfafa_faff),
        Token::AccentForeground => (0xfafa_faff, 0x1717_17ff),
        Token::Success => (0x22c5_5eff, 0x4ade_80ff),
        Token::Warning => (0xeab3_08ff, 0xfacc_15ff),
        Token::Danger => (0xef44_44ff, 0xf871_71ff),
        Token::ListHover => (0xf5f5_f5ff, 0x2626_2699),
        Token::ListActive => (0xbfdb_fe33, 0x1e40_af33),
    };
    Shade { light, dark }
}

/// Colors for one appearance, with the stylesheet's `colors` over the roles.
#[derive(Clone, Copy)]
pub(crate) struct Palette<'a> {
    pub(crate) dark: bool,
    pub(crate) overrides: &'a [(Token, Shade)],
}

impl Palette<'_> {
    pub(crate) fn shade(self, shade: Shade) -> RuneColor {
        rgba(if self.dark {
            shade.dark
        } else {
            shade.light
        })
    }

    pub(crate) fn token(self, token: Token) -> RuneColor {
        let found = self.overrides.iter().find(|(named, _)| *named == token);
        self.shade(found.map_or_else(|| role(token), |(_, shade)| *shade))
    }

    pub(crate) fn color(self, color: &Color) -> RuneColor {
        let base = match &color.paint {
            Paint::Token(token) => self.token(*token),
            Paint::Fixed(shade) => self.shade(*shade),
        };
        base.alpha(base.a * color.alpha)
    }

    pub(crate) fn tone(self, tone: Tone) -> RuneColor {
        self.token(match tone {
            Tone::Normal => Token::Foreground,
            Tone::Muted => Token::MutedForeground,
            Tone::Accent => Token::Accent,
            Tone::Success => Token::Success,
            Tone::Warning => Token::Warning,
            Tone::Danger => Token::Danger,
        })
    }

    pub(crate) fn surface(self, surface: Surface) -> RuneColor {
        self.token(match surface {
            Surface::Background => Token::Background,
            Surface::Sidebar => Token::Sidebar,
            Surface::Muted => Token::Muted,
            Surface::Secondary => Token::Secondary,
        })
    }
}
