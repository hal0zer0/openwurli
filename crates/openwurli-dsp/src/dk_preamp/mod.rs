//! DK preamp — feature-toggled between legacy and melange solvers.
//!
//! Default: hand-written 8-node MNA solver (proven).
//! `--features melange-preamp`: melange-generated 12-node M=3 solver
//! with pot-as-rebuild. Settles at 100K nominal, tremolo sweeps smoothly.

#[cfg(not(feature = "melange-preamp"))]
pub use crate::dk_preamp_legacy::DkPreamp;

#[cfg(feature = "melange-preamp")]
mod melange_adapter;
#[cfg(feature = "melange-preamp")]
pub use melange_adapter::DkPreamp;

#[cfg(test)]
mod melange_gate_tests {
    use crate::gen_preamp::{self, CircuitState};
    use crate::preamp::PreampModel;
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
            main.pot_0_resistance = r;
            shadow.pot_0_resistance = r;
            gen_preamp::process_sample(0.0, &mut main);
            gen_preamp::process_sample(0.0, &mut shadow);
        }
        main.pot_0_resistance = r_ldr;
        shadow.pot_0_resistance = r_ldr;

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
}
