//! Wurlitzer 200A power amplifier — feature-toggled between circuit and behavioral models.
//!
//! Default: melange-generated 7-BJT Class AB circuit solver.
//! `--features legacy-power-amp`: behavioral closed-loop NR approximation (A/B diagnostics only).

#[cfg(feature = "legacy-power-amp")]
mod behavioral {
    //! Behavioral closed-loop negative feedback model.

    const OPEN_LOOP_GAIN: f64 = 19_000.0;
    const FEEDBACK_BETA: f64 = 220.0 / (220.0 + 15_000.0);
    const HEADROOM: f64 = 22.0;
    const CROSSOVER_VT: f64 = 0.013;
    const QUIESCENT_GAIN: f64 = 0.1;
    const NR_MAX_ITER: usize = 8;
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
            let mut y = (input * self.closed_loop_gain)
                .clamp(-self.rail_limit + NR_TOL, self.rail_limit - NR_TOL);

            for _ in 0..NR_MAX_ITER {
                let error = input - self.feedback_beta * y;
                let v = self.open_loop_gain * error;
                let (f_val, f_deriv) = self.forward_path(v);
                let residual = y - f_val;
                let jacobian = 1.0 + self.open_loop_gain * self.feedback_beta * f_deriv;
                let delta = residual / jacobian;
                y -= delta;
                if delta.abs() < NR_TOL {
                    break;
                }
            }

            y / self.rail_limit
        }

        #[inline]
        fn forward_path(&self, v: f64) -> (f64, f64) {
            let v_sq = v * v;
            let vt_sq = self.crossover_vt * self.crossover_vt;
            let exp_term = (-v_sq / vt_sq).exp();
            let q = self.quiescent_gain;
            let cross_gain = q + (1.0 - q) * (1.0 - exp_term);
            let v_cross = v * cross_gain;
            let dcross_dv = cross_gain + v * (1.0 - q) * (2.0 * v / vt_sq) * exp_term;
            let tanh_arg = v_cross / self.rail_limit;
            let tanh_val = tanh_arg.tanh();
            let f_val = self.rail_limit * tanh_val;
            let f_deriv = (1.0 - tanh_val * tanh_val) * dcross_dv;
            (f_val, f_deriv)
        }

        /// Behavioral model has no solver, no divergence to detect — returns
        /// the same clamped result as `process`. Exists only for API parity
        /// with the melange adapter's diagnostic probe.
        pub fn diag_raw_process(&mut self, input: f64) -> f64 {
            self.process(input)
        }

        pub fn diag_snapshot(&self) -> (u64, u64, f64) {
            (0, 0, 0.0)
        }

        pub fn reset(&mut self) {}
    }

    impl Default for PowerAmp {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(feature = "legacy-power-amp")]
pub use behavioral::PowerAmp;

#[cfg(not(feature = "legacy-power-amp"))]
mod melange_adapter {
    //! Melange-generated 7-BJT Class AB circuit solver.

    use crate::gen_power_amp::{self, CircuitState};
    use std::sync::OnceLock;

    /// Rail headroom for output normalization (matches behavioral model).
    const HEADROOM: f64 = 22.0;

    static SETTLED_STATE: OnceLock<CircuitState> = OnceLock::new();

    fn compute_settled_state() -> CircuitState {
        let mut s = CircuitState::default();
        for _ in 0..44100 {
            gen_power_amp::process_sample(0.0, &mut s);
        }
        s
    }

    fn init_state(sample_rate: f64) -> CircuitState {
        let cached = SETTLED_STATE.get_or_init(compute_settled_state);
        let mut state = cached.clone();
        if (sample_rate - gen_power_amp::SAMPLE_RATE).abs() > 0.5 {
            state.set_sample_rate(sample_rate);
        }
        state
    }

    pub struct PowerAmp {
        state: CircuitState,
        sample_rate: f64,
        /// Last confirmed-good adapter output; used to hold continuity when
        /// the melange solver diverges and we have to reset.
        last_good: f64,
    }

    impl PowerAmp {
        pub fn new() -> Self {
            Self {
                state: init_state(44100.0),
                sample_rate: 44100.0,
                last_good: 0.0,
            }
        }

        pub fn process(&mut self, input: f64) -> f64 {
            let raw = gen_power_amp::process_sample(input, &mut self.state)[0];
            let result = raw / HEADROOM;

            // Divergence guard. Under continuous polyphonic playing, the
            // melange 7-BJT NR solver intermittently fails to converge and
            // the BE fallback also produces non-physical output (observed
            // internal node voltages up to 1e272 V on stress tests). The
            // visible symptom at the output is a clamp-saturated rail slam
            // that the speaker's HPF/LPF ring on and POST_SPEAKER_GAIN
            // amplifies to +20 dBFS spikes — enough to trip DAW peak-protect
            // muting. Three signals we use to detect it:
            //
            //   1. Non-finite raw output (NaN/Inf propagation)
            //   2. NR exhausted MAX_ITER without converging (signals the
            //      BE fallback ran, which can also silently diverge)
            //   3. Any internal node voltage above 100 V (physical rails
            //      are ±22 V + supplies; anything past this is garbage)
            //
            // On any signal, reset the solver state to its cached DC
            // operating point and hold the last confirmed-good output.
            // Holding (rather than silencing) keeps the waveform continuous
            // across a divergence burst — otherwise a 25-sample run of
            // zeros would click audibly. Next sample's NR starts from a
            // clean state and normally picks up tracking the input.
            //
            // Upstream: this is a melange robustness issue with the Class AB
            // push-pull topology under certain polyphonic transient patterns.
            // File upstream once a minimal reproducer is extracted.
            let nr_failed = self.state.last_nr_iterations
                >= gen_power_amp::MAX_ITER as u32 - 1;
            let state_insane = self
                .state
                .v_prev
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 100.0);
            if !result.is_finite() || nr_failed || state_insane {
                self.reset();
                return self.last_good;
            }
            let clamped = result.clamp(-1.0, 1.0);
            self.last_good = clamped;
            clamped
        }

        /// Raw, pre-clamp output of the melange solver. Audio-unsafe for normal
        /// use (not bounded), but needed for diagnostic probing of solver
        /// divergence: if |raw| exceeds the circuit rails (±22 V physically,
        /// ±1.0 after the HEADROOM normalization), the NR converged to a
        /// non-physical branch. Not called by the normal plugin path.
        pub fn diag_raw_process(&mut self, input: f64) -> f64 {
            gen_power_amp::process_sample(input, &mut self.state)[0] / HEADROOM
        }

        /// Snapshot of melange NR diagnostics:
        /// `(clamp_count, nr_max_iter_count, peak_output_volts)`.
        pub fn diag_snapshot(&self) -> (u64, u64, f64) {
            (
                self.state.diag_clamp_count,
                self.state.diag_nr_max_iter_count,
                self.state.diag_peak_output,
            )
        }

        pub fn reset(&mut self) {
            self.state = init_state(self.sample_rate);
            // Do NOT clear last_good — the divergence-guard hold relies on it
            // surviving the reset. Only `new()` zeros it.
        }
    }

    impl Default for PowerAmp {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(not(feature = "legacy-power-amp"))]
pub use melange_adapter::PowerAmp;

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const SR: f64 = 44100.0;

    /// Measure gain using a sine wave (works for both memoryless and circuit models).
    fn measure_gain(pa: &mut PowerAmp, freq: f64, amp: f64) -> f64 {
        let settle = (SR * 0.3) as usize;
        for i in 0..settle {
            pa.process(amp * (2.0 * PI * freq * i as f64 / SR).sin());
        }
        let measure = (SR * 0.1) as usize;
        let mut peak = 0.0f64;
        for i in 0..measure {
            let t = (settle + i) as f64 / SR;
            peak = peak.max(pa.process(amp * (2.0 * PI * freq * t).sin()).abs());
        }
        20.0 * (peak / amp).log10()
    }

    #[test]
    fn test_closed_loop_gain() {
        let mut pa = PowerAmp::new();
        let gain_db = measure_gain(&mut pa, 1000.0, 0.001);
        // 69× / 22V normalization = 3.14×. 20*log10(3.14) = 9.9 dB
        assert!(
            gain_db > 5.0 && gain_db < 20.0,
            "Gain should be ~10-16 dB (69x normalized): got {gain_db:.1} dB"
        );
    }

    #[test]
    fn test_rail_clipping() {
        let mut pa = PowerAmp::new();
        // Large sine should clip near ±1.0
        let settle = (SR * 0.1) as usize;
        let mut peak = 0.0f64;
        for i in 0..(SR * 0.2) as usize {
            let x = 5.0 * (2.0 * PI * 100.0 * i as f64 / SR).sin();
            let y = pa.process(x);
            if i > settle {
                peak = peak.max(y.abs());
            }
        }
        assert!(
            peak > 0.85 && peak <= 1.0,
            "Should clip near 1.0: got {peak}"
        );
    }

    #[test]
    fn test_crossover_reduced_by_feedback() {
        let mut pa = PowerAmp::new();
        let freq = 440.0;
        let amplitude = 0.001;

        let n = (SR * 0.3) as usize;
        let mut samples = Vec::new();
        for i in 0..n {
            let x = amplitude * (2.0 * PI * freq * i as f64 / SR).sin();
            let y = pa.process(x);
            if i > n / 2 {
                samples.push(y);
            }
        }

        let f1 = dft_mag(&samples, freq, SR);
        let f3 = dft_mag(&samples, 3.0 * freq, SR);
        let h3_db = 20.0 * (f3 / f1).log10();
        assert!(
            h3_db < -30.0,
            "Feedback should suppress H3 below -30 dB: got {h3_db:.1} dB"
        );
    }

    #[test]
    fn test_output_bounded() {
        let mut pa = PowerAmp::new();
        for &input in &[0.0, 0.001, 0.01, 0.1, 0.5, 1.0, 5.0, -0.1, -1.0, -5.0] {
            // Feed several samples to let coupling caps charge
            for _ in 0..100 {
                pa.process(input);
            }
            let output = pa.process(input);
            assert!(
                output.is_finite() && output.abs() <= 1.0,
                "Output should be bounded for input {input}: got {output}"
            );
        }
    }

    fn dft_mag(samples: &[f64], freq: f64, sr: f64) -> f64 {
        let n = samples.len() as f64;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &s) in samples.iter().enumerate() {
            let phase = 2.0 * PI * freq * i as f64 / sr;
            re += s * phase.cos();
            im += s * phase.sin();
        }
        (re * re + im * im).sqrt() / n
    }
}
