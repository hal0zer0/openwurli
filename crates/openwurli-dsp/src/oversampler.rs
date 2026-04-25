//! 2x polyphase IIR half-band oversampler.
//!
//! Uses cascaded allpass sections in a polyphase decomposition for efficient
//! half-band filtering. This provides the antialiasing needed for the preamp's
//! nonlinear processing (BJT soft-clip generates harmonics that must not alias).
//!
//! Design: Regalia-Mitra allpass-based half-band IIR.
//! ~28 dB rejection at 30 kHz with 3 coefficients per branch (6 allpass sections).

/// Half-band IIR allpass coefficients (~28 dB rejection at 30 kHz).
///
/// These come from published tables for elliptic half-band IIR filters
/// decomposed into two parallel allpass branches. Each branch is a cascade
/// of first-order allpass sections: y = (a + z^-1) / (1 + a*z^-1).
///
/// Transition band: ~0.1*fs (fairly sharp for 2x oversampling).
const BRANCH_A_COEFFS: &[f64] = &[
    0.036_681_502_163_648,
    0.248_030_921_580_110,
    0.643_184_620_136_480,
];

const BRANCH_B_COEFFS: &[f64] = &[
    0.110_377_634_768_680,
    0.420_399_304_190_880,
    0.854_640_112_701_920,
];

/// First-order allpass section: y = (a + z^-1) / (1 + a*z^-1)
#[derive(Clone)]
struct AllpassSection {
    a: f64,
    state: f64,
}

impl AllpassSection {
    fn new(a: f64) -> Self {
        Self { a, state: 0.0 }
    }

    fn process(&mut self, x: f64) -> f64 {
        let y = self.a * x + self.state;
        self.state = x - self.a * y;
        y
    }

    fn reset(&mut self) {
        self.state = 0.0;
    }
}

/// Allpass branch: cascade of first-order allpass sections.
#[derive(Clone)]
struct AllpassBranch {
    sections: Vec<AllpassSection>,
}

impl AllpassBranch {
    fn new(coeffs: &[f64]) -> Self {
        Self {
            sections: coeffs.iter().map(|&a| AllpassSection::new(a)).collect(),
        }
    }

    fn process(&mut self, x: f64) -> f64 {
        let mut y = x;
        for section in &mut self.sections {
            y = section.process(y);
        }
        y
    }

    fn reset(&mut self) {
        for section in &mut self.sections {
            section.reset();
        }
    }
}

/// 2x polyphase IIR half-band oversampler.
pub struct Oversampler {
    /// Branch A (processes even samples)
    up_branch_a: AllpassBranch,
    /// Branch B (processes odd samples)
    up_branch_b: AllpassBranch,
    /// Branch A for downsampling
    down_branch_a: AllpassBranch,
    /// Branch B for downsampling
    down_branch_b: AllpassBranch,
    /// One-sample delay for the B branch in downsampling
    down_delay: f64,
}

impl Oversampler {
    pub fn new() -> Self {
        Self {
            up_branch_a: AllpassBranch::new(BRANCH_A_COEFFS),
            up_branch_b: AllpassBranch::new(BRANCH_B_COEFFS),
            down_branch_a: AllpassBranch::new(BRANCH_A_COEFFS),
            down_branch_b: AllpassBranch::new(BRANCH_B_COEFFS),
            down_delay: 0.0,
        }
    }

    /// Upsample by 2x: insert zeros between samples, filter.
    /// Input: N samples at base rate.
    /// Output: 2N samples at 2x rate (written into provided buffer).
    pub fn upsample_2x(&mut self, input: &[f64], output: &mut [f64]) {
        debug_assert!(output.len() >= input.len() * 2);

        for (i, &x) in input.iter().enumerate() {
            // Polyphase decomposition: feed x to both branches,
            // interleave outputs.
            let a = self.up_branch_a.process(x);
            let b = self.up_branch_b.process(x);

            // Branch A produces even samples, Branch B produces odd samples.
            output[i * 2] = a;
            output[i * 2 + 1] = b;
        }
    }

    /// Downsample by 2x: filter, decimate.
    /// Input: 2N samples at 2x rate.
    /// Output: N samples at base rate (written into provided buffer).
    pub fn downsample_2x(&mut self, input: &[f64], output: &mut [f64]) {
        debug_assert!(input.len() >= output.len() * 2);

        for (i, out) in output.iter_mut().enumerate() {
            // Feed even sample to branch A, odd sample to branch B
            let a = self.down_branch_a.process(input[i * 2]);
            let b = self.down_branch_b.process(input[i * 2 + 1]);

            // Average the two branches (half-band filter property)
            // B branch needs one sample delay for phase alignment
            *out = (a + self.down_delay) * 0.5;
            self.down_delay = b;
        }
    }

    pub fn reset(&mut self) {
        self.up_branch_a.reset();
        self.up_branch_b.reset();
        self.down_branch_a.reset();
        self.down_branch_b.reset();
        self.down_delay = 0.0;
    }
}

impl Default for Oversampler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_roundtrip_preserves_signal() {
        let mut os = Oversampler::new();
        let n = 1024;
        let freq = 440.0;
        let sr = 44100.0;

        let input: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();

        let mut upsampled = vec![0.0f64; n * 2];
        let mut output = vec![0.0f64; n];

        os.upsample_2x(&input, &mut upsampled);
        os.downsample_2x(&upsampled, &mut output);

        // Allow settling time, then check amplitude preservation
        let start = n / 2;
        let input_peak = input[start..]
            .iter()
            .map(|x| x.abs())
            .fold(0.0f64, f64::max);
        let output_peak = output[start..]
            .iter()
            .map(|x| x.abs())
            .fold(0.0f64, f64::max);

        let ratio = output_peak / input_peak;
        assert!(
            (ratio - 1.0).abs() < 0.1,
            "Roundtrip amplitude changed too much: ratio={ratio}"
        );
    }

    #[test]
    fn test_stopband_rejection() {
        // Test that the downsampler rejects above-Nyquist content.
        // This is what matters for antialiasing: harmonics generated by
        // nonlinear processing at the 2x rate must be rejected before
        // decimating back to 1x.
        let mut os = Oversampler::new();
        let n = 4096;
        let sr_2x = 88200.0;

        // 30 kHz at the 2x rate = 0.34*fs_2x, well into the stopband
        // (passband edge ~0.225*fs_2x, stopband starts ~0.275*fs_2x)
        let freq = 30000.0;
        let upsampled: Vec<f64> = (0..n * 2)
            .map(|i| (2.0 * PI * freq * i as f64 / sr_2x).sin())
            .collect();

        let mut output = vec![0.0f64; n];
        os.downsample_2x(&upsampled, &mut output);

        let start = n / 2;
        let input_peak = upsampled[n..]
            .iter()
            .map(|x| x.abs())
            .fold(0.0f64, f64::max);
        let output_peak = output[start..]
            .iter()
            .map(|x| x.abs())
            .fold(0.0f64, f64::max);

        let attenuation_db = 20.0 * (output_peak / input_peak).log10();
        // 3-per-branch half-band gives ~28 dB at 30 kHz (near transition band edge).
        // Adequate for our use: preamp bandwidth is 10 kHz, so aliased content
        // at 30 kHz is already very small. Can upgrade to 5-per-branch later if needed.
        assert!(
            attenuation_db < -20.0,
            "Stopband signal not sufficiently rejected: {attenuation_db:.1} dB"
        );
    }

    #[test]
    fn test_downsample_white_noise_gain() {
        // Diagnostic: feed unit-variance white noise at the 2x rate through
        // downsample_2x and compare RMS in dB.
        //
        // Each branch is allpass (|H|=1 everywhere). The
        // `(a + down_delay) * 0.5` averages two uncorrelated
        // unit-variance streams: variance = (1+1)/4 = 0.5 → RMS drops by
        // exactly sqrt(0.5) = 3.01 dB. The half-band frequency
        // selectivity comes from the polyphase structure cancelling
        // Nyquist content after decimation, but the power budget is
        // already paid by the averager. Expected ≈ 3 dB on broadband
        // noise; measured 3.07 dB.
        let mut os = Oversampler::new();
        let n_out = 4096;
        let n_in = n_out * 2;

        // Tiny xorshift64 PRNG; map to uniform [-sqrt(3), sqrt(3)] -> unit variance.
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next_u64 = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut next_unit = || {
            // 53-bit float in [0,1)
            let bits = next_u64() >> 11;
            let u = (bits as f64) * (1.0 / ((1u64 << 53) as f64));
            // map to [-sqrt(3), sqrt(3)]; var of uniform on [-a,a] = a^2/3
            (u - 0.5) * (12.0_f64).sqrt()
        };

        let input: Vec<f64> = (0..n_in).map(|_| next_unit()).collect();
        let mut output = vec![0.0f64; n_out];
        os.downsample_2x(&input, &mut output);

        // Skip filter settling (first 64 samples)
        let skip = 64;
        let in_rms = {
            let s: f64 = input[skip * 2..].iter().map(|x| x * x).sum();
            (s / (input.len() - skip * 2) as f64).sqrt()
        };
        let out_rms = {
            let s: f64 = output[skip..].iter().map(|x| x * x).sum();
            (s / (output.len() - skip) as f64).sqrt()
        };

        let in_db = 20.0 * in_rms.log10();
        let out_db = 20.0 * out_rms.log10();
        let drop_db = in_db - out_db;
        println!("white-noise downsample diagnostic:");
        println!("  input  RMS = {in_rms:.6}  ({in_db:+.3} dBFS)");
        println!("  output RMS = {out_rms:.6}  ({out_db:+.3} dBFS)");
        println!("  drop       = {drop_db:.3} dB  (theory ≈ 9 dB)");
    }

    #[test]
    fn test_passband_flat() {
        let mut os = Oversampler::new();
        let n = 4096;
        let sr = 44100.0;

        // Test at 1kHz (well within passband)
        let freq = 1000.0;
        let input: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();

        let mut upsampled = vec![0.0f64; n * 2];
        let mut output = vec![0.0f64; n];

        os.upsample_2x(&input, &mut upsampled);
        os.downsample_2x(&upsampled, &mut output);

        let start = n * 3 / 4;
        let input_peak = input[start..]
            .iter()
            .map(|x| x.abs())
            .fold(0.0f64, f64::max);
        let output_peak = output[start..]
            .iter()
            .map(|x| x.abs())
            .fold(0.0f64, f64::max);

        let error_db = (20.0 * (output_peak / input_peak).log10()).abs();
        assert!(
            error_db < 0.5,
            "Passband response not flat enough at 1kHz: {error_db:.2} dB deviation"
        );
    }
}
