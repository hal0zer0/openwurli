/// Hammer model: Gaussian dwell filter + attack noise burst.
///
/// The dwell filter is a one-shot spectral shaping applied at note-on.
/// It models the finite contact duration of the felt hammer on the reed,
/// which acts as a low-pass filter on the initial mode excitation.
///
/// The attack noise is an exponentially decaying bandpass-filtered noise burst
/// that models the mechanical impact transient.

use crate::tables::NUM_MODES;

/// Compute per-mode attenuation from the Gaussian dwell filter.
///
/// Uses a wide Gaussian (sigma=8 in f*T units) per the recommendation in
/// Section 4.3.4 of reed-and-hammer-physics.md. This is gentler than the
/// original sigma=2.5, preserving upper mode content as physics dictates.
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
    // Contact duration: shorter at higher velocity (harder strike = faster rebound)
    // Range: 0.5 ms (ff) to 2.5 ms (pp) — Section 4.2
    let t_dwell = 0.0005 + 0.002 * (1.0 - velocity);

    // Wide Gaussian spectral envelope: exp(-(f*T)^2 / (2*sigma^2))
    // sigma = 8.0 in normalized f*T units (Section 4.3.4)
    let sigma_sq = 8.0 * 8.0;

    let mut atten = [0.0f64; NUM_MODES];
    for i in 0..NUM_MODES {
        let ft = fundamental_hz * mode_ratios[i] * t_dwell;
        atten[i] = (-ft * ft / (2.0 * sigma_sq)).exp();
    }

    // Normalize to fundamental
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
    /// Amplitude envelope: noise_amp * exp(-t/tau)
    amplitude: f64,
    /// Decay rate in amplitude/sample
    decay_per_sample: f64,
    /// Remaining samples to render
    remaining: u32,
    /// Biquad bandpass filter state
    bpf: BiquadBpf,
    /// Simple LCG PRNG state
    rng_state: u32,
}

impl AttackNoise {
    /// Create a new attack noise burst.
    ///
    /// - `velocity`: 0.0 to 1.0
    /// - `sample_rate`: audio sample rate
    pub fn new(velocity: f64, sample_rate: f64) -> Self {
        let noise_amp = 0.15 * velocity * velocity;
        let tau = 0.003; // 3 ms time constant
        let decay_per_sample = (-1.0 / (tau * sample_rate)).exp();
        let duration_samples = (0.015 * sample_rate) as u32; // 15 ms

        // Bandpass: center at geometric mean of 200 and 5000 = 1000 Hz, Q ~ 1.0
        let center_freq = 1000.0;
        let q = 1.0;

        Self {
            amplitude: noise_amp,
            decay_per_sample,
            remaining: duration_samples,
            bpf: BiquadBpf::new(center_freq, q, sample_rate),
            rng_state: 0x12345678,
        }
    }

    /// Render attack noise into the output buffer (additive).
    /// Returns the number of samples actually rendered.
    pub fn render(&mut self, output: &mut [f64]) -> usize {
        let count = (self.remaining as usize).min(output.len());
        let mut amp = self.amplitude;

        for sample in &mut output[..count] {
            // White noise from simple LCG
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
        // LCG: simple, fast, adequate for noise
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        // Map to -1.0..1.0
        (self.rng_state as i32 as f64) / (i32::MAX as f64)
    }
}

/// Simple biquad bandpass filter.
struct BiquadBpf {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl BiquadBpf {
    fn new(center_freq: f64, q: f64, sample_rate: f64) -> Self {
        let w0 = 2.0 * std::f64::consts::PI * center_freq / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();

        let b0 = alpha;
        let b1 = 0.0;
        let b2 = -alpha;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
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

        // At ff, upper modes should be more present (less attenuation)
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
        let mut noise = AttackNoise::new(1.0, 44100.0);
        let mut buf = vec![0.0f64; 700]; // ~15 ms at 44100
        noise.render(&mut buf);

        // Energy at start should be larger than at end
        let start_energy: f64 = buf[..100].iter().map(|x| x * x).sum();
        let end_energy: f64 = buf[600..].iter().map(|x| x * x).sum();
        assert!(start_energy > end_energy * 5.0);
    }

    #[test]
    fn test_attack_noise_is_done() {
        let mut noise = AttackNoise::new(1.0, 44100.0);
        let mut buf = vec![0.0f64; 1000];
        noise.render(&mut buf);
        assert!(noise.is_done());
    }
}
