//! Diagnostic probe for the melange-generated power-amp solver (raw codegen,
//! bypassing the PowerAmp adapter — which defaults to the behavioral amp).
//!
//! Run explicitly: cargo test -p openwurli-dsp --release --test raw_probe -- --ignored --nocapture
//!
//! 2026-08-03 findings (regen @ melange ca2d7e6, M=14, .linearize Q9, BE-primary):
//! - residual overdrive bug CONFIRMED: internal diag_peak_output reaches
//!   110-195 V (on ±22.5 V rails) at drive as low as 0.05; delivered output
//!   only stays ≤30 V because the codegen clamp saturates ("rail slam").
//! - CPU: 9.5-157x slower than realtime for the amp alone (0.5 s @ 88.2 kHz).
//! - Strictly better than the prior in-tree codegen (M=7, de9dc81-era),
//!   which showed 1e272-V internal excursions + NaN-reset storms on the
//!   same sweep, settled or cold.
//! Both gates for making the melange amp the default remain CLOSED.
//!
//! 2026-08-03 LATER (regen @ melange 1ed5e2f — overdrive fix e6d5db8/7906d30):
//! - overdrive bug FIXED, verified here: internal peaks physical at all
//!   drives (3.6-20.6 V, clean rail clip, zero NaN, monotonic).
//! - CPU improved 1.2-7.4x realtime (was 9.5-157x) — still ~100x too slow
//!   for a plugin default (behavioral amp ~0.7% of a core). CPU gate stays
//!   closed; overdrive gate cleared.
use openwurli_dsp::gen_power_amp as gpa;
use std::time::Instant;

#[test]
#[ignore = "diagnostic probe, ~4 min in release; run with -- --ignored"]
fn raw_codegen_sweep() {
    let sr = gpa::SAMPLE_RATE;
    let n = (sr * 0.5) as usize; // 0.5 s of audio per level
    for amp in [0.05, 0.1, 0.3, 0.5, 1.0, 2.0] {
        let mut st = gpa::CircuitState::default();
        // settle 1 s of silence, like the adapter's SETTLED_STATE
        for _ in 0..(sr as usize) {
            gpa::process_sample(0.0, &mut st);
        }
        let mut peak = 0.0f64;
        let mut non_finite = 0u32;
        let t0 = Instant::now();
        for i in 0..n {
            let t = i as f64 / sr;
            let x = amp * (2.0 * std::f64::consts::PI * 1000.0 * t).sin();
            let y = gpa::process_sample(x, &mut st)[0];
            if y.is_finite() {
                peak = peak.max(y.abs());
            } else {
                non_finite += 1;
            }
        }
        let dt = t0.elapsed().as_secs_f64();
        println!(
            "amp {amp:>5.2} -> delivered peak {peak:>7.3} V, internal peak {:>10.3e} V, \
             {dt:>7.3}s/0.5s ({:>5.1}x RT), nr_max {}, be_fb {}, nan_rst {}, nonfinite {non_finite}",
            st.diag_peak_output,
            dt / 0.5,
            st.diag_nr_max_iter_count,
            st.diag_be_fallback_count,
            st.diag_nan_reset_count
        );
        assert!(
            non_finite == 0 && peak < 35.0,
            "delivered-output blowup at amp {amp}: peak {peak:.1} V"
        );
    }
}
