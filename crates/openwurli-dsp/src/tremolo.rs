//! Wurlitzer 200A tremolo — feature-toggled between circuit (default) and behavioral oscillator.
//!
//! Default: melange-generated Twin-T oscillator circuit (real waveform shape).
//! `--features legacy-tremolo`: behavioral sine LFO (for A/B testing).
//!
//! Both share the same CdS LDR model (LED drive → asymmetric envelope → power-law R).

#[cfg(feature = "legacy-tremolo")]
use std::f64::consts::PI;

#[cfg(not(feature = "legacy-tremolo"))]
use crate::gen_tremolo;

/// Empty `.inject` argument for the v0.1.7 generated oscillator.
///
/// API drift at the melange pin bump de9dc81 -> 7ecb36c (v0.1.7):
/// `process_sample` gained an `injections_inner` parameter and now returns
/// `(outputs, taps_inner)` instead of a bare output array. The tremolo deck
/// declares no `.inject` sources, so `NUM_INJECT == 0` and this argument is
/// always an empty array — zero runtime cost.
const NO_INJECT: [[f64; gen_tremolo::NUM_INJECT]; gen_tremolo::OVERSAMPLING_FACTOR] =
    [[0.0; gen_tremolo::NUM_INJECT]; gen_tremolo::OVERSAMPLING_FACTOR];

/// CdS vactrol dynamics — LG-1 (#142312, VTL5C-class LED/LDR opto).
/// Datasheet range: rise ~2.5 ms, fall ~35 ms; power-law exponent ~0.7–0.9.
const ATTACK_TAU: f64 = 0.0025;
const RELEASE_TAU: f64 = 0.035;
const GAMMA: f64 = 0.9;
/// CdS photoresistance range under the 200A's *actual* LED drive. The LED
/// runs at only ~0.84 mA (fixed, through R17 = 4.7 kΩ off the oscillator),
/// so the cell never leaves the kΩ regime: ~9 kΩ illuminated ↔ ~1 MΩ dark.
/// (An earlier model fudged the bright floor to 18,320 Ω to fake a 19 kΩ
/// shunt endpoint — that was really the 18 kΩ + R18 network folded into the
/// cell. The real cell is weakly driven and sits at ~9 kΩ bright; the
/// tremolo depth comes from the divider network below, not a hot LDR.)
const R_LDR_MIN: f64 = 9_000.0;
const R_LDR_MAX: f64 = 1_000_000.0;

/// 200A vibrato depth network, per schematic #203720-S-3. The 50 kΩ front-panel
/// VIBRATO pot is a 3-terminal divider in the fb_junction→LDR shunt leg:
/// top→fb_junction, bottom→ground, wiper→the LDR branch. An 18 kΩ resistor
/// bridges top→wiper, and the LDR sits DIRECTLY on the wiper branch.
///
/// **R-18 (680 Ω) is NOT here** — it is in the LED DRIVE path
/// (+14.5 V → R-18 → LED → R-17 → TR-3 collector). The 2026-09-13 instrumented
/// re-read showed cable pin 5 running straight to LG-1 pin 4 with two line HOPS
/// en route, not junctions. Its removal from this leg is the 2026-09-14
/// light-law re-fit; it was the last superseded-topology reading in shipping
/// code.
///
/// Shunt impedance seen by fb_junction:
///   Z = (R_upper ∥ 18 kΩ) + (R_lower ∥ R_ldr)
/// with R_upper = 50 kΩ·(1−depth), R_lower = 50 kΩ·depth. depth = 1.0 puts the
/// wiper at the fb end (max depth); depth = 0 grounds the LDR branch (vibrato
/// off, fb sees a fixed 50 kΩ ∥ 18 kΩ ≈ 13.2 kΩ). See `shunt_impedance`.
const R_VIB_BRIDGE: f64 = 18_000.0;
const R_VIB_POT: f64 = 50_000.0;

// ── LED drive path and the TIL209A light law (2026-09-14 re-fit) ──────────
//
// The drawn LED chain is  +14.5 V → R-18 (680 Ω) → LG-1 LED → R-17 (4.7 K
// trimmer) → TR-3 collector.  R-18 is HERE, not in the LDR leg.  The
// `wurli-tremolo` deck exposes the LED anode/cathode as raw inner-rate taps
// precisely so this module can consume real current instead of inferring
// brightness from the collector swing:
//
//     I_LED(t) = (V_RAIL − v(led_anode)) / R-18
//
// Measured over a settled cycle at the shipped R-17 = 4.7 K (the trimmer's FULL
// value = weakest LED drive, which is the position the deck models and the one
// the service manual calls the starting point): anode 12.950–14.252 V, diode
// drop 1.501–1.548 V, and therefore
//
//     I_LED = 0.365 … 2.279 mA
//
// The whole operating range sits in the sub-mA-to-few-mA regime.  That matters
// for the light law below: the 10–40 mA part of the TIL209A curve is never
// reached by this circuit.
const V_RAIL: f64 = 14.5;
const R18_LED_SERIES: f64 = 680.0;

/// Relative luminous intensity vs forward current, TI TIL209A bulletin DL-S
/// 12024 (June 1973) Fig. 4 — the only period sub-mA curve for this part.
///
/// The curve is not a single power law: its log-log slope runs ≈1.36 at
/// 0.5–1 mA and tapers to ≈0.88 at 10–40 mA.  Encoded here as a piecewise
/// power law, i.e. straight segments in log-log space, with the local exponent
/// as the segment slope.
///
/// ⚠ **Provenance, stated plainly.** The exponents are taken from the digitised
/// figure recorded in the project's coordination record and the deck header;
/// this module encodes them, it did not re-digitise the artwork. The TI figure
/// is DRAFTED artwork — use it as a LEVEL, not as device physics.
///
/// ⚠ **Below 0.35 mA this is EXTRAPOLATION.** The deck's I-V card is fitted over
/// 0.35–20 mA and no period data exists below 0.1 mA. The oscillator's minimum
/// (0.365 mA) sits just inside the fitted range, so the extrapolated region is
/// touched only transiently, but the bottom segment's slope is an assumption.
///
/// (mA, local log-log exponent applied from this current up to the next)
const LED_INTENSITY_LAW: [(f64, f64); 5] = [
    (0.10, 1.36), // extrapolated below 0.35 mA — see caveat above
    (1.00, 1.22),
    (3.00, 1.02),
    (10.00, 0.88),
    (40.00, 0.88), // held flat above the documented range
];

/// Forward current treated as "fully illuminated" — the cell is at
/// `R_LDR_MIN` here.  Set to the oscillator's measured peak (2.279 mA at
/// R-17 = 4.7 K), so `drive` reaches 1.0 exactly at the top of the cycle and
/// the mapping carries no hidden headroom.  Derived, not fitted.
const LED_I_FULL_MA: f64 = 2.279;

/// Size of the precomputed current→normalised-light table.  The law needs a
/// `ln`/`exp` pair per evaluation; at 88.2 kHz that is not free, so it is
/// tabulated once at construction and linearly interpolated per sample.
const LED_LUT_N: usize = 256;

pub struct Tremolo {
    // --- Oscillator state ---
    /// Behavioral: LFO phase
    #[cfg(feature = "legacy-tremolo")]
    phase: f64,
    #[cfg(feature = "legacy-tremolo")]
    phase_inc: f64,

    /// Circuit: Twin-T oscillator state
    #[cfg(not(feature = "legacy-tremolo"))]
    osc_state: gen_tremolo::CircuitState,

    // --- Shared LDR model ---
    /// Oscillator/processing sample rate — retained so `reset()` can re-settle
    /// the Twin-T oscillator to its steady amplitude (a host `reset()` that
    /// zeroed it without re-settling left the tremolo silent, since the
    /// oscillator needs ~2 s to build up).
    sample_rate: f64,
    depth: f64,
    r_ldr: f64,
    ldr_envelope: f64,
    ldr_attack: f64,
    ldr_release: f64,
    r_ldr_max: f64,
    gamma: f64,
    ln_r_max: f64,
    ln_min_minus_max: f64,
    /// Precomputed normalised light vs LED current (index ∝ current).
    #[cfg(not(feature = "legacy-tremolo"))]
    led_lut: [f64; LED_LUT_N],
}

/// Relative intensity at `i_ma` from the piecewise log-log law, normalised so
/// that `led_intensity(LED_I_FULL_MA) == 1.0`.
fn led_intensity(i_ma: f64) -> f64 {
    if i_ma <= 0.0 {
        return 0.0;
    }
    // Integrate the piecewise-constant exponent in log-current space.
    let ln_at = |i: f64| -> f64 {
        let mut acc = 0.0;
        let mut prev_i = LED_INTENSITY_LAW[0].0;
        let mut prev_n = LED_INTENSITY_LAW[0].1;
        if i <= prev_i {
            // Below the first breakpoint: continue the first segment's slope.
            return prev_n * (i / prev_i).ln();
        }
        for &(bp_i, bp_n) in LED_INTENSITY_LAW.iter().skip(1) {
            let hi = i.min(bp_i);
            acc += prev_n * (hi / prev_i).ln();
            if i <= bp_i {
                return acc;
            }
            prev_i = bp_i;
            prev_n = bp_n;
        }
        acc + prev_n * (i / prev_i).ln()
    };
    (ln_at(i_ma) - ln_at(LED_I_FULL_MA)).exp()
}

/// Fixed oscillator rate for the legacy behavioral LFO (Hz).
#[cfg(feature = "legacy-tremolo")]
const LEGACY_RATE_HZ: f64 = 5.63;

impl Tremolo {
    pub fn new(depth: f64, sample_rate: f64) -> Self {
        Self {
            #[cfg(feature = "legacy-tremolo")]
            phase: 0.0,
            #[cfg(feature = "legacy-tremolo")]
            phase_inc: 2.0 * PI * LEGACY_RATE_HZ / sample_rate,

            #[cfg(not(feature = "legacy-tremolo"))]
            osc_state: {
                let mut s = gen_tremolo::CircuitState::default();
                if (sample_rate - gen_tremolo::SAMPLE_RATE).abs() > 0.5 {
                    s.set_sample_rate(sample_rate);
                }
                // Settle oscillator to reach steady-state amplitude
                for _ in 0..(sample_rate * 2.0) as usize {
                    gen_tremolo::process_sample(0.0, &NO_INJECT, &mut s);
                }
                s
            },

            sample_rate,
            depth,
            r_ldr: R_LDR_MAX,
            ldr_envelope: 0.0,
            ldr_attack: (-1.0 / (ATTACK_TAU * sample_rate)).exp(),
            ldr_release: (-1.0 / (RELEASE_TAU * sample_rate)).exp(),
            r_ldr_max: R_LDR_MAX,
            gamma: GAMMA,
            ln_r_max: R_LDR_MAX.ln(),
            ln_min_minus_max: R_LDR_MIN.ln() - R_LDR_MAX.ln(),
            #[cfg(not(feature = "legacy-tremolo"))]
            led_lut: std::array::from_fn(|k| {
                let i_ma = LED_I_FULL_MA * k as f64 / (LED_LUT_N - 1) as f64;
                led_intensity(i_ma).clamp(0.0, 1.0)
            }),
        }
    }

    pub fn set_depth(&mut self, depth: f64) {
        self.depth = depth.clamp(0.0, 1.0);
    }

    pub fn process(&mut self) -> f64 {
        // Step 1: Oscillator LED drive (0..1). FIXED amplitude — depth does NOT
        // scale the LED; it lives in the shunt divider (Step 4). The real 200A
        // drives the LED at a constant ~0.84 mA off the oscillator through R17.
        let led_drive = self.oscillator_drive();

        // Step 2: CdS LDR envelope (asymmetric attack/release)
        let coeff = if led_drive > self.ldr_envelope {
            self.ldr_attack
        } else {
            self.ldr_release
        };
        self.ldr_envelope = led_drive + coeff * (self.ldr_envelope - led_drive);

        // Step 3: CdS power-law resistance
        let drive = self.ldr_envelope.clamp(0.0, 1.0);
        if drive < 1e-6 {
            self.r_ldr = self.r_ldr_max;
        } else {
            let log_r = self.ln_r_max + self.ln_min_minus_max * drive.powf(self.gamma);
            self.r_ldr = log_r.exp();
        }

        // Step 4: depth divider → shunt impedance seen by fb_junction
        self.shunt_impedance()
    }

    /// Shunt impedance from fb_junction to ground through the vibrato depth
    /// network: `Z = (R_upper ∥ 18 kΩ) + (R_lower ∥ R_ldr)`, with the
    /// 50 kΩ pot split by `depth` (wiper). See the constants block for the
    /// topology. At depth = 0 the LDR branch is grounded (vibrato off).
    fn shunt_impedance(&self) -> f64 {
        let r_upper = R_VIB_POT * (1.0 - self.depth);
        let r_lower = R_VIB_POT * self.depth;
        let top = if r_upper > 0.0 {
            r_upper * R_VIB_BRIDGE / (r_upper + R_VIB_BRIDGE)
        } else {
            0.0
        };
        let branch = self.r_ldr;
        let low = if r_lower > 0.0 {
            r_lower * branch / (r_lower + branch)
        } else {
            0.0
        };
        top + low
    }

    /// Get the oscillator's LED drive signal (0..1).
    #[cfg(feature = "legacy-tremolo")]
    fn oscillator_drive(&mut self) -> f64 {
        let lfo = self.phase.sin();
        self.phase += self.phase_inc;
        if self.phase >= 2.0 * PI {
            self.phase -= 2.0 * PI;
        }
        lfo.max(0.0) // half-wave rectify
    }

    #[cfg(not(feature = "legacy-tremolo"))]
    fn oscillator_drive(&mut self) -> f64 {
        // `.1` is `taps_inner` — the raw LED anode/cathode node voltages. The
        // light law consumes REAL LED CURRENT from these taps; the old
        // collector-voltage map (and its fixed-current assumption) is retired.
        //
        // Averaged across the inner samples rather than decimated: the CdS cell
        // integrates light, so the mean over the inner step is the physically
        // right reduction and it anti-aliases the drive for free.
        let (_out, taps) = gen_tremolo::process_sample(0.0, &NO_INJECT, &mut self.osc_state);
        let mut light = 0.0;
        for t in taps.iter() {
            let i_ma = ((V_RAIL - t[0]) / R18_LED_SERIES) * 1000.0;
            // LUT lookup with linear interpolation; index ∝ current.
            let x = (i_ma / LED_I_FULL_MA).clamp(0.0, 1.0) * (LED_LUT_N - 1) as f64;
            let k = x as usize;
            light += if k + 1 < LED_LUT_N {
                let f = x - k as f64;
                self.led_lut[k] * (1.0 - f) + self.led_lut[k + 1] * f
            } else {
                self.led_lut[LED_LUT_N - 1]
            };
        }
        light / taps.len() as f64
    }

    pub fn current_resistance(&self) -> f64 {
        self.shunt_impedance()
    }

    pub fn reset(&mut self) {
        #[cfg(feature = "legacy-tremolo")]
        {
            self.phase = 0.0;
        }
        #[cfg(not(feature = "legacy-tremolo"))]
        {
            // Rebuild the Twin-T oscillator exactly as `new()` does, then settle
            // it to steady amplitude. `CircuitState::reset()` sets v_prev to the
            // DC operating point — the oscillator's *unstable equilibrium* — so a
            // clean solver started there never begins oscillating; the tremolo
            // stays silent after any host `reset()`. `default()` carries the tiny
            // startup perturbation that kicks the oscillation off. The settle
            // touches only the cheap LFO solver, not the preamp chain.
            let mut s = gen_tremolo::CircuitState::default();
            if (self.sample_rate - gen_tremolo::SAMPLE_RATE).abs() > 0.5 {
                s.set_sample_rate(self.sample_rate);
            }
            for _ in 0..(self.sample_rate * 2.0) as usize {
                gen_tremolo::process_sample(0.0, &NO_INJECT, &mut s);
            }
            self.osc_state = s;
        }
        self.ldr_envelope = 0.0;
        self.r_ldr = self.r_ldr_max;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn probe_raw_oscillator() {
        #[cfg(not(feature = "legacy-tremolo"))]
        {
            let sr = 48000.0;
            let mut s = gen_tremolo::CircuitState::default();
            if (sr - gen_tremolo::SAMPLE_RATE).abs() > 0.5 {
                s.set_sample_rate(sr);
            }
            for _ in 0..(sr * 2.0) as usize {
                gen_tremolo::process_sample(0.0, &NO_INJECT, &mut s);
            }
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            let mut samples = Vec::new();
            for _ in 0..(sr * 2.0) as usize {
                let v = gen_tremolo::process_sample(0.0, &NO_INJECT, &mut s).0[0];
                lo = lo.min(v);
                hi = hi.max(v);
                samples.push(v);
            }
            let mean = samples.iter().sum::<f64>() / samples.len() as f64;
            let mut crossings = 0;
            for i in 1..samples.len() {
                if samples[i - 1] < mean && samples[i] >= mean {
                    crossings += 1;
                }
            }
            let freq = crossings as f64 / 2.0;
            eprintln!(
                "osc raw: low={lo:.3}V high={hi:.3}V mean={mean:.3}V swing={:.3}V freq~{freq:.2}Hz",
                hi - lo
            );
            // Probe the real LED drive: current from the taps, then the light law.
            let mut s2 = gen_tremolo::CircuitState::default();
            if (sr - gen_tremolo::SAMPLE_RATE).abs() > 0.5 {
                s2.set_sample_rate(sr);
            }
            for _ in 0..(sr * 2.0) as usize {
                gen_tremolo::process_sample(0.0, &NO_INJECT, &mut s2);
            }
            let (mut i_min, mut i_max) = (f64::INFINITY, f64::NEG_INFINITY);
            let (mut l_min, mut l_max) = (f64::INFINITY, f64::NEG_INFINITY);
            for _ in 0..(sr * 0.5) as usize {
                let (_o, taps) = gen_tremolo::process_sample(0.0, &NO_INJECT, &mut s2);
                for t in taps.iter() {
                    let i_ma = ((V_RAIL - t[0]) / R18_LED_SERIES) * 1000.0;
                    i_min = i_min.min(i_ma);
                    i_max = i_max.max(i_ma);
                    let l = led_intensity(i_ma).clamp(0.0, 1.0);
                    l_min = l_min.min(l);
                    l_max = l_max.max(l);
                }
            }
            eprintln!("LED current: {i_min:.4}..{i_max:.4} mA");
            eprintln!("normalised light: {l_min:.4}..{l_max:.4}");
        }
    }

    #[test]
    fn test_oscillator_frequency() {
        let sr = 44100.0;
        let mut trem = Tremolo::new(1.0, sr);

        let n = (sr * 2.0) as usize;
        let mut values = Vec::with_capacity(n);
        for _ in 0..n {
            values.push(trem.process());
        }

        let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
        let mut crossings = 0u32;
        for i in 1..values.len() {
            if values[i - 1] < mean && values[i] >= mean {
                crossings += 1;
            }
        }

        // Twin-T oscillator is ~5.3-5.6 Hz; legacy is 5.63 Hz
        // Over 2 seconds expect ~11 crossings
        assert!(
            (8..=14).contains(&crossings),
            "Expected ~11 oscillations in 2s, got {crossings}"
        );
    }

    #[test]
    fn test_oscillator_survives_reset() {
        // Regression guard: a host `reset()` (called before playback) must NOT
        // leave the tremolo silent. `CircuitState::reset()` parks the Twin-T at
        // its DC operating point (unstable equilibrium) where a clean solver
        // never starts oscillating — that shipped the tremolo dead in DAWs while
        // offline renders (fresh `Tremolo::new`) looked fine. `Tremolo::reset()`
        // must rebuild + re-settle the oscillator so modulation persists.
        let sr = 44100.0;
        let mut trem = Tremolo::new(1.0, sr);
        trem.reset();

        let n = (sr * 2.0) as usize;
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for _ in 0..n {
            let r = trem.process();
            lo = lo.min(r);
            hi = hi.max(r);
        }
        // After reset the shunt must still swing meaningfully (dead oscillator
        // would pin it to a single value → ratio ≈ 1.0).
        let swing_db = 20.0 * (hi / lo).log10();
        assert!(
            swing_db > 6.0,
            "Tremolo shunt barely moves after reset() ({swing_db:.1} dB, R∈[{lo:.0},{hi:.0}]) \
             — oscillator likely dead (reset parked it at the DC equilibrium)"
        );
    }

    #[test]
    fn test_depth_zero_is_static() {
        let sr = 44100.0;
        let mut trem = Tremolo::new(0.0, sr);
        trem.set_depth(0.0);

        let n = (sr * 0.5) as usize;
        let mut min_r = f64::MAX;
        let mut max_r = 0.0f64;

        for _ in 0..n {
            let r = trem.process();
            min_r = min_r.min(r);
            max_r = max_r.max(r);
        }

        let range_db = 20.0 * (max_r / min_r).log10();
        assert!(
            range_db < 20.0,
            "At depth 0, resistance should not vary much: {range_db:.1} dB range"
        );
    }

    #[test]
    fn test_resistance_range() {
        // At full depth the shunt impedance seen by fb_junction is the vibrato
        // divider's output: bright ≈ 50 kΩ ∥ R_ldr_min(≈9 kΩ) ≈ 7.7 kΩ,
        // dark ≈ 50 kΩ ∥ settled-R_ldr ≈ low-40 kΩ. The divider CAPS the
        // dark side well below the raw 1 MΩ cell resistance (the grounded pot
        // leg limits it) — this is the loaded-divider fingerprint, not the old
        // fb→R_ldr→gnd shunt that reached ~1 MΩ.
        let sr = 44100.0;
        let mut trem = Tremolo::new(1.0, sr);
        trem.set_depth(1.0);

        let n = (sr * 2.0) as usize;
        let mut min_r = f64::MAX;
        let mut max_r = 0.0f64;

        for _ in 0..n {
            let r = trem.process();
            min_r = min_r.min(r);
            max_r = max_r.max(r);
        }

        // Bright phase pulls the divider output down near ~8 kΩ.
        assert!(
            (5_000.0..15_000.0).contains(&min_r),
            "Bright-phase shunt out of range: {min_r:.0} (expected ~8 kΩ)"
        );
        // Dark phase is capped by the 50 kΩ pot leg — tens of kΩ, never ~1 MΩ.
        assert!(
            (25_000.0..80_000.0).contains(&max_r),
            "Dark-phase shunt out of range: {max_r:.0} (expected ~40–48 kΩ, capped by the pot)"
        );
    }

    #[test]
    fn test_depth_swing_monotonic() {
        // Regression guard on the depth→swing curve. Historically flattened
        // twice: first by a pot double-count (pre-Apr-2026), then by scaling
        // the LED drive with depth (`led_drive = osc * depth`, the melange-era
        // mechanism that made 0.25–0.75 nearly inert). Both are gone — depth
        // now lives solely in the shunt divider (`shunt_impedance`), LED drive
        // is fixed. log10(R_max/R_min) must be monotonically non-decreasing in
        // depth (per schematic #203720-S-3; see the constants block).
        let sr = 44100.0;
        let warmup = sr as usize;
        let measure = (sr * 1.0) as usize;
        let mut swings = Vec::new();
        for depth in [0.25, 0.50, 0.75, 1.00] {
            let mut trem = Tremolo::new(depth, sr);
            trem.set_depth(depth);
            for _ in 0..warmup {
                trem.process();
            }
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for _ in 0..measure {
                let r = trem.process();
                lo = lo.min(r);
                hi = hi.max(r);
            }
            swings.push((depth, (hi / lo).log10()));
        }
        for w in swings.windows(2) {
            let (d0, s0) = w[0];
            let (d1, s1) = w[1];
            assert!(
                s1 >= s0 - 0.02,
                "depth→swing non-monotonic: depth={d0} log-swing={s0:.3} > \
                 depth={d1} log-swing={s1:.3}. Full curve: {swings:?}"
            );
        }
    }

    #[test]
    fn test_asymmetric_envelope() {
        let sr = 44100.0;
        let mut trem = Tremolo::new(1.0, sr);
        trem.set_depth(1.0);

        let n = (sr * 1.0) as usize;
        let mut values = Vec::with_capacity(n);
        for _ in 0..n {
            values.push(trem.process());
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let above_count = values.iter().filter(|&&v| v > mean).count();
        let below_count = values.len() - above_count;

        assert!(
            below_count > above_count,
            "Fast attack + slow release → resistance should spend more time low: above={above_count}, below={below_count}"
        );
    }
}
