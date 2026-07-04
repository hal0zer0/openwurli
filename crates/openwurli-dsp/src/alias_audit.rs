//! Click-band aliasing detector for the full WurliEngine signal chain.
//!
//! Background: between v0.4.0 and v0.5.0 the power-amp solver was moved from
//! base rate into the 2x-oversampled block (commit `00168ca`, Apr 2026). That
//! fixed a sustained "click-band tear" on harmonic-rich pickup output —
//! audible as a non-monotonic plateau in H6–H11 of upper-register notes —
//! at the cost of doubling power-amp CPU. Any future change that pulls the
//! amp back to base rate (or otherwise breaks the anti-alias guarantee) must
//! not silently re-introduce that artifact. This module measures it.
//!
//! Two metrics, both computed from the steady-state tail of a canonical
//! render (C6 v=120, vol=0.5 — exactly what the original fix commit
//! diagnosed against):
//!
//! * `max_step_up_db` — largest positive delta between adjacent harmonics
//!   in H6..H11. Real harmonic decay is monotonically descending;
//!   alias-folded energy creates a plateau or bump. Pre-fix: +5 dB.
//!   Post-fix: −1 dB.
//! * `hf_band_dbc` — RMS of the 5 kHz–18 kHz band of the rendered output,
//!   expressed in dB relative to the fundamental (H1) magnitude. Broadband
//!   insurance against alias hash that doesn't land on integer-harmonic
//!   bins.

use crate::WurliEngine;
use crate::filters::Biquad;

/// MIDI note number for the canonical alias-audit stimulus (C6).
pub const STIMULUS_NOTE: u8 = 84;
/// MIDI velocity (0–127) for the canonical stimulus, matching commit 00168ca.
pub const STIMULUS_VELOCITY: u8 = 120;
/// User volume for the canonical stimulus.
pub const STIMULUS_VOLUME: f64 = 0.5;
/// Multi-note stimulus set — three points across the register that each hit a
/// different harmonic regime under the current preamp/pickup/amp combination:
///
/// * C5 (72) — midrange, exercises a typical playing area where the H6–H11
///   band has moderate energy and is sensitive to amp linearization changes.
/// * C6 (84) — the original stimulus from commit `00168ca` ("Power amp at OS
///   rate"). Kept for continuity with the historical click-band tear case.
/// * G6 (91) — upper register where the absolute HF-band level is much higher
///   relative to H1 and any new HF generation downstream of the pickup
///   shows up immediately in `hf_band_dbc`.
pub const STIMULUS_NOTES: &[u8] = &[72, 84, 91];
/// Sample rate at which the canonical stimulus is rendered.
pub const STIMULUS_SAMPLE_RATE: f64 = 44_100.0;
/// Total render duration (seconds). Long enough for the attack to clear and
/// for the steady-state tail to give 0.5 s of clean analysis window.
pub const STIMULUS_RENDER_SECONDS: f64 = 1.5;
/// Analysis window — the tail end of the render where the attack envelope
/// has settled. Long enough for sub-Hz DFT resolution at H12 (~12.6 kHz).
pub const STIMULUS_ANALYZE_SECONDS: f64 = 0.5;

/// Number of harmonics measured (H1 through H_N).
pub const NUM_HARMONICS: usize = 12;
/// First harmonic index (inclusive) that participates in the plateau metric.
pub const PLATEAU_FIRST_HARMONIC: usize = 6;
/// Last harmonic index (inclusive) for the plateau metric.
pub const PLATEAU_LAST_HARMONIC: usize = 11;
/// Lower edge of the broadband HF measurement band.
pub const HF_BAND_LO_HZ: f64 = 5_000.0;
/// Upper edge of the broadband HF measurement band.
pub const HF_BAND_HI_HZ: f64 = 18_000.0;

/// Output of [`run`] — everything the CLI and the regression test need.
#[derive(Debug, Clone)]
pub struct AliasAuditResult {
    /// Detected fundamental frequency (Hz). Searched within a narrow window
    /// around the nominal pitch to absorb per-note detuning + MLP offsets.
    pub f0_hz: f64,
    /// H1 magnitude expressed in dB FS (raw, not relative to anything). Used
    /// to sanity-check that the analysis window actually captured a note —
    /// a value below ~-50 dBFS suggests the render decayed away or the engine
    /// produced silence.
    pub h1_dbfs: f64,
    /// `harmonic_db[i]` = magnitude of H(i+1) in dB FS (raw, not relative).
    pub harmonic_db: [f64; NUM_HARMONICS],
    /// `harmonic_dbc[i]` = magnitude of H(i+1) relative to H1 in dB.
    /// `harmonic_dbc[0]` is always 0.0.
    pub harmonic_dbc: [f64; NUM_HARMONICS],
    /// Largest positive `harmonic_dbc[n+1] - harmonic_dbc[n]` for
    /// `n` in `PLATEAU_FIRST_HARMONIC..PLATEAU_LAST_HARMONIC`. Negative or
    /// zero = monotonic descent (clean). Positive = aliasing plateau.
    pub max_step_up_db: f64,
    /// Index of the harmonic where the worst step-up occurred (1-based;
    /// the rise is FROM this harmonic TO the next).
    pub max_step_up_from_harmonic: usize,
    /// RMS of the 5 kHz–18 kHz bandpassed render, expressed in dB relative
    /// to H1 magnitude. Catches broadband alias hash that doesn't fall on
    /// integer harmonics.
    pub hf_band_dbc: f64,
}

/// Render the canonical single-note stimulus through `WurliEngine` and
/// measure the two alias indicators. Pure: builds + tears down its own engine.
pub fn run() -> AliasAuditResult {
    run_with_note(STIMULUS_NOTE, STIMULUS_VELOCITY)
}

/// Variant with overridable note + velocity. Used by the CLI exploration mode
/// and by [`run_sweep`]. Production regression tests should call [`run_sweep`]
/// so the gate covers the full canonical stimulus set.
pub fn run_with_note(note: u8, velocity: u8) -> AliasAuditResult {
    let signal = render_stimulus(note, velocity);
    let nominal_f0 = midi_note_hz(note);
    analyze(&signal, STIMULUS_SAMPLE_RATE, nominal_f0)
}

/// One sweep entry: the stimulus note paired with its measurement result.
#[derive(Debug, Clone)]
pub struct SweepEntry {
    pub note: u8,
    pub velocity: u8,
    pub result: AliasAuditResult,
}

/// Render every note in [`STIMULUS_NOTES`] at [`STIMULUS_VELOCITY`] and
/// collect the measurements. This is the canonical multi-note baseline
/// the regression test compares against — covers three distinct harmonic
/// regimes (midrange, historical click-band note, upper-register HF).
pub fn run_sweep() -> Vec<SweepEntry> {
    STIMULUS_NOTES
        .iter()
        .map(|&note| SweepEntry {
            note,
            velocity: STIMULUS_VELOCITY,
            result: run_with_note(note, STIMULUS_VELOCITY),
        })
        .collect()
}

fn render_stimulus(note: u8, velocity: u8) -> Vec<f64> {
    let sr = STIMULUS_SAMPLE_RATE;
    let mut eng = WurliEngine::new(sr);
    eng.ensure_buffer_capacity(1024);
    eng.set_volume(STIMULUS_VOLUME);
    eng.set_tremolo_depth(0.0); // hold gain steady so harmonics are stationary
    eng.set_speaker_character(0.0);
    eng.set_mlp_enabled(true);
    eng.set_noise_enabled(false);

    // Settle the linear smoothers (default vol=0.5 → STIMULUS_VOLUME is a
    // no-op here, but the speaker_character and tremolo_depth still ramp).
    let mut warmup = vec![0.0f32; 1024];
    for _ in 0..6 {
        eng.render(&mut warmup);
    }

    eng.note_on(note, velocity as f32 / 127.0);

    let total = (sr * STIMULUS_RENDER_SECONDS) as usize;
    let mut signal = Vec::with_capacity(total);
    let mut buf = vec![0.0f32; 1024];
    let mut pos = 0;
    while pos < total {
        let len = 1024.min(total - pos);
        eng.render(&mut buf[..len]);
        signal.extend(buf[..len].iter().map(|s| *s as f64));
        pos += len;
    }
    signal
}

fn analyze(signal: &[f64], sr: f64, nominal_f0: f64) -> AliasAuditResult {
    // Steady-state tail: skip the attack envelope, analyze the last
    // `STIMULUS_ANALYZE_SECONDS` worth of samples.
    let analyze_n = (sr * STIMULUS_ANALYZE_SECONDS) as usize;
    assert!(
        signal.len() >= analyze_n,
        "alias_audit signal too short: {} samples for {analyze_n} analysis window",
        signal.len()
    );
    let tail = &signal[signal.len() - analyze_n..];

    let f0 = refine_f0(tail, sr, nominal_f0);

    let mut harmonic_db = [0.0f64; NUM_HARMONICS];
    let mut harmonic_dbc = [0.0f64; NUM_HARMONICS];
    let h1 = dft_magnitude(tail, f0, sr);
    let h1_db = mag_to_db(h1);
    for k in 0..NUM_HARMONICS {
        let mag = dft_magnitude(tail, (k + 1) as f64 * f0, sr);
        harmonic_db[k] = mag_to_db(mag);
        harmonic_dbc[k] = if h1 > 0.0 {
            20.0 * (mag / h1).log10()
        } else {
            -200.0
        };
    }
    // harmonic_dbc[0] is exactly 0.0 by definition (H1 / H1).
    harmonic_dbc[0] = 0.0;

    let (max_step_up_db, max_step_up_from_harmonic) = plateau_metric(&harmonic_dbc);

    let hf_rms = bandpass_rms(tail, sr, HF_BAND_LO_HZ, HF_BAND_HI_HZ);
    let hf_band_dbc = if h1 > 0.0 {
        20.0 * (hf_rms / h1).log10()
    } else {
        -200.0
    };

    AliasAuditResult {
        f0_hz: f0,
        h1_dbfs: h1_db,
        harmonic_db,
        harmonic_dbc,
        max_step_up_db,
        max_step_up_from_harmonic,
        hf_band_dbc,
    }
}

fn plateau_metric(harmonic_dbc: &[f64; NUM_HARMONICS]) -> (f64, usize) {
    // Indices are 0-based for the array (H1 = harmonic_dbc[0]). The user-
    // facing "harmonic number" is 1-based. PLATEAU_FIRST_HARMONIC=6 →
    // array index 5; PLATEAU_LAST_HARMONIC=11 → array index 10.
    let first_idx = PLATEAU_FIRST_HARMONIC - 1;
    let last_idx = PLATEAU_LAST_HARMONIC - 1;
    let mut worst = f64::NEG_INFINITY;
    let mut worst_from = PLATEAU_FIRST_HARMONIC;
    for i in first_idx..last_idx {
        let delta = harmonic_dbc[i + 1] - harmonic_dbc[i];
        if delta > worst {
            worst = delta;
            worst_from = i + 1; // 1-based harmonic number we're stepping UP from
        }
    }
    (worst, worst_from)
}

fn dft_magnitude(signal: &[f64], freq: f64, sr: f64) -> f64 {
    let n = signal.len() as f64;
    let mut re = 0.0;
    let mut im = 0.0;
    let omega = 2.0 * std::f64::consts::PI * freq / sr;
    for (i, &s) in signal.iter().enumerate() {
        let phase = omega * i as f64;
        re += s * phase.cos();
        im -= s * phase.sin();
    }
    2.0 * ((re / n).powi(2) + (im / n).powi(2)).sqrt()
}

fn mag_to_db(mag: f64) -> f64 {
    if mag > 0.0 {
        20.0 * mag.log10()
    } else {
        -200.0
    }
}

/// Scan DFT magnitude in a ±5 Hz window around `nominal_f0` at 0.1 Hz steps
/// and return the peak. Absorbs per-note detuning + MLP frequency offsets.
fn refine_f0(signal: &[f64], sr: f64, nominal_f0: f64) -> f64 {
    let mut best_f = nominal_f0;
    let mut best_mag = dft_magnitude(signal, nominal_f0, sr);
    let mut f = nominal_f0 - 5.0;
    while f <= nominal_f0 + 5.0 {
        let mag = dft_magnitude(signal, f, sr);
        if mag > best_mag {
            best_mag = mag;
            best_f = f;
        }
        f += 0.1;
    }
    best_f
}

/// Bandpass-RMS of a signal between `lo` and `hi`. Implemented as a
/// 4th-order highpass at `lo` cascaded with a 4th-order lowpass at `hi`
/// (two biquads each, Q=0.707 per stage → Butterworth-like response).
fn bandpass_rms(signal: &[f64], sr: f64, lo: f64, hi: f64) -> f64 {
    let mut hp1 = Biquad::highpass(lo, std::f64::consts::FRAC_1_SQRT_2, sr);
    let mut hp2 = Biquad::highpass(lo, std::f64::consts::FRAC_1_SQRT_2, sr);
    let mut lp1 = Biquad::lowpass(hi, std::f64::consts::FRAC_1_SQRT_2, sr);
    let mut lp2 = Biquad::lowpass(hi, std::f64::consts::FRAC_1_SQRT_2, sr);
    let mut sum_sq = 0.0;
    for &x in signal {
        let y = lp2.process(lp1.process(hp2.process(hp1.process(x))));
        sum_sq += y * y;
    }
    (sum_sq / signal.len() as f64).sqrt()
}

#[inline]
fn midi_note_hz(note: u8) -> f64 {
    440.0 * 2.0_f64.powf((note as f64 - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plateau_metric_detects_monotonic_descent() {
        // -50, -55, -60, -65, -70, -75, -80, -85, -90, -95, -100, -105
        let mut h = [0.0; NUM_HARMONICS];
        for (i, slot) in h.iter_mut().enumerate() {
            *slot = -50.0 - 5.0 * i as f64;
        }
        let (delta, _) = plateau_metric(&h);
        assert!(
            delta < 0.0,
            "monotonic descent should give negative step-up, got {delta}"
        );
    }

    #[test]
    fn plateau_metric_detects_pre_fix_signature() {
        // Numbers from commit 00168ca "Power amp at OS rate" — pre-fix:
        // H6=-67, H7=-63, H8=-58, H9=-58, H10=-58, H11=-61
        // (H1..H5 don't matter for the metric; use a descent.)
        let h: [f64; NUM_HARMONICS] = [
            0.0, -10.0, -20.0, -30.0, -50.0, // H1..H5 (descending baseline)
            -67.0, -63.0, -58.0, -58.0, -58.0, -61.0, // H6..H11 plateau
            -70.0, // H12
        ];
        let (delta, from) = plateau_metric(&h);
        assert!(
            delta > 0.0,
            "alias plateau should give positive step-up, got {delta}"
        );
        // Worst rise is H7→H8 (-63 → -58 = +5) or H8→H9 etc. The max is +5.
        assert!(
            (delta - 5.0).abs() < 0.001,
            "expected max step +5 dB, got {delta} (from H{from})"
        );
    }

    #[test]
    fn plateau_metric_detects_post_fix_signature() {
        // Numbers from commit 00168ca — post-fix:
        // H6=-74, H7=-72, H8=-71, H9=-70, H10=-84, H11=-79
        let h: [f64; NUM_HARMONICS] = [
            0.0, -10.0, -20.0, -30.0, -50.0, // H1..H5
            -74.0, -72.0, -71.0, -70.0, -84.0, -79.0, // H6..H11 (mostly descending)
            -90.0, // H12
        ];
        let (delta, _) = plateau_metric(&h);
        // H6→H7 = +2, H7→H8 = +1, H8→H9 = +1, H9→H10 = -14, H10→H11 = +5.
        // Worst is H10→H11 at +5. Note: this fixture is slightly worse than
        // the real post-fix render — kept for symmetry with the pre-fix test.
        // The threshold the regression gate uses (≤ +1.0) refers to real
        // captured renders, not this fixture.
        assert!(delta > 0.0, "fixture should still trip; got {delta}");
    }

    #[test]
    fn dft_magnitude_recovers_known_sinusoid() {
        let sr = 44_100.0;
        let f = 1000.0;
        let n = (sr * 0.5) as usize;
        let amp = 0.7;
        let signal: Vec<f64> = (0..n)
            .map(|i| amp * (2.0 * std::f64::consts::PI * f * i as f64 / sr).sin())
            .collect();
        let mag = dft_magnitude(&signal, f, sr);
        assert!(
            (mag - amp).abs() < 0.01,
            "DFT should recover amplitude {amp}, got {mag}"
        );
    }
}
