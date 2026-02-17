/// Single BJT common-emitter stage — NR solver + asymmetric soft-clip.
///
/// Models the Ebers-Moll exponential transfer function:
///   raw = (A/B) * (exp(B * vbe_eff) - 1)
///   where A = gm*Rc (open-loop gain), B = 1/(n*Vt) = 38.5
///
/// For small signals: raw ≈ A * vbe_eff (linearized gain).
///
/// With local emitter degeneration (Stage 2):
///   vbe_eff = input - k * raw   where k = Re_unbypassed / Rc
///   Solved iteratively via Newton-Raphson.
///
/// Asymmetric soft-clip models collector voltage rail limits:
///   - Positive raw (toward saturation): limited by Vce - Vce_sat
///   - Negative raw (toward cutoff): limited by Vcc - Vc
///
/// Both stages use PostNonlinearity flow (exp_transfer -> soft_clip -> Miller LPF),
/// matching the real circuit where Miller capacitance limits the gain-bandwidth
/// of the overall stage (not the input to the nonlinearity).
///
/// Stage 1 uses TptLpf (bilinear) for accurate phase at the 23 Hz dominant pole,
/// which enables ZDF convergence (~50% instantaneous coupling vs 0.16% for forward Euler).
/// Stage 2 uses forward Euler because its 81 kHz Miller pole is near Nyquist —
/// discretization error is negligible and TPT's tan() hits frequency warping.

use crate::filters::{LpfState, OnePoleLpf, TptLpf, TptLpfState};

/// Miller filter variants — TPT (bilinear) or forward Euler.
enum MillerFilter {
    Tpt(TptLpf),
    ForwardEuler(OnePoleLpf),
}

/// Snapshot of Miller filter state for ZDF iteration.
#[derive(Clone, Copy)]
enum MillerState {
    Tpt(TptLpfState),
    ForwardEuler(LpfState),
}

/// Snapshot of BjtStage state for ZDF feedback iteration.
#[derive(Clone, Copy)]
pub struct BjtState {
    miller_state: MillerState,
    prev_raw: f64,
}

pub struct BjtStage {
    /// Exponential scale factor: A/B = gm*Rc / (1/(n*Vt)) = Ic_q * Rc * (n*Vt)
    scale: f64,
    /// Exponential coefficient: 1/(n*Vt) = 38.5
    b: f64,
    /// Local emitter degeneration fraction: Re_unbypassed / Rc
    /// Stage 1: 0 (external feedback via R-10)
    /// Stage 2: 820/1800 = 0.456
    k: f64,
    /// Headroom toward saturation (Vce - Vce_sat): positive output limit
    /// Stage 1: 2.05V, Stage 2: 5.3V
    pos_limit: f64,
    /// Headroom toward cutoff (Vcc - Vc): negative output limit
    /// Stage 1: 10.9V, Stage 2: 6.2V
    neg_limit: f64,
    /// Miller-effect dominant pole LPF
    miller: MillerFilter,
    /// Previous raw output (NR initial guess)
    prev_raw: f64,
}

impl BjtStage {
    /// Create Stage 1 (TR-1): high-gain, high-asymmetry.
    ///
    /// Uses TptLpf (bilinear) for accurate phase at the 23 Hz dominant pole:
    ///   exp_transfer -> soft_clip -> Miller_TPT(23 Hz) -> output
    ///
    /// Emitter feedback from R-10 is handled externally by the preamp assembly.
    pub fn stage1(sample_rate: f64) -> Self {
        Self {
            scale: 420.0 / 38.5,  // gm1 * Rc1 / B = 2.80 mA/V * 150K / 38.5
            b: 38.5,              // 1 / (1.0 * 0.026V)
            k: 0.0,               // No local degeneration (R-10 feedback is external)
            pos_limit: 2.05,      // Vce1 - Vce_sat = 2.15 - 0.10 (saturation)
            neg_limit: 10.9,      // Vcc - Vc1 = 15.0 - 4.1 (cutoff)
            miller: MillerFilter::Tpt(TptLpf::new(23.0, sample_rate)),
            prev_raw: 0.0,
        }
    }

    /// Create Stage 2 (TR-2): low-gain buffer with 820 ohm unbypassed emitter.
    ///
    /// Uses forward Euler for the 81 kHz Miller pole (near Nyquist):
    ///   exp_transfer -> NR_solve -> soft_clip -> Miller_FE(81 kHz) -> output
    ///
    /// Forward Euler is adequate because discretization error is negligible
    /// at 81 kHz, and TPT's tan() would hit frequency warping issues.
    pub fn stage2(sample_rate: f64) -> Self {
        Self {
            scale: 238.0 / 38.5,  // gm2 * Rc2 / B = 127 mA/V * 1.8K / 38.5
            b: 38.5,              // 1 / (1.0 * 0.026V)
            k: 0.456,             // Re2b / Rc2 = 820 / 1800
            pos_limit: 5.3,       // Vce2 - Vce_sat = 5.4 - 0.10 (saturation)
            neg_limit: 6.2,       // Vcc - Vc2 = 15.0 - 8.8 (cutoff)
            miller: MillerFilter::ForwardEuler(OnePoleLpf::new(81_000.0, sample_rate)),
            prev_raw: 0.0,
        }
    }

    /// Exponential transfer function: raw = (A/B) * (exp(B * vbe_eff) - 1).
    ///
    /// For k=0: direct solution. For k>0: Newton-Raphson iteration.
    fn compute_raw(&self, input_eff: f64) -> f64 {
        if self.k < 1e-10 {
            let arg = (self.b * input_eff).clamp(-20.0, 20.0);
            self.scale * (arg.exp() - 1.0)
        } else {
            // NR solver for implicit equation with local degeneration.
            // Linearized initial guess: raw = A*x / (1 + A*k)
            let a = self.scale * self.b;
            let mut raw = a * input_eff / (1.0 + a * self.k);
            for _ in 0..4 {
                let arg = (self.b * (input_eff - self.k * raw)).clamp(-20.0, 20.0);
                let exp_val = arg.exp();
                let f = self.scale * (exp_val - 1.0) - raw;
                let df = -self.scale * self.b * self.k * exp_val - 1.0;
                raw -= f / df;
            }
            raw
        }
    }

    /// Asymmetric exponential soft-clip for collector rail limits.
    fn soft_clip(&self, raw: f64) -> f64 {
        if raw >= 0.0 {
            self.pos_limit * (1.0 - (-raw / self.pos_limit).exp())
        } else {
            -self.neg_limit * (1.0 - (raw / self.neg_limit).exp())
        }
    }

    /// Process one sample.
    ///
    /// Signal flow: exp_transfer -> soft_clip -> Miller LPF -> output
    ///
    /// - `input`: Base drive voltage (small-signal AC)
    /// - `fb`: External feedback subtracted from input (emitter feedback from R-10)
    pub fn process(&mut self, input: f64, fb: f64) -> f64 {
        let input_eff = input - fb;
        let raw = self.compute_raw(input_eff);
        let clipped = self.soft_clip(raw);
        self.prev_raw = raw;
        self.miller_process(clipped)
    }

    fn miller_process(&mut self, x: f64) -> f64 {
        match &mut self.miller {
            MillerFilter::Tpt(f) => f.process(x),
            MillerFilter::ForwardEuler(f) => f.process(x),
        }
    }

    /// Save stage state for ZDF feedback iteration.
    pub fn save_state(&self) -> BjtState {
        let miller_state = match &self.miller {
            MillerFilter::Tpt(f) => MillerState::Tpt(f.save_state()),
            MillerFilter::ForwardEuler(f) => MillerState::ForwardEuler(f.save_state()),
        };
        BjtState {
            miller_state,
            prev_raw: self.prev_raw,
        }
    }

    /// Restore previously saved stage state.
    pub fn restore_state(&mut self, state: BjtState) {
        match (&mut self.miller, state.miller_state) {
            (MillerFilter::Tpt(f), MillerState::Tpt(s)) => f.restore_state(s),
            (MillerFilter::ForwardEuler(f), MillerState::ForwardEuler(s)) => f.restore_state(s),
            _ => unreachable!("Miller filter type mismatch in restore_state"),
        }
        self.prev_raw = state.prev_raw;
    }

    pub fn reset(&mut self) {
        match &mut self.miller {
            MillerFilter::Tpt(f) => f.reset(),
            MillerFilter::ForwardEuler(f) => f.reset(),
        }
        self.prev_raw = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_stage1_small_signal_gain() {
        let sr = 88200.0;
        let mut stage = BjtStage::stage1(sr);

        // Apply a sustained DC step and wait for the 23 Hz Miller LPF to settle.
        // Time constant = 1/(2*pi*23) = 6.9ms. Wait 50ms (>7 time constants).
        let input = 0.00001; // 10 uV — deep in linear region
        let n = (sr * 0.05) as usize;
        let mut output = 0.0;
        for _ in 0..n {
            output = stage.process(input, 0.0);
        }

        // Small-signal gain should be ~420 (= A = gm * Rc)
        let ratio = output / input;
        assert!(ratio > 300.0, "Stage 1 small-signal gain too low: {ratio}");
        assert!(ratio < 500.0, "Stage 1 small-signal gain too high: {ratio}");
    }

    #[test]
    fn test_stage1_asymmetric_clipping() {
        let sr = 88200.0;
        let mut stage = BjtStage::stage1(sr);

        // Positive input -> positive raw -> limited by pos_limit (2.05V, less headroom)
        let pos_out = stage.process(0.01, 0.0);
        stage.reset();

        // Negative input -> negative raw -> limited by neg_limit (10.9V, more headroom)
        let neg_out = stage.process(-0.01, 0.0);

        // Negative output should be larger in magnitude (more headroom)
        assert!(
            neg_out.abs() > pos_out.abs(),
            "Expected asymmetric clipping: pos={pos_out:.4} neg={neg_out:.4}"
        );
    }

    #[test]
    fn test_stage2_gain() {
        let sr = 88200.0;
        let mut stage = BjtStage::stage2(sr);

        // Small signal through Stage 2
        // Degenerated gain = A / (1 + A*k) = 238 / (1 + 238*0.456) = 238/109.6 = 2.17
        let input = 0.001;
        let output = stage.process(input, 0.0);

        let ratio = output / input;
        assert!(ratio > 1.5, "Stage 2 gain too low: {ratio}");
        assert!(ratio < 4.0, "Stage 2 gain too high: {ratio}");
    }

    #[test]
    fn test_stage2_nearly_symmetric() {
        let sr = 88200.0;
        let mut stage = BjtStage::stage2(sr);

        // Use sustained signals to let the 81 kHz Miller LPF settle.
        // Stage 2's Miller pole is very high, but use sustained input anyway
        // for consistency and to give the NR solver multiple iterations.
        let n = (sr * 0.005) as usize; // 5ms
        let mut pos_out = 0.0;
        for _ in 0..n {
            pos_out = stage.process(0.1, 0.0);
        }

        stage.reset();
        let mut neg_out = 0.0;
        for _ in 0..n {
            neg_out = stage.process(-0.1, 0.0);
        }

        // Stage 2 asymmetry: pos_limit=5.3V, neg_limit=6.2V -> ratio 1.17:1
        let ratio = pos_out.abs() / neg_out.abs();
        assert!(ratio > 0.5, "Stage 2 too asymmetric: ratio={ratio}");
        assert!(ratio < 2.0, "Stage 2 too asymmetric: ratio={ratio}");
    }

    #[test]
    fn test_h2_dominates_h3() {
        // Stage 1 with no feedback should produce predominantly H2
        let sr = 88200.0;
        let mut stage = BjtStage::stage1(sr);

        let freq = 440.0;
        let n_samples = (sr * 0.2) as usize;
        let mut output = vec![0.0f64; n_samples];

        // Drive into nonlinear region — the exponential itself produces H2
        // At 0.5 mV, raw ≈ 420 * 0.0005 = 0.21V (well within soft-clip range)
        // but the exponential nonlinearity generates harmonics
        for i in 0..n_samples {
            let t = i as f64 / sr;
            let input = 0.0005 * (2.0 * PI * freq * t).sin();
            output[i] = stage.process(input, 0.0);
        }

        // DFT at H2 and H3 (last quarter for steady state)
        let start = n_samples * 3 / 4;
        let h2 = dft_magnitude(&output[start..], 2.0 * freq, sr);
        let h3 = dft_magnitude(&output[start..], 3.0 * freq, sr);

        assert!(
            h2 > h3,
            "H2 ({h2:.2e}) should dominate H3 ({h3:.2e}) — Wurli bark"
        );
    }

    #[test]
    fn test_stability_impulse() {
        let sr = 88200.0;
        let mut stage = BjtStage::stage1(sr);

        // Small impulse (within linear range)
        let _ = stage.process(0.0001, 0.0);

        // The Miller LPF at 23 Hz has a long time constant (~7ms).
        // Need enough samples to settle.
        let mut last = 0.0;
        for _ in 0..(sr * 0.5) as usize {
            last = stage.process(0.0, 0.0);
        }

        assert!(
            last.abs() < 1e-6,
            "Stage should decay to zero after impulse, got {last}"
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
        ((re / n).powi(2) + (im / n).powi(2)).sqrt()
    }
}
