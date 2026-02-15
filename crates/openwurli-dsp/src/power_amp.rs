/// Wurlitzer 200A power amplifier model — Class AB crossover + rail clipping.
///
/// The real 200A has a quasi-complementary push-pull Class AB output stage
/// (TIP35C/TIP36C, ±24V rails, 20W into 8Ω). At moderate levels it's nearly
/// transparent; the preamp dominates tonal character. At ff polyphonic or with
/// aged bias, crossover distortion and rail clipping become audible.
///
/// Model: Hermite smoothstep dead zone (crossover, C1 continuous) + hard clip (rails).
///
/// Crossover width of 0.003 models a typical lightly-aged instrument (~5-7 mA
/// bias vs factory 10 mA). The ±3 mV dead zone adds subtle odd-harmonic grit
/// at low volume settings where signal amplitude is in the 0.01-0.1V range.

pub struct PowerAmp {
    /// Dead zone half-width (crossover distortion).
    /// Factory-fresh: ~0.0005. Lightly aged (typical): 0.003. Worn: 0.005-0.01.
    crossover_width: f64,
    /// Symmetric rail clipping limit (normalized to signal headroom).
    rail_limit: f64,
}

impl PowerAmp {
    pub fn new() -> Self {
        Self {
            crossover_width: 0.003,
            rail_limit: 1.0,
        }
    }

    pub fn process(&mut self, input: f64) -> f64 {
        // Crossover distortion: Hermite smoothstep dead zone (C1 continuous)
        let abs_in = input.abs();
        let out = if abs_in < self.crossover_width {
            let ratio = abs_in / self.crossover_width;
            let smooth = ratio * ratio * (3.0 - 2.0 * ratio);
            input.signum() * abs_in * smooth
        } else {
            input
        };

        // Hard clip at rails
        out.clamp(-self.rail_limit, self.rail_limit)
    }

    pub fn reset(&mut self) {
        // Stateless — nothing to reset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transparent_at_moderate_levels() {
        let mut pa = PowerAmp::new();
        // Signal well above crossover, below rails
        let input = 0.5;
        let output = pa.process(input);
        assert!((output - input).abs() < 1e-10, "Should be transparent: {output}");
    }

    #[test]
    fn test_crossover_distortion() {
        let mut pa = PowerAmp::new();
        // Signal in the dead zone (below crossover_width of 0.003)
        let input = 0.001;
        let output = pa.process(input);
        assert!(output < input, "Should attenuate in dead zone: in={input}, out={output}");
    }

    #[test]
    fn test_rail_clipping() {
        let mut pa = PowerAmp::new();
        let output = pa.process(5.0);
        assert_eq!(output, 1.0, "Should clip at rail limit");
        let output_neg = pa.process(-5.0);
        assert_eq!(output_neg, -1.0, "Should clip at negative rail");
    }

    #[test]
    fn test_symmetry() {
        let mut pa = PowerAmp::new();
        let pos = pa.process(0.3);
        let neg = pa.process(-0.3);
        assert!((pos + neg).abs() < 1e-15, "Should be symmetric");
    }

    #[test]
    fn test_crossover_generates_distortion() {
        // A small sine signal should show measurable THD from crossover
        use std::f64::consts::PI;

        let mut pa = PowerAmp::new();
        let sr = 44100.0;
        let freq = 440.0;
        let amplitude = 0.01; // Small signal — close to crossover width

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
        assert!(h3_ratio > 0.001,
            "Crossover should produce measurable H3 at low amplitude: {h3_ratio:.5}");
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
