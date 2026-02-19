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
//!   input -> voltage gain (VAS/driver) -> crossover dead zone -> tanh soft-clip -> output
//!
//! The voltage gain is 1 + R31/R30 = 1 + 15K/220Ω = 69x (37 dB), set by
//! the R-31 negative feedback network. HEADROOM matches the real ±24V rails
//! minus ~2V Vce_sat = ±22V. The effective gain ratio (69/22 = 3.136) is
//! nearly identical to the previous simplified model (8/2.5 = 3.2), so
//! output levels shift by only -0.17 dB. But crossover distortion now
//! operates at the correct absolute signal level.
//!
//! Crossover width of 0.026 models a typical lightly-aged instrument (~5-7 mA
//! bias vs factory 10 mA). This is the same physical ~26mV dead zone as
//! before (0.003 × 8 = 0.024 ≈ 0.026 × 1.0 after rescaling), just expressed
//! in the correct voltage domain.

/// Power amp voltage gain: 1 + R31/R30 = 1 + 15K/220Ω = 69x (37 dB).
const VOLTAGE_GAIN: f64 = 69.0;

/// Rail headroom: ±24V supply minus ~2V Vce_sat = ±22V effective swing.
///
/// PA output is normalized to ±1.0 by dividing by HEADROOM after clipping.
const HEADROOM: f64 = 22.0;

pub struct PowerAmp {
    /// VAS/driver voltage gain.
    voltage_gain: f64,
    /// Dead zone half-width (crossover distortion, in amplified signal units).
    /// Factory-fresh: ~0.004. Lightly aged (typical): 0.026. Worn: 0.04-0.09.
    crossover_width: f64,
    /// Symmetric rail clipping limit (amplified signal units, NOT normalized).
    rail_limit: f64,
}

impl PowerAmp {
    pub fn new() -> Self {
        Self {
            voltage_gain: VOLTAGE_GAIN,
            crossover_width: 0.026,
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

        // Soft-clip at rails: tanh models gradual transistor saturation.
        // Real Class AB output transistors compress smoothly into saturation —
        // waveform peaks round off rather than flat-topping.
        // For small signals: tanh(x) ≈ x (linear region preserved).
        // At rail voltage: tanh(1.0) = 0.762 → ~2.4 dB compression.
        (out / self.rail_limit).tanh()
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
        // 0.005 * 69 = 0.345 (below HEADROOM=22, above crossover=0.026)
        // normalized = 0.345/22 = 0.01568 → tanh(0.01568) ≈ 0.01568 (deep linear region)
        let input = 0.005;
        let output = pa.process(input);
        let expected = (input * VOLTAGE_GAIN / HEADROOM).tanh();
        assert!(
            (output - expected).abs() < 1e-10,
            "Should apply gain + tanh soft-clip: expected {expected}, got {output}"
        );
    }

    #[test]
    fn test_crossover_distortion() {
        let mut pa = PowerAmp::new();
        // Tiny input: 0.0001 * 69 = 0.0069, which is inside dead zone (0.026)
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
        // 5.0 * 69 = 345.0, far above rail_limit of 22.0
        // tanh(345/22) ≈ 1.0 (asymptotic, never exact)
        let output = pa.process(5.0);
        assert!(
            output > 0.999 && output < 1.0,
            "Should soft-clip near 1.0: got {output}"
        );
        let output_neg = pa.process(-5.0);
        assert!(
            output_neg < -0.999 && output_neg > -1.0,
            "Should soft-clip near -1.0: got {output_neg}"
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
        // After 69x gain, amplitude = 0.069 → close to crossover width of 0.026
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
