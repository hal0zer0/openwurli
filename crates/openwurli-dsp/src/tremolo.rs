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
const R_LDR_MIN: f64 = 50.0;
const R_LDR_MAX: f64 = 1_000_000.0;
const GAMMA: f64 = 1.1;

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
    r_series: f64,
}

impl Tremolo {
    pub fn new(rate: f64, depth: f64, sample_rate: f64) -> Self {
        Self {
            #[cfg(feature = "legacy-tremolo")]
            phase: 0.0,
            #[cfg(feature = "legacy-tremolo")]
            phase_inc: 2.0 * PI * rate / sample_rate,

            // Circuit oscillator frequency is set by components, not `rate`.
            #[cfg(not(feature = "legacy-tremolo"))]
            osc_state: {
                let _ = rate;
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
            r_series: 18_000.0,
        }
    }

    pub fn set_rate(&mut self, rate: f64, sample_rate: f64) {
        #[cfg(feature = "legacy-tremolo")]
        {
            self.phase_inc = 2.0 * PI * rate / sample_rate;
        }
        #[cfg(not(feature = "legacy-tremolo"))]
        {
            // The circuit oscillator's frequency is set by components, not a parameter.
            // Rate knob is ignored — the Twin-T frequency is fixed at ~5.3 Hz.
            let _ = (rate, sample_rate);
        }
    }

    pub fn set_depth(&mut self, depth: f64) {
        self.depth = depth.clamp(0.0, 1.0);
        let pot_resistance = 50_000.0 * (1.0 - self.depth);
        self.r_series = 18_000.0 + pot_resistance;
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

        self.r_series + self.r_ldr
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
        self.r_series + self.r_ldr
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
    fn test_lfo_frequency() {
        let sr = 44100.0;
        let rate = 5.5;
        let mut trem = Tremolo::new(rate, 1.0, sr);

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

        let expected = (rate * 2.0) as u32;
        assert!(
            crossings.abs_diff(expected) <= 2,
            "Expected ~{expected} oscillations, got {crossings}"
        );
    }

    #[test]
    fn test_depth_zero_is_static() {
        let sr = 44100.0;
        let mut trem = Tremolo::new(5.5, 0.0, sr);
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
        let mut trem = Tremolo::new(5.5, 1.0, sr);
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
    fn test_asymmetric_envelope() {
        let sr = 44100.0;
        let mut trem = Tremolo::new(5.5, 1.0, sr);
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
