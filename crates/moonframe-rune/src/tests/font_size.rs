use {
    crate::{style::Ink, theme::Palette},
    moonframe_core::{Style, TextSize},
    rune::Color,
};

fn ink() -> Ink {
    Ink {
        color: Color::WHITE,
        base: 14.,
        size: 14.,
        weight: 400.,
        mono: false,
    }
}

fn palette() -> Palette<'static> {
    Palette {
        dark: true,
        overrides: &[],
    }
}

#[test]
fn a_font_size_sets_the_pixels_over_a_named_size() {
    let style = Style {
        size: Some(TextSize::Small),
        font_size: Some(13.),
        ..Style::default()
    };
    assert_eq!(ink().under(&style, palette()).size, 13.);
}

#[test]
fn children_keep_a_font_size_until_they_name_their_own() {
    let parent = ink().under(
        &Style {
            font_size: Some(16.),
            ..Style::default()
        },
        palette(),
    );
    assert_eq!(parent.under(&Style::default(), palette()).size, 16.);
    let small = Style {
        size: Some(TextSize::Small),
        ..Style::default()
    };
    assert_eq!(parent.under(&small, palette()).size, 14. * 0.85);
}
