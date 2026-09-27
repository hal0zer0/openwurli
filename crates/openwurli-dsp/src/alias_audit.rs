//! Rate-convergence audit for the full WurliEngine signal chain.
//!
//! Background: between v0.4.0 and v0.5.0 the power-amp solver was moved from
//! base rate into the 2x-oversampled block (commit `00168ca`, Apr 2026). That
//! fixed a sustained "click-band tear" on harmonic-rich pickup output (a
//! nonlinear solver integrated at too low a rate) at the cost of doubling
//! power-amp CPU. Any future change that pulls a stage back to base rate, or
//! otherwise leaves the chain under-sampled, must not silently re-introduce
//! that kind of artifact. This module measures it.
//!
//! The test is rate convergence: the canonical stimulus is rendered through
//! the engine at [`STIMULUS_SAMPLE_RATE`] (44.1 kHz host; preamp and power amp
//! at 88.2 kHz behind the 2x oversampler) and at [`REFERENCE_SAMPLE_RATE`]
//! (176.4 kHz host; no oversampling, every stage at 176.4 kHz). A chain whose
//! processing is adequate at the base rate renders the same note at both:
//!
//! * the harmonic ladder H1..H12 (Hann-windowed DFT at the common f0) must
//!   agree — a solver or nonlinearity that is under-sampled at the base rate
//!   moves on-grid harmonics (the historical tear's mechanism);
//! * the energy OFF the harmonic grid that the base rate adds must stay
//!   negligible — aliasing folds a nonlinearity's products above Nyquist to
//!   |m·fs − k·f0|, off the grid, at 44.1 kHz but not at 176.4 kHz. Measured
//!   as the rate-attributable excess (base residual minus reference residual,
//!   in energy), which must stay below [`ALIAS_EXCESS_TOL_DBC`];
//! * `hf_band_dbc` (all energy in exactly 5–18 kHz, re H1, from the same
//!   spectrum at both rates) must agree — convergence of the content above
//!   H12. A biquad band-pass is not usable here: it is a different filter at
//!   each rate (at 44.1 kHz its 18 kHz corner sits against Nyquist).
//!
//! The gate compares the engine with itself, so it needs no stored baseline
//! and holds for every feature configuration (either power amp, either
//! preamp). It replaced (2026-09-27) a baseline comparison of "largest step up
//! between adjacent harmonics in H6..H11": that metric assumed real harmonics
//! fall monotonically, which the field-solved pickup law's odd-dominant
//! spectra do not, and its rectangular-window DFT had a leakage floor near
//! −80 dBc that both manufactured and hid steps.
//!
//! The reed holds its pitch exactly, so the engine renders the same note at
//! both rates and converges to ≤ 0.07 dB above −80 dBc. (Until 2026-09-27 the
//! reed carried a random pitch jitter whose realization differed per rate and
//! alone moved upper harmonics by several dB; it has been removed.)

use crate::WurliEngine;

/// MIDI note number for the canonical single-note stimulus (C6).
pub const STIMULUS_NOTE: u8 = 84;
/// MIDI velocity (0–127) for the canonical stimulus, matching commit 00168ca.
pub const STIMULUS_VELOCITY: u8 = 120;
/// User volume for the canonical stimulus.
pub const STIMULUS_VOLUME: f64 = 0.5;
/// Multi-note stimulus set — three points across the upper register:
///
/// * C5 (72) — midrange, where the H6–H11 band has moderate energy.
/// * C6 (84) — the original stimulus from commit `00168ca` ("Power amp at OS
///   rate"). Kept for continuity with the historical click-band tear case.
/// * G6 (91) — upper register, where HF content relative to H1 is highest.
pub const STIMULUS_NOTES: &[u8] = &[72, 84, 91];
/// Host rate under test.
pub const STIMULUS_SAMPLE_RATE: f64 = 44_100.0;
/// Reference host rate: 4x the base, and at or above 88.2 kHz the engine does
/// not oversample, so every stage runs at this rate.
pub const REFERENCE_SAMPLE_RATE: f64 = 176_400.0;
/// Total render duration (seconds). Long enough for the attack to clear and
/// for the steady-state tail to give 0.5 s of clean analysis window.
pub const STIMULUS_RENDER_SECONDS: f64 = 1.5;
/// Analysis window — the tail end of the render where the attack envelope
/// has settled.
pub const STIMULUS_ANALYZE_SECONDS: f64 = 0.5;

/// Number of harmonics measured (H1 through H_N).
pub const NUM_HARMONICS: usize = 12;
/// Lower edge of the broadband HF measurement band.
pub const HF_BAND_LO_HZ: f64 = 5_000.0;
/// Upper edge of the broadband HF measurement band.
pub const HF_BAND_HI_HZ: f64 = 18_000.0;

/// A harmonic louder than this (dBc, at either rate) is compared in dB ...
pub const LADDER_REL_ABOVE_DBC: f64 = -80.0;
/// ... and must agree to within this many dB. The engine converges to
/// ≤ 0.07 dB here (2026-09-27); the backward-Euler fixture misses by dB.
pub const LADDER_REL_TOL_DB: f64 = 0.5;
/// Quieter harmonics are compared linearly: |a − b| re H1 must stay below
/// this (a dB gap is meaningless near the numerical floor).
pub const LADDER_LIN_TOL_DBC: f64 = -100.0;
/// `hf_band_dbc` must agree between the two rates to within this many dB when
/// either is above [`LADDER_REL_ABOVE_DBC`]; below that it is compared linearly
/// against [`LADDER_LIN_TOL_DBC`], as the quiet harmonics are (the melange power
/// amp leaves G6's band near −110 dBc, where a dB ratio is numerical dust).
pub const HF_BAND_TOL_DB: f64 = 1.0;
/// The off-grid energy the base rate adds (dBc) must stay below this. The engine
/// measures −103..−108 dBc (2026-09-27, both power amps); a hard nonlinearity run
/// at the host rate (the alias fixture) measures −72 dBc.
pub const ALIAS_EXCESS_TOL_DBC: f64 = -90.0;
/// Band over which off-grid (inharmonic) energy is summed.
pub const INHARMONIC_LO_HZ: f64 = 100.0;
/// Upper edge of the inharmonic band (below the base-rate Nyquist).
pub const INHARMONIC_HI_HZ: f64 = 20_000.0;
/// Kaiser window shape for the inharmonic spectrum.
pub const KAISER_BETA: f64 = 20.0;
/// Half-width of the mask around every harmonic of f0: the Kaiser mainlobe
/// (±6.5 bins of the unpadded 2 Hz resolution = ±12.9 Hz) plus the f0 error
/// times the harmonic number.
pub const HARMONIC_MASK_HZ: f64 = 20.0;

/// Measurements of one render's steady-state tail.
#[derive(Debug, Clone)]
pub struct Measurement {
    /// Host sample rate of the render.
    pub sample_rate: f64,
    /// H1 magnitude in dB FS (sanity check that the window captured a note).
    pub h1_dbfs: f64,
    /// `harmonic_dbc[i]` = magnitude of H(i+1) relative to H1 in dB
    /// (`harmonic_dbc[0]` is 0.0).
    pub harmonic_dbc: [f64; NUM_HARMONICS],
    /// All energy in `HF_BAND_LO_HZ..HF_BAND_HI_HZ`, dB relative to the energy in H1's mask
    /// (for a sinusoid, its level in dBc).
    pub hf_band_dbc: f64,
    /// Energy off the harmonic grid (`INHARMONIC_LO_HZ..INHARMONIC_HI_HZ`, every
    /// harmonic masked ±`HARMONIC_MASK_HZ`), dB relative to the energy in H1's mask.
    pub inharmonic_dbc: f64,
}

/// Result of one rate-convergence comparison.
#[derive(Debug, Clone)]
pub struct AliasAuditResult {
    /// Fundamental (Hz) measured on the base render and used for both.
    pub f0_hz: f64,
    /// The render at [`STIMULUS_SAMPLE_RATE`].
    pub base: Measurement,
    /// The render at [`REFERENCE_SAMPLE_RATE`].
    pub reference: Measurement,
    /// Largest |dB difference| over harmonics above [`LADDER_REL_ABOVE_DBC`].
    pub ladder_worst_rel_db: f64,
    /// Harmonic number (1-based) where it occurred.
    pub ladder_worst_rel_harmonic: usize,
    /// Largest linear difference re H1 (dBc) over the quieter harmonics.
    pub ladder_worst_lin_dbc: f64,
    /// `base.hf_band_dbc − reference.hf_band_dbc`.
    pub hf_band_delta_db: f64,
    /// Off-grid energy the base rate adds over the reference, dBc (−200 = none).
    pub alias_excess_dbc: f64,
}

impl AliasAuditResult {
    /// Every failed gate condition, in words; empty = converged.
    pub fn failures(&self) -> Vec<String> {
        let mut f = Vec::new();
        if self.ladder_worst_rel_db > LADDER_REL_TOL_DB {
            f.push(format!(
                "H{} differs by {:.2} dB between {:.1} and {:.1} kHz (tolerance {LADDER_REL_TOL_DB} dB)",
                self.ladder_worst_rel_harmonic,
                self.ladder_worst_rel_db,
                self.base.sample_rate / 1000.0,
                self.reference.sample_rate / 1000.0
            ));
        }
        if self.ladder_worst_lin_dbc > LADDER_LIN_TOL_DBC {
            f.push(format!(
                "quiet harmonics differ by {:.1} dBc linear (tolerance {LADDER_LIN_TOL_DBC} dBc)",
                self.ladder_worst_lin_dbc
            ));
        }
        if self.alias_excess_dbc > ALIAS_EXCESS_TOL_DBC {
            f.push(format!(
                "the base rate adds {:.1} dBc of off-grid energy (tolerance {ALIAS_EXCESS_TOL_DBC} dBc)",
                self.alias_excess_dbc
            ));
        }
        let (hb, hr) = (self.base.hf_band_dbc, self.reference.hf_band_dbc);
        if hb.max(hr) > LADDER_REL_ABOVE_DBC {
            if self.hf_band_delta_db.abs() > HF_BAND_TOL_DB {
                f.push(format!(
                    "hf_band differs by {:+.2} dB between the rates (tolerance {HF_BAND_TOL_DB} dB)",
                    self.hf_band_delta_db
                ));
            }
        } else {
            // Energy ratios, so the linear difference is taken in energy.
            let lin = mag_to_db((10f64.powf(hb / 10.0) - 10f64.powf(hr / 10.0)).abs().sqrt());
            if lin > LADDER_LIN_TOL_DBC {
                f.push(format!(
                    "quiet hf_band differs by {lin:.1} dBc (tolerance {LADDER_LIN_TOL_DBC} dBc)"
                ));
            }
        }
        f
    }
}

/// Audit the canonical single-note stimulus.
pub fn run() -> AliasAuditResult {
    run_with_note(STIMULUS_NOTE, STIMULUS_VELOCITY)
}

/// Variant with overridable note + velocity. Production regression tests
/// should call [`run_sweep`] so the gate covers the full stimulus set.
pub fn run_with_note(note: u8, velocity: u8) -> AliasAuditResult {
    run_with_note_at_volume(note, velocity, STIMULUS_VOLUME)
}

/// Variant with an overridable pot position. Since 2026-09-23 user volume is
/// the drawn pot between preamp and power amp, so the amp's drive depends on
/// it. The canonical stimulus uses `STIMULUS_VOLUME`.
pub fn run_with_note_at_volume(note: u8, velocity: u8, volume: f64) -> AliasAuditResult {
    let base = render_stimulus_at(note, velocity, volume, STIMULUS_SAMPLE_RATE);
    let reference = render_stimulus_at(note, velocity, volume, REFERENCE_SAMPLE_RATE);
    compare(
        &base,
        STIMULUS_SAMPLE_RATE,
        &reference,
        REFERENCE_SAMPLE_RATE,
        midi_note_hz(note),
    )
}

/// One sweep entry: the stimulus note paired with its measurement result.
#[derive(Debug, Clone)]
pub struct SweepEntry {
    pub note: u8,
    pub velocity: u8,
    pub result: AliasAuditResult,
}

/// Audit every note in [`STIMULUS_NOTES`] at [`STIMULUS_VELOCITY`].
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

/// The audit's stimulus signal itself (engine output at [`STIMULUS_SAMPLE_RATE`]), for
/// inspecting it with other analysers.
pub fn render_stimulus(note: u8, velocity: u8, volume: f64) -> Vec<f64> {
    render_stimulus_at(note, velocity, volume, STIMULUS_SAMPLE_RATE)
}

/// [`render_stimulus`] at an arbitrary host sample rate: the plugin's chain.
pub fn render_stimulus_at(note: u8, velocity: u8, volume: f64, sr: f64) -> Vec<f64> {
    let mut eng = WurliEngine::new(sr);
    eng.ensure_buffer_capacity(1024);
    eng.set_volume(volume);
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

/// Compare two renders of the same stimulus at different host rates. The
/// fundamental is measured on `base` and used for both (the reed's pitch is
/// rate-independent, so a pitch difference also shows as a failure).
pub fn compare(
    base: &[f64],
    base_sr: f64,
    reference: &[f64],
    reference_sr: f64,
    nominal_f0: f64,
) -> AliasAuditResult {
    let base_tail = tail(base, base_sr);
    let f0 = refine_f0(base_tail, base_sr, nominal_f0);
    let b = measure(base_tail, base_sr, f0);
    let r = measure(tail(reference, reference_sr), reference_sr, f0);

    let (mut worst_rel, mut worst_h, mut worst_lin) = (0.0f64, 1usize, -200.0f64);
    for k in 1..NUM_HARMONICS {
        let (x, y) = (b.harmonic_dbc[k], r.harmonic_dbc[k]);
        if x.max(y) > LADDER_REL_ABOVE_DBC {
            if (x - y).abs() > worst_rel {
                worst_rel = (x - y).abs();
                worst_h = k + 1;
            }
        } else {
            let lin = (10f64.powf(x / 20.0) - 10f64.powf(y / 20.0)).abs();
            worst_lin = worst_lin.max(mag_to_db(lin));
        }
    }
    let excess = 10f64.powf(b.inharmonic_dbc / 10.0) - 10f64.powf(r.inharmonic_dbc / 10.0);
    AliasAuditResult {
        f0_hz: f0,
        hf_band_delta_db: b.hf_band_dbc - r.hf_band_dbc,
        alias_excess_dbc: if excess > 0.0 {
            10.0 * excess.log10()
        } else {
            -200.0
        },
        base: b,
        reference: r,
        ladder_worst_rel_db: worst_rel,
        ladder_worst_rel_harmonic: worst_h,
        ladder_worst_lin_dbc: worst_lin,
    }
}

/// The steady-state tail: the last `STIMULUS_ANALYZE_SECONDS` of the render.
fn tail(signal: &[f64], sr: f64) -> &[f64] {
    let n = (sr * STIMULUS_ANALYZE_SECONDS) as usize;
    assert!(
        signal.len() >= n,
        "alias_audit signal too short: {} samples for {n} analysis window",
        signal.len()
    );
    &signal[signal.len() - n..]
}

fn measure(tail: &[f64], sr: f64, f0: f64) -> Measurement {
    let h1 = dft_magnitude(tail, f0, sr);
    let mut harmonic_dbc = [0.0f64; NUM_HARMONICS];
    for (k, slot) in harmonic_dbc.iter_mut().enumerate().skip(1) {
        let mag = dft_magnitude(tail, (k + 1) as f64 * f0, sr);
        *slot = if h1 > 0.0 {
            20.0 * (mag / h1).log10()
        } else {
            -200.0
        };
    }
    let (inharmonic_dbc, hf_band_dbc) = spectral_energies_dbc(tail, sr, f0);
    Measurement {
        sample_rate: sr,
        h1_dbfs: mag_to_db(h1),
        harmonic_dbc,
        hf_band_dbc,
        inharmonic_dbc,
    }
}

/// (off-grid energy, all energy in `HF_BAND_LO_HZ..HF_BAND_HI_HZ`), each relative to H1.
///
/// Off-grid energy relative to H1: Kaiser window (beta `KAISER_BETA`, sidelobes ~−150 dB, so a
/// harmonic's leakage cannot pose as residual), zero-padded power-of-two FFT, every harmonic of
/// `f0` masked ±`HARMONIC_MASK_HZ`, the rest of `INHARMONIC_LO_HZ..INHARMONIC_HI_HZ` summed and
/// divided by the energy inside H1's mask. A ratio of energies under one window, so no window
/// normalisation enters. (A −93 dB-sidelobe window, Blackman-Harris or Nuttall, summed to a
/// −89 dBc floor over the band, which hid real aliasing.)
fn spectral_energies_dbc(signal: &[f64], sr: f64, f0: f64) -> (f64, f64) {
    let n = signal.len();
    let n_fft = n.next_power_of_two();
    let mut re = vec![0.0f64; n_fft];
    let mut im = vec![0.0f64; n_fft];
    let i0_beta = bessel_i0(KAISER_BETA);
    let half = (n.max(2) - 1) as f64 / 2.0;
    for (i, &x) in signal.iter().enumerate() {
        let r = (i as f64 - half) / half;
        re[i] = x * bessel_i0(KAISER_BETA * (1.0 - r * r).max(0.0).sqrt()) / i0_beta;
    }
    fft_in_place(&mut re, &mut im);
    let bin_hz = sr / n_fft as f64;
    let (mut off, mut h1, mut band) = (0.0f64, 0.0f64, 0.0f64);
    for k in 1..n_fft / 2 {
        let f = k as f64 * bin_hz;
        let p = re[k] * re[k] + im[k] * im[k];
        if (HF_BAND_LO_HZ..=HF_BAND_HI_HZ).contains(&f) {
            band += p;
        }
        let m = (f / f0).round().max(1.0);
        let on_grid = (f - m * f0).abs() <= HARMONIC_MASK_HZ;
        if on_grid && m == 1.0 {
            h1 += p;
        } else if !on_grid && (INHARMONIC_LO_HZ..=INHARMONIC_HI_HZ).contains(&f) {
            off += p;
        }
    }
    let db = |e: f64| {
        if h1 > 0.0 && e > 0.0 {
            10.0 * (e / h1).log10()
        } else {
            -200.0
        }
    };
    (db(off), db(band))
}

/// Modified Bessel function of the first kind, order 0 (power series; converges fast for the
/// arguments used here, |x| <= KAISER_BETA).
fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, q) = (1.0f64, 1.0f64, x * x / 4.0);
    for k in 1..200 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// Iterative radix-2 complex FFT, in place (`re.len()` a power of two).
fn fft_in_place(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        let (w_re, w_im) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut c_re, mut c_im) = (1.0f64, 0.0f64);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let t_re = re[b] * c_re - im[b] * c_im;
                let t_im = re[b] * c_im + im[b] * c_re;
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                let nc = c_re * w_re - c_im * w_im;
                c_im = c_re * w_im + c_im * w_re;
                c_re = nc;
            }
        }
        len <<= 1;
    }
}

/// Single-frequency DFT magnitude through a Hann window, normalised by the window's coherent
/// gain so a steady sinusoid reads its amplitude.
///
/// Hann, not rectangular (changed 2026-09-27): the stimulus tail is a DECAYING tone, and a
/// rectangular window's far sidelobes (6 dB/oct) leak the fundamental and strong neighbours into
/// every harmonic bin at roughly -75..-82 dBc. Harmonics below that floor read as the floor, or as
/// a leakage-cancelled "notch" (note 72, melange power amp: the Hann reading of the same bin was
/// 12 dB higher). Hann's sidelobes fall at 18 dB/oct, far below anything measured here.
fn dft_magnitude(signal: &[f64], freq: f64, sr: f64) -> f64 {
    let n = signal.len();
    let mut re = 0.0;
    let mut im = 0.0;
    let mut w_sum = 0.0;
    let omega = 2.0 * std::f64::consts::PI * freq / sr;
    let hann_step = 2.0 * std::f64::consts::PI / (n.max(2) - 1) as f64;
    for (i, &s) in signal.iter().enumerate() {
        let w = 0.5 - 0.5 * (hann_step * i as f64).cos();
        let phase = omega * i as f64;
        re += w * s * phase.cos();
        im -= w * s * phase.sin();
        w_sum += w;
    }
    2.0 * (re * re + im * im).sqrt() / w_sum
}

fn mag_to_db(mag: f64) -> f64 {
    if mag > 0.0 {
        20.0 * mag.log10()
    } else {
        -200.0
    }
}

/// Peak of the DFT magnitude within ±5 Hz of `nominal_f0`: a 0.1 Hz scan, then a
/// 0.005 Hz scan around its best point. Absorbs per-note detuning and MLP offsets; the
/// fine step keeps the error at H12 well inside the Hann mainlobe.
fn refine_f0(signal: &[f64], sr: f64, nominal_f0: f64) -> f64 {
    let scan = |lo: f64, hi: f64, step: f64, mut best: (f64, f64)| {
        let mut f = lo;
        while f <= hi {
            let mag = dft_magnitude(signal, f, sr);
            if mag > best.1 {
                best = (f, mag);
            }
            f += step;
        }
        best
    };
    let coarse = scan(
        nominal_f0 - 5.0,
        nominal_f0 + 5.0,
        0.1,
        (nominal_f0, dft_magnitude(signal, nominal_f0, sr)),
    );
    scan(coarse.0 - 0.1, coarse.0 + 0.1, 0.005, coarse).0
}

#[inline]
fn midi_note_hz(note: u8) -> f64 {
    440.0 * 2.0_f64.powf((note as f64 - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_F0: f64 = 1045.2023;

    /// The same analog input at any rate: a decaying, harmonic-rich C6-ish tone
    /// (f0 not commensurate with 44.1 kHz).
    fn fixture_tone(sr: f64) -> Vec<f64> {
        (0..(sr * STIMULUS_RENDER_SECONDS) as usize)
            .map(|i| {
                let t = i as f64 / sr;
                let w = 2.0 * std::f64::consts::PI * FIXTURE_F0 * t;
                (-t * 12.0 / 8.686).exp()
                    * (0.6 * w.sin() + 0.2 * (2.0 * w).sin() + 0.1 * (3.0 * w).sin())
            })
            .collect()
    }

    /// Run a per-sample process at the base and reference rates and compare.
    fn audit_process(process: impl Fn(&[f64], f64) -> Vec<f64>) -> AliasAuditResult {
        let (b, r) = (STIMULUS_SAMPLE_RATE, REFERENCE_SAMPLE_RATE);
        compare(
            &process(&fixture_tone(b), b),
            b,
            &process(&fixture_tone(r), r),
            r,
            FIXTURE_F0,
        )
    }

    #[test]
    fn fft_matches_direct_dft() {
        let n = 64;
        let x: Vec<f64> = (0..n)
            .map(|i| ((i * 7919) % 97) as f64 / 97.0 - 0.5)
            .collect();
        let (mut re, mut im) = (x.clone(), vec![0.0; n]);
        fft_in_place(&mut re, &mut im);
        for k in 0..n {
            let (mut dr, mut di) = (0.0, 0.0);
            for (i, &v) in x.iter().enumerate() {
                let a = -2.0 * std::f64::consts::PI * (k * i) as f64 / n as f64;
                dr += v * a.cos();
                di += v * a.sin();
            }
            assert!(
                (re[k] - dr).abs() < 1e-9 && (im[k] - di).abs() < 1e-9,
                "bin {k}"
            );
        }
    }

    #[test]
    fn inharmonic_residual_is_quiet_on_a_clean_harmonic_tone() {
        let sr = STIMULUS_SAMPLE_RATE;
        let x = fixture_tone(sr);
        let (r, _) = spectral_energies_dbc(tail(&x, sr), sr, FIXTURE_F0);
        assert!(
            r < -100.0,
            "clean harmonic tone should leave no off-grid energy, got {r:.1} dBc"
        );
    }

    #[test]
    fn converged_linear_process_passes() {
        // A bilinear (prewarped) one-pole lowpass at 5 kHz: a linear filter matched in
        // frequency at every rate, so the gate must see no difference.
        let r = audit_process(|x, sr| {
            let w = (std::f64::consts::PI * 5_000.0 / sr).tan();
            let (b0, a1) = (w / (1.0 + w), (1.0 - w) / (1.0 + w));
            let (mut y, mut xp) = (0.0, 0.0);
            x.iter()
                .map(|&v| {
                    y = b0 * (v + xp) + a1 * y;
                    xp = v;
                    y
                })
                .collect()
        });
        assert!(
            r.failures().is_empty(),
            "linear filter must converge: {:?}",
            r.failures()
        );
    }

    #[test]
    fn catches_under_sampled_stiff_solver() {
        // The historical tear's mechanism: a stiff nonlinear circuit integrated with backward
        // Euler at too low a rate. Diode clipper, C dv/dt = (x - v)/R - 2 Is sinh(v / nVt),
        // R 2.2k, C 10 nF (7.2 kHz corner), Newton on each step.
        let r = audit_process(|x, sr| {
            let (rr, c, is, nvt) = (2.2e3, 10e-9, 2.52e-9, 0.045);
            let dt = 1.0 / sr;
            let mut v = 0.0f64;
            x.iter()
                .map(|&xin| {
                    let xin = 2.0 * xin; // drive into clipping
                    let mut vn = v;
                    for _ in 0..50 {
                        let i_d = 2.0 * is * (vn / nvt).sinh();
                        let g = vn - v - dt / c * ((xin - vn) / rr - i_d);
                        let dg = 1.0 + dt / c * (1.0 / rr + 2.0 * is / nvt * (vn / nvt).cosh());
                        let step = g / dg;
                        vn -= step;
                        if step.abs() < 1e-12 {
                            break;
                        }
                    }
                    v = vn;
                    v
                })
                .collect()
        });
        eprintln!(
            "BE clipper: worst H{} {:.2} dB, hf_band {:+.2} dB",
            r.ladder_worst_rel_harmonic, r.ladder_worst_rel_db, r.hf_band_delta_db
        );
        assert!(
            r.ladder_worst_rel_db > 4.0 * LADDER_REL_TOL_DB,
            "an under-sampled stiff solver must fail the ladder by a wide margin: {:.2} dB",
            r.ladder_worst_rel_db
        );
    }

    #[test]
    fn catches_folded_alias_hash() {
        // Classic aliasing: a hard memoryless nonlinearity at the host rate folds its
        // products above Nyquist back into the audio band at 44.1 kHz, not at 176.4 kHz.
        // Its in-band harmonics swamp the alias hash in an RMS band measure (hf_band moves only
        // ~0.2 dB); the off-grid excess is what sees it.
        let r = audit_process(|x, _sr| x.iter().map(|&v| (20.0 * v).tanh()).collect());
        eprintln!(
            "hard tanh: worst H{} {:.2} dB, hf_band {:+.2} dB, alias excess {:.1} dBc",
            r.ladder_worst_rel_harmonic,
            r.ladder_worst_rel_db,
            r.hf_band_delta_db,
            r.alias_excess_dbc
        );
        assert!(
            r.alias_excess_dbc > ALIAS_EXCESS_TOL_DBC + 12.0,
            "folded alias hash must fail the off-grid excess by a wide margin: {:.1} dBc",
            r.alias_excess_dbc
        );
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

    #[test]
    fn dft_magnitude_resolves_weak_harmonic_under_decaying_fundamental() {
        // A decaying fundamental (not periodic in the window) and a weak upper harmonic. A
        // rectangular window leaked the fundamental into H10's bin at about -75..-82 dBc and
        // misread it; the windowed DFT must read the harmonic's true level.
        let sr = 44_100.0;
        let f0 = 522.4511;
        let n = (sr * 0.5) as usize;
        let h10_db = -90.0; // below the rectangular window's floor (it read -81.5 here)
        let a10 = 10f64.powf(h10_db / 20.0);
        let signal: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                let env = (-t * 12.0 / 8.686).exp(); // 12 dB/s decay
                let w = 2.0 * std::f64::consts::PI * f0 * t;
                env * (w.sin() + a10 * (10.0 * w + 0.7).sin())
            })
            .collect();
        let h1 = dft_magnitude(&signal, f0, sr);
        let h10 = dft_magnitude(&signal, 10.0 * f0, sr);
        let got = 20.0 * (h10 / h1).log10();
        assert!(
            (got - h10_db).abs() < 0.5,
            "H10 should read {h10_db} dBc through leakage, got {got:.2}"
        );
    }
}
