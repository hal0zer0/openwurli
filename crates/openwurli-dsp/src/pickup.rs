//! Electrostatic pickup model — nonlinear capacitance + RC HPF.
//!
//! The Wurlitzer 200A pickup is a capacitive sensor: reed vibration modulates
//! the capacitance between the reed and a charged metal plate (+147V DC).
//!
//! The reed-plate capacitance varies nonlinearly with displacement:
//!   C(y) = C_0 / (1 - y)
//! where y = x/d_0 is the normalized displacement (fraction of rest gap,
//! positive toward the plate).
//!
//! This 1/(1-y) nonlinearity is the PRIMARY source of the Wurlitzer "bark."
//! It generates H2 that scales with displacement amplitude:
//!   y=0.02 (pp): THD 1.7%,  H2 = -35 dB
//!   y=0.10 (mf): THD 8.7%,  H2 = -21 dB
//!   y=0.20 (f):  THD 17.6%, H2 = -15 dB
//! (Validated against SPICE pickup model, tb_pickup.cir)
//!
//! The preamp, by contrast, produces < 0.01% THD at normal playing levels.
//! The bark comes from HERE, not the preamp.
//!
//! One high-pass filter shapes the frequency response:
//!   Pickup RC: 1-pole HPF at 2312 Hz (R_total=287K, C=240pF)
//!   R_total = R_feed (1M) || (R-1 + R-2||R-3) = 1M || 402K = 287K
//!
//! The HPF also amplifies H2 relative to H1 (since H2 is at 2f, where
//! the HPF has higher gain), adding ~1.9x boost to the H2/H1 ratio.

use crate::filters::OnePoleHpf;

/// Pickup sensitivity: V_hv * C_0 / (C_0 + C_p) = 147 * 3/240 = 1.8375 V
/// Applied to the nonlinear displacement y/(1-y).
const SENSITIVITY: f64 = 1.8375;

/// Convert reed model displacement units to physical y = x/d_0.
///
/// The reed model outputs in normalized units (fundamental amplitude = 1.0).
/// These are NOT physical displacement fractions — they're ~10-15x too large.
/// This constant converts to the physical ratio y = x/d_0 where d_0 is the
/// rest gap between reed tip and pickup plate.
///
/// At C4 vel=127 (ff), y_peak ≈ 0.55, producing ~49% H2/H1 after HPF.
/// At C4 vel=80 (mf), y_peak ≈ 0.20, producing ~16% H2/H1 after HPF.
/// Value chosen by ear from a sweep of 0.15–0.75, constrained by research
/// on the physical reed-to-pickup gap (estimated 0.3–1.5 mm, Pfeifle 2017).
/// At 0.35 the sound was too clean ("wooden, like tuned wood blocks") —
/// insufficient even-harmonic content from the 1/(1-y) nonlinearity.
///
/// NOTE: This default is overridden per-note by tables::pickup_displacement_scale()
/// in voice.rs. Only used if set_displacement_scale() is never called.
const DISPLACEMENT_SCALE: f64 = 0.70;

/// Maximum allowed displacement fraction (safety clamp).
/// The reed physically cannot touch the plate (y=1.0 is a singularity).
/// In practice, y rarely exceeds 0.25 even at extreme velocities.
const MAX_Y: f64 = 0.90;

pub struct Pickup {
    hpf: OnePoleHpf,
    displacement_scale: f64,
}

impl Pickup {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            hpf: OnePoleHpf::new(2312.0, sample_rate),
            displacement_scale: DISPLACEMENT_SCALE,
        }
    }

    /// Override the displacement scale (default: 0.70).
    /// Higher = tighter gap = more nonlinearity = more bark.
    pub fn set_displacement_scale(&mut self, scale: f64) {
        self.displacement_scale = scale;
    }

    /// Process a buffer of reed displacement samples in-place.
    ///
    /// Input: reed displacement in normalized model units.
    /// Output: pickup voltage in volts (millivolt-scale signals).
    ///
    /// The nonlinear transfer function models the variable capacitance:
    ///   C(y) = C_0 / (1-y)  →  signal ∝ y/(1-y)
    /// where y = displacement * displacement_scale.
    ///
    /// This produces H2 that increases with displacement amplitude,
    /// which is the primary source of the Wurlitzer bark.
    pub fn process(&mut self, buffer: &mut [f64]) {
        let scale = self.displacement_scale;
        for sample in buffer.iter_mut() {
            // Convert to physical displacement fraction
            let y = (*sample * scale).clamp(-MAX_Y, MAX_Y);

            // Nonlinear capacitance: C(y) = C_0/(1-y)
            // Signal voltage ∝ delta_C/C_total = y/(1-y)
            // Asymmetry: positive y (toward plate) amplified more than
            // negative y (away from plate). This generates H2.
            let nonlinear = y / (1.0 - y);

            // Scale to voltage: V = V_hv * C_0/(C_0+C_p) * y/(1-y)
            let v = nonlinear * SENSITIVITY;

            // Pickup RC highpass at 2312 Hz
            *sample = self.hpf.process(v);
        }
    }

    pub fn reset(&mut self) {
        self.hpf.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_hpf_passes_high_freq() {
        // At 10 kHz, the HPF is nearly unity gain.
        // Input: unit sine (displacement). After DISPLACEMENT_SCALE + nonlinear + SENSITIVITY + HPF,
        // output ≈ SENSITIVITY * (DISPLACEMENT_SCALE / (1 - DISPLACEMENT_SCALE)) * HPF_gain
        // With DISPLACEMENT_SCALE = 0.70: y_peak = 0.70, y/(1-y) = 2.33
        // Output ≈ 2.33 * 1.8375 * ~0.97 = 4.15 (peak, includes nonlinear asymmetry)
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let freq = 10000.0;

        let n = (sr * 0.05) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        let peak = buf[n / 2..].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(peak > 1.0, "pickup output too low at 10kHz: {peak}");
        assert!(peak < 5.5, "pickup output too high at 10kHz: {peak}");
    }

    #[test]
    fn test_hpf_attenuates_bass() {
        // At 100 Hz, the HPF attenuates heavily (gain ≈ 0.043).
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let freq = 100.0;

        let n = (sr * 0.1) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        let peak = buf[n / 2..].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        // With DISPLACEMENT_SCALE=0.70: Output ≈ 2.33 * 1.8375 * 0.043 = 0.184
        assert!(peak < 0.25, "pickup should heavily attenuate 100Hz: {peak}");
    }

    #[test]
    fn test_nonlinearity_produces_h2() {
        // Drive the pickup with a large-amplitude sine and verify H2 > H3.
        // This is the core test: 1/(1-y) generates even harmonics.
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let freq = 2000.0; // Above HPF corner for cleaner measurement

        // Amplitude of 1.0 in reed units → y = 0.35 → meaningful nonlinearity
        let amplitude = 1.0;
        let n = (sr * 0.2) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| amplitude * (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        // DFT at H1, H2, H3 (steady-state, last quarter)
        let start = n * 3 / 4;
        let signal = &buf[start..];
        let h1 = dft_magnitude(signal, freq, sr);
        let h2 = dft_magnitude(signal, 2.0 * freq, sr);
        let h3 = dft_magnitude(signal, 3.0 * freq, sr);

        assert!(
            h2 > h3,
            "H2 ({h2:.2e}) should dominate H3 ({h3:.2e}) from 1/(1-y)"
        );
        // At y_peak = 0.35: H2/H1 ≈ y/2 * HPF_boost ≈ 17.5% * ~1.1 ≈ 19%
        let h2_ratio = h2 / h1;
        assert!(
            h2_ratio > 0.07,
            "H2/H1 too low ({h2_ratio:.4}), expected >7% from nonlinearity"
        );
    }

    #[test]
    fn test_asymmetry() {
        // The nonlinear pickup should produce asymmetric output:
        // positive excursions (toward plate) larger than negative.
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let freq = 3000.0; // Well above HPF corner

        let amplitude = 1.2; // y_peak = 0.42
        let n = (sr * 0.1) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| amplitude * (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        let pos_peak = buf[n / 2..].iter().cloned().fold(0.0f64, f64::max);
        let neg_peak = buf[n / 2..].iter().cloned().fold(0.0f64, f64::min).abs();

        // Positive excursion (toward plate) should produce larger signal
        assert!(
            pos_peak > neg_peak * 1.05,
            "Expected asymmetry: pos={pos_peak:.6} neg={neg_peak:.6}"
        );
    }

    fn dft_magnitude(signal: &[f64], freq: f64, sr: f64) -> f64 {
        let n = signal.len() as f64;
        let mut re = 0.0;
        let mut im = 0.0;
        for (i, &s) in signal.iter().enumerate() {
            let phase = 2.0 * PI * freq * i as f64 / sr;
            re += s * phase.cos();
            im -= s * phase.sin();
        }
        2.0 * ((re / n).powi(2) + (im / n).powi(2)).sqrt()
    }
}
