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
}
