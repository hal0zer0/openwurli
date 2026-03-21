//! Filter primitives for the Wurlitzer 200A signal chain.
//!
//! Biquad is backed by melange-primitives (identical Audio EQ Cookbook
//! coefficients and Direct Form II Transposed structure).

use melange_primitives::{Biquad as MelangeBiquad, BiquadType};

/// Biquad filter — Direct Form II Transposed.
///
/// Backed by melange-primitives. Identical Audio EQ Cookbook coefficients
/// and DF-II Transposed structure — bit-identical output.
pub struct Biquad(MelangeBiquad);

impl Biquad {
    /// Bandpass filter (constant skirt gain, Audio EQ Cookbook).
    pub fn bandpass(center_hz: f64, q: f64, sample_rate: f64) -> Self {
        Self(MelangeBiquad::new(
            BiquadType::Bandpass { fc: center_hz, q },
            sample_rate,
        ))
    }

    /// Low-pass filter (Audio EQ Cookbook).
    pub fn lowpass(cutoff_hz: f64, q: f64, sample_rate: f64) -> Self {
        Self(MelangeBiquad::new(
            BiquadType::Lowpass { fc: cutoff_hz, q },
            sample_rate,
        ))
    }

    /// High-pass filter (Audio EQ Cookbook).
    pub fn highpass(cutoff_hz: f64, q: f64, sample_rate: f64) -> Self {
        Self(MelangeBiquad::new(
            BiquadType::Highpass { fc: cutoff_hz, q },
            sample_rate,
        ))
    }

    /// Update coefficients to highpass without resetting filter state.
    pub fn set_highpass(&mut self, cutoff_hz: f64, q: f64, sample_rate: f64) {
        self.0
            .set_type(BiquadType::Highpass { fc: cutoff_hz, q }, sample_rate);
    }

    /// Update coefficients to lowpass without resetting filter state.
    pub fn set_lowpass(&mut self, cutoff_hz: f64, q: f64, sample_rate: f64) {
        self.0
            .set_type(BiquadType::Lowpass { fc: cutoff_hz, q }, sample_rate);
    }

    /// Process one sample (Direct Form II Transposed).
    pub fn process(&mut self, x: f64) -> f64 {
        self.0.process(x)
    }

    pub fn reset(&mut self) {
        self.0.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_biquad_bandpass() {
        let sr = 44100.0;
        let center = 1000.0;
        let mut bpf = Biquad::bandpass(center, 1.0, sr);

        // Feed 1000 Hz — should pass
        let n = (sr * 0.1) as usize;
        let mut peak_center = 0.0f64;
        for i in 0..n {
            let x = (2.0 * PI * center * i as f64 / sr).sin();
            let y = bpf.process(x);
            if i > n / 2 {
                peak_center = peak_center.max(y.abs());
            }
        }

        bpf.reset();

        // Feed 100 Hz — should attenuate
        let mut peak_low = 0.0f64;
        for i in 0..n {
            let x = (2.0 * PI * 100.0 * i as f64 / sr).sin();
            let y = bpf.process(x);
            if i > n / 2 {
                peak_low = peak_low.max(y.abs());
            }
        }

        assert!(
            peak_center > peak_low * 3.0,
            "BPF center ({peak_center}) should be much louder than off-center ({peak_low})"
        );
    }
}
