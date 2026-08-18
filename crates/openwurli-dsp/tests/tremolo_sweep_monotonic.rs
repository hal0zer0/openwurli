//! Interior guard for the tremolo gain-vs-shunt-R curve.
//!
//! Preamp gain is physically required to be strictly decreasing in the LDR
//! path resistance (more shunt R = more feedback reaching the emitter = less
//! gain). The historical endpoint checks (19 kΩ, 1 MΩ, range) are blind to
//! interior defects by construction — a stale baseline artifact carried a
//! 2.4 dB non-monotonic hole at 35.5 kΩ for months while both endpoints and
//! the range passed (found by an outside reader, 2026-08-18). Monotonicity
//! is the statistic that carries the signal; this test asserts it across the
//! interior.

use openwurli_dsp::dk_preamp::DkPreamp;
use openwurli_dsp::preamp::PreampModel;
use std::f64::consts::PI;

const OS_SR: f64 = 88_200.0;

fn gain_db_at(preamp: &mut DkPreamp, r_ldr: f64) -> f64 {
    preamp.reset();
    preamp.set_ldr_resistance(r_ldr);
    let amplitude = 0.001;
    let freq = 1_000.0;
    let n_settle = (OS_SR * 0.12) as usize;
    let n_measure = (OS_SR * 0.08) as usize;
    for i in 0..n_settle {
        let t = i as f64 / OS_SR;
        preamp.process_sample(amplitude * (2.0 * PI * freq * t).sin());
    }
    let mut peak = 0.0f64;
    for i in 0..n_measure {
        let t = (n_settle + i) as f64 / OS_SR;
        let y = preamp.process_sample(amplitude * (2.0 * PI * freq * t).sin());
        peak = peak.max(y.abs());
    }
    20.0 * (peak / amplitude).log10()
}

#[test]
fn tremolo_sweep_gain_is_strictly_monotonic_in_r_ldr() {
    let mut preamp = DkPreamp::new(OS_SR);
    let (r_min, r_max, steps) = (19_000.0f64, 1_000_000.0f64, 10usize);
    let (log_min, log_max) = (r_min.ln(), r_max.ln());

    let mut prev: Option<(f64, f64)> = None;
    let mut curve = String::new();
    for i in 0..steps {
        let frac = i as f64 / (steps - 1) as f64;
        let r = (log_min + frac * (log_max - log_min)).exp();
        let g = gain_db_at(&mut preamp, r);
        curve.push_str(&format!("  {r:>9.0} Ω  {g:>6.2} dB\n"));
        if let Some((r_prev, g_prev)) = prev {
            assert!(
                g < g_prev,
                "gain must strictly decrease with shunt R: \
                 {g:.2} dB at {r:.0} Ω is not below {g_prev:.2} dB at {r_prev:.0} Ω\n\
                 sweep so far:\n{curve}"
            );
            // A step change far above the curve's local slope is the
            // false-convergence signature even if monotonicity survives.
            assert!(
                g_prev - g < 3.0,
                "suspicious {:.2} dB jump between adjacent sweep points \
                 ({r_prev:.0} -> {r:.0} Ω)\nsweep so far:\n{curve}",
                g_prev - g
            );
        }
        prev = Some((r, g));
    }
}
