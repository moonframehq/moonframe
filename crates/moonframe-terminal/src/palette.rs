//! The xterm 256-color palette, for an emulator that leaves colors as
//! indexes: 16 named colors, a 6×6×6 cube, and 24 grays.

/// The 16 named colors, as xterm's defaults.
const NAMED: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00),
    (0xcd, 0x31, 0x31),
    (0x0d, 0xbc, 0x79),
    (0xe5, 0xe5, 0x10),
    (0x24, 0x72, 0xc8),
    (0xbc, 0x3f, 0xbc),
    (0x11, 0xa8, 0xcd),
    (0xe5, 0xe5, 0xe5),
    (0x66, 0x66, 0x66),
    (0xf1, 0x4c, 0x4c),
    (0x23, 0xd1, 0x8b),
    (0xf5, 0xf5, 0x43),
    (0x3b, 0x8e, 0xea),
    (0xd6, 0x70, 0xd6),
    (0x29, 0xb8, 0xdb),
    (0xff, 0xff, 0xff),
];

/// The RGB of palette entry `index`.
pub(crate) fn rgb(index: u8) -> (u8, u8, u8) {
    match index {
        0..=15 => NAMED[usize::from(index)],
        16..=231 => {
            let cube = index - 16;
            let level = |step: u8| {
                if step == 0 {
                    0
                } else {
                    55 + step * 40
                }
            };
            (level(cube / 36), level(cube / 6 % 6), level(cube % 6))
        },
        232..=255 => {
            let gray = 8 + (index - 232) * 10;
            (gray, gray, gray)
        },
    }
}
