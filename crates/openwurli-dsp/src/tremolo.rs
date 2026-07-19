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

/// 200A vibrato depth network, per schematic #203720-S-3 (schemer 2026-07-19,
/// verified on the 36 MP scan). The 50 kΩ front-panel VIBRATO pot is a
/// 3-terminal divider in the fb_junction→LDR shunt leg: top→fb_junction,
/// bottom→ground, wiper→the LDR branch. An 18 kΩ resistor bridges top→wiper;
/// R18 (680 Ω) is in series in the LDR branch off the wiper. The LED drive is
/// FIXED (depth does not scale it) — depth is the wiper position alone.
/// Shunt impedance seen by fb_junction:
///   Z = (R_upper ∥ 18 kΩ) + (R_lower ∥ (680 Ω + R_ldr))
/// with R_upper = 50 kΩ·(1−depth), R_lower = 50 kΩ·depth. depth = 1.0 puts the
/// wiper at the fb end (max depth); depth = 0 grounds the LDR branch (vibrato
/// off, fb sees a fixed 50 kΩ ∥ 18 kΩ ≈ 13 kΩ). See `shunt_impedance`.
const R18_SERIES: f64 = 680.0;
const R_VIB_BRIDGE: f64 = 18_000.0;
const R_VIB_POT: f64 = 50_000.0;

/// Twin-T oscillator output voltage range (from ngspice/melange validation).
#[cfg(not(feature = "legacy-tremolo"))]
const V_OUT_MIN: f64 = 0.70;
#[cfg(not(feature = "legacy-tremolo"))]
const V_OUT_MAX: f64 = 10.95;

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
    depth: f64,
    r_ldr: f64,
    ldr_envelope: f64,
    ldr_attack: f64,
    ldr_release: f64,
    r_ldr_max: f64,
    gamma: f64,
    ln_r_max: f64,
    ln_min_minus_max: f64,
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
                    gen_tremolo::process_sample(0.0, &mut s);
                }
                s
            },

            depth,
            r_ldr: R_LDR_MAX,
            ldr_envelope: 0.0,
            ldr_attack: (-1.0 / (ATTACK_TAU * sample_rate)).exp(),
            ldr_release: (-1.0 / (RELEASE_TAU * sample_rate)).exp(),
            r_ldr_max: R_LDR_MAX,
            gamma: GAMMA,
            ln_r_max: R_LDR_MAX.ln(),
            ln_min_minus_max: R_LDR_MIN.ln() - R_LDR_MAX.ln(),
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
    /// network: `Z = (R_upper ∥ 18 kΩ) + (R_lower ∥ (R18 + R_ldr))`, with the
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
        let branch = R18_SERIES + self.r_ldr;
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
        let v_out = gen_tremolo::process_sample(0.0, &mut self.osc_state)[0];
        // Map collector voltage to LED drive: low V = bright LED = high drive
        ((V_OUT_MAX - v_out) / (V_OUT_MAX - V_OUT_MIN)).clamp(0.0, 1.0)
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
            self.osc_state.reset();
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
                gen_tremolo::process_sample(0.0, &mut s);
            }
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            let mut samples = Vec::new();
            for _ in 0..(sr * 2.0) as usize {
                let v = gen_tremolo::process_sample(0.0, &mut s)[0];
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
            eprintln!("expected: V_OUT_MIN={V_OUT_MIN:.2} V_OUT_MAX={V_OUT_MAX:.2}");

            // Also probe what led_drive looks like via the mapping
            let mut ld_min = f64::INFINITY;
            let mut ld_max = f64::NEG_INFINITY;
            for v in &samples {
                let ld = ((V_OUT_MAX - v) / (V_OUT_MAX - V_OUT_MIN)).clamp(0.0, 1.0);
                ld_min = ld_min.min(ld);
                ld_max = ld_max.max(ld);
            }
            eprintln!(
                "led_drive: min={ld_min:.3} max={ld_max:.3} swing={:.3}",
                ld_max - ld_min
            );
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
            crossings >= 8 && crossings <= 14,
            "Expected ~11 oscillations in 2s, got {crossings}"
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
        // divider's output: bright ≈ 50 kΩ ∥ (680 + R_ldr_min≈9 kΩ) ≈ 8 kΩ,
        // dark ≈ 50 kΩ ∥ (680 + settled-R_ldr) ≈ mid-40 kΩ. The divider CAPS the
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
