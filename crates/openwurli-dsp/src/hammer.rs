/// Hammer model: Gaussian dwell filter + attack noise burst.
///
/// The dwell filter is a one-shot spectral shaping applied at note-on.
/// It models the finite contact duration of the felt hammer on the reed,
/// which acts as a low-pass filter on the initial mode excitation.
///
/// The attack noise is an exponentially decaying bandpass-filtered noise burst
/// that models the mechanical impact transient.

use crate::filters::Biquad;
use crate::tables::NUM_MODES;

/// Hammer dwell time (contact duration) in seconds.
///
/// Velocity-dependent: ff (vel=1.0) = 0.5ms, pp (vel=0.0) = 2.5ms.
/// This is both the spectral filter width (via `dwell_attenuation`) and the
/// onset ramp duration (via `ModalReed`'s raised-cosine ramp).
pub fn dwell_time(velocity: f64) -> f64 {
    0.0005 + 0.002 * (1.0 - velocity)
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
    let t_dwell = dwell_time(velocity);
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
/// Duration: ~15 ms. Band: 200 Hz to 5 kHz.
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
    /// - `sample_rate`: audio sample rate
    /// - `seed`: RNG seed (derive from note + counter to decorrelate simultaneous notes)
    pub fn new(velocity: f64, sample_rate: f64, seed: u32) -> Self {
        let noise_amp = 0.15 * velocity * velocity;
        let tau = 0.003;
        let decay_per_sample = (-1.0 / (tau * sample_rate)).exp();
        let duration_samples = (0.015 * sample_rate) as u32;

        Self {
            amplitude: noise_amp,
            decay_per_sample,
            remaining: duration_samples,
            bpf: Biquad::bandpass(1000.0, 1.0, sample_rate),
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
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
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
        let mut noise = AttackNoise::new(1.0, 44100.0, 0x12345678);
        let mut buf = vec![0.0f64; 700];
        noise.render(&mut buf);

        let start_energy: f64 = buf[..100].iter().map(|x| x * x).sum();
        let end_energy: f64 = buf[600..].iter().map(|x| x * x).sum();
        assert!(start_energy > end_energy * 5.0);
    }

    #[test]
    fn test_attack_noise_is_done() {
        let mut noise = AttackNoise::new(1.0, 44100.0, 0x12345678);
        let mut buf = vec![0.0f64; 1000];
        noise.render(&mut buf);
        assert!(noise.is_done());
    }

}
