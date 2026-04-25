//! Wurlitzer 200A power amplifier — feature-toggled between circuit and behavioral models.
//!
//! Default: melange-generated 7-BJT Class AB circuit solver.
//! `--features legacy-power-amp`: behavioral closed-loop NR approximation (A/B diagnostics only).
//!
//! Rail sag (melange path only): under load, the unregulated ±22 V rails sag from
//! their idle ~±24.5 V toward the spec ±22 V. Modeled by [`RailDynamics`] and pushed
//! per-sample into the melange solver via the `.runtime V` directives on V1/V2.
//! Calibration anchor: `docs/research/output-stage.md` §4.3.1.

/// Open-circuit (idle, light-load) rail magnitude in volts.
/// Service-manual measurement; matches `tb_power_supply.cir` light-load test.
const RAIL_V_OPEN: f64 = 24.5;

/// Static DC bias of V1/V2 in `wurli-power-amp.cir` (matches the spec ±22 V
/// nominal). The runtime offset is `actual_rail - RAIL_DC_BIAS`.
const RAIL_DC_BIAS: f64 = 22.5;

/// Effective DC source impedance per rail [Ω]. Back-solved from the documented
/// load line: ±24.5 V at idle → ±22 V at rated 20 W into 8 Ω (avg per-rail
/// current Ipk/π = 0.71 A → 2.5 V drop / 0.71 A = 3.5 Ω).
const RAIL_R_EFF: f64 = 3.5;

/// Speaker / output load impedance [Ω]. Two 16 Ω drivers in parallel.
/// Used to compute load current for rail sag from the melange amp's output voltage.
const SPEAKER_LOAD_OHMS: f64 = 8.0;

/// Attack time constant — how fast a rail sags when load increases.
/// Approximated by R_eff × C_filter (3.5 Ω × 2200 µF) ≈ 7.7 ms; rounded to 8 ms.
const RAIL_TAU_ATTACK: f64 = 0.008;

/// Release time constant — how fast a rail recovers when load drops.
/// Slower than attack: full-wave rectifier only conducts during AC peaks
/// (8.3 ms apart at 60 Hz), so recovery is gated by line cycle.
const RAIL_TAU_RELEASE: f64 = 0.015;

/// Behavioral rail sag dynamics. Tracks the two unregulated rail magnitudes
/// based on instantaneous load current, asymmetric one-pole. See module-level
/// docs and `docs/research/output-stage.md` §4.3.1 for the calibration rationale.
///
/// Per-sample cost: 4 muls, 4 adds, 4 compares, 1 reciprocal-divide; no
/// transcendentals. The exp()-based one-pole coefficients are precomputed in
/// `set_sample_rate` so audio-rate stepping stays branch-light.
#[derive(Debug, Clone, Copy)]
pub struct RailDynamics {
    /// Current positive rail magnitude [V]. Idle ~+24.5 V, sags toward +22 V at rated load.
    v_rail_pos: f64,
    /// Current negative rail magnitude [V] (positive number).
    v_rail_neg: f64,
    /// Precomputed `1 - exp(-dt / tau_attack)` — fast attack toward the load-line target.
    alpha_attack: f64,
    /// Precomputed `1 - exp(-dt / tau_release)` — slower release back toward V_OPEN.
    alpha_release: f64,
}

impl RailDynamics {
    /// Starts rails at the static DC bias (`RAIL_DC_BIAS = 22.5 V`) — i.e.
    /// runtime offsets begin at zero. The cached melange settled state was
    /// computed at this same bias, so the solver opens cleanly. Dynamics then
    /// pull rails toward `RAIL_V_OPEN = 24.5 V` (idle target) over ~80 ms,
    /// which is well within typical plugin warmup.
    pub fn new(sample_rate: f64) -> Self {
        let mut s = Self {
            v_rail_pos: RAIL_DC_BIAS,
            v_rail_neg: RAIL_DC_BIAS,
            alpha_attack: 0.0,
            alpha_release: 0.0,
        };
        s.set_sample_rate(sample_rate);
        s
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        let dt = 1.0 / sample_rate;
        self.alpha_attack = 1.0 - (-dt / RAIL_TAU_ATTACK).exp();
        self.alpha_release = 1.0 - (-dt / RAIL_TAU_RELEASE).exp();
    }

    pub fn reset(&mut self) {
        self.v_rail_pos = RAIL_DC_BIAS;
        self.v_rail_neg = RAIL_DC_BIAS;
    }

    /// Rail magnitudes as `(positive, negative)`, both as positive numbers.
    pub fn rail_voltages(&self) -> (f64, f64) {
        (self.v_rail_pos, self.v_rail_neg)
    }

    /// Step one sample. `v_out` is the previous output voltage in volts (raw,
    /// pre-normalization). Positive output draws from +rail, negative from −rail.
    #[inline]
    pub fn step(&mut self, v_out: f64) {
        let i_pos = (v_out / SPEAKER_LOAD_OHMS).max(0.0);
        let i_neg = (-v_out / SPEAKER_LOAD_OHMS).max(0.0);
        let target_pos = RAIL_V_OPEN - i_pos * RAIL_R_EFF;
        let target_neg = RAIL_V_OPEN - i_neg * RAIL_R_EFF;
        let alpha_p = if target_pos < self.v_rail_pos {
            self.alpha_attack
        } else {
            self.alpha_release
        };
        let alpha_n = if target_neg < self.v_rail_neg {
            self.alpha_attack
        } else {
            self.alpha_release
        };
        self.v_rail_pos += alpha_p * (target_pos - self.v_rail_pos);
        self.v_rail_neg += alpha_n * (target_neg - self.v_rail_neg);
    }

    /// Runtime V offsets to push into the melange CircuitState
    /// (additive to V1/V2's ±22.5 V DC bias).
    pub fn offsets(&self) -> (f64, f64) {
        (
            self.v_rail_pos - RAIL_DC_BIAS,
            self.v_rail_neg - RAIL_DC_BIAS,
        )
    }
}

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

        /// No-op on the behavioral path — rails are folded into the closed-loop
        /// approximation, so dynamic sag isn't separable. Kept for API parity.
        pub fn set_rail_sag(&mut self, _on: bool) {}

        pub fn rail_sag_enabled(&self) -> bool {
            false
        }

        pub fn rail_voltages(&self) -> (f64, f64) {
            (super::RAIL_DC_BIAS, super::RAIL_DC_BIAS)
        }
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
        /// Behavioral rail dynamics; only consulted when `rail_sag_on` is true.
        rails: super::RailDynamics,
        /// When true, push per-sample rail offsets into the solver via runtime V.
        /// Default false to preserve historical ideal-rail behavior; flip for A/B.
        rail_sag_on: bool,
    }

    impl PowerAmp {
        pub fn new() -> Self {
            Self {
                state: init_state(44100.0),
                sample_rate: 44100.0,
                last_good: 0.0,
                rails: super::RailDynamics::new(44100.0),
                // Rail sag is correct physics with negligible CPU cost (+0.66%
                // measured) and a small audible effect (~0.5 dB chord
                // compression, ~+0.3 dB single-note headroom). Default ON.
                // Toggle via `set_rail_sag(false)` for A/B against ideal rails.
                rail_sag_on: true,
            }
        }

        /// Enable / disable rail sag modeling. When off, rails stay fixed at
        /// ±22.5 V (V1/V2 DC bias, runtime offsets = 0) — bit-compat with the
        /// pre-rail-sag adapter. Defaults to on.
        pub fn set_rail_sag(&mut self, on: bool) {
            self.rail_sag_on = on;
            if !on {
                self.state.v_rail_pos_offset = 0.0;
                self.state.v_rail_neg_offset = 0.0;
            }
        }

        pub fn rail_sag_enabled(&self) -> bool {
            self.rail_sag_on
        }

        /// Current rail magnitudes `(positive, negative)` as positive numbers.
        /// When rail sag is off, returns the fixed ±22.5 V DC bias.
        pub fn rail_voltages(&self) -> (f64, f64) {
            if self.rail_sag_on {
                self.rails.rail_voltages()
            } else {
                (super::RAIL_DC_BIAS, super::RAIL_DC_BIAS)
            }
        }

        pub fn process(&mut self, input: f64) -> f64 {
            // Push runtime rail offsets BEFORE process_sample so the solver
            // sees the rail state computed from the previous sample's draw.
            if self.rail_sag_on {
                let (off_pos, off_neg) = self.rails.offsets();
                self.state.v_rail_pos_offset = off_pos;
                self.state.v_rail_neg_offset = off_neg;
            }

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
            let nr_failed = self.state.last_nr_iterations >= gen_power_amp::MAX_ITER as u32 - 1;
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

            // Update rail state from the just-computed output for the NEXT
            // sample. `raw` is in volts (pre-normalization), which is what
            // RailDynamics expects to compute load current via v_out / 8 Ω.
            if self.rail_sag_on {
                self.rails.step(raw);
            }

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
            self.rails.reset();
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

    // ── Rail sag tests ──────────────────────────────────────────────────────
    //
    // These guard the calibration anchor in docs/research/output-stage.md
    // §4.3.1: idle ≈ ±24.5 V, sustained-rated load ≈ ±22 V. Run on the
    // melange path only; the behavioral path's rail-sag stubs are no-ops.

    #[test]
    fn test_rail_sag_default_is_on() {
        // Rail sag defaults to ON on the melange path. The behavioral path's
        // rail_sag_enabled() is hard-wired to false (no separable rail model).
        let pa = PowerAmp::new();
        #[cfg(not(feature = "legacy-power-amp"))]
        assert!(pa.rail_sag_enabled(), "Default should be ON on melange path");
        #[cfg(feature = "legacy-power-amp")]
        assert!(
            !pa.rail_sag_enabled(),
            "Behavioral path has no separable rails"
        );
    }

    #[test]
    fn test_rail_sag_off_preserves_static_bias() {
        // With rail sag explicitly off, rail_voltages reports the static DC
        // bias and runtime offsets stay zero. Bit-compat invariant for A/B.
        let mut pa = PowerAmp::new();
        pa.set_rail_sag(false);
        let (vp, vn) = pa.rail_voltages();
        assert!(
            (vp - 22.5).abs() < 1e-9,
            "Static-bias vp should be 22.5: got {vp}"
        );
        assert!(
            (vn - 22.5).abs() < 1e-9,
            "Static-bias vn should be 22.5: got {vn}"
        );
        for _ in 0..100 {
            pa.process(0.0);
        }
        let (vp, vn) = pa.rail_voltages();
        assert!((vp - 22.5).abs() < 1e-9);
        assert!((vn - 22.5).abs() < 1e-9);
    }

    #[test]
    fn test_rail_sag_idle_voltage() {
        // With rail sag on and zero load, rails should ramp from the static
        // DC bias (22.5 V — matches cached settled state) up to the light-load
        // measurement (24.5 V) over ~5 tau_release ~= 75 ms. Generous warmup.
        #[cfg(not(feature = "legacy-power-amp"))]
        {
            let mut pa = PowerAmp::new();
            pa.set_rail_sag(true);
            for _ in 0..(SR as usize / 4) {
                // 250 ms — well past 5 tau_release for asymptotic convergence
                pa.process(0.0);
            }
            let (vp, vn) = pa.rail_voltages();
            assert!(
                (vp - 24.5).abs() < 0.05,
                "Idle vp should be +24.5 V: got {vp}"
            );
            assert!(
                (vn - 24.5).abs() < 0.05,
                "Idle vn should be +24.5 V (mag): got {vn}"
            );
        }
    }

    #[test]
    fn test_rail_sag_sustained_load_drops_rails() {
        // With a sustained input that drives the amp toward the rails, the
        // rails should sag below idle. Don't pin to exactly 22 V — the actual
        // load current depends on the closed-loop response and the test is a
        // qualitative load-line check, not a calibration regression.
        #[cfg(not(feature = "legacy-power-amp"))]
        {
            let mut pa = PowerAmp::new();
            pa.set_rail_sag(true);

            // Settle at idle first
            for _ in 0..(SR as usize / 10) {
                pa.process(0.0);
            }
            let (vp_idle, _) = pa.rail_voltages();

            // 200 mV sine sustained — closed-loop ~69x → ~14 V output
            // → ~1.7 A peak through 8 Ω → noticeable rail sag
            let freq = 220.0;
            let amp = 0.20;
            let n = (SR * 0.5) as usize;
            for i in 0..n {
                let x = amp * (2.0 * PI * freq * i as f64 / SR).sin();
                pa.process(x);
            }
            let (vp_loaded, vn_loaded) = pa.rail_voltages();
            assert!(
                vp_loaded < vp_idle - 0.1,
                "Loaded vp ({vp_loaded:.3}) should be below idle ({vp_idle:.3}) by >0.1 V"
            );
            assert!(
                vn_loaded < vp_idle - 0.1,
                "Loaded vn ({vn_loaded:.3}) should be below idle ({vp_idle:.3}) by >0.1 V"
            );
            // Sanity: sag shouldn't drop rails below the spec floor (~22 V)
            // by a wide margin under a normal music-level signal.
            assert!(vp_loaded > 20.0, "vp sagged too far: {vp_loaded}");
            assert!(vn_loaded > 20.0, "vn sagged too far: {vn_loaded}");
        }
    }

    #[test]
    fn test_rail_sag_recovery_after_load() {
        // After sustained load is removed, rails should recover toward idle.
        #[cfg(not(feature = "legacy-power-amp"))]
        {
            let mut pa = PowerAmp::new();
            pa.set_rail_sag(true);

            // Drive hard for 200 ms to sag the rails
            for i in 0..(SR * 0.2) as usize {
                let x = 0.3 * (2.0 * PI * 110.0 * i as f64 / SR).sin();
                pa.process(x);
            }
            let (vp_loaded, _) = pa.rail_voltages();
            assert!(vp_loaded < 24.0, "Should be sagged: {vp_loaded}");

            // Now silence for 200 ms
            for _ in 0..(SR * 0.2) as usize {
                pa.process(0.0);
            }
            let (vp_recovered, _) = pa.rail_voltages();
            assert!(
                vp_recovered > vp_loaded + 0.5,
                "Rail should recover (loaded {vp_loaded:.3} → recovered {vp_recovered:.3})"
            );
            assert!(
                (vp_recovered - 24.5).abs() < 0.05,
                "Should be back near idle: {vp_recovered}"
            );
        }
    }

    #[test]
    fn test_rail_sag_toggle_zeros_offsets() {
        // Toggling rail-sag off should immediately zero the runtime offsets,
        // returning the solver to ideal-rail behavior.
        #[cfg(not(feature = "legacy-power-amp"))]
        {
            let mut pa = PowerAmp::new();
            pa.set_rail_sag(true);
            // Drive hard to sag the rails
            for i in 0..(SR * 0.05) as usize {
                pa.process(0.5 * (2.0 * PI * 220.0 * i as f64 / SR).sin());
            }
            // Toggle off — rail_voltages should immediately report the static DC bias
            pa.set_rail_sag(false);
            let (vp, vn) = pa.rail_voltages();
            assert!((vp - 22.5).abs() < 1e-9);
            assert!((vn - 22.5).abs() < 1e-9);
        }
    }

    #[test]
    fn test_rail_dynamics_unit() {
        // Pure RailDynamics test, no melange. Verifies the one-pole math.
        let mut rails = RailDynamics::new(SR);
        // Initialized at DC bias 22.5 V (matches cached melange state)
        let (vp, _) = rails.rail_voltages();
        assert!((vp - 22.5).abs() < 1e-9);

        // No load: rails should ramp toward 24.5 V.
        for _ in 0..(SR as usize / 4) {
            rails.step(0.0);
        }
        let (vp, _) = rails.rail_voltages();
        assert!(
            (vp - 24.5).abs() < 0.05,
            "Unloaded rail should approach 24.5 V: got {vp}"
        );

        // Step with v_out=8V → I_pos = 1A → target_pos = 24.5 - 1.0*3.5 = 21.0 V
        for _ in 0..(SR as usize / 10) {
            rails.step(8.0);
        }
        let (vp, vn) = rails.rail_voltages();
        assert!(
            (vp - 21.0).abs() < 0.1,
            "vp should converge to 21.0 under 1A load: got {vp}"
        );
        assert!((vn - 24.5).abs() < 0.05, "vn untouched at 24.5: got {vn}");
    }

    #[test]
    fn test_rail_dynamics_offsets() {
        // Offsets should be (rail - 22.5) so additive on V1/V2 DC bias.
        let mut rails = RailDynamics::new(SR);
        // At init: rails at 22.5 V → offsets = 0 (matches cached melange state)
        let (off_pos, off_neg) = rails.offsets();
        assert!(off_pos.abs() < 1e-9);
        assert!(off_neg.abs() < 1e-9);

        // Settle at idle
        for _ in 0..(SR as usize / 4) {
            rails.step(0.0);
        }
        let (off_pos, off_neg) = rails.offsets();
        // Rails at ~24.5 → offset ~+2
        assert!((off_pos - 2.0).abs() < 0.05);
        assert!((off_neg - 2.0).abs() < 0.05);

        // Settle under load
        for _ in 0..(SR as usize / 5) {
            rails.step(8.0); // I_pos = 1.0 A
        }
        let (off_pos, off_neg) = rails.offsets();
        // pos rail at ~21.0 V → offset ≈ -1.5
        assert!(
            off_pos < -1.0 && off_pos > -2.0,
            "off_pos under load should be ~-1.5: got {off_pos}"
        );
        // neg rail back near +2 (idle)
        assert!((off_neg - 2.0).abs() < 0.05);
    }
}
