//! The one transform `ui.video` applies to a pixel. The shader does this
//! for what is shown and [`Tone::apply`] for what a probe reports, so the
//! two can't disagree.

/// What becomes of a value between the source and the display.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tone {
    /// The value shown as black, in the source's own code values.
    pub black: f32,
    /// The value shown as white, likewise.
    pub white: f32,
    /// Multiplies the result; 1 changes nothing.
    pub gain: f32,
    /// A display gamma to encode with; 1 changes nothing, so linear
    /// pixels and pixels already encoded each stay as they are.
    pub gamma: f32,
}

impl Tone {
    /// Leaves pixels as they are, for a source whose full range is `white`.
    pub fn identity(white: f32) -> Self {
        Self {
            black: 0.,
            white,
            gain: 1.,
            gamma: 1.,
        }
    }

    /// `value`, in code values, as a fraction for the display.
    pub fn apply(&self, value: f32) -> f32 {
        let span = (self.white - self.black).max(f32::EPSILON);
        let level = ((value - self.black) / span * self.gain).max(0.);
        if (self.gamma - 1.).abs() < f32::EPSILON {
            level
        } else {
            level.powf(1. / self.gamma)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_identity_only_scales_to_the_source_range() {
        let tone = Tone::identity(4095.);
        assert_eq!(tone.apply(0.), 0.);
        assert_eq!(tone.apply(4095.), 1.);
    }

    #[test]
    fn black_white_gain_and_gamma_apply_in_that_order() {
        let tone = Tone {
            black: 100.,
            white: 1100.,
            gain: 2.,
            gamma: 2.,
        };
        assert_eq!(tone.apply(100.), 0.);
        assert!((tone.apply(225.) - 0.5).abs() < 1e-6);
        // Under black stays black; nothing is clipped above white, which
        // an extended display can still show.
        assert_eq!(tone.apply(0.), 0.);
        assert!(tone.apply(2100.) > 1.);
    }
}
