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
///
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
///
/// REFRESHED 2026-09-23 (b) for the Reed Bar Trim default (12.5K -> 17.2K,
/// drive -1.3 dB at the canonical stimulus) and for speaker character 0
/// becoming a true passthrough (the 20 Hz / 20 kHz bypass biquads no longer
/// run). Deltas: step_up 72: 2.401 -> 2.980; 84: 5.408 -> 8.003; 91: 9.991 ->
/// 9.631; hf_band unchanged within 0.05 dB on all three. Attribution: note 84
/// at vol 0.59 (old drive restored) reads 6.3, so ~0.9 dB is the filter
/// removal and ~1.7 dB the drive; the H9 notch the metric keys on moved
/// -2.5 dB. No fold-back signature.
///
/// REFRESHED 2026-09-23 (a) for the drawn volume network: user volume is now the
/// pot between preamp and power amp, so the canonical stimulus (vol 0.5)
/// drives the amp ~14 dB less than the pinned drive it replaced. Deltas vs the
/// 2026-09-13 capture: step_up 72: 3.836 -> 2.401 (-1.44); 84: 1.578 -> 5.408
/// (+3.83); 91: 7.968 -> 9.991 (+2.02); hf_band unchanged within 0.25 dB on all
/// three. A volume sweep puts note 84 back at 1.52 at vol 1.0 (old drive), so
/// the plateau rise is the amp's crossover residual scaling with drive, not
/// fold-back (a real alias regression moves both metrics together).
///
/// Earlier: REFRESHED 2026-09-13 for the pickup-network refit (corner 2312 Hz
/// -> 880 Hz on the drawn topology). Deltas vs the v0.5.1 capture:
///
/// | note | step_up (aliasing) | hf_band (HF energy) |
/// |------|--------------------|---------------------|
/// |  72  | 7.951 -> 3.836  (**-4.12**) | -52.647 -> -48.639 (+4.01) |
/// |  84  | 8.183 -> 1.578  (**-6.61**) | -47.809 -> -44.393 (+3.42) |
/// |  91  | 6.862 -> 7.968  (+1.11, in tol) | -39.164 -> -40.854 (-1.69) |
///
/// The refresh is justified by the SIGN SPLIT, not by convenience: the two
/// notes whose broadband HF energy rose are exactly the two whose click-band
/// PLATEAU — `max_step_up_db`, the metric that actually detects aliasing —
/// improved sharply. More genuine harmonic content, less alias hash. A real
/// alias regression moves both metrics the same way (the v0.5.0 tear moved
/// click-band harmonics +5 to +13 dB with the plateau worsening); this is the
/// opposite pattern. Expected: moving the pickup corner below the stimulus
/// fundamentals reduces the high-order nonlinear products available to fold.
const BASELINE: &[BaselineEntry] = &[
    BaselineEntry {
        note: 72,
        max_step_up_db: 2.980,
        hf_band_dbc: -48.751,
    },
    BaselineEntry {
        note: 84,
        max_step_up_db: 8.003,
        hf_band_dbc: -44.558,
    },
    BaselineEntry {
        note: 91,
        max_step_up_db: 9.631,
        hf_band_dbc: -40.699,
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
