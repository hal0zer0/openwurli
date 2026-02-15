/// Wurlitzer 200A tremolo — LFO + LED + CdS LDR model.
///
/// The tremolo modulates preamp gain by varying the LDR resistance in the
/// emitter feedback path. The signal flow is:
///
///   Twin-T oscillator (~5.5 Hz) -> half-wave rectified -> LED current
///   -> CdS LDR (light-dependent resistor) -> R_ldr
///   -> feedback junction -> modulates preamp closed-loop gain
///
/// LDR characteristics:
///   - CdS photoresistors have asymmetric response: fast attack (~3ms),
///     slow decay (~50ms) — the "memory" effect.
///   - Resistance follows approximate power law: R = R_dark * (I_led / I_ref)^(-gamma)
///   - R_dark ~ 1M ohm, gamma ~ 0.7 for typical CdS cells

use std::f64::consts::PI;

pub struct Tremolo {
    /// LFO phase (0..2*PI)
    phase: f64,
    /// Phase increment per sample
    phase_inc: f64,
    /// Depth: 0.0 = no tremolo, 1.0 = full depth
    depth: f64,
    /// Current LDR resistance (ohms)
    r_ldr: f64,
    /// LDR envelope state (smoothed LED drive)
    ldr_envelope: f64,
    /// LDR attack coefficient (fast: ~3ms)
    ldr_attack: f64,
    /// LDR release coefficient (slow: ~50ms)
    ldr_release: f64,
    /// Minimum LDR resistance (fully illuminated): ~50 ohms
    r_ldr_min: f64,
    /// Maximum LDR resistance (dark): ~1M ohms
    r_ldr_max: f64,
    /// CdS power-law exponent
    gamma: f64,
    /// Series resistance in LDR path: 18K (fixed) + vibrato pot position
    r_series: f64,
}

impl Tremolo {
    /// Create a new tremolo at the given rate and sample rate.
    ///
    /// - `rate`: LFO frequency in Hz (default ~5.5)
    /// - `depth`: 0.0 to 1.0 (maps to vibrato pot position)
    /// - `sample_rate`: audio sample rate
    pub fn new(rate: f64, depth: f64, sample_rate: f64) -> Self {
        let attack_tau = 0.003; // 3ms
        let release_tau = 0.050; // 50ms

        Self {
            phase: 0.0,
            phase_inc: 2.0 * PI * rate / sample_rate,
            depth,
            r_ldr: 1_000_000.0,
            ldr_envelope: 0.0,
            ldr_attack: (-1.0 / (attack_tau * sample_rate)).exp(),
            ldr_release: (-1.0 / (release_tau * sample_rate)).exp(),
            r_ldr_min: 50.0,
            r_ldr_max: 1_000_000.0,
            gamma: 0.7,
            r_series: 18_000.0,
        }
    }

    /// Set LFO rate in Hz.
    pub fn set_rate(&mut self, rate: f64, sample_rate: f64) {
        self.phase_inc = 2.0 * PI * rate / sample_rate;
    }

    /// Set tremolo depth (0.0 = off, 1.0 = full).
    /// Maps to the 50K vibrato pot: higher depth = less series resistance.
    pub fn set_depth(&mut self, depth: f64) {
        self.depth = depth.clamp(0.0, 1.0);
        // Vibrato pot: 50K at depth=0 (minimum tremolo), ~0 at depth=1 (max tremolo)
        let pot_resistance = 50_000.0 * (1.0 - self.depth);
        self.r_series = 18_000.0 + pot_resistance;
    }

    /// Process one sample: advance LFO, update LDR resistance.
    /// Returns the total LDR path resistance (R_series + R_ldr) in ohms,
    /// suitable for `PreampModel::set_ldr_resistance()`.
    pub fn process(&mut self) -> f64 {
        // LFO: sinusoidal oscillator (twin-T output is ~sinusoidal)
        let lfo = self.phase.sin();
        self.phase += self.phase_inc;
        if self.phase >= 2.0 * PI {
            self.phase -= 2.0 * PI;
        }

        // Half-wave rectify: LED only conducts on positive half-cycle.
        // Scale by depth: vibrato pot controls LED current.
        // At depth=0, pot adds 50K series resistance → negligible LED current.
        let led_drive = lfo.max(0.0) * self.depth;

        // LDR envelope follower with asymmetric time constants
        // (CdS material has fast response to light increase, slow decay)
        let coeff = if led_drive > self.ldr_envelope {
            self.ldr_attack
        } else {
            self.ldr_release
        };
        self.ldr_envelope = led_drive + coeff * (self.ldr_envelope - led_drive);

        // CdS power-law: R_ldr = R_dark * (drive + epsilon)^(-gamma)
        // Normalized so full drive -> R_min, zero drive -> R_max
        let drive = self.ldr_envelope.clamp(0.0, 1.0);
        if drive < 1e-6 {
            self.r_ldr = self.r_ldr_max;
        } else {
            // Power law mapping
            self.r_ldr = self.r_ldr_min + (self.r_ldr_max - self.r_ldr_min) * (1.0 - drive).powf(1.0 / self.gamma);
        }

        // Total path resistance: series resistors + LDR
        self.r_series + self.r_ldr
    }

    /// Get the current LDR path resistance without advancing the LFO.
    pub fn current_resistance(&self) -> f64 {
        self.r_series + self.r_ldr
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
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

        // Count zero crossings of the effective resistance
        // (resistance oscillates between min and max)
        let n = (sr * 2.0) as usize; // 2 seconds
        let mut values = Vec::with_capacity(n);
        for _ in 0..n {
            values.push(trem.process());
        }

        // Find the mean resistance
        let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;

        // Count upward zero crossings through the mean
        let mut crossings = 0u32;
        for i in 1..values.len() {
            if values[i - 1] < mean && values[i] >= mean {
                crossings += 1;
            }
        }

        // Should be approximately rate * 2 seconds = 11 crossings
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

        // At depth 0, the series resistance is 50K + 18K = 68K + LDR
        // The LDR still oscillates, but the pot resistance dominates
        // and the modulation range is much smaller
        let range_db = 20.0 * (max_r / min_r).log10();

        // At depth=0, modulation should be small (vibrato pot adds 50K series)
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

        // At full depth: min should approach 18K + ~50 ohm (LDR bright)
        // max should approach 18K + ~1M (LDR dark)
        assert!(min_r < 100_000.0, "Min resistance too high: {min_r:.0}");
        assert!(max_r > 200_000.0, "Max resistance too low: {max_r:.0}");
    }

    #[test]
    fn test_asymmetric_envelope() {
        let sr = 44100.0;
        let mut trem = Tremolo::new(5.5, 1.0, sr);
        trem.set_depth(1.0);

        // Run for a few cycles to settle
        let n = (sr * 1.0) as usize;
        let mut values = Vec::with_capacity(n);
        for _ in 0..n {
            values.push(trem.process());
        }

        // The LDR should have asymmetric response: fast decrease (attack),
        // slow increase (release). Check by measuring fall time vs rise time.
        // Find a min->max transition and a max->min transition
        let mean = values.iter().sum::<f64>() / values.len() as f64;

        // This is a qualitative test — just verify the waveform isn't symmetric
        // by checking that the resistance spends more time below the mean than above.
        // Fast attack (3ms) means the envelope reaches peak quickly and stays high,
        // keeping resistance LOW for most of each cycle. Slow release (50ms) means
        // the envelope only partially decays during the LFO's off-phase.
        let above_count = values.iter().filter(|&&v| v > mean).count();
        let below_count = values.len() - above_count;

        assert!(
            below_count > above_count,
            "Fast attack + slow release → resistance should spend more time low: above={above_count}, below={below_count}"
        );
    }
}
