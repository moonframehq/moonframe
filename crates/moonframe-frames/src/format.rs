//! How a frame's pixels are laid out.

/// A frame's pixel format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Red, green, blue and alpha, 8 bits each.
    Rgba8,
    /// The same, 16 bits each, 0 to 65535.
    Rgba16,
    /// The same, 16-bit floats.
    Rgba16Float,
    /// One 16-bit value per pixel.
    Mono16,
    /// One 16-bit sensor value per pixel, in a Bayer [`Pattern`].
    Bayer16,
}

impl Format {
    /// The format stored as `value` in a slot.
    pub fn from_raw(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Rgba8),
            2 => Some(Self::Rgba16),
            3 => Some(Self::Rgba16Float),
            4 => Some(Self::Mono16),
            5 => Some(Self::Bayer16),
            _ => None,
        }
    }

    /// What a slot stores for it.
    pub fn raw(self) -> u32 {
        match self {
            Self::Rgba8 => 1,
            Self::Rgba16 => 2,
            Self::Rgba16Float => 3,
            Self::Mono16 => 4,
            Self::Bayer16 => 5,
        }
    }

    /// Bytes each pixel takes.
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgba8 => 4,
            Self::Rgba16 | Self::Rgba16Float => 8,
            Self::Mono16 | Self::Bayer16 => 2,
        }
    }

    /// As views read it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rgba8 => "rgba8",
            Self::Rgba16 => "rgba16",
            Self::Rgba16Float => "rgba16f",
            Self::Mono16 => "mono16",
            Self::Bayer16 => "bayer16",
        }
    }
}

/// The colours of a Bayer sensor's top left two by two pixels, row by row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pattern {
    #[default]
    Rggb,
    Bggr,
    Grbg,
    Gbrg,
}

impl Pattern {
    /// The pattern stored as `value` in a slot.
    pub fn from_raw(value: u32) -> Self {
        match value {
            1 => Self::Bggr,
            2 => Self::Grbg,
            3 => Self::Gbrg,
            _ => Self::Rggb,
        }
    }

    /// What a slot stores for it.
    pub fn raw(self) -> u32 {
        match self {
            Self::Rggb => 0,
            Self::Bggr => 1,
            Self::Grbg => 2,
            Self::Gbrg => 3,
        }
    }

    /// The channel (0 red, 1 green, 2 blue) the sensor reads at `x`, `y`.
    pub fn channel(self, x: u32, y: u32) -> usize {
        let cell = [[[0, 1], [1, 2]], [[2, 1], [1, 0]], [[1, 0], [2, 1]], [
            [1, 2],
            [0, 1],
        ]][self.raw() as usize];
        cell[(y % 2) as usize][(x % 2) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_survive_the_trip_through_a_slot() {
        for format in [
            Format::Rgba8,
            Format::Rgba16,
            Format::Rgba16Float,
            Format::Mono16,
            Format::Bayer16,
        ] {
            assert_eq!(Format::from_raw(format.raw()), Some(format));
        }
        assert_eq!(Format::from_raw(0), None);
    }

    #[test]
    fn a_pattern_says_which_colour_each_photosite_reads() {
        assert_eq!(Pattern::Rggb.channel(0, 0), 0);
        assert_eq!(Pattern::Rggb.channel(1, 1), 2);
        assert_eq!(Pattern::Bggr.channel(0, 0), 2);
        assert_eq!(Pattern::Grbg.channel(1, 0), 0);
        assert_eq!(Pattern::Gbrg.channel(0, 1), 0);
        assert_eq!(Pattern::Gbrg.channel(3, 2), 2);
    }
}
