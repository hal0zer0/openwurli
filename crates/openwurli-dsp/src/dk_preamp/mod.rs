//! DK preamp — feature-toggled between melange (default) and legacy solvers.
//!
//! Default: melange-generated 12-node M=3 solver with Sherman-Morrison pot
//! correction. Settles at 100K nominal, tremolo sweeps smoothly.
//! `--features legacy-preamp`: hand-written 8-node MNA solver (for A/B testing).

#[cfg(feature = "legacy-preamp")]
pub use crate::dk_preamp_legacy::DkPreamp;

#[cfg(not(feature = "legacy-preamp"))]
mod melange_adapter;
#[cfg(not(feature = "legacy-preamp"))]
pub use melange_adapter::DkPreamp;

#[cfg(test)]
mod melange_gate_tests {
    use crate::gen_preamp::{self, CircuitState};
    use crate::preamp::PreampModel; // needed for leg_gain() trait method calls
    use std::f64::consts::PI;
    use std::sync::OnceLock;

    const SR: f64 = 88200.0;

    static CACHE: OnceLock<CircuitState> = OnceLock::new();

    /// Settle at compile-time nominal (100K). No pot changes.
    fn settled_state() -> &'static CircuitState {
        CACHE.get_or_init(|| {
            let mut s = CircuitState::default();
            for _ in 0..176_400 {
                gen_preamp::process_sample(0.0, &mut s);
            }
            s
        })
    }

    /// Measure gain by ramping R_ldr from 100K to target (no abrupt jump).
    fn mel_gain(freq: f64, r_ldr: f64, amp: f64) -> f64 {
        let mut main = settled_state().clone();
        let mut shadow = settled_state().clone();

        // Ramp from nominal (100K) to target over 0.2s
        let ramp = (SR * 0.2) as usize;
        let r_start: f64 = 100_000.0;
        for i in 0..ramp {
            let t = i as f64 / ramp as f64;
            let r = r_start + t * (r_ldr - r_start);
            main.set_runtime_R_r_ldr(r);
            shadow.set_runtime_R_r_ldr(r);
            gen_preamp::process_sample(0.0, &mut main);
            gen_preamp::process_sample(0.0, &mut shadow);
        }
        main.set_runtime_R_r_ldr(r_ldr);
        shadow.set_runtime_R_r_ldr(r_ldr);

        // Settle at target
        let settle = (SR * 0.3) as usize;
        for i in 0..settle {
            let x = amp * (2.0 * PI * freq * i as f64 / SR).sin();
            gen_preamp::process_sample(x, &mut main);
            gen_preamp::process_sample(0.0, &mut shadow);
        }

        // Measure
        let measure = (SR * 0.1) as usize;
        let mut peak = 0.0f64;
        for i in 0..measure {
            let t = (settle + i) as f64 / SR;
            let x = amp * (2.0 * PI * freq * t).sin();
            let m = gen_preamp::process_sample(x, &mut main)[0];
            let s = gen_preamp::process_sample(0.0, &mut shadow)[0];
            peak = peak.max((m - s).abs());
        }
        20.0 * (peak / amp).log10()
    }

    fn leg_gain(freq: f64, r_ldr: f64, amp: f64) -> f64 {
        let mut leg = crate::dk_preamp_legacy::DkPreamp::new(SR);
        leg.set_ldr_resistance(r_ldr);
        let settle = (SR * 0.5) as usize;
        for i in 0..settle {
            leg.process_sample(amp * (2.0 * PI * freq * i as f64 / SR).sin());
        }
        let mut peak = 0.0f64;
        for i in 0..(SR * 0.1) as usize {
            let t = (settle + i) as f64 / SR;
            peak = peak.max(leg.process_sample(amp * (2.0 * PI * freq * t).sin()).abs());
        }
        20.0 * (peak / amp).log10()
    }

    /// Gate test: melange gain at both R_ldr endpoints must match legacy within 2 dB.
    #[test]
    fn test_melange_vs_legacy_gain_gate() {
        let mel_1m = mel_gain(1000.0, 1_000_000.0, 0.001);
        let mel_19k = mel_gain(1000.0, 19_000.0, 0.001);
        let leg_1m = leg_gain(1000.0, 1_000_000.0, 0.001);
        let leg_19k = leg_gain(1000.0, 19_000.0, 0.001);

        eprintln!(
            "1M:  mel={mel_1m:.2} leg={leg_1m:.2} delta={:.2}",
            mel_1m - leg_1m
        );
        eprintln!(
            "19K: mel={mel_19k:.2} leg={leg_19k:.2} delta={:.2}",
            mel_19k - leg_19k
        );
        assert!((mel_1m - leg_1m).abs() < 2.0, "1M delta too large");
        assert!((mel_19k - leg_19k).abs() < 2.0, "19K delta too large");
    }

    /// Regression guard against melange pot-change click bugs (commits 07712a0,
    /// 4411bb4, 03c290e). Sweeps R_ldr over its full tremolo range during a
    /// 1 kHz carrier and asserts no sample-to-sample jump exceeds what the
    /// carrier itself can naturally produce by a wide margin. Runs through the
    /// DkPreamp adapter so shadow-pump cancellation is active (plugin path).
    #[test]
    fn test_ldr_sweep_no_clicks() {
        let mut preamp = crate::dk_preamp::DkPreamp::new(SR);
        let freq = 1000.0_f64;
        let amp = 0.3_f64;

        // Natural max inter-sample step for amp·sin(2πf·t) × preamp gain.
        let nat_step = amp * 2.0 * PI * freq / SR;
        let preamp_gain = 10f64.powf(7.5 / 20.0);
        let click_threshold = 20.0 * nat_step * preamp_gain;

        // Pre-roll at nominal 100K to establish steady-state carrier.
        preamp.set_ldr_resistance(100_000.0);
        for i in 0..(SR * 0.1) as usize {
            let x = amp * (2.0 * PI * freq * i as f64 / SR).sin();
            preamp.process_sample(x);
        }

        // Sweep LDR 1M → 19K → 1M (triangle, 0.4s) with carrier active.
        let dur = (SR * 0.4) as usize;
        let mut prev_out = 0.0_f64;
        let mut max_jump = 0.0_f64;
        for i in 0..dur {
            let t_norm = i as f64 / dur as f64;
            let r = if t_norm < 0.5 {
                1_000_000.0 + (19_000.0 - 1_000_000.0) * (t_norm * 2.0)
            } else {
                19_000.0 + (1_000_000.0 - 19_000.0) * ((t_norm - 0.5) * 2.0)
            };
            preamp.set_ldr_resistance(r);

            let sig_t = ((SR * 0.1) as usize + i) as f64 / SR;
            let x = amp * (2.0 * PI * freq * sig_t).sin();
            let out = preamp.process_sample(x);
            if i > 0 {
                max_jump = max_jump.max((out - prev_out).abs());
            }
            prev_out = out;
        }

        eprintln!(
            "LDR sweep max inter-sample jump: {max_jump:.4} (threshold {click_threshold:.4})"
        );
        assert!(
            max_jump < click_threshold,
            "inter-sample jump {max_jump:.4} exceeds threshold {click_threshold:.4} — \
             pot rebuild may be producing clicks"
        );
    }

    /// Regression guard against the Nyquist-rate limit cycle (commit 232ec5f).
    /// Drives a 19 kHz sine burst through the DkPreamp, then holds input at
    /// zero and asserts the output decays to quiet. A self-sustained
    /// oscillation at or near Nyquist would violate the bound.
    #[test]
    fn test_no_nyquist_limit_cycle() {
        let mut preamp = crate::dk_preamp::DkPreamp::new(SR);
        preamp.set_ldr_resistance(1_000_000.0);

        // Warm up — the adapter's cached settled state is at SAMPLE_RATE,
        // so let the new SR equilibrate before the measurement window.
        for _ in 0..(SR * 0.1) as usize {
            preamp.process_sample(0.0);
        }

        // 50 ms of 19 kHz excitation at a modest level.
        let burst = (SR * 0.05) as usize;
        let f = 19_000.0_f64;
        let amp = 0.01_f64;
        for i in 0..burst {
            let x = amp * (2.0 * PI * f * i as f64 / SR).sin();
            preamp.process_sample(x);
        }

        // 100 ms of silence; measure output RMS over the last 50 ms.
        let total_silence = (SR * 0.1) as usize;
        let measure_start = (SR * 0.05) as usize;
        let mut sumsq = 0.0_f64;
        let mut count = 0usize;
        for i in 0..total_silence {
            let out = preamp.process_sample(0.0);
            if i >= measure_start {
                sumsq += out * out;
                count += 1;
            }
        }
        let rms = (sumsq / count as f64).sqrt();
        let rms_dbfs = 20.0 * rms.max(1e-20).log10();

        eprintln!("post-burst RMS 50-100 ms after input stopped: {rms_dbfs:.1} dBFS");
        // A lingering ~Nyquist tone would sit tens of dB above this bound.
        assert!(
            rms_dbfs < -60.0,
            "preamp still ringing at {rms_dbfs:.1} dBFS after 19 kHz burst — \
             possible Nyquist limit cycle"
        );
    }

    /// Regression test for the user-visible tremolo depth.
    ///
    /// Measured, not just the control variable: feeds a 1 kHz sine through the
    /// melange preamp while the `Tremolo` adapter drives the LDR at full depth
    /// (and, separately, at depth=0 to establish the no-tremolo baseline).
    /// Computes the envelope ratio `trem_on / trem_off` at each 5 ms window and
    /// reports the 5–95 % percentile spread — that is the effective AM depth
    /// the player hears, with note decay already factored out.
    ///
    /// Target band: **4.0–8.0 dB** peak-to-peak at depth=1.0.
    ///
    /// - Lower bound: the MEMORY-calibrated 6.1 dB preamp gain range (19 kΩ
    ///   bright vs 1 MΩ dim) with a ~2 dB margin for the CdS envelope not
    ///   reaching both endpoints every cycle.
    /// - Upper bound: 8 dB catches regressions that would make the tremolo
    ///   unphysically deep (e.g. accidentally reintroducing the depth-scaled
    ///   r_series double-count in reverse).
    ///
    /// Also verifies the rate is ~5–6 Hz (counts cycles of the envelope-ratio
    /// zero-crossings around its mean).
    #[test]
    fn test_tremolo_am_depth_at_full_depth() {
        use crate::tremolo::Tremolo;

        const PREAMP_SR: f64 = 88_200.0; // 2x oversampled at 44.1 kHz host rate
        const TONE_HZ: f64 = 1_000.0;
        const AMP: f64 = 0.01;
        const SETTLE_S: f64 = 1.5;
        const MEASURE_S: f64 = 3.0;
        const ENV_WIN_S: f64 = 0.005;

        fn render(depth: f64) -> Vec<f64> {
            let mut preamp = crate::dk_preamp::DkPreamp::new(PREAMP_SR);
            let mut tremolo = Tremolo::new(depth, PREAMP_SR);
            tremolo.set_depth(depth);

            let settle = (PREAMP_SR * SETTLE_S) as usize;
            let measure = (PREAMP_SR * MEASURE_S) as usize;
            let mut out = Vec::with_capacity(measure);

            for i in 0..(settle + measure) {
                preamp.set_ldr_resistance(tremolo.process());
                let t = i as f64 / PREAMP_SR;
                let y = preamp.process_sample(AMP * (2.0 * PI * TONE_HZ * t).sin());
                if i >= settle {
                    out.push(y);
                }
            }
            out
        }

        let off = render(0.0);
        let on = render(1.0);

        let win = (PREAMP_SR * ENV_WIN_S) as usize;
        let env = |x: &[f64]| -> Vec<f64> {
            (0..x.len() / win)
                .map(|i| {
                    let s = i * win;
                    let rms = x[s..s + win].iter().map(|v| v * v).sum::<f64>() / win as f64;
                    rms.sqrt()
                })
                .collect()
        };
        let env_off = env(&off);
        let env_on = env(&on);

        let mut ratio_db: Vec<f64> = env_on
            .iter()
            .zip(env_off.iter())
            .map(|(a, b)| 20.0 * (a / b.max(1e-12)).log10())
            .collect();
        // Sort a copy for percentiles
        let mut sorted = ratio_db.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p05 = sorted[sorted.len() * 5 / 100];
        let p95 = sorted[sorted.len() * 95 / 100];
        let swing = p95 - p05;

        // Cycle count via zero-crossings of the mean-removed ratio
        let mean: f64 = ratio_db.iter().sum::<f64>() / ratio_db.len() as f64;
        for v in &mut ratio_db {
            *v -= mean;
        }
        let crossings: usize = ratio_db
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        let rate = crossings as f64 / MEASURE_S;

        eprintln!(
            "tremolo AM at depth=1.0: {swing:.2} dB swing (p05={p05:+.2}, p95={p95:+.2}), \
             {rate:.2} Hz"
        );

        assert!(
            (4.0..=8.0).contains(&swing),
            "Tremolo AM swing {swing:.2} dB out of spec band 4-8 dB (target ~6.1 dB \
             from MEMORY preamp gain range). Preamp or tremolo calibration drifted — \
             investigate before merging."
        );
        assert!(
            (4.5..=7.5).contains(&rate),
            "Tremolo rate {rate:.2} Hz out of real-200A range 5.3-7 Hz"
        );
    }

    /// Phase 5 RAW noise measurement — bypasses the shadow subtraction
    /// entirely. Just calls `gen_preamp::process_sample(0.0, &mut state)`
    /// with `noise_enabled=true` on a single CircuitState. Compare against
    /// Mr Schemey's analytical target of ~34 µV at the output node.
    ///
    ///   cargo test -p openwurli-dsp --release -- --ignored --nocapture phase5_raw_noise
    #[test]
    #[ignore]
    fn phase5_raw_noise() {
        for sr in [48_000.0_f64, 88_200.0, 176_400.0] {
            // Two states: one runs with noise OFF (deterministic baseline =
            // pure DC + zero-input drift). The other runs with noise ON.
            // Both are seeded identically and start from the same settled
            // bias point, so subtracting them gives the pure noise
            // contribution without any DC bias or shadow-divergence
            // artifacts.
            let mut s_off = gen_preamp::CircuitState::default();
            if (sr - gen_preamp::SAMPLE_RATE).abs() > 0.5 {
                s_off.set_sample_rate(sr);
            }
            for _ in 0..(sr as usize) {
                gen_preamp::process_sample(0.0, &mut s_off);
            }
            // Snapshot the settled state and clone it for the noise-on run.
            let mut s_on = s_off.clone();
            s_on.set_noise_enabled(true);
            // Settle the noise-on state briefly so the RNG is past startup.
            for _ in 0..((sr as usize) / 4) {
                gen_preamp::process_sample(0.0, &mut s_off);
                gen_preamp::process_sample(0.0, &mut s_on);
            }
            let n = (10.0 * sr) as usize;
            // ── Method A: twin-state subtraction (y_on - y_off). ──
            // Susceptible to BJT nonlinearity amplifying state divergence.
            let mut ss_twin = 0.0f64;
            let mut sum_off = 0.0f64;
            let mut sum_on = 0.0f64;
            // ── Method B: single-state with running DC removal. ──
            // The noise RMS = sqrt(E[(y - E[y])²]) computed in one pass
            // using a Welford-style running variance to avoid catastrophic
            // cancellation between giant DC and tiny noise.
            let mut mean_on = 0.0f64;
            let mut m2_on = 0.0f64; // sum of squared deviations
            let mut k_on = 0usize;
            for _ in 0..n {
                let y_off = gen_preamp::process_sample(0.0, &mut s_off)[0];
                let y_on = gen_preamp::process_sample(0.0, &mut s_on)[0];
                let diff = y_on - y_off;
                ss_twin += diff * diff;
                sum_off += y_off;
                sum_on += y_on;
                k_on += 1;
                let delta = y_on - mean_on;
                mean_on += delta / k_on as f64;
                let delta2 = y_on - mean_on;
                m2_on += delta * delta2;
            }
            let rms_twin = (ss_twin / n as f64).sqrt();
            let dc_off = sum_off / n as f64;
            let dc_on = sum_on / n as f64;
            let var_single = m2_on / n as f64;
            let rms_single = var_single.sqrt();
            eprintln!(
                "phase5_raw_noise sr={:>7.0} Hz | twin: rms={:.3e} V ({:+.2} dBV) | single (Welford): rms={:.3e} V ({:+.2} dBV) | DC_off={:.4} V, DC_on={:.4} V",
                sr,
                rms_twin,
                20.0 * rms_twin.max(1e-18).log10(),
                rms_single,
                20.0 * rms_single.max(1e-18).log10(),
                dc_off,
                dc_on,
            );
        }
    }

    /// Phase 5 noise-floor measurement (manual run):
    ///   cargo test -p openwurli-dsp --release -- --ignored --nocapture phase5_noise_floor
    ///
    /// Prints preamp-output RMS/peak with noise ON vs OFF. Not an assertion —
    /// a measurement scaffold for tuning thermal_gain if the idle hiss ends up
    /// too loud or too quiet when auditioned through the full plugin chain.
    #[test]
    #[ignore]
    fn phase5_noise_floor() {
        use crate::dk_preamp::DkPreamp;
        use crate::preamp::PreampModel;

        const SR: f64 = 88_200.0;
        const SETTLE: usize = 44_100;
        const MEASURE: usize = 882_000; // 10 s

        for noise_on in [false, true] {
            let mut preamp = DkPreamp::new(SR);
            preamp.set_noise_enabled(noise_on);
            for _ in 0..SETTLE {
                preamp.process_sample(0.0);
            }
            let mut sum_sq = 0.0;
            let mut peak = 0.0f64;
            for _ in 0..MEASURE {
                let y = preamp.process_sample(0.0);
                sum_sq += y * y;
                peak = peak.max(y.abs());
            }
            let rms = (sum_sq / MEASURE as f64).sqrt();
            let rms_dbv = 20.0 * rms.max(1e-18).log10();
            let peak_dbv = 20.0 * peak.max(1e-18).log10();
            eprintln!(
                "phase5_noise_floor noise={:<3}: rms={:.3e} V ({:+.2} dBV), peak={:.3e} V ({:+.2} dBV)",
                if noise_on { "ON" } else { "OFF" },
                rms,
                rms_dbv,
                peak,
                peak_dbv,
            );
        }
    }
}
