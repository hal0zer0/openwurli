//! Wurlitzer 200A power amplifier — closed-loop negative feedback model.
//!
//! The real 200A has a differential input (TR-7/TR-8), VAS (TR-11),
//! driver/output stages (TR-10/TR-12) with R-31/R-30 negative feedback.
//! This feedback linearizes the transfer function dramatically:
//!
//!   - Open-loop gain A_ol ≈ 19,000 (diff pair × VAS × output stage)
//!   - Feedback factor β = R30/(R30+R31) = 220/15220 = 0.01445
//!   - Loop gain T = A_ol × β ≈ 275 at DC
//!   - Closed-loop gain = A_ol/(1+T) ≈ 69× (37 dB)
//!   - THD reduced by factor (1+T) ≈ 275× (49 dB) at DC
//!
//! The previous model applied gain THEN nonlinearity (open-loop behavior),
//! which created massive intermodulation on polyphonic material: tanh(sum/22)
//! compressed 12-voice pp sums by ~15%, generating audible buzzy artifacts.
//!
//! This model solves the implicit feedback equation per sample:
//!   y = f(A_ol × (input - β×y))
//! where f() = output stage crossover + tanh rail saturation.
//! Newton-Raphson converges in 2-4 iterations for typical signals.

/// Open-loop voltage gain: differential pair × VAS × output stage.
///
/// Estimated from the 200A discrete transistor topology:
///   - TR-7/TR-8 diff pair: gm×Rc ≈ 38mA/V × 1.8K ≈ 68×
///   - TR-11 VAS: ≈ 300× (loaded by output stage)
///   - Output stage: ≈ 0.95× (emitter follower)
///   - Total: ~19,000× (86 dB)
///
/// Closed-loop: 19000/(1 + 19000×0.01445) = 69.0× ✓
const OPEN_LOOP_GAIN: f64 = 19_000.0;

/// Feedback network: β = R30/(R30+R31) = 220Ω / (220Ω + 15KΩ) = 0.01445.
const FEEDBACK_BETA: f64 = 220.0 / (220.0 + 15_000.0);

/// Rail headroom: ±24V supply minus ~2V Vce_sat = ±22V effective swing.
const HEADROOM: f64 = 22.0;

/// Crossover thermal voltage (output stage BJTs).
///
/// Models the exponential I-V crossover of the push-pull output pair.
/// Effective dead zone ≈ ±2×vt = ±26mV at the output. With feedback,
/// the closed-loop crossover distortion is reduced by the loop gain.
///
///   Factory-fresh (10 mA bias): vt ≈ 0.004
///   Lightly aged (5-7 mA):      vt ≈ 0.013
///   Worn (2-3 mA):              vt ≈ 0.030
const CROSSOVER_VT: f64 = 0.013;

/// Quiescent gain of the output stage at zero signal.
///
/// Even at zero signal, the output transistors carry quiescent bias
/// current and have nonzero transconductance. This prevents the
/// crossover model from having zero gain at v=0 (which is unphysical
/// and causes NR convergence issues).
///
///   Factory-fresh (10 mA): ~0.3-0.5
///   Lightly aged (5-7 mA): ~0.1-0.2
///   Worn (2-3 mA):         ~0.02-0.05
const QUIESCENT_GAIN: f64 = 0.1;

/// Maximum Newton-Raphson iterations per sample.
const NR_MAX_ITER: usize = 8;

/// NR convergence threshold (volts in output domain).
const NR_TOL: f64 = 1e-6;

pub struct PowerAmp {
    open_loop_gain: f64,
    feedback_beta: f64,
    crossover_vt: f64,
    rail_limit: f64,
    closed_loop_gain: f64,
    quiescent_gain: f64,
}

impl PowerAmp {
    pub fn new() -> Self {
        Self {
            open_loop_gain: OPEN_LOOP_GAIN,
            feedback_beta: FEEDBACK_BETA,
            crossover_vt: CROSSOVER_VT,
            rail_limit: HEADROOM,
            closed_loop_gain: OPEN_LOOP_GAIN / (1.0 + OPEN_LOOP_GAIN * FEEDBACK_BETA),
            quiescent_gain: QUIESCENT_GAIN,
        }
    }

    pub fn process(&mut self, input: f64) -> f64 {
        // Initial guess: linear closed-loop, clamped within rails
        let mut y = (input * self.closed_loop_gain)
            .clamp(-self.rail_limit + NR_TOL, self.rail_limit - NR_TOL);

        for _ in 0..NR_MAX_ITER {
            let error = input - self.feedback_beta * y;
            let v = self.open_loop_gain * error;

            let (f_val, f_deriv) = self.forward_path(v);

            // Residual: g(y) = y - f(A_ol × (input - β×y)) = 0
            let residual = y - f_val;

            // Jacobian: dg/dy = 1 + A_ol × β × f'(v)
            let jacobian = 1.0 + self.open_loop_gain * self.feedback_beta * f_deriv;

            let delta = residual / jacobian;
            y -= delta;

            if delta.abs() < NR_TOL {
                break;
            }
        }

        // Normalize to [-1, 1]
        y / self.rail_limit
    }

    /// Open-loop forward path: crossover distortion + tanh rail saturation.
    /// Returns (output_voltage, derivative_wrt_v).
    #[inline]
    fn forward_path(&self, v: f64) -> (f64, f64) {
        // Crossover: gain = q + (1-q) × (1 - exp(-v²/vt²))
        //
        // At v=0: gain = q (quiescent — not zero)
        // At |v| >> vt: gain → 1.0
        let v_sq = v * v;
        let vt_sq = self.crossover_vt * self.crossover_vt;
        let exp_term = (-v_sq / vt_sq).exp();
        let q = self.quiescent_gain;
        let cross_gain = q + (1.0 - q) * (1.0 - exp_term);
        let v_cross = v * cross_gain;

        // Crossover derivative: d(v_cross)/dv
        let dcross_dv = cross_gain + v * (1.0 - q) * (2.0 * v / vt_sq) * exp_term;

        // Rail saturation: rail × tanh(v_cross / rail)
        let tanh_arg = v_cross / self.rail_limit;
        let tanh_val = tanh_arg.tanh();
        let f_val = self.rail_limit * tanh_val;

        // Chain rule: df/dv = sech²(v_cross/rail) × dcross_dv
        let f_deriv = (1.0 - tanh_val * tanh_val) * dcross_dv;

        (f_val, f_deriv)
    }

    pub fn reset(&mut self) {
        // Memoryless — nothing to reset
    }
}

impl Default for PowerAmp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_closed_loop_gain() {
        let mut pa = PowerAmp::new();
        // Small input well below rails — feedback keeps output linear.
        // Expected: input × 69 / 22 (closed-loop gain, normalized)
        let input = 0.001;
        let output = pa.process(input);
        let expected = input * pa.closed_loop_gain / HEADROOM;
        let error_pct = ((output - expected) / expected).abs() * 100.0;
        assert!(
            error_pct < 1.0,
            "Closed-loop gain should be ~69x: expected {expected:.6}, got {output:.6}, error {error_pct:.2}%"
        );
    }

    #[test]
    fn test_feedback_linearization() {
        let mut pa = PowerAmp::new();
        // Moderate input that causes significant tanh compression without
        // feedback. With feedback, the loop compensates and output tracks
        // the linear target closely.
        let input = 0.2; // 0.2 × 69 = 13.8V (below 22V rails)
        let output = pa.process(input);
        let linear = input * pa.closed_loop_gain / HEADROOM;
        let error_pct = ((output - linear) / linear).abs() * 100.0;
        assert!(
            error_pct < 5.0,
            "Feedback should linearize: linear={linear:.4}, got={output:.4}, error={error_pct:.1}%"
        );
    }

    #[test]
    fn test_rail_clipping() {
        let mut pa = PowerAmp::new();
        // Large input drives output to rails. Feedback can't help here —
        // the output stage is saturated.
        let output = pa.process(5.0);
        assert!(
            output > 0.95 && output <= 1.0,
            "Should clip near 1.0: got {output}"
        );
        let output_neg = pa.process(-5.0);
        assert!(
            output_neg < -0.95 && output_neg >= -1.0,
            "Should clip near -1.0: got {output_neg}"
        );
    }

    #[test]
    fn test_symmetry() {
        let mut pa = PowerAmp::new();
        let pos = pa.process(0.05);
        let neg = pa.process(-0.05);
        assert!(
            (pos + neg).abs() < 1e-10,
            "Should be symmetric: {pos} + {neg} = {}",
            pos + neg
        );
    }

    #[test]
    fn test_crossover_reduced_by_feedback() {
        // With feedback, crossover distortion is greatly reduced compared to
        // open-loop. H3 should be present but very small.
        use std::f64::consts::PI;

        let mut pa = PowerAmp::new();
        let sr = 44100.0;
        let freq = 440.0;
        let amplitude = 0.001;

        let n = (sr * 0.2) as usize;
        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let x = amplitude * (2.0 * PI * freq * i as f64 / sr).sin();
            samples.push(pa.process(x));
        }

        let start = n / 2;
        let slice = &samples[start..];
        let f1 = dft_mag(slice, freq, sr);
        let f3 = dft_mag(slice, 3.0 * freq, sr);

        let h3_db = 20.0 * (f3 / f1).log10();
        // Feedback should suppress H3 below -40 dB
        assert!(
            h3_db < -40.0,
            "Feedback should suppress H3 below -40 dB: got {h3_db:.1} dB"
        );
    }

    #[test]
    fn test_nr_converges() {
        // Verify NR produces reasonable output across the full input range
        let mut pa = PowerAmp::new();
        for &input in &[0.0, 0.001, 0.01, 0.1, 0.2, 0.3, 0.5, 1.0, -0.1, -0.3] {
            let output = pa.process(input);
            assert!(
                output.is_finite() && output.abs() <= 1.0,
                "NR should converge for input {input}: got {output}"
            );
            // Sign should match input
            if input.abs() > 1e-10 {
                assert!(
                    output.signum() == input.signum(),
                    "Output sign should match input: input={input}, output={output}"
                );
            }
        }
    }

    fn dft_mag(samples: &[f64], freq: f64, sr: f64) -> f64 {
        use std::f64::consts::PI;
        let n = samples.len() as f64;
        let mut re = 0.0;
        let mut im = 0.0;
        for (i, &s) in samples.iter().enumerate() {
            let phase = 2.0 * PI * freq * i as f64 / sr;
            re += s * phase.cos();
            im += s * phase.sin();
        }
        (re * re + im * im).sqrt() / n
    }
}
