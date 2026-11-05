//! Sound as engines want it: 16 kHz mono 16-bit WAV, whatever rate the
//! microphone runs at.

/// The rate every speech engine takes.
pub(crate) const RATE: u32 = 16_000;
/// The loudest a take may peak at and still count as silence. A microphone
/// the system refused access to delivers exact zeros.
const SILENCE: f32 = 0.002;
/// The quietest level [`level`] shows, in dBFS.
const FLOOR_DB: f32 = -60.0;

/// What a take recorded: mono samples from -1 to 1.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Audio {
    pub samples: Vec<f32>,
    pub rate: u32,
}

impl Audio {
    pub(crate) fn seconds(&self) -> f32 {
        if self.rate == 0 {
            return 0.0;
        }
        self.samples.len() as f32 / self.rate as f32
    }

    /// Whether anything but silence reached the microphone.
    pub(crate) fn heard(&self) -> bool {
        self.samples.iter().any(|sample| sample.abs() > SILENCE)
    }

    /// The take at [`RATE`] as a WAV file's bytes.
    pub(crate) fn wav(&self) -> Vec<u8> {
        wav(&resample(&self.samples, self.rate, RATE), RATE)
    }
}

/// `samples` at `to` instead of `from`: each output sample averages the
/// input it covers when going down, which keeps what lies above the new
/// limit from folding back as noise, and interpolates when going up.
pub(crate) fn resample(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || from == 0 || to == 0 || samples.is_empty() {
        return samples.to_vec();
    }
    let step = f64::from(from) / f64::from(to);
    let length = (samples.len() as f64 / step).floor() as usize;
    let last = samples.len() - 1;
    (0..length)
        .map(|index| {
            let start = index as f64 * step;
            if step > 1.0 {
                let first = (start as usize).min(last);
                let end = (((start + step) as usize).max(first + 1)).min(samples.len());
                let span = &samples[first..end];
                span.iter().sum::<f32>() / span.len() as f32
            } else {
                let first = (start as usize).min(last);
                let next = (first + 1).min(last);
                let part = (start - first as f64) as f32;
                samples[first] + (samples[next] - samples[first]) * part
            }
        })
        .collect()
}

/// 16-bit mono PCM in a RIFF container.
pub(crate) fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes()); // bytes a second
    out.extend_from_slice(&2u16.to_le_bytes()); // bytes a frame
    out.extend_from_slice(&16u16.to_le_bytes()); // bits a sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// How loud a block of samples is, from 0 to 1 along a decibel scale, for a
/// meter: speech sits in the middle instead of at the bottom.
pub(crate) fn level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let power = samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32;
    let db = 10.0 * power.max(1e-12).log10();
    ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}
