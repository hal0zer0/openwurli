/// Modal reed oscillator — 7 damped sinusoidal modes.
///
/// Each mode: A_n * sin(phase_n) * exp(-alpha_n * t)
/// where alpha_n = decay_dB_per_sec / 8.686 (convert dB/s to nepers/s).

use crate::tables::NUM_MODES;

pub struct ModalReed {
    /// Phase accumulators (radians) for each mode
    phases: [f64; NUM_MODES],
    /// Phase increment per sample for each mode (2π * f_n / sr)
    phase_incs: [f64; NUM_MODES],
    /// Initial amplitudes (after dwell filter + variation)
    amplitudes: [f64; NUM_MODES],
    /// Decay rates in nepers/sample (alpha_n / sr)
    decay_per_sample: [f64; NUM_MODES],
    /// Current sample index (for decay computation)
    sample: u64,
}

impl ModalReed {
    /// Create a new modal reed oscillator.
    ///
    /// - `fundamental_hz`: fundamental frequency after detuning
    /// - `mode_ratios`: f_n / f_1 for each mode
    /// - `amplitudes`: initial amplitude for each mode (post dwell-filter, post variation)
    /// - `decay_rates_db`: decay rate in dB/s for each mode
    /// - `sample_rate`: audio sample rate in Hz
    pub fn new(
        fundamental_hz: f64,
        mode_ratios: &[f64; NUM_MODES],
        amplitudes: &[f64; NUM_MODES],
        decay_rates_db: &[f64; NUM_MODES],
        sample_rate: f64,
    ) -> Self {
        let two_pi = 2.0 * std::f64::consts::PI;
        let mut phase_incs = [0.0f64; NUM_MODES];
        let mut decay_per_sample = [0.0f64; NUM_MODES];

        for i in 0..NUM_MODES {
            let freq = fundamental_hz * mode_ratios[i];
            phase_incs[i] = two_pi * freq / sample_rate;
            // Convert dB/s to nepers/s, then to nepers/sample
            let alpha_nepers = decay_rates_db[i] / 8.686;
            decay_per_sample[i] = alpha_nepers / sample_rate;
        }

        Self {
            phases: [0.0; NUM_MODES],
            phase_incs,
            amplitudes: *amplitudes,
            decay_per_sample,
            sample: 0,
        }
    }

    /// Render samples into the output buffer (additive, does NOT clear buffer).
    pub fn render(&mut self, output: &mut [f64]) {
        for sample in output.iter_mut() {
            let mut sum = 0.0f64;
            let n = self.sample as f64;

            for i in 0..NUM_MODES {
                let decay = (-self.decay_per_sample[i] * n).exp();
                sum += self.amplitudes[i] * self.phases[i].sin() * decay;
                self.phases[i] += self.phase_incs[i];
            }

            // Wrap phases to avoid precision loss at large sample counts
            if self.sample & 0xFFFF == 0 {
                let two_pi = 2.0 * std::f64::consts::PI;
                for p in &mut self.phases {
                    *p %= two_pi;
                }
            }

            *sample += sum;
            self.sample += 1;
        }
    }

    /// Check if the reed has decayed below a threshold (all modes).
    pub fn is_silent(&self, threshold_db: f64) -> bool {
        let n = self.sample as f64;
        let threshold_linear = f64::powf(10.0, threshold_db / 20.0);
        for i in 0..NUM_MODES {
            let envelope = self.amplitudes[i] * (-self.decay_per_sample[i] * n).exp();
            if envelope.abs() > threshold_linear {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_mode_sine() {
        // Single mode at 440 Hz, no decay
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES]; // no decay

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, 44100.0);
        let mut buf = vec![0.0f64; 44100];
        reed.render(&mut buf);

        // Check that output has correct frequency — find zero crossings
        let mut crossings = 0u32;
        for i in 1..buf.len() {
            if buf[i - 1] < 0.0 && buf[i] >= 0.0 {
                crossings += 1;
            }
        }
        // 440 Hz = 440 positive zero crossings per second
        assert!((crossings as f64 - 440.0).abs() < 2.0);
    }

    #[test]
    fn test_decay() {
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let mut decays = [0.0f64; NUM_MODES];
        decays[0] = 60.0; // 60 dB/s → T60 = 1 second

        let sr = 44100.0;
        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, sr);

        // Render 0.5s
        let half_sec = (sr * 0.5) as usize;
        let mut buf = vec![0.0f64; half_sec];
        reed.render(&mut buf);

        // After 0.5s at 60 dB/s, should be down ~30 dB → amplitude ~0.032
        let peak = buf[buf.len() - 200..].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(peak < 0.1, "expected decay, got peak {peak}");
        assert!(peak > 0.01, "decayed too much, got peak {peak}");
    }
}
