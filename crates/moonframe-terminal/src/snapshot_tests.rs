//! Selection needs the same wrap and wide-cell metadata from either engine.

use crate::{Emulator, Engine, Size};

fn emulators() -> impl Iterator<Item = (Engine, Box<dyn Emulator>)> {
    [Engine::Alacritty, Engine::Ghostty]
        .into_iter()
        .filter(|engine| engine.available())
        .map(|engine| {
            let size = Size {
                cols: 20,
                rows: 4,
                ..Size::default()
            };
            (engine, engine.emulator(size).expect("engine starts"))
        })
}

#[test]
fn snapshots_distinguish_soft_wraps_from_hard_line_breaks() {
    for (engine, mut emulator) in emulators() {
        emulator.feed(b"abcdefghijklmnopqrstUV\r\nhard\r\nlast");
        let screen = emulator.screen();
        assert_eq!(screen.wrapped, [true, false, false, false], "{engine:?}");

        // Moving the viewport must move its row metadata with the text.
        emulator.feed(b"\r\nnew");
        assert_eq!(emulator.screen().row(0)[0].ch, 'U', "{engine:?}");
        assert!(!emulator.screen().is_wrapped(0), "{engine:?}");
        emulator.scroll(-1);
        let screen = emulator.screen();
        assert_eq!(screen.row(0)[0].ch, 'a', "{engine:?}");
        assert_eq!(screen.wrapped, [true, false, false, false], "{engine:?}");
    }
}

#[test]
fn filling_the_last_column_does_not_imply_a_soft_wrap() {
    for (engine, mut emulator) in emulators() {
        emulator.feed(b"abcdefghijklmnopqrst\r\nnext");
        assert!(!emulator.screen().is_wrapped(0), "{engine:?}");
    }
}

#[test]
fn resizing_updates_the_wrap_metadata() {
    for (engine, mut emulator) in emulators() {
        emulator.feed(b"abcdefghijklmnopqrstUV");
        assert!(emulator.screen().is_wrapped(0), "{engine:?}");
        emulator.resize(Size {
            cols: 40,
            rows: 4,
            ..Size::default()
        });
        let screen = emulator.screen();
        assert_eq!(screen.row(0)[20].ch, 'U', "{engine:?}");
        assert!(!screen.is_wrapped(0), "{engine:?}");
    }
}

#[test]
fn snapshots_identify_both_kinds_of_wide_spacer() {
    for (engine, mut emulator) in emulators() {
        // Only one column remains, so the wide character wraps as a whole.
        emulator.feed("abcdefghijklmnopqrs界x".as_bytes());
        let screen = emulator.screen();
        assert!(screen.is_wrapped(0), "{engine:?}");
        let padding = screen.row(0)[19];
        assert!(padding.wrap_spacer && !padding.spacer, "{engine:?}");
        let wide = screen.row(1)[0];
        assert_eq!(wide.ch, '界', "{engine:?}");
        assert!(!wide.spacer && !wide.wrap_spacer, "{engine:?}");
        let tail = screen.row(1)[1];
        assert!(tail.spacer && !tail.wrap_spacer, "{engine:?}");
        assert_eq!(screen.row(1)[2].ch, 'x', "{engine:?}");
    }
}
