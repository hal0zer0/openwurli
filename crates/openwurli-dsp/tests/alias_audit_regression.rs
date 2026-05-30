//! Regression test for the click-band alias detector.
//!
//! Runs the canonical multi-note sweep through `WurliEngine` and asserts that
//! each metric stays within tolerance of the baseline captured at v0.5.1. If
//! this test fails, the click-band spectrum has shifted — either a real
//! audible regression (likely the v0.5.0-era power-amp tear or something
//! analogous) or an intentional re-tuning. In the latter case, refresh
//! `tests/baselines/alias_audit_v0_5_1.json` and call it out in the commit.
//!
//! Tolerances are set above run-to-run float noise (~0.05 dB observed) but
//! below the smallest deltas that would be audibly meaningful.

use openwurli_dsp::alias_audit;

/// How far each metric may worsen vs. the baseline before we fail. Both
/// metrics are in dB; "worsen" = move in the *positive* direction (more
/// step-up = more plateau-like; more hf_band = more HF content).
///
/// Calibration notes (2026-05-30):
///   * Run-to-run float determinism: 0.00 dB observed across repeated runs.
///   * Tight (0.5 dB) catches even cosmetic changes — e.g. toggling MLP off
///     trips the note-72 hf_band gate by +0.75 dB. Useful during DSP work
///     where every spectral shift should be conscious.
///   * Loose (1.5–2.0 dB) lets minor re-tunings pass while still catching
///     the historical v0.5.0-era tear, which moved click-band harmonics by
///     +5 to +13 dB (commit 00168ca). Use this for the regression-gate role.
/// Currently set to the loose values — this test guards against catastrophic
/// regressions (the actual "tear"), not benign spectrum drift.
const MAX_STEP_UP_TOLERANCE_DB: f64 = 1.5;
const HF_BAND_TOLERANCE_DB: f64 = 2.0;

#[derive(Debug)]
struct BaselineEntry {
    note: u8,
    max_step_up_db: f64,
    hf_band_dbc: f64,
}

/// Hand-parsed baseline — small enough that a JSON dep isn't worth pulling in.
/// Must stay in lockstep with `tests/baselines/alias_audit_v0_5_1.json`.
const BASELINE: &[BaselineEntry] = &[
    BaselineEntry {
        note: 72,
        max_step_up_db: 7.951,
        hf_band_dbc: -52.647,
    },
    BaselineEntry {
        note: 84,
        max_step_up_db: 8.183,
        hf_band_dbc: -47.809,
    },
    BaselineEntry {
        note: 91,
        max_step_up_db: 6.862,
        hf_band_dbc: -39.164,
    },
];

#[test]
fn alias_audit_sweep_no_regression_vs_baseline() {
    let sweep = alias_audit::run_sweep();
    assert_eq!(
        sweep.len(),
        BASELINE.len(),
        "stimulus set drifted from baseline: got {} notes, baseline has {}",
        sweep.len(),
        BASELINE.len()
    );

    let mut failures = Vec::new();
    for (entry, base) in sweep.iter().zip(BASELINE.iter()) {
        assert_eq!(
            entry.note, base.note,
            "stimulus note order drifted from baseline at note {}",
            base.note
        );
        let step_delta = entry.result.max_step_up_db - base.max_step_up_db;
        let hf_delta = entry.result.hf_band_dbc - base.hf_band_dbc;

        if step_delta > MAX_STEP_UP_TOLERANCE_DB {
            failures.push(format!(
                "note {}: max_step_up_db {:.3} > baseline {:.3} + {:.1} \
                 (delta {:+.3} dB) — click-band plateau worsened",
                entry.note,
                entry.result.max_step_up_db,
                base.max_step_up_db,
                MAX_STEP_UP_TOLERANCE_DB,
                step_delta
            ));
        }
        if hf_delta > HF_BAND_TOLERANCE_DB {
            failures.push(format!(
                "note {}: hf_band_dbc {:.3} > baseline {:.3} + {:.1} \
                 (delta {:+.3} dB) — broadband HF energy worsened",
                entry.note,
                entry.result.hf_band_dbc,
                base.hf_band_dbc,
                HF_BAND_TOLERANCE_DB,
                hf_delta
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Click-band alias regression detected vs v0.5.1 baseline:\n  {}\n\n\
         Run `cargo run -p preamp-bench --release -- alias-audit --note <n> \
         --velocity 120 --json` for each stimulus note to inspect the full \
         spectrum. If this regression is an intentional re-tuning, refresh \
         crates/openwurli-dsp/tests/baselines/alias_audit_v0_5_1.json and the \
         BASELINE constant in this test, and document the delta in the commit.",
        failures.join("\n  ")
    );
}

/// Sanity check: baseline file is reachable and matches the BASELINE constant.
/// Guards against drift between the JSON (used by humans and CI inspection)
/// and the in-test constant (used by the assertion).
#[test]
fn baseline_constant_matches_json() {
    let json = std::fs::read_to_string("tests/baselines/alias_audit_v0_5_1.json")
        .expect("baseline JSON not found — running from wrong cwd?");
    for base in BASELINE {
        let needle = format!("\"note\": {},", base.note);
        let pos = json
            .find(&needle)
            .unwrap_or_else(|| panic!("note {} missing from baseline JSON", base.note));
        let chunk = &json[pos..pos + 600.min(json.len() - pos)];
        // Cheap-and-dirty: look for the numeric values verbatim. If anyone
        // hand-edits the JSON to different precision they need to update the
        // constant to match — that's the point.
        assert!(
            chunk.contains(&format!("{:.3}", base.max_step_up_db)),
            "note {}: BASELINE.max_step_up_db {:.3} not found in JSON near `{}`",
            base.note,
            base.max_step_up_db,
            needle
        );
        assert!(
            chunk.contains(&format!("{:.3}", base.hf_band_dbc)),
            "note {}: BASELINE.hf_band_dbc {:.3} not found in JSON near `{}`",
            base.note,
            base.hf_band_dbc,
            needle
        );
    }
}
