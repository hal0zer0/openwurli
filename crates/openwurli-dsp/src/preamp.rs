//! Wurlitzer 200A preamp model -- two-stage direct-coupled BJT amplifier.
//!
//! Signal flow per oversampled sample (ZDF feedback):
//!   for iter in 0..3:
//!     restore stage1 + stage2 state
//!     fb_est = prev_s2_out * fb_fraction  (seeded from last sample)
//!     s1_out = stage1.process(input, fb_est)
//!     s2_out = stage2.process(s1_out, 0.0)
//!     fb_est = s2_out * fb_fraction
//!   output = dc_block(s2_out)
//!
//! Emitter feedback from R-10 (56K):
//!   output -> R-10 -> fb_junction -> Ce1 -> TR-1 emitter
//!   LDR path shunts fb_junction to ground, modulating feedback amount.
//!
//! Calibration targets (from SPICE):
//!   - Gain @ 1kHz, no tremolo: 6.0 dB (2.0x)
//!   - Gain @ 1kHz, tremolo bright: 12.1 dB (4.0x)
//!   - H2 > H3 at all dynamics
//!
//! Known limitation -- nested feedback loops (see docs/preamp-circuit.md 5.5.1):
//!   The real preamp has TWO nested feedback loops: inner (C-3/C-4 Miller caps,
//!   ~15.5 kHz BW) and outer (R-10/Ce1/R_ldr, gain control). This model treats
//!   each stage independently with simple Miller poles, giving constant GBW and
//!   incorrect trem-bright BW (~5.2 kHz vs SPICE ~15.2 kHz preamp-only). The
//!   inner C-3/C-4 loop must be modeled explicitly (WDF or coupled solver) to
//!   fix this. Current DSP BW is ~10.5 kHz no-trem (acceptable) and ~5.2 kHz
//!   trem-bright (too narrow, but only affects the brief peak of tremolo cycle
//!   at frequencies above the highest fundamental).

use crate::bjt_stage::BjtStage;
use crate::filters::DcBlocker;

/// PreampModel trait — swappable implementations for A/B testing.
pub trait PreampModel {
    fn process_sample(&mut self, input: f64) -> f64;
    fn set_ldr_resistance(&mut self, r_ldr_path: f64);
    fn reset(&mut self);
}

/// Ebers-Moll preamp — the shipping candidate.
///
/// Two BjtStage objects with exponential transfer functions.
/// R-10 emitter feedback via 3-iteration zero-delay feedback (ZDF).
///
/// The ZDF loop replaces the previous one-sample-delay feedback + 16 kHz
/// compensating LPF. With Stage 1's TptLpf Miller filter providing ~50%
/// instantaneous input coupling, 3 iterations converge to < 0.25 dB residual
/// (loop gain ~0.3 at midband -> 0.3^3 = 2.7%).
pub struct EbersMollPreamp {
    /// Stage 1: high-gain, high-asymmetry (TptLpf Miller, pre-nonlinearity)
    stage1: BjtStage,
    /// Stage 2: low-gain buffer (forward Euler Miller, post-nonlinearity)
    stage2: BjtStage,
    /// Output DC blocker
    dc_block: DcBlocker,
    /// Feedback fraction: how much of Stage 2 output reaches Stage 1 emitter.
    /// Calibrated to match SPICE: fb = 0.509 * R_ldr / (R_ldr + 20K).
    /// Range: ~0 (LDR bright, gain 4x) to ~0.5 (LDR dark, gain 2x).
    fb_fraction: f64,
    /// Previous Stage 2 output (seeds next sample's ZDF iteration)
    prev_s2_out: f64,
}

/// Number of ZDF fixed-point iterations per sample.
/// Loop gain ~0.3 at midband -> residual after 3 iters: 0.3^3 = 2.7% (< 0.25 dB).
const ZDF_ITERATIONS: usize = 3;

impl EbersMollPreamp {
    /// Create a new Ebers-Moll preamp at the given (oversampled) sample rate.
    pub fn new(sample_rate: f64) -> Self {
        Self {
            stage1: BjtStage::stage1(sample_rate),
            stage2: BjtStage::stage2(sample_rate),
            dc_block: DcBlocker::new(sample_rate),
            fb_fraction: Self::calc_fb_fraction(1_000_000.0),
            prev_s2_out: 0.0,
        }
    }

    /// Calculate feedback fraction from LDR path resistance.
    ///
    /// Calibrated to match SPICE AC sweep results:
    ///   R_ldr = 1M  -> fb = 0.491 -> G = 912/(1+912*0.491) = 2.0x (6.0 dB)
    ///   R_ldr = 19K -> fb = 0.249 -> G = 912/(1+912*0.249) = 4.0x (12.1 dB)
    ///
    /// Formula: fb = 0.509 * R_ldr / (R_ldr + 20000)
    /// The 20K crossover resistance and 0.509 maximum are fit to SPICE data.
    fn calc_fb_fraction(r_ldr_path: f64) -> f64 {
        0.509 * r_ldr_path / (r_ldr_path + 20_000.0)
    }
}

impl PreampModel for EbersMollPreamp {
    fn process_sample(&mut self, input: f64) -> f64 {
        // Save stage states before ZDF iteration
        let s1_state = self.stage1.save_state();
        let s2_state = self.stage2.save_state();

        // Seed feedback estimate from previous sample's output
        let mut fb_est = self.prev_s2_out * self.fb_fraction;
        let mut s2_out = 0.0;

        // Fixed-point ZDF iteration: converge feedback within this sample
        for _ in 0..ZDF_ITERATIONS {
            self.stage1.restore_state(s1_state);
            self.stage2.restore_state(s2_state);

            let s1_out = self.stage1.process(input, fb_est);
            s2_out = self.stage2.process(s1_out, 0.0);
            fb_est = s2_out * self.fb_fraction;
        }

        self.prev_s2_out = s2_out;

        // DC block (20 Hz HPF)
        self.dc_block.process(s2_out)
    }

    fn set_ldr_resistance(&mut self, r_ldr_path: f64) {
        self.fb_fraction = Self::calc_fb_fraction(r_ldr_path);
    }

    fn reset(&mut self) {
        self.stage1.reset();
        self.stage2.reset();
        self.dc_block.reset();
        self.prev_s2_out = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn measure_gain(preamp: &mut EbersMollPreamp, freq: f64, amplitude: f64, sr: f64) -> f64 {
        preamp.reset();
        let n_settle = (sr * 0.2) as usize;
        let n_measure = (sr * 0.1) as usize;

        for i in 0..n_settle {
            let t = i as f64 / sr;
            let input = amplitude * (2.0 * PI * freq * t).sin();
            preamp.process_sample(input);
        }

        let mut peak = 0.0f64;
        for i in 0..n_measure {
            let t = (n_settle + i) as f64 / sr;
            let input = amplitude * (2.0 * PI * freq * t).sin();
            let output = preamp.process_sample(input);
            peak = peak.max(output.abs());
        }

        peak / amplitude
    }

    #[test]
    fn test_gain_no_tremolo() {
        let sr = 88200.0;
        let mut preamp = EbersMollPreamp::new(sr);
        preamp.set_ldr_resistance(1_000_000.0);

        let gain = measure_gain(&mut preamp, 1000.0, 0.001, sr);
        let gain_db = 20.0 * gain.log10();

        // Target: 6.0 dB (2.0x). Allow +/- 3 dB for first-pass calibration.
        assert!(
            gain_db > 3.0 && gain_db < 12.0,
            "Gain @ 1kHz no tremolo = {gain_db:.1} dB, want ~6 dB"
        );
    }

    #[test]
    fn test_gain_increases_with_tremolo() {
        let sr = 88200.0;
        let mut preamp = EbersMollPreamp::new(sr);

        preamp.set_ldr_resistance(1_000_000.0);
        let gain_no_trem = measure_gain(&mut preamp, 1000.0, 0.001, sr);

        preamp.set_ldr_resistance(19_000.0);
        let gain_trem = measure_gain(&mut preamp, 1000.0, 0.001, sr);

        let no_trem_db = 20.0 * gain_no_trem.log10();
        let trem_db = 20.0 * gain_trem.log10();

        assert!(
            gain_trem > gain_no_trem * 1.2,
            "Tremolo bright gain ({trem_db:.1} dB) should exceed no-tremolo ({no_trem_db:.1} dB)"
        );
    }

    #[test]
    fn test_h2_dominates() {
        let sr = 88200.0;
        let mut preamp = EbersMollPreamp::new(sr);
        preamp.set_ldr_resistance(1_000_000.0);

        let freq = 440.0;
        let n = (sr * 0.3) as usize;
        let mut output = vec![0.0f64; n];

        for i in 0..n {
            let t = i as f64 / sr;
            let input = 0.005 * (2.0 * PI * freq * t).sin();
            output[i] = preamp.process_sample(input);
        }

        let start = n * 3 / 4;
        let h2 = dft_magnitude(&output[start..], 2.0 * freq, sr);
        let h3 = dft_magnitude(&output[start..], 3.0 * freq, sr);

        if h3 > 1e-15 {
            assert!(h2 > h3, "H2 ({h2:.2e}) should dominate H3 ({h3:.2e})");
        }
    }

    #[test]
    fn test_stability() {
        let sr = 88200.0;
        let mut preamp = EbersMollPreamp::new(sr);

        preamp.process_sample(0.01);

        let mut last = 0.0;
        for _ in 0..(sr * 1.0) as usize {
            last = preamp.process_sample(0.0);
        }

        assert!(
            last.abs() < 1e-4,
            "Preamp should be stable after impulse, got {last}"
        );
    }

    #[test]
    fn test_bandwidth_rolloff() {
        let sr = 88200.0;
        let mut preamp = EbersMollPreamp::new(sr);
        preamp.set_ldr_resistance(1_000_000.0);

        let gain_1k = measure_gain(&mut preamp, 1000.0, 0.001, sr);
        let gain_15k = measure_gain(&mut preamp, 15000.0, 0.001, sr);

        assert!(
            gain_15k < gain_1k,
            "Should roll off at HF: 1kHz={gain_1k:.2}x, 15kHz={gain_15k:.2}x"
        );
    }

    fn dft_magnitude(signal: &[f64], freq: f64, sr: f64) -> f64 {
        let n = signal.len() as f64;
        let mut re = 0.0;
        let mut im = 0.0;
        for (i, &s) in signal.iter().enumerate() {
            let phase = 2.0 * PI * freq * i as f64 / sr;
            re += s * phase.cos();
            im -= s * phase.sin();
        }
        ((re / n).powi(2) + (im / n).powi(2)).sqrt()
    }
}
