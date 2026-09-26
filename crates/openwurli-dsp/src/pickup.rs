//! Electrostatic pickup model — time-varying RC circuit.
//!
//! The Wurlitzer 200A pickup is a capacitive sensor: reed vibration modulates
//! the capacitance between the reed and a charged metal comb (+147V DC).
//!
//! # Geometry and law (2026-09 overlap-law revision)
//!
//! The pickup is a **through-slot comb**: each reed sits laterally between two
//! fingers, across a small side clearance, with no floor under it. At rest the
//! reed is offset vertically just outside the finger band (about a sheet of
//! paper); bass reeds sit below the band, treble reeds above it. As the reed
//! swings into the band its capacitance rises with the reed-to-finger OVERLAP,
//! fringe-rounded at the band edges. It does not follow a parallel-plate
//! `1/(1-y)` — that law assumed a plate facing the reed across a gap, which the
//! 200A does not have.
//!
//! The law `C(u)` (u = penetration past the near finger face, mm) is a 2D field
//! solve of the cross-section per measured reed range (reed width and side
//! clearance per range 1-14 | 15-20 | 21-42 | 43-64; finger thickness
//! changes at reeds 20|21 and 42|43), compiled in as a C2 cubic spline
//! ([`PickupLawTable`], `pickup_tables.rs`). Spec, evidence and numerics:
//! docs/research/pickup-overlap-law.md.
//!
//! # Where the nonlinearity lives
//!
//! **The nonlinearity is in the SOURCE, not in the network.** Only ONE reed
//! moves. Its capacitance changes deeply over a swing, but that reed is only
//! ~3 pF of the ~240 pF sitting on the plate node; the other 63 reeds, the stray
//! and the cable do not move. So the node equation is
//!
//! ```text
//!   C_node(t) * dv/dt + v/R_net = -V_pol * dC_reed/dt
//!   C_node(t) = C_TOTAL + (C_reed(t) - C_reed(rest))
//! ```
//!
//! 1. **The law is the SOURCE term** — the charge the moving reed injects. It is
//!    frequency-independent: it is there at 5 kHz exactly as much as at 50 Hz.
//! 2. **The network is very nearly LTI.** `C_node` moves by about
//!    `C_REED0/C_TOTAL = 1/80` of the source depth.
//!
//! An earlier model modulated the ENTIRE node capacitance, an 80x
//! over-modulation of the relaxation time constant; its harmonics inherited the
//! filter's frequency dependence and vanished above the corner. That defect is
//! gone and stays gone: the source carries the harmonics.
//!
//! ## Structure
//!
//! ```text
//!   u = band_sign * amplitude_mm * displacement - PICKUP_REST_OFFSET_MM
//!   s = law(u) - law(rest)                                  // reed charge, THE nonlinearity
//!   m = 1 + C_REED_RATIO * s                                // node cap, ~LTI
//!   w = (w*(m_avg - g0) - (s - s_prev)) / (m_avg + g0)      // network
//!   out = w * PICKUP_SENSITIVITY
//! ```
//!
//! The `w` recursion is the exact trapezoidal discretization of the node
//! equation above, so it is a differentiator inside a one-pole lag — i.e. the
//! `jw`-driven high-pass the circuit reference describes, not a lowpass. Its
//! small-signal response reproduces a one-pole high-pass at `PICKUP_FC` to
//! **0.011 dB** over 100 Hz - 10 kHz.
//!
//! # Drawn-topology refit (2026-09-13)
//!
//! The pre-revision model used `TAU = 287k x 240pF` (f_c = 2312 Hz). That 287k
//! came from a superseded reading of the input network (R-2 as a 2 MEG bias
//! resistor at TR-1's base). On the drawn topology R-2 is a 1 MEG polarizing
//! feed on the PICKUP side of the input coupling cap, and C-2 (220 pF) sits at
//! TR-1's base — tied to the plate's own capacitance through C-1 (0.022 uF),
//! which is a short at audio. The plate therefore works into very nearly
//! `C_TOTAL + C2_BASE` rather than `C_TOTAL` alone, and the corner drops by
//! roughly an octave and a half.
//!
//! ## Structure: one pole, no correction stage — and why
//!
//! The corrected network is NOT first-order (it is a two-capacitor divider
//! bridged by R-1 inside an R-2/R-3 resistive network), so a correction stage
//! was budgeted for. It turned out not to be worth carrying. Fitted against the
//! cross-checked jw-driven response (reed charge current into TR-1's base,
//! normalised at 10 kHz), a single pole reproduces the network to **0.23 dB
//! max error over 55 Hz - 10 kHz**:
//!
//! | f (Hz) |   55 |  110 |  220 |  262 |  440 |  880 | 1760 | 3520 | 7040 |
//! |--------|------|------|------|------|------|------|------|------|------|
//! | target |-24.3 |-18.0 |-12.1 |-10.6 | -6.7 | -2.8 | -0.8 | -0.1 | +0.0 |
//! | model  |-24.2 |-18.3 |-12.4 |-11.0 | -7.1 | -3.1 | -1.0 | -0.2 | -0.0 |
//! | error  |+0.07 |-0.29 |-0.35 |-0.36 |-0.35 |-0.28 |-0.17 |-0.07 |-0.02 |
//!
//! Adding a fitted biquad shelf on top improves the worst case from 0.229 dB
//! to 0.196 dB — 0.03 dB, for a biquad's state, CPU and phase shift. It is not
//! worth it, and more importantly it would be false precision: `C_TOTAL` itself
//! is LOW confidence (see below) and a +-20% error there moves the corner by
//! far more than the shape error the biquad would remove. If an LCR measurement
//! ever pins C_TOTAL tightly AND a residual shape error still matters, the
//! shelf to add is `1 + (G-1)*lowpass(fc, Q)` with G = +0.96 dB, fc = 1504 Hz,
//! Q = 0.427 (fitted against the 1012 Hz lumped-RC core, not this one).
//!
//! The one-pole fit is robust across the C_TOTAL uncertainty band: worst-case
//! error stays <= 0.28 dB for C_TOTAL anywhere in 120-500 pF.
//!
//! ## What this changes audibly
//!
//! At constant treble level (anchored at 10 kHz) the corrected network passes
//! **~+8 dB more bass below 440 Hz** than the 2312 Hz model: +8.0 dB at 55 Hz,
//! +7.7 dB at C4 (262 Hz), +5.7 dB at 880 Hz, +1.1 dB at 3.5 kHz, ~0 above
//! 7 kHz. That is the intended physics correction, not collateral. Downstream
//! voicing (tables.rs, DS constants, output_scale) is deliberately NOT
//! recalibrated here — that is a Phase-3 decision with a listening session.
//!
//! This produces:
//! - Small-signal HPF at f_c = PICKUP_FC (880 Hz with C-2 returned to ground)
//! - Harmonic generation from the reed's own charge, **independent of
//!   frequency** — the reed enters the finger band on one half of its swing
//!   only, so the source is strongly one-sided (even harmonics dominate).
//! - Correct polarity: reed into the band gives more capacitance at ~constant
//!   charge, i.e. a negative-going plate voltage.

use std::f64::consts::PI;

/// Total system capacitance at the pickup plate.
///
/// **LOW CONFIDENCE.** This is a single forum measurement, adversarially
/// reviewed and found to have no primary source (no patent, service manual or
/// paper gives a pickup capacitance for the 200A). It is retained because it is
/// the only number available and is geometrically plausible, but it is an
/// ASSUMPTION, not a measurement, and the corner inherits that confidence.
/// An LCR reading of a real reed bar would settle it — and is expected.
///
/// Corner sensitivity (from the scaling law below, which is what the code
/// applies): 150 pF -> 1095 Hz, 240 pF -> 880 Hz, 400 pF -> 653 Hz. Changing
/// this constant moves `PICKUP_FC` automatically.
pub const C_TOTAL: f64 = 240.0e-12;

/// C-2, the 220 pF capacitor at TR-1's base. Fixed 200A part. It is bridged to
/// the plate's own capacitance by C-1 (0.022 uF, a short at audio), which is
/// why it lands in the pickup's corner at all.
const C2_BASE: f64 = 220.0e-12;

/// **Open fork — C-2's return node.**
///
/// C-2's bottom rail is drawn BROKEN on both circulating scan surfaces (a
/// drawing defect, not a reading failure). Ground is the settled assumption and
/// what the netlists carry. The alternative is a return to TR-1's emitter,
/// where the high loop gain bootstraps C-2 almost entirely away and the corner
/// rises to ~1436 Hz.
///
/// Real-hardware evidence may settle this within days. Flipping this one `bool`
/// is the whole change — the corner, the tests' expectations and the scaling
/// law all follow from it.
const C2_RETURNS_TO_GROUND: bool = true;

/// Reference corner at `C_TOTAL` = 240 pF, C-2 returned to GROUND.
///
/// Not the network's -3 dB point (that is 897-900 Hz on the cross-check) but
/// the single pole that best reproduces the WHOLE in-band curve — fitting the
/// curve beats matching one point, and costs 0.23 dB instead of 0.37 dB.
const FC_REF_C2_GROUND: f64 = 880.5;

/// Reference corner with C-2 returned to TR-1's emitter (bootstrapped away).
/// Same fit quality: 0.23 dB max error over 55 Hz - 10 kHz.
const FC_REF_C2_EMITTER: f64 = 1436.0;

/// Reference total capacitance the two `FC_REF_*` constants were fitted at.
const C_TOTAL_REF: f64 = 240.0e-12;

/// Small-signal corner of the pickup network.
///
/// Scales with the node capacitance so a future LCR measurement of `C_TOTAL` is
/// a one-constant change: `f = f_ref * (C_ref + C2) / (C_total + C2)`. Verified
/// against the full network across 120-500 pF, accurate to +-4.3% there.
pub const PICKUP_FC: f64 = {
    let f_ref = if C2_RETURNS_TO_GROUND {
        FC_REF_C2_GROUND
    } else {
        FC_REF_C2_EMITTER
    };
    f_ref * (C_TOTAL_REF + C2_BASE) / (C_TOTAL + C2_BASE)
};

/// RC time constant of the pickup's charge dynamics, `1 / (2*pi*PICKUP_FC)`.
///
/// This is the time constant the NONLINEAR core runs on — moving it is the
/// intended physics change, not a side effect: the source relaxes on the
/// drawn network's dominant pole instead of the superseded 2312 Hz one.
const TAU: f64 = 1.0 / (2.0 * PI * PICKUP_FC);

/// Polarizing voltage on the pickup plate.
pub const V_POLARIZING: f64 = 147.0;

/// Rest capacitance of ONE reed to the plate — the only capacitance that moves.
///
/// This was always implicit in `PICKUP_SENSITIVITY` (147 * 3/240 = 1.8375); the
/// 2026-09-14 restructure needs it explicitly, because the ratio
/// `C_REED0 / C_TOTAL` is exactly how much of the node the nonlinearity is
/// allowed to modulate. Naming it also exposes its confidence: it inherits
/// `C_TOTAL`'s LOW confidence and adds its own (the per-reed share of the node
/// is a geometric estimate, not a measurement). The same LCR reading that would
/// settle `C_TOTAL` should settle this.
pub const C_REED0: f64 = 3.0e-12;

/// Fraction of the plate node the moving reed represents: `C_REED0 / C_TOTAL`,
/// nominally 1/80.
///
/// **This is the constant the previous model got wrong.** It modulated the whole
/// node (an implicit ratio of 1.0) instead of this 0.0125 — the 80x
/// over-modulation of the relaxation time constant that the module header
/// describes. Setting this to 1.0 recovers the previous model's network
/// behaviour, which is a useful thing to know when reading old measurements.
const C_REED_RATIO: f64 = C_REED0 / C_TOTAL;

/// Pickup sensitivity: the high-frequency asymptotic gain, `V_pol * C_REED0 / C_TOTAL`.
///
/// Derived rather than hard-coded so it cannot drift away from the capacitances
/// it is made of. Evaluates to 1.8375, the long-standing project value.
///
/// Physically this is the gain on the normalised source `s` well above the
/// corner: the plate node swings by `V_pol * C_REED0/C_TOTAL * s`, with every
/// harmonic of `s` intact (the previous model's HF asymptote was linear in the
/// displacement, which is why its harmonics vanished above the corner).
pub const PICKUP_SENSITIVITY: f64 = V_POLARIZING * C_REED0 / C_TOTAL;

/// One keyboard section's capacitance-vs-position law: the reed-to-pickup capacitance as a
/// function of `u`, the reed's penetration past the near finger face (mm), from a 2D field solve
/// of the through-slot comb (docs/research/pickup-overlap-law.md). Stored as a natural cubic
/// spline on a uniform grid so the law and its first two derivatives are continuous — the
/// harmonics come from the curve's bends, so C1 interpolation would not do.
pub struct PickupLawTable {
    /// Penetration of the first knot (mm).
    pub u0: f64,
    /// Knot spacing (mm).
    pub du: f64,
    /// Reed thickness this table was solved for (mm).
    pub t_r: f64,
    /// Finger thickness this table was solved for (mm).
    pub t_p: f64,
    /// Capacitance at each knot, normalised (see `pickup_tables`).
    pub y: &'static [f64],
    /// Second derivative of the spline at each knot.
    pub m: &'static [f64],
}

impl PickupLawTable {
    /// Penetration of the last knot (mm).
    pub fn u_max(&self) -> f64 {
        self.u0 + self.du * (self.y.len() - 1) as f64
    }

    /// Normalised capacitance at penetration `u` (mm). Outside the solved range the value is held
    /// at the end knot; the amplitude guard keeps every note inside it.
    #[inline]
    pub fn eval(&self, u: f64) -> f64 {
        let last = self.y.len() - 2;
        let pos = ((u - self.u0) / self.du).clamp(0.0, (last + 1) as f64);
        let i = (pos as usize).min(last);
        let t = pos - i as f64;
        let a = 1.0 - t;
        let h2 = self.du * self.du / 6.0;
        a * self.y[i]
            + t * self.y[i + 1]
            + ((a * a * a - a) * self.m[i] + (t * t * t - t) * self.m[i + 1]) * h2
    }
}

/// The law for a note's reed. One table per measured reed range (reed 1 = MIDI 33): reed width
/// and side clearance come from one measured row per range (1-14 | 15-20 | 21-42 | 43-64, one
/// 140B comb); finger thickness follows the finger sections (thick 1-20 | thin 21-42 | thick
/// 43-64).
/// The bass/treble solder flip at 20|21 puts the pickup ABOVE bass reeds and BELOW treble reeds.
pub fn pickup_law_for(midi: u8) -> &'static PickupLawTable {
    use crate::pickup_tables::{R01_14, R15_20, R21_42, R43_64};
    match midi.saturating_sub(32) {
        0..=14 => &R01_14,
        15..=20 => &R15_20,
        21..=42 => &R21_42,
        _ => &R43_64,
    }
}

/// Direction from the reed's positive (hammer-side, upward) displacement toward the finger band:
/// +1 where the pickup sits above the reed (bass, reeds 1-20), -1 where it sits below (treble).
pub fn pickup_band_sign(midi: u8) -> f64 {
    if midi.saturating_sub(32) <= 20 {
        1.0
    } else {
        -1.0
    }
}

/// Vertical rest offset between the reed and the finger band (mm): "a piece of paper … but just
/// barely" (technician, bass, first-hand); assumed equal in the treble. Middle of the 0.07-0.10 mm
/// evidence band.
pub const PICKUP_REST_OFFSET_MM: f64 = 0.085;

pub struct Pickup {
    /// Network state: normalized plate-node voltage perturbation `w = v / SENSITIVITY`.
    w: f64,
    /// Previous source sample (the recursion needs `ds/dt`).
    s_prev: f64,
    /// Previous normalized node capacitance `m = 1 + C_REED_RATIO * s`.
    m_prev: f64,
    /// Precomputed: `dt / (2 * TAU)` = `pi * PICKUP_FC / sample_rate`.
    beta: f64,
    /// This note's section law (field-solved capacitance vs penetration).
    law: &'static PickupLawTable,
    /// +1 if the finger band is above the reed (bass), -1 if below (treble).
    band_sign: f64,
    /// Law value at rest, so the source is zero with the reed at rest.
    c_rest: f64,
    /// Reed swing at the pickup per unit of model displacement (mm).
    amplitude_mm: f64,
}

impl Pickup {
    /// Pickup for one note: its section's law, the side of the band it sits on, and the swing
    /// (mm) that a unit of model displacement stands for.
    pub fn new_for_note(sample_rate: f64, midi: u8, amplitude_mm: f64) -> Self {
        let dt = 1.0 / sample_rate;
        let law = pickup_law_for(midi);
        Self {
            w: 0.0,
            s_prev: 0.0,
            m_prev: 1.0,
            beta: dt / (2.0 * TAU),
            law,
            band_sign: pickup_band_sign(midi),
            c_rest: law.eval(-PICKUP_REST_OFFSET_MM),
            amplitude_mm,
        }
    }

    /// Change the swing scale (mm per unit of model displacement).
    pub fn set_amplitude_mm(&mut self, amplitude_mm: f64) {
        self.amplitude_mm = amplitude_mm;
    }

    /// Process a buffer of reed displacement samples in-place.
    ///
    /// Input: reed displacement in normalized model units (positive = the hammer's direction,
    /// upward). Output: pickup voltage in volts (millivolt-scale signals).
    ///
    /// The nonlinearity lives in the SOURCE — the moving reed's capacitance to the fingers, a
    /// field-solved function of how far it has entered the finger band — and is
    /// frequency-independent; the network that follows is very nearly LTI, a differentiator
    /// inside a one-pole lag with its corner at PICKUP_FC.
    pub fn process(&mut self, buffer: &mut [f64]) {
        let beta = self.beta;
        let scale = self.band_sign * self.amplitude_mm;
        for sample in buffer.iter_mut() {
            // ── Source: the moving reed's capacitance to the fingers. THE nonlinearity. ──
            // u = penetration past the near finger face; at rest the reed sits just outside the
            // band (u = -PICKUP_REST_OFFSET_MM).
            let u = scale * *sample - PICKUP_REST_OFFSET_MM;
            let s = self.law.eval(u) - self.c_rest;

            // ── Network: one moving reed against a fixed node. ~LTI. ──
            let m = 1.0 + C_REED_RATIO * s;
            let m_avg = 0.5 * (m + self.m_prev);

            // Exact trapezoidal discretization of
            //   C_node(t)*dv/dt + v/R_net = -V_pol * dC_reed/dt
            // normalized by C_TOTAL and PICKUP_SENSITIVITY.
            self.w = (self.w * (m_avg - beta) - (s - self.s_prev)) / (m_avg + beta);

            self.s_prev = s;
            self.m_prev = m;
            *sample = self.w * PICKUP_SENSITIVITY;
        }
    }

    pub fn reset(&mut self) {
        self.w = 0.0;
        self.s_prev = 0.0;
        self.m_prev = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// Middle C at ff, as the voice drives it (provisional swing, mm).
    const A_C4: f64 = 1.19;

    fn render(midi: u8, amp: f64, freq: f64, secs: f64) -> Vec<f64> {
        let sr = 44100.0;
        let mut pickup = Pickup::new_for_note(sr, midi, A_C4);
        let n = (sr * secs) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| amp * (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);
        buf
    }

    #[test]
    fn test_dc_equilibrium() {
        // Zero displacement should produce zero output: the source is referenced to rest.
        let peak = render(60, 0.0, 100.0, 0.05)
            .iter()
            .fold(0.0f64, |a, x| a.max(x.abs()));
        assert!(peak < 1e-12, "zero displacement gave {peak:.2e}");
    }

    #[test]
    fn test_frequency_response_matches_rc() {
        // Small-signal sweep against the pickup network: a one-pole high-pass at PICKUP_FC,
        // with the law's slope at rest as the gain. The 0.6 dB tolerance covers the bilinear
        // warping at 44.1 kHz (~0.35 dB at 10 kHz) and fails on any real corner drift.
        let law = pickup_law_for(60);
        let e = 1e-4;
        let slope = (law.eval(-PICKUP_REST_OFFSET_MM + e) - law.eval(-PICKUP_REST_OFFSET_MM - e))
            / (2.0 * e);
        let amplitude = 0.01; // 4.5 um at the pickup: linear regime
        for &freq in &[100.0, 500.0, 880.0, 2000.0, 5000.0, 10000.0] {
            let buf = render(60, amplitude, freq, 0.1);
            let measured = buf[buf.len() / 2..]
                .iter()
                .fold(0.0f64, |a, x| a.max(x.abs()));
            let hpf_gain = freq / (freq * freq + PICKUP_FC * PICKUP_FC).sqrt();
            let expected = amplitude * A_C4 * slope * PICKUP_SENSITIVITY * hpf_gain;
            let ratio_db = 20.0 * (measured / expected).log10();
            assert!(
                ratio_db.abs() < 0.6,
                "at {freq} Hz: measured={measured:.6}, expected={expected:.6}, error={ratio_db:.2} dB"
            );
        }
    }

    #[test]
    fn test_corner_tracks_c_total_and_fork() {
        // The corner must follow C_TOTAL (an LCR measurement is expected to
        // replace it) and the C-2 return fork (drawing defect, may be settled
        // by hardware). Both are one-constant changes; this pins the law.
        assert!(
            (PICKUP_FC - 880.5).abs() < 1.0,
            "default (C_TOTAL 240 pF, C-2 to ground) must give ~880 Hz, got {PICKUP_FC:.1}"
        );
        // Scaling law: f = f_ref * (C_ref + C2) / (C_total + C2).
        // Cross-checked against the full network: 150 pF -> 1064 Hz,
        // 400 pF -> 670 Hz (fitted), law reproduces both within 4.3%.
        let law = |c_total: f64| FC_REF_C2_GROUND * (C_TOTAL_REF + C2_BASE) / (c_total + C2_BASE);
        assert!((law(150.0e-12) - 1064.0).abs() / 1064.0 < 0.05);
        assert!((law(400.0e-12) - 670.0).abs() / 670.0 < 0.05);
        // The emitter-return fork sits ~1.6x higher — C-2 is bootstrapped away.
        let (ground_fc, emitter_fc) = (FC_REF_C2_GROUND, FC_REF_C2_EMITTER);
        assert!(
            emitter_fc > ground_fc * 1.5,
            "emitter-return fork should sit well above the ground fork: \
             {emitter_fc} vs {ground_fc}"
        );
    }

    #[test]
    fn test_hpf_attenuates_bass() {
        // At ff drive, 100 Hz must stay clearly below the passband: the network is a high-pass
        // whatever the law.
        let peak = |f: f64| {
            let b = render(60, 1.0, f, 0.1);
            b[b.len() / 2..].iter().fold(0.0f64, |a, x| a.max(x.abs()))
        };
        let (bass, treble) = (peak(100.0), peak(10000.0));
        assert!(treble > 0.0, "no output at 10 kHz");
        assert!(
            bass < treble * 0.75,
            "100 Hz ({bass:.4}) vs passband ({treble:.4})"
        );
    }

    #[test]
    fn test_nonlinearity_produces_h2() {
        // Below full overlap (C4 at 0.3 of its ff swing, ~0.36 mm) the reed only enters the
        // finger band on one half of its swing: an edge rectifier, so a second harmonic well
        // above the linear floor, with H3 below it.
        let sr = 44100.0;
        for freq in [880.0f64, 2000.0] {
            let buf = render(60, 0.3, freq, 0.2);
            let signal = &buf[buf.len() * 3 / 4..];
            let h1 = dft_magnitude(signal, freq, sr);
            let h2 = dft_magnitude(signal, 2.0 * freq, sr);
            let h3 = dft_magnitude(signal, 3.0 * freq, sr);
            assert!(h2 > h3, "H2 ({h2:.2e}) should dominate H3 ({h3:.2e})");
            assert!(
                h2 / h1 > 0.05,
                "H2/H1 too low ({:.4}) at {freq} Hz",
                h2 / h1
            );
        }
    }

    #[test]
    fn test_full_overlap_brings_odd_harmonics() {
        // At its ff swing (1.19 mm) a middle reed passes full overlap with the 0.6 mm fingers:
        // the law bends at the plateau and far face, which is what produces odd harmonics. An
        // edge rectifier alone (the law below full overlap) has almost none.
        let sr = 44100.0;
        let freq = 880.0;
        let ratio = |amp: f64| {
            let buf = render(60, amp, freq, 0.2);
            let signal = &buf[buf.len() * 3 / 4..];
            dft_magnitude(signal, 3.0 * freq, sr) / dft_magnitude(signal, freq, sr)
        };
        let (edge, full) = (ratio(0.3), ratio(1.0));
        assert!(full > 0.1, "H3/H1 at full swing only {full:.4}");
        assert!(full > 10.0 * edge, "H3/H1 full {full:.4} vs edge {edge:.4}");
    }

    #[test]
    fn test_into_band_excursion_is_negative_going() {
        // The half-cycle that carries the reed INTO the finger band raises its capacitance, and
        // at ~constant charge that pulls the plate DOWN — so the larger excursion is negative,
        // in both registers (bass enters on the upswing, treble on the downswing).
        for midi in [40u8, 60, 84] {
            let buf = render(midi, 1.0, 2000.0, 0.2);
            let tail = &buf[buf.len() / 2..];
            let pos = tail.iter().cloned().fold(0.0f64, f64::max);
            let neg = tail.iter().cloned().fold(0.0f64, f64::min).abs();
            assert!(
                neg > pos * 1.05,
                "MIDI {midi}: into-band excursion must be the larger, negative one: pos={pos:.5} neg={neg:.5}"
            );
        }
    }

    #[test]
    fn test_first_swing_polarity_by_register() {
        // The hammer strikes from below, so the first swing is upward: INTO the band for a bass
        // reed (pickup above) — output negative — and OUT of it for a treble reed (pickup
        // below) — output positive. Checked at the first swing's peak, 2 kHz (above the corner,
        // so the network's phase lead is small).
        let sr: f64 = 44100.0;
        let quarter = (sr / 2000.0 / 4.0).round() as usize;
        assert!(
            render(40, 1.0, 2000.0, 0.01)[quarter] < 0.0,
            "bass first swing must be negative"
        );
        assert!(
            render(84, 1.0, 2000.0, 0.01)[quarter] > 0.0,
            "treble first swing must be positive"
        );
    }

    #[test]
    fn test_nonlinearity_is_frequency_independent() {
        // The nonlinearity is in the SOURCE, so H2/H1 at a given swing must be the same below
        // and far above the corner once the high-pass's own tilt (H2 an octave further up its
        // slope than H1) is removed.
        let sr = 44100.0;
        let h2_h1_at = |freq: f64| -> f64 {
            let buf = render(60, 1.0, freq, 0.5);
            let n = buf.len();
            let steady = &buf[n / 2..];
            let mag = |k: f64| -> f64 {
                let (mut re, mut im) = (0.0f64, 0.0f64);
                for (i, &v) in steady.iter().enumerate() {
                    let p = 2.0 * PI * k * freq * (i + n / 2) as f64 / sr;
                    re += v * p.cos();
                    im -= v * p.sin();
                }
                (re * re + im * im).sqrt()
            };
            mag(2.0) / mag(1.0)
        };
        let hpf = |f: f64| f / (f * f + PICKUP_FC * PICKUP_FC).sqrt();
        let predicted_db = |f: f64| 20.0 * (hpf(2.0 * f) / hpf(f)).log10();
        let (f_lo, f_hi) = (65.41, 4000.0);
        let measured_db = 20.0 * (h2_h1_at(f_lo) / h2_h1_at(f_hi)).log10();
        let residual = measured_db - (predicted_db(f_lo) - predicted_db(f_hi));
        assert!(
            residual.abs() < 1.0,
            "H2/H1 spread {measured_db:.2} dB vs the high-pass's own tilt — residual {residual:.2} dB \
             means the nonlinearity has drifted into the network"
        );
    }

    // ── Overlap-law tables (field-solved) ───────────────────────────────────

    use crate::pickup_tables::{R01_14, R15_20, R21_42, R43_64};

    fn sections() -> [(&'static str, &'static PickupLawTable); 4] {
        [
            ("reeds 1-14", &R01_14),
            ("reeds 15-20", &R15_20),
            ("reeds 21-42", &R21_42),
            ("reeds 43-64", &R43_64),
        ]
    }

    /// Harmonic magnitudes 1..=k of the static law driven by u = rest + A sin(theta).
    fn static_harmonics(law: &PickupLawTable, rest: f64, amp: f64, k: usize) -> Vec<f64> {
        let n = 1024;
        let c: Vec<f64> = (0..n)
            .map(|i| law.eval(rest + amp * (2.0 * PI * i as f64 / n as f64).sin()))
            .collect();
        (1..=k)
            .map(|h| {
                let (mut re, mut im) = (0.0, 0.0);
                for (i, v) in c.iter().enumerate() {
                    let p = 2.0 * PI * (h * i) as f64 / n as f64;
                    re += v * p.cos();
                    im -= v * p.sin();
                }
                (re * re + im * im).sqrt()
            })
            .collect()
    }

    #[test]
    fn test_law_spline_matches_generator() {
        // The generator writes off-knot values from its own (scipy) natural spline; the Rust
        // evaluation must reproduce them, so the table means what the field solve says.
        use crate::pickup_tables::{R01_14_CHECK, R15_20_CHECK, R21_42_CHECK, R43_64_CHECK};
        for (name, law, check) in [
            ("reeds 1-14", &R01_14, &R01_14_CHECK[..]),
            ("reeds 15-20", &R15_20, &R15_20_CHECK[..]),
            ("reeds 21-42", &R21_42, &R21_42_CHECK[..]),
            ("reeds 43-64", &R43_64, &R43_64_CHECK[..]),
        ] {
            for &(u, v) in check {
                let got = law.eval(u);
                assert!(
                    (got - v).abs() < 1e-10,
                    "{name}: eval({u}) = {got}, generator {v}"
                );
            }
            for (i, &y) in law.y.iter().enumerate() {
                let u = law.u0 + law.du * i as f64;
                assert!(
                    (law.eval(u) - y).abs() < 1e-12,
                    "{name}: knot {i} not interpolated"
                );
            }
        }
    }

    #[test]
    fn test_law_second_derivative_continuous() {
        // Harmonics come from the law's curvature; a kink in C'' at a knot would be a spurious
        // harmonic source. Check C'' from both sides of every interior knot.
        for (name, law) in sections() {
            let e = 1e-4 * law.du;
            let d2 = |u: f64| (law.eval(u + e) - 2.0 * law.eval(u) + law.eval(u - e)) / (e * e);
            let scale = law.m.iter().fold(0.0f64, |a, &b| a.max(b.abs()));
            for i in 2..law.y.len() - 2 {
                let u = law.u0 + law.du * i as f64;
                let (left, right) = (d2(u - 3.0 * e), d2(u + 3.0 * e));
                assert!(
                    (left - right).abs() < 1e-3 * scale,
                    "{name}: C'' jumps at knot {i}: {left} vs {right}"
                );
            }
        }
    }

    #[test]
    fn test_law_shape() {
        // Outside the band the capacitance is a weak fringe floor; it rises as the reed enters
        // the finger band (0 < u < t_r) and falls again once the reed passes the far face.
        for (name, law) in sections() {
            let (t_r, t_p) = (law.t_r, law.t_p);
            let mut prev = law.eval(-2.0);
            let mut u = -2.0;
            while u < 0.9 * t_r.min(t_p) {
                u += 0.01;
                let c = law.eval(u);
                assert!(c > prev, "{name}: law not rising at u = {u}");
                prev = c;
            }
            let peak = law.eval(0.5 * (t_r + t_p));
            assert!(peak > 3.0 * law.eval(-1.0), "{name}: in-band peak too weak");
            assert!(
                law.eval(t_r + t_p + 0.5) < 0.6 * peak,
                "{name}: law must fall past the far face"
            );
        }
    }

    #[test]
    fn test_section_boundaries_and_band_side() {
        // Reed 20 = MIDI 52 (E3) is the last bass reed; the solder flip and the pickup moving
        // from above to below the reed both happen at 20|21. Thin fingers end at reed 42 (D5).
        // The measured side-clearance rows also change at 14|15.
        for (midi, law, sign) in [
            (33u8, &R01_14, 1.0),
            (46, &R01_14, 1.0),
            (47, &R15_20, 1.0),
            (52, &R15_20, 1.0),
            (53, &R21_42, -1.0),
            (74, &R21_42, -1.0),
            (75, &R43_64, -1.0),
            (96, &R43_64, -1.0),
        ] {
            assert!(
                std::ptr::eq(pickup_law_for(midi), law),
                "section of MIDI {midi}"
            );
            assert_eq!(pickup_band_sign(midi), sign, "band side of MIDI {midi}");
        }
    }

    #[test]
    fn test_law_edge_regime_fundamental_dominates() {
        // At the factory setting (rest just outside the band) the pickup passes the fundamental
        // with a modest second harmonic and little third — the manual's "correct" voicing.
        // No source gives an absolute H2 level: the edge's softness scales with the side gap
        // (0.19–0.267 mm here gives −21.6..−26.0 dB at 0.2 mm), so the rectifier is checked
        // structurally instead — H2/H1 must grow with swing.
        for (name, law) in sections() {
            let ratio_db = |a: f64| {
                let h = static_harmonics(law, -PICKUP_REST_OFFSET_MM, a, 3);
                (20.0 * (h[1] / h[0]).log10(), 20.0 * (h[2] / h[0]).log10())
            };
            let (h2, h3) = ratio_db(0.2);
            assert!(h2 < -6.0, "{name}: H2/H1 {h2:.1} dB");
            assert!(
                h3 < h2 - 10.0,
                "{name}: H3 {h3:.1} dB should sit well below H2"
            );
            let (h2_small, _) = ratio_db(0.1);
            let (h2_large, _) = ratio_db(0.3);
            assert!(
                h2_small + 3.0 < h2 && h2 + 1.0 < h2_large,
                "{name}: H2/H1 must grow with swing ({h2_small:.1}, {h2:.1}, {h2_large:.1} dB)"
            );
        }
    }

    #[test]
    fn test_law_centred_rest_is_the_fault_state() {
        // Service manual §18: "When a reed at rest is positioned inside the pickup, a very strong
        // undesirable harmonic will be generated." With the reed centred in the band the law is
        // even about rest, so the fundamental vanishes and the doubled frequency takes over.
        for (name, law) in sections() {
            let centre = 0.5 * (law.t_r + law.t_p);
            let h = static_harmonics(law, centre, 0.4 * law.t_r.min(law.t_p), 2);
            assert!(
                h[1] > 10.0 * h[0],
                "{name}: centred rest must give 2f, got H1 {} H2 {}",
                h[0],
                h[1]
            );
        }
    }

    fn dft_magnitude(signal: &[f64], freq: f64, sr: f64) -> f64 {
        let n = signal.len() as f64;
        let mut re = 0.0;
        let mut im = 0.0;
        for (i, &s) in signal.iter().enumerate() {
            let phase = 2.0 * PI * freq * i as f64 / sr;
            re += s * phase.cos();
            im -= s * phase.sin();
        }
        2.0 * ((re / n).powi(2) + (im / n).powi(2)).sqrt()
    }
}
