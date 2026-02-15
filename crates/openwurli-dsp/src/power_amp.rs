/// Wurlitzer 200A power amplifier model — Class AB crossover + rail clipping.
///
/// The real 200A has a quasi-complementary push-pull Class AB output stage
/// (TIP35C/TIP36C, ±24V rails, 20W into 8Ω). At moderate levels it's nearly
/// transparent; the preamp dominates tonal character. At ff polyphonic or with
/// aged bias, crossover distortion and rail clipping become audible.
///
/// Model: quadratic dead zone (crossover) + hard clip (rails).

pub struct PowerAmp {
    /// Dead zone half-width (crossover distortion).
    /// Well-biased: ~0.0005. Aged: 0.002-0.01.
    crossover_width: f64,
    /// Symmetric rail clipping limit.
    rail_limit: f64,
}

impl PowerAmp {
    pub fn new() -> Self {
        Self {
            crossover_width: 0.0005,
            rail_limit: 1.5,
        }
    }

    pub fn process(&mut self, input: f64) -> f64 {
        // Crossover distortion: quadratic dead zone
        let abs_in = input.abs();
        let out = if abs_in < self.crossover_width {
            let ratio = abs_in / self.crossover_width;
            input.signum() * abs_in * ratio * ratio
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
        // Signal in the dead zone
        let input = 0.0001;
        let output = pa.process(input);
        assert!(output < input, "Should attenuate in dead zone: in={input}, out={output}");
    }

    #[test]
    fn test_rail_clipping() {
        let mut pa = PowerAmp::new();
        let output = pa.process(5.0);
        assert_eq!(output, 1.5, "Should clip at rail limit");
        let output_neg = pa.process(-5.0);
        assert_eq!(output_neg, -1.5, "Should clip at negative rail");
    }

    #[test]
    fn test_symmetry() {
        let mut pa = PowerAmp::new();
        let pos = pa.process(0.3);
        let neg = pa.process(-0.3);
        assert!((pos + neg).abs() < 1e-15, "Should be symmetric");
    }
}
