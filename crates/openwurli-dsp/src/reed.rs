/// Modal reed oscillator — 7 damped sinusoidal modes.
///
/// Each mode: A_n * sin(phase_n) * exp(-alpha_n * t)
/// where alpha_n = decay_dB_per_sec / 8.686 (convert dB/s to nepers/s).

use crate::tables::NUM_MODES;

pub struct ModalReed {
    phases: [f64; NUM_MODES],
    phase_incs: [f64; NUM_MODES],
    amplitudes: [f64; NUM_MODES],
    decay_per_sample: [f64; NUM_MODES],
    sample: u64,
    // Damper state
    damper_active: bool,
    damper_rates: [f64; NUM_MODES],
    damper_ramp_samples: f64,
    damper_release_count: f64,
    damper_integral: [f64; NUM_MODES],
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
            let alpha_nepers = decay_rates_db[i] / 8.686;
            decay_per_sample[i] = alpha_nepers / sample_rate;
        }

        Self {
            phases: [0.0; NUM_MODES],
            phase_incs,
            amplitudes: *amplitudes,
            decay_per_sample,
            sample: 0,
            damper_active: false,
            damper_rates: [0.0; NUM_MODES],
            damper_ramp_samples: 0.0,
            damper_release_count: 0.0,
            damper_integral: [0.0; NUM_MODES],
        }
    }

    /// Start the damper (called on note_off).
    ///
    /// Three-phase progressive model: felt progressively contacts the reed,
    /// with higher modes damped more aggressively.
    ///
    /// - `midi_note`: for register-dependent ramp time
    /// - `sample_rate`: for time constant conversion
    pub fn start_damper(&mut self, midi_note: u8, sample_rate: f64) {
        // Top 5 keys: no damper (natural decay only)
        if midi_note >= 92 {
            return;
        }

        let base_rate = 55.0 * 2.0_f64.powf((midi_note as f64 - 60.0) / 24.0).max(0.5);
        for m in 0..NUM_MODES {
            let factor = (base_rate * 3.0_f64.powi(m as i32)).min(2000.0);
            // Convert nepers/sec to nepers/sample
            self.damper_rates[m] = factor / sample_rate;
        }

        // Register-dependent ramp time
        let ramp_time = if midi_note < 48 {
            0.050 // Bass: 50ms
        } else if midi_note < 72 {
            0.025 // Mid: 25ms
        } else {
            0.008 // Treble: 8ms
        };

        self.damper_ramp_samples = ramp_time * sample_rate;
        self.damper_active = true;
        self.damper_release_count = 0.0;
        self.damper_integral = [0.0; NUM_MODES];
    }

    /// Render samples into the output buffer (additive, does NOT clear buffer).
    pub fn render(&mut self, output: &mut [f64]) {
        for sample in output.iter_mut() {
            let mut sum = 0.0f64;
            let n = self.sample as f64;

            // Advance damper if active
            if self.damper_active {
                self.damper_release_count += 1.0;
                let t = self.damper_release_count;
                let ramp = self.damper_ramp_samples;
                for m in 0..NUM_MODES {
                    let inst_rate = if t <= ramp {
                        self.damper_rates[m] * t / ramp
                    } else {
                        self.damper_rates[m]
                    };
                    self.damper_integral[m] += inst_rate;
                }
            }

            for i in 0..NUM_MODES {
                let total_decay = -self.decay_per_sample[i] * n - self.damper_integral[i];
                sum += self.amplitudes[i] * self.phases[i].sin() * total_decay.exp();
                self.phases[i] += self.phase_incs[i];
            }

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
            let total_decay = -self.decay_per_sample[i] * n - self.damper_integral[i];
            let envelope = self.amplitudes[i] * total_decay.exp();
            if envelope.abs() > threshold_linear {
                return false;
            }
        }
        true
    }

    /// Check if damper is active.
    pub fn is_damping(&self) -> bool {
        self.damper_active
    }

    /// Get release time in seconds (for safety timeout).
    pub fn release_seconds(&self, sample_rate: f64) -> f64 {
        if self.damper_active {
            self.damper_release_count / sample_rate
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_mode_sine() {
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, 44100.0);
        let mut buf = vec![0.0f64; 44100];
        reed.render(&mut buf);

        let mut crossings = 0u32;
        for i in 1..buf.len() {
            if buf[i - 1] < 0.0 && buf[i] >= 0.0 {
                crossings += 1;
            }
        }
        assert!((crossings as f64 - 440.0).abs() < 2.0);
    }

    #[test]
    fn test_decay() {
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let mut decays = [0.0f64; NUM_MODES];
        decays[0] = 60.0;

        let sr = 44100.0;
        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, sr);

        let half_sec = (sr * 0.5) as usize;
        let mut buf = vec![0.0f64; half_sec];
        reed.render(&mut buf);

        let peak = buf[buf.len() - 200..].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(peak < 0.1, "expected decay, got peak {peak}");
        assert!(peak > 0.01, "decayed too much, got peak {peak}");
    }
}
