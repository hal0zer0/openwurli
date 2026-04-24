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

/// CdS LDR parameters (shared between behavioral and circuit paths).
const ATTACK_TAU: f64 = 0.003;
const RELEASE_TAU: f64 = 0.050;
/// LDR bright-phase floor. Set to match the preamp's documented
/// "tremolo bright" calibration point (19 kΩ total shunt → 12.61 dB gain).
/// Shunt is `R_SHUNT_SERIES + r_ldr`, so r_ldr_min = 19_000 − 680 ≈ 18 320.
/// Previously 50 Ω (datasheet CdS min) — but shunt values below about
/// 1 kΩ clamp at the preamp's `.runtime R 1k 1Meg` floor and waste drive.
/// Keeping the bright phase at the preamp's documented bright point gives
/// monotonic depth→swing behavior and hits the 6.1 dB gain-range target.
const R_LDR_MIN: f64 = 18_320.0;
const R_LDR_MAX: f64 = 1_000_000.0;
const GAMMA: f64 = 1.1;

/// Fixed series resistance in the LDR shunt path from fb_junction to GND:
/// R-??? (680 Ω on LG-1 pin 5) in series with the LDR itself. The 50 kΩ
/// VIBRATO pot + 18 kΩ are in the LED drive path (controlling LED current
/// and hence brightness), **not** in this shunt path — see `set_depth`.
/// Pre-Apr-2026 versions double-counted the pot into both paths, which
/// flattened the depth→swing curve at high settings (100 % actually milder
/// than 75 %). See `memory/known-issues.md` for the full write-up.
const R_SHUNT_SERIES: f64 = 680.0;

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
        // Step 1: Get oscillator drive (0..1)
        let led_drive = self.oscillator_drive() * self.depth;

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

        R_SHUNT_SERIES + self.r_ldr
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
        R_SHUNT_SERIES + self.r_ldr
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
            eprintln!("led_drive: min={ld_min:.3} max={ld_max:.3} swing={:.3}", ld_max - ld_min);
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

        assert!(min_r < 100_000.0, "Min resistance too high: {min_r:.0}");
        assert!(max_r > 200_000.0, "Max resistance too low: {max_r:.0}");
    }

    #[test]
    fn test_depth_swing_monotonic() {
        // Regression guard against the pre-Apr-2026 bug where `set_depth` mixed
        // the 50 kΩ VIBRATO pot into both the LED drive path AND the feedback
        // shunt path. That double-count made depth=1.0 produce *less* swing than
        // depth=0.75 because the small r_series at high depth raised the dim-
        // phase floor. Fix: moved the pot out of the shunt (pot only affects
        // LED drive via the `led_drive = osc * depth` scaling), left the
        // shunt at R_SHUNT_SERIES + r_ldr. log10(R_max/R_min) must be
        // monotonically non-decreasing in depth.
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

