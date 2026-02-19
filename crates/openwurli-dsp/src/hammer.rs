//! Hammer model: Gaussian dwell filter + attack noise burst.
//!
//! The dwell filter is a one-shot spectral shaping applied at note-on.
//! It models the finite contact duration of the felt hammer on the reed,
//! which acts as a low-pass filter on the initial mode excitation.
//!
//! The attack noise is an exponentially decaying bandpass-filtered noise burst
//! that models the mechanical impact transient.

use crate::filters::Biquad;
use crate::tables::NUM_MODES;

/// Hammer dwell time (contact duration) in seconds — for spectral filtering only.
///
/// Register-dependent per Miessner patent US 2,932,231: the hammer contacts
/// the reed for "three fourths to one cycle of vibration at its fundamental
/// frequency." This makes dwell strongly register-dependent:
///   - A1 (55 Hz) ff: 13.6 ms    pp: 18.2 ms
///   - C4 (262 Hz) ff: 2.9 ms    pp: 3.8 ms
///   - C6 (1047 Hz) ff: 0.72 ms  pp: 0.95 ms
///
/// The velocity mapping: ff = 0.75 cycles (hard, brief contact), pp = 1.0 cycle
/// (soft, lingering contact from neoprene foam compression).
///
/// For the time-domain onset ramp, see `onset_ramp_time`.
pub fn dwell_time(velocity: f64, fundamental_hz: f64) -> f64 {
    let cycles = 0.75 + 0.25 * (1.0 - velocity);
    (cycles / fundamental_hz).clamp(0.0003, 0.020)
}

/// Register-dependent onset ramp time — models reed mechanical inertia.
///
/// Heavier bass reeds take longer to reach full vibration amplitude after
/// the hammer strike. OBM cycle-by-cycle analysis shows the real 200A
/// reaches 90% amplitude by cycle 2 across all registers:
///   - D3 (147 Hz): 50% at cycle 1, 90% at cycle 2
///   - D4 (294 Hz): 50% at cycle 1, 90% at cycle 2
///   - D6 (1175 Hz): near-instant (full by cycle 0-1)
///
/// Formula: 2.0 periods at ff, 4.0 at pp, clamped to [2ms, 30ms].
/// Matches OBM cycle-by-cycle data: 90% by cycle 2 across all registers.
/// Longer than previous (1.5+1.5, 20ms) to reduce bass overshoot
/// (3.47x → ~2.0-2.5x) by giving modal interference more time to average.
pub fn onset_ramp_time(velocity: f64, fundamental_hz: f64) -> f64 {
    let period_s = 1.0 / fundamental_hz;
    let periods = 2.0 + 2.0 * (1.0 - velocity);
    (periods * period_s).clamp(0.002, 0.030)
}

/// Compute per-mode attenuation from the Gaussian dwell filter.
///
/// Uses a wide Gaussian (sigma=8 in f*T units) per the recommendation in
/// Section 4.3.4 of reed-and-hammer-physics.md.
///
/// - `velocity`: 0.0 (pp) to 1.0 (ff)
/// - `fundamental_hz`: fundamental frequency of this note
/// - `mode_ratios`: f_n/f_1 for each mode
///
/// Returns per-mode multipliers (normalized so mode 0 = 1.0).
pub fn dwell_attenuation(
    velocity: f64,
    fundamental_hz: f64,
    mode_ratios: &[f64; NUM_MODES],
) -> [f64; NUM_MODES] {
    let t_dwell = dwell_time(velocity, fundamental_hz);
    let sigma_sq = 8.0 * 8.0;

    let mut atten = [0.0f64; NUM_MODES];
    for i in 0..NUM_MODES {
        let ft = fundamental_hz * mode_ratios[i] * t_dwell;
        atten[i] = (-ft * ft / (2.0 * sigma_sq)).exp();
    }

    let a0 = atten[0];
    if a0 > 1e-30 {
        for a in &mut atten {
            *a /= a0;
        }
    }
    atten
}

/// Attack noise generator — exponentially decaying bandpass noise.
///
/// Models the mechanical impact transient of felt hammer on steel reed.
/// Duration: ~15 ms. Center frequency tracks the note (4× fundamental,
/// clamped 200–2000 Hz) so the noise sits within the harmonic neighborhood
/// rather than in a spectrally disconnected band.
pub struct AttackNoise {
    amplitude: f64,
    decay_per_sample: f64,
    remaining: u32,
    bpf: Biquad,
    rng_state: u32,
}

impl AttackNoise {
    /// Create a new attack noise burst.
    ///
    /// - `velocity`: 0.0 to 1.0
    /// - `fundamental_hz`: fundamental frequency of the note (for tracking)
    /// - `sample_rate`: audio sample rate
    /// - `seed`: RNG seed (derive from note + counter to decorrelate simultaneous notes)
    pub fn new(velocity: f64, fundamental_hz: f64, sample_rate: f64, seed: u32) -> Self {
        let noise_amp = 0.025 * velocity * velocity;
        let tau = 0.003;
        let decay_per_sample = (-1.0 / (tau * sample_rate)).exp();
        let duration_samples = (0.015 * sample_rate) as u32;

        // Center frequency tracks the note: 5× fundamental, clamped to
        // 200–2000 Hz. Higher multiplier adds more "thwack" to the attack.
        // Keeps the noise in the harmonic neighborhood so it blends with
        // the reed tone instead of sounding like a disconnected "poof".
        let center = (fundamental_hz * 5.0).clamp(200.0, 2000.0);

        Self {
            amplitude: noise_amp,
            decay_per_sample,
            remaining: duration_samples,
            bpf: Biquad::bandpass(center, 0.7, sample_rate),
            rng_state: seed,
        }
    }

    /// Render attack noise into the output buffer (additive).
    /// Returns the number of samples actually rendered.
    pub fn render(&mut self, output: &mut [f64]) -> usize {
        let count = (self.remaining as usize).min(output.len());
        let mut amp = self.amplitude;

        for sample in &mut output[..count] {
            let noise = self.next_noise();
            let filtered = self.bpf.process(noise);
            *sample += amp * filtered;
            amp *= self.decay_per_sample;
        }

        self.amplitude = amp;
        self.remaining -= count as u32;
        count
    }

    /// Check if the noise burst is complete.
    pub fn is_done(&self) -> bool {
        self.remaining == 0
    }

    fn next_noise(&mut self) -> f64 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        (self.rng_state as i32 as f64) / (i32::MAX as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dwell_ff_brighter_than_pp() {
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let ff = dwell_attenuation(1.0, 262.0, &ratios);
        let pp = dwell_attenuation(0.1, 262.0, &ratios);

        for i in 1..NUM_MODES {
            assert!(ff[i] >= pp[i], "mode {i}: ff={} < pp={}", ff[i], pp[i]);
        }
    }

    #[test]
    fn test_dwell_fundamental_unity() {
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let atten = dwell_attenuation(0.5, 440.0, &ratios);
        assert!((atten[0] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_attack_noise_decays() {
        let mut noise = AttackNoise::new(1.0, 440.0, 44100.0, 0x12345678);
        let mut buf = vec![0.0f64; 700];
        noise.render(&mut buf);

        let start_energy: f64 = buf[..100].iter().map(|x| x * x).sum();
        let end_energy: f64 = buf[600..].iter().map(|x| x * x).sum();
        assert!(start_energy > end_energy * 5.0);
    }

    #[test]
    fn test_attack_noise_is_done() {
        let mut noise = AttackNoise::new(1.0, 440.0, 44100.0, 0x12345678);
        let mut buf = vec![0.0f64; 1000];
        noise.render(&mut buf);
        assert!(noise.is_done());
    }

    #[test]
    fn test_onset_ramp_register_dependent() {
        // Bass reeds should have longer onset than treble reeds
        let bass = onset_ramp_time(1.0, 65.0); // C2 ff
        let mid = onset_ramp_time(1.0, 262.0); // C4 ff
        let treble = onset_ramp_time(1.0, 1047.0); // C6 ff

        assert!(
            bass > mid,
            "bass onset ({bass:.4}) should exceed mid ({mid:.4})"
        );
        assert!(
            mid > treble,
            "mid onset ({mid:.4}) should exceed treble ({treble:.4})"
        );

        // C2 ff: 2.0 periods of 65 Hz = 30.8ms, clamped to 30ms ceiling
        assert!(
            (bass - 0.030).abs() < 0.001,
            "C2 ff should be 30ms (clamped), got {bass:.4}"
        );
        // Treble should hit the 2ms floor (2.0 periods of 1047 Hz = 1.9ms)
        assert!(
            (treble - 0.002).abs() < 1e-6,
            "C6 ff should clamp to 2ms, got {treble:.6}"
        );
        // C4 ff: 2.0/262 = 7.6ms (unclamped)
        assert!(
            (mid - 2.0 / 262.0).abs() < 0.001,
            "C4 ff should be ~7.6ms, got {mid:.4}"
        );
    }

    #[test]
    fn test_onset_ramp_velocity_dependent() {
        // pp should have longer onset than ff (softer hit = longer contact)
        let ff = onset_ramp_time(1.0, 262.0);
        let pp = onset_ramp_time(0.0, 262.0);

        assert!(pp > ff, "pp onset ({pp:.4}) should exceed ff ({ff:.4})");
        // ff = 2.0 periods, pp = 4.0 periods
        let expected_ff = 2.0 / 262.0;
        let expected_pp = 4.0 / 262.0;
        assert!((ff - expected_ff).abs() < 0.001);
        assert!((pp - expected_pp).abs() < 0.001);
    }
}
