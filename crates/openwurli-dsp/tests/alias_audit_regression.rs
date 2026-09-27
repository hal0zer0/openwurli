//! Regression gate: the engine renders the same note at 44.1 kHz and at
//! 176.4 kHz.
//!
//! Runs the canonical multi-note stimulus through `WurliEngine` at both host
//! rates (see `openwurli_dsp::alias_audit`) and fails if the harmonic ladder,
//! the HF band or the off-grid energy differ beyond the module's tolerances —
//! the signature of a stage that is under-sampled at the base rate (the
//! v0.5.0-era power-amp "click-band tear", commit 00168ca) or of new aliasing.
//!
//! The gate compares the engine with itself: no stored baseline, and it holds
//! in every feature configuration (either power amp, either preamp). It
//! replaced (2026-09-27) a baseline comparison of a harmonic "step-up" metric
//! that the field-solved pickup law's odd-dominant spectra invalidated.

use openwurli_dsp::alias_audit;

#[test]
fn engine_converges_between_base_and_reference_rate() {
    let sweep = alias_audit::run_sweep();
    assert_eq!(sweep.len(), alias_audit::STIMULUS_NOTES.len());

    let mut failures = Vec::new();
    for entry in &sweep {
        let r = &entry.result;
        eprintln!(
            "note {}: f0 {:.3} Hz, H1 {:.1} dBFS | ladder worst {:.3} dB (H{}), quiet {:.1} dBc | \
             hf_band {:.1} dBc ({:+.3} dB) | alias excess {:.1} dBc",
            entry.note,
            r.f0_hz,
            r.base.h1_dbfs,
            r.ladder_worst_rel_db,
            r.ladder_worst_rel_harmonic,
            r.ladder_worst_lin_dbc,
            r.base.hf_band_dbc,
            r.hf_band_delta_db,
            r.alias_excess_dbc
        );
        failures.extend(
            r.failures()
                .into_iter()
                .map(|f| format!("note {}: {f}", entry.note)),
        );
    }

    assert!(
        failures.is_empty(),
        "Rate-convergence regression (44.1 vs 176.4 kHz):\n  {}\n\n\
         Inspect with `cargo run -p preamp-bench --release -- alias-audit --note <n>`. A stage \
         that is under-sampled at the base rate, or new aliasing, shows here; do not widen the \
         tolerances to pass — find the stage.",
        failures.join("\n  ")
    );
}
