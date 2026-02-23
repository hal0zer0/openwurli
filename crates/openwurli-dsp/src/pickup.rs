//! Electrostatic pickup model — time-varying RC circuit.
//!
//! The Wurlitzer 200A pickup is a capacitive sensor: reed vibration modulates
//! the capacitance between the reed and a charged metal plate (+147V DC).
//!
//! The reed-plate capacitance varies nonlinearly with displacement:
//!   C(y) = C_0 / (1 - y)
//! where y = x/d_0 is the normalized displacement (fraction of rest gap,
//! positive toward the plate).
//!
//! Unlike the old model which applied y/(1-y) then a separate HPF, this model
//! discretizes the actual RC circuit with bilinear transform, coupling the
//! nonlinearity and filtering into a single physical system:
//!
//!   R_total * C(y) * dV/dt + V = V_hv
//!
//! Normalized charge q = Q/(C_0 * V_eq), equilibrium at q=1. The bilinear
//! discretization with time-varying capacitance c_n = 1/(1-y):
//!
//!   alpha = beta / c_n = beta * (1 - y)
//!   q_next = (q * (1 - alpha) + 2*beta) / (1 + alpha)
//!   output = (1 - q_next/c_n) * SENSITIVITY
//!
//! This produces:
//! - Identical small-signal HPF at f_c = 1/(2π*R*C_0) = 2312 Hz
//! - Coupled nonlinear harmonic generation (H2 from capacitance modulation)
//! - Frequency-dependent nonlinearity (stronger near/below RC corner)
//! - Correct asymmetry (positive y amplified more than negative)

/// RC time constant: R_total * C_0
/// R_total = R_feed (1M) || (R-1 + R-2||R-3) = 1M || 402K = 287K
/// C_0 = 240 pF (rest capacitance)
const TAU: f64 = 287.0e3 * 240.0e-12; // 68.88 µs → f_c = 2312 Hz

/// Pickup sensitivity: V_hv * C_0 / (C_0 + C_p) = 147 * 3/240 = 1.8375 V
/// Applied to the AC voltage perturbation from charge dynamics.
pub const PICKUP_SENSITIVITY: f64 = 1.8375;

/// Maximum allowed displacement fraction (safety clamp).
/// The reed physically cannot touch the plate (y=1.0 is a singularity in c_n).
///
/// The old static model needed a tight clamp (0.90) because y/(1-y) at 0.90 = 9.0
/// produced huge intermediate signals. The time-varying RC model self-limits via
/// charge dynamics — output is bounded at ~±SENSITIVITY regardless of y, so we
/// can safely allow y closer to 1.0. At y=0.98, c_n=50, alpha=0.008 — numerically
/// well-behaved. Only y→1.0 is a true singularity.
///
/// With DS_CLAMP=(0.02, 0.82) and reed onset peaks up to ~1.05, y_raw can reach
/// ~0.86. The 0.98 clamp provides headroom without hard-clipping onset transients.
pub const PICKUP_MAX_Y: f64 = 0.98;

/// Convert reed model displacement units to physical y = x/d_0.
///
/// NOTE: This default is overridden per-note by tables::pickup_displacement_scale()
/// in voice.rs. Only used if set_displacement_scale() is never called.
const DISPLACEMENT_SCALE: f64 = 0.85;

pub struct Pickup {
    /// Normalized charge state (equilibrium = 1.0).
    q: f64,
    /// Precomputed: dt / (2 * TAU). Bilinear integration coefficient.
    beta: f64,
    displacement_scale: f64,
}

impl Pickup {
    pub fn new(sample_rate: f64) -> Self {
        Self::new_with_scale(sample_rate, DISPLACEMENT_SCALE)
    }

    /// Construct with explicit displacement scale (for bark-audit/calibrate tools).
    pub fn new_with_scale(sample_rate: f64, displacement_scale: f64) -> Self {
        let dt = 1.0 / sample_rate;
        let beta = dt / (2.0 * TAU);
        Self {
            q: 1.0,
            beta,
            displacement_scale,
        }
    }

    /// Override the displacement scale (default: 0.85).
    /// Higher = tighter gap = more nonlinearity = more bark.
    pub fn set_displacement_scale(&mut self, scale: f64) {
        self.displacement_scale = scale;
    }

    /// Process a buffer of reed displacement samples in-place.
    ///
    /// Input: reed displacement in normalized model units.
    /// Output: pickup voltage in volts (millivolt-scale signals).
    ///
    /// The time-varying RC circuit couples the 1/(1-y) capacitance nonlinearity
    /// with the charge dynamics, producing frequency-dependent harmonic generation.
    /// At frequencies well below the RC corner (2312 Hz), the circuit generates
    /// H2 proportional to displacement² (same as the static y/(1-y) model).
    /// At frequencies near/above the corner, the charge can't follow the fast
    /// capacitance changes, reducing the nonlinear contribution — physically
    /// correct behavior that the old separated model couldn't capture.
    pub fn process(&mut self, buffer: &mut [f64]) {
        let scale = self.displacement_scale;
        let beta = self.beta;
        for sample in buffer.iter_mut() {
            let y = (*sample * scale).clamp(-PICKUP_MAX_Y, PICKUP_MAX_Y);
            // Eliminate c_n = 1/(1-y) division: use (1-y) directly.
            // alpha = beta / c_n = beta * (1-y)
            let one_minus_y = 1.0 - y;
            let alpha = beta * one_minus_y;
            // Bilinear (trapezoidal) integration of: TAU * dq/dt = 1 - q/c_n
            // Driving term is 2*beta (from the constant V_hv source), NOT 2*alpha
            let q_next = (self.q * (1.0 - alpha) + 2.0 * beta) / (1.0 + alpha);
            self.q = q_next;
            // Output: (q/c_n - 1) = (q*(1-y) - 1) — no division needed
            *sample = (q_next * one_minus_y - 1.0) * PICKUP_SENSITIVITY;
        }
    }

    pub fn reset(&mut self) {
        self.q = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_dc_equilibrium() {
        // Zero displacement should produce zero output (DC blocked by RC).
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let n = (sr * 0.05) as usize;
        let mut buf = vec![0.0f64; n];
        pickup.process(&mut buf);

        let peak = buf.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(
            peak < 1e-10,
            "zero displacement should produce zero output, got peak={peak:.2e}"
        );
    }

    #[test]
    fn test_frequency_response_matches_rc() {
        // Small-signal sweep: the time-varying RC should match a 1-pole HPF at 2312 Hz
        // within ~1 dB for small amplitudes (linear regime).
        let sr = 44100.0;
        let fc = 1.0 / (2.0 * PI * TAU); // 2312 Hz
        let amplitude = 0.01; // Very small — linear regime (y_peak = 0.0085)

        for &freq in &[100.0, 500.0, 1000.0, 2312.0, 5000.0, 10000.0] {
            let mut pickup = Pickup::new(sr);
            let n = (sr * 0.1) as usize;
            let mut buf: Vec<f64> = (0..n)
                .map(|i| amplitude * (2.0 * PI * freq * i as f64 / sr).sin())
                .collect();
            pickup.process(&mut buf);

            let steady = &buf[n / 2..];
            let measured = steady.iter().map(|x| x.abs()).fold(0.0f64, f64::max);

            // Expected: amplitude * DS * SENSITIVITY * HPF_gain
            // For small y: output ≈ HPF(y) * SENSITIVITY = HPF(amplitude*DS*sin) * S
            let y_amp = amplitude * DISPLACEMENT_SCALE;
            let hpf_gain = freq / (freq * freq + fc * fc).sqrt();
            let expected = y_amp * PICKUP_SENSITIVITY * hpf_gain;

            let ratio_db = 20.0 * (measured / expected).log10();
            // 2 dB tolerance: bilinear transform has frequency warping vs analog HPF
            assert!(
                ratio_db.abs() < 2.0,
                "at {freq} Hz: measured={measured:.6}, expected={expected:.6}, error={ratio_db:.2} dB"
            );
        }
    }

    #[test]
    fn test_hpf_passes_high_freq() {
        // At 10 kHz, the time-varying RC passes high-freq signals.
        // For the RC model, at very high frequencies q can't follow c_n,
        // so output ≈ y * SENSITIVITY (reduced from old y/(1-y) * S).
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let freq = 10000.0;

        let n = (sr * 0.05) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        let peak = buf[n / 2..].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        // At 10 kHz with DS=0.85: y approaches MAX_Y.
        // The RC model at high freq gives output ~ y * SENSITIVITY for small y,
        // but for large y the nonlinear charge dynamics still produce amplification.
        assert!(peak > 0.5, "pickup output too low at 10kHz: {peak}");
        assert!(peak < 12.0, "pickup output too high at 10kHz: {peak}");
    }

    #[test]
    fn test_hpf_attenuates_bass() {
        // At 100 Hz, the RC circuit's charge tracks the capacitance changes,
        // attenuating the output — same HPF behavior as before.
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let freq = 100.0;

        let n = (sr * 0.1) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        let peak = buf[n / 2..].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(peak < 0.65, "pickup should heavily attenuate 100Hz: {peak}");
    }

    #[test]
    fn test_nonlinearity_produces_h2() {
        // Drive the pickup with a large-amplitude sine and verify H2 > H3.
        // The time-varying capacitance generates even harmonics.
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let freq = 2000.0; // Near HPF corner for strong nonlinear coupling

        let amplitude = 1.0;
        let n = (sr * 0.2) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| amplitude * (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        let start = n * 3 / 4;
        let signal = &buf[start..];
        let h1 = dft_magnitude(signal, freq, sr);
        let h2 = dft_magnitude(signal, 2.0 * freq, sr);
        let h3 = dft_magnitude(signal, 3.0 * freq, sr);

        assert!(
            h2 > h3,
            "H2 ({h2:.2e}) should dominate H3 ({h3:.2e}) from capacitance modulation"
        );
        let h2_ratio = h2 / h1;
        assert!(
            h2_ratio > 0.05,
            "H2/H1 too low ({h2_ratio:.4}), expected >5% from nonlinearity"
        );
    }

    #[test]
    fn test_asymmetry() {
        // The time-varying RC should produce asymmetric output.
        // Must test BELOW the RC corner (2312 Hz) where charge dynamics
        // interact with the asymmetric capacitance function. Above the corner,
        // charge can't follow and output approaches linear y (no asymmetry) —
        // this is physically correct and different from the old static model.
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let freq = 500.0; // Well below HPF corner — strong nonlinear coupling

        let amplitude = 0.5; // y_peak = 0.5 * 0.85 = 0.425, no clipping
        let n = (sr * 0.2) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| amplitude * (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        let pos_peak = buf[n / 2..].iter().cloned().fold(0.0f64, f64::max);
        let neg_peak = buf[n / 2..].iter().cloned().fold(0.0f64, f64::min).abs();

        // Positive excursion (toward plate) should produce larger signal
        // because C(y) = C_0/(1-y) amplifies positive displacements more.
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
