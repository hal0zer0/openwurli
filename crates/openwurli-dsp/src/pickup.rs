//! Electrostatic pickup model — time-varying RC circuit.
//!
//! The Wurlitzer 200A pickup is a capacitive sensor: reed vibration modulates
//! the capacitance between the reed and a charged metal plate (+147V DC).
//!
//! The reed-plate capacitance varies nonlinearly with displacement:
//!   C(y) = C_0 / (1 - y)
//! where y = x/d_0 is the normalized displacement (fraction of rest gap,
//! positive toward the plate).
//!
//! # Where the nonlinearity lives (2026-09-14 restructure)
//!
//! **The nonlinearity is in the SOURCE, not in the network.** This is the
//! correction that this module's previous two revisions both got wrong, in
//! opposite directions.
//!
//! Only ONE reed moves. Its capacitance to the plate is `C_reed(t) =
//! C_REED0/(1-y)` — a deep modulation, because the reed really does travel a
//! large fraction of its rest gap. But that reed is only ~3 pF of the ~240 pF
//! sitting on the plate node; the other 63 reeds, the stray and the cable do not
//! move. So the node equation is
//!
//! ```text
//!   C_node(t) * dv/dt + v/R_net = -V_pol * dC_reed/dt
//!   C_node(t) = C_TOTAL + C_REED0 * y/(1-y)
//! ```
//!
//! Two consequences, and they are the whole design:
//!
//! 1. **The strong `1/(1-y)` nonlinearity is the SOURCE term** — the charge the
//!    moving reed injects. It is frequency-independent: it is there at 5 kHz
//!    exactly as much as at 50 Hz.
//! 2. **The network is very nearly LTI.** `C_node` swings only by
//!    `C_REED0/C_TOTAL = 1/80` of the source depth — 240 pF to 274 pF even at
//!    y = 0.92, a 14% corner shift at the extreme rather than the 12.5x the
//!    previous model applied.
//!
//! ## What the previous model did instead
//!
//! It modulated the ENTIRE node capacitance as `C_TOTAL/(1-y)` — i.e. it
//! implicitly assumed the moving reed **was** the whole node, an 80x
//! over-modulation of the relaxation time constant. A term-knockout measurement
//! showed the consequence: >99% of its harmonic generation came from that
//! spurious time-constant swing, and almost none from the capacitance ratio
//! (disabling the `c_n` output term cost only 0.5 dB of H2; disabling the alpha
//! modulation cost 47 dB).
//!
//! Generating harmonics from a *time constant* makes them inherit the filter's
//! own frequency dependence, and that is a measurable defect: the old model's
//! high-frequency asymptote was `SENSITIVITY * y` — **linear**, no harmonics at
//! all above the corner — where the physical asymptote is
//! `SENSITIVITY * y/(1-y)`. Harmonic energy therefore fell off per harmonic
//! ORDER (measured: -1.5 / -5.9 / -10.8 / -14.5 dB at H4 / H5 / H6 / H7 for a
//! ff C4 when the corner moved 2312 -> 880 Hz), and the 2.5-6 kHz presence band
//! went with it. That is a model artifact, not the instrument.
//!
//! `DISPLACEMENT_SCALE` was silently absorbing the 80x; it now means what its
//! name says — the fraction of the rest gap the reed actually travels.
//!
//! ## Structure
//!
//! ```text
//!   y = soft_saturate(displacement * DISPLACEMENT_SCALE)   // gap fraction
//!   s = y / (1 - y)                                        // reed charge, THE nonlinearity
//!   m = 1 + C_REED_RATIO * s                               // node cap, ~LTI
//!   w = (w*(m_avg - g0) - (s - s_prev)) / (m_avg + g0)     // network
//!   out = w * PICKUP_SENSITIVITY
//! ```
//!
//! The `w` recursion is the exact trapezoidal discretization of the node
//! equation above, so it is a differentiator inside a one-pole lag — i.e. the
//! `jw`-driven high-pass the circuit reference describes, not a lowpass. Its
//! small-signal response reproduces a one-pole high-pass at `PICKUP_FC` to
//! **0.011 dB** over 100 Hz - 10 kHz, so the linear behaviour verified by the
//! drawn-topology revision is preserved exactly; only the harmonic structure
//! changes.
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
//! - Harmonic generation from the reed's own charge (H2 from the capacitance
//!   ratio), **independent of frequency** — see the restructure section above.
//!   The line that stood here previously ("stronger near/below RC corner") was
//!   the 80x artifact, not the instrument.
//! - Correct asymmetry: reed toward the plate gives the larger excursion, and it
//!   is negative-going (more capacitance at ~constant charge = lower voltage).

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
/// Corner sensitivity (fitted one-pole, from the cross-checked network):
/// 150 pF -> 1064 Hz, 240 pF -> 880 Hz, 400 pF -> 670 Hz. Changing this
/// constant moves `PICKUP_FC` automatically via the scaling law below.
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
/// intended physics change, not a side effect. The 1/(1-y) charge-dynamics
/// mechanism (the bark source, >98% of H2 at normal dynamics) is untouched;
/// it now relaxes on the corrected network's dominant pole instead of the
/// superseded 2312 Hz one.
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
/// Physically this is the gain on `y/(1-y)` well above the corner: a reed that
/// closes a fraction y of its gap swings the plate node by
/// `V_pol * C_REED0/C_TOTAL * y/(1-y)`. Note the `1/(1-y)` — the PREVIOUS model's
/// HF asymptote was `SENSITIVITY * y`, linear, which is why its harmonics
/// vanished above the corner.
pub const PICKUP_SENSITIVITY: f64 = V_POLARIZING * C_REED0 / C_TOTAL;

/// Asymptotic displacement-fraction limit. The reed physically cannot touch
/// the plate (y=1.0 is a singularity in c_n=1/(1-y)). With the smooth-saturation
/// curve below, |y_out| approaches but never reaches this value.
///
/// The old static model needed a tight clamp (0.90) because y/(1-y) at 0.90 = 9.0
/// produced huge intermediate signals. The time-varying RC model self-limits via
/// charge dynamics — output is bounded at ~±SENSITIVITY regardless of y, so we
/// can safely allow y close to 1.0. At y=0.98, c_n=50, alpha=0.008 — numerically
/// well-behaved.
pub const PICKUP_MAX_Y: f64 = 0.98;

/// Knee where the smooth saturation begins. Below this, `pickup_soft_saturate`
/// is the identity function (no compression, no distortion). Above this, it
/// smoothly bends toward `PICKUP_MAX_Y` so the upper-envelope tip never crosses
/// a hard corner. Picked to leave the entire normal operating range untouched
/// (typical y_peak at v=127 is ~0.85 with DS at NEW values 0.85/0.88).
pub const PICKUP_KNEE_Y: f64 = 0.94;

/// Smooth saturation on the reed-displacement fraction `y = x / d_0`.
///
/// Below `±PICKUP_KNEE_Y` the function is the identity — no flavour change vs.
/// the old hard clamp on quiet-to-medium content. Above the knee it follows
/// `knee + (limit-knee) * tanh((|y|-knee)/(limit-knee))`, which is C¹-continuous
/// at the knee (slope = 1) and asymptotes to `±PICKUP_MAX_Y` from below. This
/// removes the derivative discontinuity at the old `clamp(±0.98)` corner that
/// was producing broadband HF "tear" hash whenever bass-heavy / chord-ff
/// content grazed the limit (~6–7× more click-band energy at NEW DS values).
///
/// Bark character is preserved: the upper-velocity range still spends most of
/// its time in the steep `1/(1-y)` zone (y in [0.85, 0.95]), where the
/// saturation barely deviates from identity. The change is concentrated at the
/// very top of the velocity range where the corner used to live.
#[inline]
fn pickup_soft_saturate(y: f64) -> f64 {
    let abs_y = y.abs();
    if abs_y < PICKUP_KNEE_Y {
        return y;
    }
    let range = PICKUP_MAX_Y - PICKUP_KNEE_Y;
    let saturated = PICKUP_KNEE_Y + range * ((abs_y - PICKUP_KNEE_Y) / range).tanh();
    saturated.copysign(y)
}

/// Convert reed model displacement units to physical y = x/d_0.
///
/// NOTE: This default is overridden per-note by tables::pickup_displacement_scale()
/// in voice.rs. Only used if set_displacement_scale() is never called.
const DISPLACEMENT_SCALE: f64 = 0.85;

pub struct Pickup {
    /// Network state: normalized plate-node voltage perturbation `w = v / SENSITIVITY`.
    w: f64,
    /// Previous source sample `s = y/(1-y)` (the recursion needs `ds/dt`).
    s_prev: f64,
    /// Previous normalized node capacitance `m = 1 + C_REED_RATIO * s`.
    m_prev: f64,
    /// Precomputed: `dt / (2 * TAU)` = `pi * PICKUP_FC / sample_rate`.
    beta: f64,
    displacement_scale: f64,
}

impl Pickup {
    pub fn new(sample_rate: f64) -> Self {
        Self::new_with_scale(sample_rate, DISPLACEMENT_SCALE)
    }

    /// Construct with explicit displacement scale (for bark-audit/calibrate tools).
    pub fn new_with_scale(sample_rate: f64, displacement_scale: f64) -> Self {
        let dt = 1.0 / sample_rate;
        let beta = dt / (2.0 * TAU);
        Self {
            w: 0.0,
            s_prev: 0.0,
            m_prev: 1.0,
            beta,
            displacement_scale,
        }
    }

    /// Override the displacement scale (default: 0.85).
    /// Higher = tighter gap = more nonlinearity = more bark.
    pub fn set_displacement_scale(&mut self, scale: f64) {
        self.displacement_scale = scale;
    }

    /// Process a buffer of reed displacement samples in-place.
    ///
    /// Input: reed displacement in normalized model units.
    /// Output: pickup voltage in volts (millivolt-scale signals).
    ///
    /// The time-varying RC circuit couples the 1/(1-y) capacitance nonlinearity
    /// with the charge dynamics, producing frequency-dependent harmonic generation.
    /// At frequencies well below the RC corner (PICKUP_FC, 880 Hz), the circuit generates
    /// H2 proportional to displacement² (same as the static y/(1-y) model).
    /// At frequencies near/above the corner, the charge can't follow the fast
    /// capacitance changes, reducing the nonlinear contribution — physically
    /// correct behavior that the old separated model couldn't capture.
    pub fn process(&mut self, buffer: &mut [f64]) {
        let scale = self.displacement_scale;
        let beta = self.beta;
        for sample in buffer.iter_mut() {
            // Smooth saturation: identity below ±PICKUP_KNEE_Y, asymptotic to
            // ±PICKUP_MAX_Y above. Replaces the old hard clamp whose derivative
            // discontinuity at the limit was producing audible HF distortion.
            let y = pickup_soft_saturate(*sample * scale);

            // ── Source: the moving reed's charge. THE nonlinearity. ──
            // C_reed(t) = C_REED0/(1-y); the AC part it injects is proportional
            // to s = 1/(1-y) - 1 = y/(1-y). Frequency-independent by
            // construction — this is what the previous model lost above the
            // corner.
            let s = y / (1.0 - y);

            // ── Network: one moving reed against a fixed node. ~LTI. ──
            // C_node(t)/C_TOTAL = 1 + C_REED_RATIO * s. The previous model used
            // 1/(1-y) here, i.e. a ratio of 1.0 instead of 1/80.
            let m = 1.0 + C_REED_RATIO * s;
            let m_avg = 0.5 * (m + self.m_prev);

            // Exact trapezoidal discretization of
            //   C_node(t)*dv/dt + v/R_net = -V_pol * dC_reed/dt
            // normalized by C_TOTAL and PICKUP_SENSITIVITY. A differentiator
            // inside a one-pole lag = the jw-driven high-pass the circuit
            // reference specifies.
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

    // ── pickup_soft_saturate unit tests ─────────────────────────────────────

    #[test]
    fn test_soft_saturate_identity_below_knee() {
        // Anything strictly inside ±PICKUP_KNEE_Y must pass through bit-exact.
        // Guards against any future rewrite that introduces a corner inside
        // the linear range. Bracket scaled by knee so the test follows
        // PICKUP_KNEE_Y if it changes.
        let edge = PICKUP_KNEE_Y - 0.001;
        for y in [-edge, -edge * 0.5, -0.1, 0.0, 0.1, edge * 0.5, edge] {
            let out = pickup_soft_saturate(y);
            assert!(
                (out - y).abs() < 1e-15,
                "below knee should be identity: {y} → {out}"
            );
        }
    }

    #[test]
    fn test_soft_saturate_continuous_at_knee() {
        // Just below = identity; just above = soft saturation. The tanh slope
        // at zero is 1, matching identity's slope, so f is C¹ at the knee.
        let just_below = pickup_soft_saturate(PICKUP_KNEE_Y - 1e-9);
        let just_above = pickup_soft_saturate(PICKUP_KNEE_Y + 1e-9);
        assert!(
            (just_below - just_above).abs() < 1e-7,
            "knee discontinuity: {just_below} vs {just_above}"
        );
    }

    #[test]
    fn test_soft_saturate_bounded_at_max() {
        // Output must never exceed PICKUP_MAX_Y. The asymptote is exactly
        // PICKUP_MAX_Y; in f64, very-large inputs land at tanh ≈ 1.0 and the
        // output equals the limit (matches the old hard-clamp magnitude).
        // What we forbid is *exceeding* the limit and re-entering the
        // 1/(1−y) singularity zone.
        for y in [0.95, 0.96, 0.98, 1.0, 2.0, 100.0, -100.0] {
            let out = pickup_soft_saturate(y);
            assert!(
                out.abs() <= PICKUP_MAX_Y + 1e-15,
                "input {y} → {out} exceeds ±{PICKUP_MAX_Y}"
            );
            assert!(
                out.abs() >= PICKUP_KNEE_Y,
                "input {y} → {out} above the knee should not undershoot it"
            );
        }
    }

    #[test]
    fn test_soft_saturate_smooth_above_knee() {
        // Inputs in the middle of the saturation band (not at tanh's f64
        // saturation tail) must land strictly between knee and limit.
        for y in [0.95, 0.96, 0.97, 0.98, 1.0, 1.5] {
            let out = pickup_soft_saturate(y);
            assert!(
                out > PICKUP_KNEE_Y && out < PICKUP_MAX_Y,
                "input {y} → {out} should land in (knee, limit)"
            );
        }
    }

    #[test]
    fn test_soft_saturate_monotonic() {
        // Across the full range the output must be monotonically non-decreasing.
        // Catches any sign mistake in the asymmetric formulation.
        let mut prev = pickup_soft_saturate(-1.5);
        for i in 1..=600 {
            let y = -1.5 + (i as f64) * 0.005;
            let out = pickup_soft_saturate(y);
            assert!(
                out >= prev - 1e-12,
                "non-monotonic at y={y}: prev={prev}, cur={out}"
            );
            prev = out;
        }
    }

    #[test]
    fn test_soft_saturate_odd_symmetric() {
        // The bend on the negative side mirrors the positive side.
        for y in [0.86, 0.9, 0.95, 0.98, 1.5, 5.0] {
            let pos = pickup_soft_saturate(y);
            let neg = pickup_soft_saturate(-y);
            assert!(
                (pos + neg).abs() < 1e-12,
                "asymmetric saturation at ±{y}: +{pos}, -{} (sum {})",
                neg.abs(),
                pos + neg
            );
        }
    }

    #[test]
    fn test_dc_equilibrium() {
        // Zero displacement should produce zero output (DC blocked by RC).
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let n = (sr * 0.05) as usize;
        let mut buf = vec![0.0f64; n];
        pickup.process(&mut buf);

        let peak = buf.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(
            peak < 1e-10,
            "zero displacement should produce zero output, got peak={peak:.2e}"
        );
    }

    #[test]
    fn test_frequency_response_matches_rc() {
        // Small-signal sweep against the CORRECTED pickup network.
        //
        // PRE-REVISION EXPECTATION REPLACED: this test swept against a 1-pole
        // HPF at 2312 Hz (TAU = 287k x 240pF) with a 2 dB tolerance. Both the
        // corner and the network behind it are void — the 287k came from
        // reading R-2 as a 2 MEG base-bias resistor, but R-2 is a 1 MEG
        // polarizing feed on the pickup side of C-1, and C-2 (220 pF) is
        // bridged onto the plate capacitance by C-1. The corner is now
        // PICKUP_FC (880 Hz at the nominal C_TOTAL).
        //
        // Tolerance tightened 2.0 dB -> 0.6 dB at the same time. The old
        // 2 dB was loose enough to hide a corner error of nearly an octave;
        // 0.6 dB covers the bilinear warping at 44.1 kHz (worst case ~0.35 dB
        // at 10 kHz) with margin but would fail on any real corner drift.
        let sr = 44100.0;
        let fc = PICKUP_FC;
        let amplitude = 0.01; // Very small — linear regime (y_peak = 0.0085)

        for &freq in &[100.0, 500.0, 880.0, 2000.0, 5000.0, 10000.0] {
            let mut pickup = Pickup::new(sr);
            let n = (sr * 0.1) as usize;
            let mut buf: Vec<f64> = (0..n)
                .map(|i| amplitude * (2.0 * PI * freq * i as f64 / sr).sin())
                .collect();
            pickup.process(&mut buf);

            let steady = &buf[n / 2..];
            let measured = steady.iter().map(|x| x.abs()).fold(0.0f64, f64::max);

            // Expected: amplitude * DS * SENSITIVITY * HPF_gain
            let y_amp = amplitude * DISPLACEMENT_SCALE;
            let hpf_gain = freq / (freq * freq + fc * fc).sqrt();
            let expected = y_amp * PICKUP_SENSITIVITY * hpf_gain;

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
    fn test_hpf_passes_high_freq() {
        // At 10 kHz, the time-varying RC passes high-freq signals.
        // For the RC model, at very high frequencies q can't follow c_n,
        // so output ≈ y * SENSITIVITY (reduced from old y/(1-y) * S).
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let freq = 10000.0;

        let n = (sr * 0.05) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        let peak = buf[n / 2..].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        // At 10 kHz with DS=0.85: y approaches MAX_Y.
        // The RC model at high freq gives output ~ y * SENSITIVITY for small y,
        // but for large y the nonlinear charge dynamics still produce amplification.
        assert!(peak > 0.5, "pickup output too low at 10kHz: {peak}");
        assert!(peak < 12.0, "pickup output too high at 10kHz: {peak}");
    }

    #[test]
    fn test_hpf_attenuates_bass() {
        // At 100 Hz the RC charge tracks the capacitance changes, attenuating
        // the output relative to the passband.
        //
        // PRE-REVISION EXPECTATION REPLACED: `peak < 0.65`, which encoded the
        // 2312 Hz corner. The corrected network's corner is PICKUP_FC (880 Hz),
        // so 100 Hz now sits far less deep into the rolloff and this full-scale
        // drive measures 0.967 (was under 0.65). The bound is re-derived, not
        // just loosened: 1.10 keeps ~13% headroom over the measured value while
        // still failing outright if the corner regressed toward the passband.
        //
        // The second assertion is the one that carries the physics and is
        // corner-independent: 100 Hz must remain clearly below the passband.
        // At full-scale drive the 1/(1-y) compression flattens the curve
        // (measured 0.967 at 100 Hz vs 1.546 at 10 kHz, a ratio of 0.63), so
        // this is a weaker statement than the small-signal sweep — that is what
        // `test_frequency_response_matches_rc` is for.
        let sr = 44100.0;
        let n = (sr * 0.1) as usize;
        let measure = |freq: f64| {
            let mut pickup = Pickup::new(sr);
            let mut buf: Vec<f64> = (0..n)
                .map(|i| (2.0 * PI * freq * i as f64 / sr).sin())
                .collect();
            pickup.process(&mut buf);
            buf[n / 2..].iter().map(|x| x.abs()).fold(0.0f64, f64::max)
        };
        let bass = measure(100.0);
        let treble = measure(10000.0);
        assert!(bass < 1.60, "pickup should attenuate 100 Hz: {bass}");
        assert!(
            bass < treble * 0.75,
            "100 Hz ({bass:.3}) must stay clearly below the passband ({treble:.3})"
        );
    }

    #[test]
    fn test_nonlinearity_produces_h2() {
        // Drive the pickup with a large-amplitude sine and verify H2 > H3.
        // The time-varying capacitance generates even harmonics.
        let sr = 44100.0;
        // Swept at two points rather than one: 880 Hz is the CORRECTED corner
        // (where charge/capacitance coupling is strongest) and 2000 Hz is the
        // legacy probe point, kept so no coverage is lost by the corner move.
        // The old comment called 2000 Hz "near the corner" — that was true of
        // the superseded 2312 Hz reading, not of this network.
        for freq in [880.0f64, 2000.0] {
            let mut pickup = Pickup::new(sr);

            let amplitude = 1.0;
            let n = (sr * 0.2) as usize;
            let mut buf: Vec<f64> = (0..n)
                .map(|i| amplitude * (2.0 * PI * freq * i as f64 / sr).sin())
                .collect();
            pickup.process(&mut buf);

            let start = n * 3 / 4;
            let signal = &buf[start..];
            let h1 = dft_magnitude(signal, freq, sr);
            let h2 = dft_magnitude(signal, 2.0 * freq, sr);
            let h3 = dft_magnitude(signal, 3.0 * freq, sr);

            assert!(
                h2 > h3,
                "H2 ({h2:.2e}) should dominate H3 ({h3:.2e}) from capacitance modulation"
            );
            let h2_ratio = h2 / h1;
            assert!(
                h2_ratio > 0.05,
                "H2/H1 too low ({h2_ratio:.4}) at {freq} Hz, expected >5% from nonlinearity"
            );
        }
    }

    #[test]
    fn test_asymmetry() {
        // The pickup's even-order asymmetry, with its DIRECTION derived rather
        // than observed.
        //
        // At high frequency the plate charge is frozen and the algebra is
        // unambiguous: out -> -SENSITIVITY * y/(1-y). Reed TOWARD the plate
        // (y > 0) raises C_reed, which at ~constant charge pulls the plate
        // voltage DOWN. So the large excursion is NEGATIVE-going, and it is
        // larger: |out| at y=+0.5 is 3x |out| at y=-0.5 (1.84 vs 0.61).
        //
        // ⚠ THE PREVIOUS MODEL HAD THIS BACKWARDS and this test encoded it.
        // It asserted pos_peak > neg_peak, i.e. the largest output during
        // NEGATIVE y (reed moving away) — measured: its max landed at y=-0.05
        // while its min landed at y=+0.37. That contradicted this module's own
        // docstring ("positive y amplified more than negative"). The old
        // asymmetry was an emergent artifact of modulating the relaxation time
        // constant; with the nonlinearity in the source where it belongs, the
        // direction follows from the capacitance and is no longer a matter of
        // observation.
        let sr = 44100.0;
        let freq = 500.0;
        let amplitude = 0.5; // y_peak = 0.5 * 0.85 = 0.425, below the knee
        let n = (sr * 0.2) as usize;
        let mut pickup = Pickup::new(sr);
        let mut buf: Vec<f64> = (0..n)
            .map(|i| amplitude * (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        let pos_peak = buf[n / 2..].iter().cloned().fold(0.0f64, f64::max);
        let neg_peak = buf[n / 2..].iter().cloned().fold(0.0f64, f64::min).abs();

        assert!(
            neg_peak > pos_peak * 1.05,
            "reed-toward-plate must give the larger (negative-going) excursion: \
             pos={pos_peak:.6} neg={neg_peak:.6}"
        );
    }

    #[test]
    fn test_nonlinearity_is_frequency_independent() {
        // The regression guard for the 2026-09-14 restructure.
        //
        // The nonlinearity is in the SOURCE, so H2/H1 at a given y must be
        // roughly the same whether the note sits below or far above the corner.
        // The previous model generated harmonics from the relaxation time
        // constant instead, so its H2/H1 collapsed with frequency — that is what
        // produced the ~3.4 dB-per-harmonic-ORDER shortfall and the 2.5-6 kHz
        // notch. Measured on the previous model, H2/H1 at y_peak 0.5 fell from
        // 53.8% at 65 Hz to 12.9% at 4 kHz; the source-driven model holds it.
        let sr = 44100.0;
        let y_peak = 0.5;
        let n = (sr * 0.5) as usize;

        let h2_h1_at = |freq: f64| -> f64 {
            let mut pickup = Pickup::new_with_scale(sr, 1.0);
            let mut buf: Vec<f64> = (0..n)
                .map(|i| y_peak * (2.0 * PI * freq * i as f64 / sr).sin())
                .collect();
            pickup.process(&mut buf);
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

        // H2/H1 is not expected to be flat: the high-pass itself sits H2 an
        // octave further up its own slope than H1, which genuinely lifts H2/H1
        // below the corner (this is the documented "pickup HPF boosts H2 ~1.9x").
        // So remove that predicted differential and assert the RESIDUAL — what
        // is left is the nonlinearity's own frequency dependence, which must be
        // ~zero because the source term has none.
        let hpf = |f: f64| f / (f * f + PICKUP_FC * PICKUP_FC).sqrt();
        let predicted_db = |f: f64| 20.0 * (hpf(2.0 * f) / hpf(f)).log10();

        let (f_lo, f_hi) = (65.41, 4000.0);
        let measured_db = 20.0 * (h2_h1_at(f_lo) / h2_h1_at(f_hi)).log10();
        let predicted_spread = predicted_db(f_lo) - predicted_db(f_hi);
        let residual = measured_db - predicted_spread;

        assert!(
            residual.abs() < 1.0,
            "H2/H1 spread {measured_db:.2} dB across {f_lo}->{f_hi} Hz vs {predicted_spread:.2} dB \
             predicted from the high-pass alone — residual {residual:.2} dB means the \
             nonlinearity has drifted back into the network. \
             (Previous model's residual on this measurement: 6.6 dB.)"
        );
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
