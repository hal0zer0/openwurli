//! Wurlitzer 200A power amplifier model -- VAS gain + Class AB crossover + rail clipping.
//!
//! The real 200A has a quasi-complementary push-pull Class AB output stage
//! (TIP35C/TIP36C, +/-24V rails, 20W into 8 ohm). The VAS and driver stages
//! provide voltage gain (differential input TR-7/TR-8, VAS TR-11, drivers
//! TR-10/TR-12). At moderate levels the amp is nearly transparent; at ff
//! polyphonic or with aged bias, crossover distortion and rail clipping
//! become audible.
//!
//! Signal flow inside the power amp model:
//!   input -> voltage gain (VAS/driver) -> crossover dead zone -> rail clip -> output
//!
//! The voltage gain of 8.0 represents the closed-loop gain of the power amp
//! with R-31 (15K) negative feedback. This gain was previously applied
//! externally as "preamp_gain" -- but it physically belongs here. Moving it
//! into the power amp means the crossover distortion and rail clipping
//! interact correctly with the actual signal level (post-volume pot).
//!
//! Crossover width of 0.003 models a typical lightly-aged instrument (~5-7 mA
//! bias vs factory 10 mA). Applied to the amplified signal, this creates
//! subtle odd-harmonic grit that becomes audible at low volume settings.

/// Power amp voltage gain from VAS/driver stages (closed-loop with R-31 feedback).
const VOLTAGE_GAIN: f64 = 8.0;

/// Headroom factor matching the real ±24V power amp rails.
///
/// The real circuit has ±24V rails (headroom ratio ~3:1 over max signal at
/// full volume). PA output is normalized to ±1.0 by dividing by HEADROOM
/// after clipping.
const HEADROOM: f64 = 2.5;

pub struct PowerAmp {
    /// VAS/driver voltage gain.
    voltage_gain: f64,
    /// Dead zone half-width (crossover distortion, in amplified signal units).
    /// Factory-fresh: ~0.0005. Lightly aged (typical): 0.003. Worn: 0.005-0.01.
    crossover_width: f64,
    /// Symmetric rail clipping limit (amplified signal units, NOT normalized).
    rail_limit: f64,
}

impl PowerAmp {
    pub fn new() -> Self {
        Self {
            voltage_gain: VOLTAGE_GAIN,
            crossover_width: 0.003,
            rail_limit: HEADROOM,
        }
    }

    pub fn process(&mut self, input: f64) -> f64 {
        // VAS/driver voltage amplification
        let amplified = input * self.voltage_gain;

        // Output stage crossover distortion: Hermite smoothstep dead zone (C1 continuous)
        let abs_amp = amplified.abs();
        let out = if abs_amp < self.crossover_width {
            let ratio = abs_amp / self.crossover_width;
            let smooth = ratio * ratio * (3.0 - 2.0 * ratio);
            amplified.signum() * abs_amp * smooth
        } else {
            amplified
        };

        // Hard clip at rails, then normalize to ±1.0 for DAC
        out.clamp(-self.rail_limit, self.rail_limit) / self.rail_limit
    }

    pub fn reset(&mut self) {
        // Stateless — nothing to reset
    }
}

impl Default for PowerAmp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gain_applied() {
        let mut pa = PowerAmp::new();
        // Input well above crossover threshold, below rail clip
        let input = 0.05; // 0.05 * 8 = 0.40 (below HEADROOM, above crossover)
        let output = pa.process(input);
        let expected = input * VOLTAGE_GAIN / HEADROOM;
        assert!(
            (output - expected).abs() < 1e-10,
            "Should apply gain + normalize: expected {expected}, got {output}"
        );
    }

    #[test]
    fn test_crossover_distortion() {
        let mut pa = PowerAmp::new();
        // Tiny input: 0.0001 * 8 = 0.0008, which is inside dead zone (0.003)
        let input = 0.0001;
        let output = pa.process(input);
        let clean = input * VOLTAGE_GAIN / HEADROOM;
        assert!(
            output < clean,
            "Should attenuate in dead zone: clean={clean}, got={output}"
        );
    }

    #[test]
    fn test_rail_clipping() {
        let mut pa = PowerAmp::new();
        // 5.0 * 8 = 40.0, far above rail_limit of 2.5
        let output = pa.process(5.0);
        assert!(
            (output - 1.0).abs() < 1e-10,
            "Should clip and normalize to 1.0"
        );
        let output_neg = pa.process(-5.0);
        assert!(
            (output_neg + 1.0).abs() < 1e-10,
            "Should clip and normalize to -1.0"
        );
    }

    #[test]
    fn test_symmetry() {
        let mut pa = PowerAmp::new();
        let pos = pa.process(0.05);
        let neg = pa.process(-0.05);
        assert!((pos + neg).abs() < 1e-15, "Should be symmetric");
    }

    #[test]
    fn test_crossover_generates_distortion() {
        // A small sine signal (after gain) should show measurable THD from crossover
        use std::f64::consts::PI;

        let mut pa = PowerAmp::new();
        let sr = 44100.0;
        let freq = 440.0;
        // After 8x gain, amplitude = 0.008 → close to crossover width of 0.003
        let amplitude = 0.001;

        let n = (sr * 0.2) as usize;
        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let x = amplitude * (2.0 * PI * freq * i as f64 / sr).sin();
            samples.push(pa.process(x));
        }

        // Measure H3 (odd harmonic from symmetric crossover)
        let start = n / 2;
        let slice = &samples[start..];
        let f1 = dft_mag(slice, freq, sr);
        let f3 = dft_mag(slice, 3.0 * freq, sr);

        let h3_ratio = f3 / f1;
        assert!(
            h3_ratio > 0.001,
            "Crossover should produce measurable H3 at low amplitude: {h3_ratio:.5}"
        );
    }

    fn dft_mag(samples: &[f64], freq: f64, sr: f64) -> f64 {
        use std::f64::consts::PI;
        let n = samples.len() as f64;
        let mut re = 0.0;
        let mut im = 0.0;
        for (i, &s) in samples.iter().enumerate() {
            let phase = 2.0 * PI * freq * i as f64 / sr;
            re += s * phase.cos();
            im += s * phase.sin();
        }
        (re * re + im * im).sqrt() / n
    }
}
