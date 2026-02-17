/// Modal reed oscillator — 7 damped sinusoidal modes.
///
/// Each mode: A_n * sin(phase_n) * exp(-alpha_n * t)
/// where alpha_n = decay_dB_per_sec / 8.686 (convert dB/s to nepers/s).
///
/// Per-mode frequency jitter (Ornstein-Uhlenbeck process) breaks the perfect
/// phase coherence of digital oscillators. Real reeds have nonlinear frequency-
/// amplitude coupling (backbone curve), pickup loading, and micro-turbulence
/// that cause each mode's frequency to wander slightly. Without this, the
/// static spectral interference pattern sounds "metallic" and "resonant."

use crate::tables::NUM_MODES;

/// RMS frequency jitter as fraction of mode frequency (~0.04% = 4 cents peak).
const JITTER_SIGMA: f64 = 0.0004;

/// OU correlation time in seconds (~20ms). Controls how fast modes drift
/// relative to each other — long enough for perceptible beating, short enough
/// to evolve within a note's sustain.
const JITTER_TAU: f64 = 0.020;

pub struct ModalReed {
    phases: [f64; NUM_MODES],
    phase_incs: [f64; NUM_MODES],
    amplitudes: [f64; NUM_MODES],
    decay_per_sample: [f64; NUM_MODES],
    sample: u64,
    // Per-mode onset ramp: raised cosine during hammer contact period.
    // Models the finite hammer dwell — reed displacement builds up smoothly
    // rather than jumping to full amplitude. Higher modes have longer ramps
    // because soft felt couples less efficiently to high-frequency modes.
    // Mode phases advance during the ramp → dwell-dependent phase scatter.
    onset_ramp_samples: [u64; NUM_MODES],
    onset_ramp_inc: [f64; NUM_MODES],
    // Damper state
    damper_active: bool,
    damper_rates: [f64; NUM_MODES],
    damper_ramp_samples: f64,
    damper_release_count: f64,
    damper_integral: [f64; NUM_MODES],
    // Per-mode Ornstein-Uhlenbeck frequency jitter
    jitter_state: u32,
    jitter_drift: [f64; NUM_MODES],
    jitter_revert: f64,   // exp(-dt/tau): mean-reversion per sample
    jitter_diffusion: f64, // noise scaling per sample
    // Hammer impact overshoot: reed tip displacement peaks above steady-state
    // during the first few cycles after hammer contact, then settles.
    // The 1/(1-y) pickup nonlinearity amplifies this → bark peaks at attack.
    impact_overshoot: f64,  // current overshoot level (decays per sample)
    impact_decay: f64,      // per-sample decay factor: exp(-1/(tau*fs))
}

impl ModalReed {
    /// Create a new modal reed oscillator.
    ///
    /// - `fundamental_hz`: fundamental frequency after detuning
    /// - `mode_ratios`: f_n / f_1 for each mode
    /// - `amplitudes`: initial amplitude for each mode (post dwell-filter, post variation)
    /// - `decay_rates_db`: decay rate in dB/s for each mode
    /// - `dwell_time_s`: hammer contact duration in seconds (0.0 = instantaneous)
    /// - `sample_rate`: audio sample rate in Hz
    /// - `jitter_seed`: RNG seed for per-mode frequency jitter (decorrelates voices)
    pub fn new(
        fundamental_hz: f64,
        mode_ratios: &[f64; NUM_MODES],
        amplitudes: &[f64; NUM_MODES],
        decay_rates_db: &[f64; NUM_MODES],
        dwell_time_s: f64,
        sample_rate: f64,
        jitter_seed: u32,
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

        // Ornstein-Uhlenbeck jitter coefficients:
        //   dx = -x/tau * dt + sigma * sqrt(2/tau) * dW
        // Discretized per sample:
        //   revert = exp(-dt/tau)
        //   diffusion = sigma * sqrt(1 - revert^2)  [exact discrete OU variance]
        let dt = 1.0 / sample_rate;
        let jitter_revert = (-dt / JITTER_TAU).exp();
        let jitter_diffusion = JITTER_SIGMA * (1.0 - jitter_revert * jitter_revert).sqrt();

        // Per-mode onset ramp: raised cosine over a mode-dependent period.
        // e_n(t) = 0.5 * (1 - cos(pi * t / T_n)) for t < T_n, then 1.0.
        //
        // Two mechanisms set each mode's ramp time:
        //   1. Dwell-based: T_dwell * ratio_n^0.25 — soft felt couples less to higher modes
        //   2. Mechanical ring-up: dwell_time_s itself (passed from voice.rs as
        //      max(hammer_dwell, N_cycles/f0)) — minimum time for ALL modes
        //
        // For bass notes, the mechanical ring-up dominates and all modes get similar
        // ramp times (no vibraphone effect). For treble, the dwell-based spread
        // may give higher modes slightly longer ramps.
        let mut onset_ramp_samples = [0u64; NUM_MODES];
        let mut onset_ramp_inc = [0.0f64; NUM_MODES];
        for i in 0..NUM_MODES {
            let dwell_based = dwell_time_s * mode_ratios[i].powf(0.25);
            let mode_ramp = dwell_based.max(dwell_time_s);
            let samps = (mode_ramp * sample_rate).round() as u64;
            onset_ramp_samples[i] = samps;
            onset_ramp_inc[i] = if samps > 0 {
                std::f64::consts::PI / samps as f64
            } else {
                0.0
            };
        }

        // Initialize jitter_drift from the OU stationary distribution N(0, JITTER_SIGMA).
        // This eliminates the ~60ms warm-up period (3*tau) — phase decorrelation is
        // immediate from sample 0. Each note starts with modes already slightly detuned,
        // just like a real reed whose frequency-amplitude coupling is always active.
        let mut jitter_state = jitter_seed.max(1);
        let mut jitter_drift = [0.0f64; NUM_MODES];
        for d in &mut jitter_drift {
            // Inline LCG + Box-Muller to generate initial draws
            jitter_state = jitter_state.wrapping_mul(1664525).wrapping_add(1013904223);
            let u1 = (jitter_state >> 1) as f64 / (u32::MAX as f64 / 2.0);
            jitter_state = jitter_state.wrapping_mul(1664525).wrapping_add(1013904223);
            let u2 = (jitter_state >> 1) as f64 / (u32::MAX as f64 / 2.0);
            let r = (-2.0 * u1.max(1e-30).ln()).sqrt();
            *d = JITTER_SIGMA * r * (2.0 * std::f64::consts::PI * u2).cos();
        }

        Self {
            phases: [0.0; NUM_MODES],
            phase_incs,
            amplitudes: *amplitudes,
            decay_per_sample,
            sample: 0,
            onset_ramp_samples,
            onset_ramp_inc,
            damper_active: false,
            damper_rates: [0.0; NUM_MODES],
            damper_ramp_samples: 0.0,
            damper_release_count: 0.0,
            damper_integral: [0.0; NUM_MODES],
            jitter_state,
            jitter_drift,
            jitter_revert,
            jitter_diffusion,
            impact_overshoot: 0.0,
            impact_decay: 1.0,
        }
    }

    /// Set hammer impact overshoot envelope.
    ///
    /// Models the transient peak displacement when the hammer rebounds from the
    /// reed. The reed tip deflects past its steady-state vibration amplitude
    /// during the first few cycles, then settles. Because 1/(1-y) is nonlinear,
    /// this overshoot produces disproportionately more H2 during attack.
    ///
    /// - `amount`: overshoot ratio (0.0 = none, 0.8 = 1.8× peak displacement)
    /// - `tau_s`: decay time constant in seconds (~8ms = a few fundamental cycles)
    /// - `sample_rate`: audio sample rate
    pub fn set_impact_overshoot(&mut self, amount: f64, tau_s: f64, sample_rate: f64) {
        self.impact_overshoot = amount;
        self.impact_decay = (-1.0 / (tau_s * sample_rate)).exp();
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

        let base_rate = (55.0 * 2.0_f64.powf((midi_note as f64 - 60.0) / 24.0)).max(0.5);
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
    // PERF: modes 5-7 are inaudible above ~MIDI 80; could skip them for high notes
    pub fn render(&mut self, output: &mut [f64]) {
        let revert = self.jitter_revert;
        let diffusion = self.jitter_diffusion;

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
                // Ornstein-Uhlenbeck jitter: mean-reverting random walk on frequency
                // drift[i] is the fractional frequency deviation (e.g. 0.0004 = 0.04%)
                let noise = self.lcg_normal();
                self.jitter_drift[i] = revert * self.jitter_drift[i] + diffusion * noise;

                // Per-mode onset ramp: fundamental arrives first, higher modes follow.
                let onset = if self.sample < self.onset_ramp_samples[i] {
                    0.5 * (1.0 - (n * self.onset_ramp_inc[i]).cos())
                } else {
                    1.0
                };

                let total_decay = -self.decay_per_sample[i] * n - self.damper_integral[i];
                sum += self.amplitudes[i] * self.phases[i].sin() * onset * total_decay.exp();
                self.phases[i] += self.phase_incs[i] * (1.0 + self.jitter_drift[i]);
            }

            if self.sample & 0x3FF == 0 {
                let two_pi = 2.0 * std::f64::consts::PI;
                for p in &mut self.phases {
                    *p %= two_pi;
                }
            }

            // Apply hammer impact overshoot: reed displacement peaks above
            // steady-state during first few ms, boosting attack bark.
            let overshoot_env = 1.0 + self.impact_overshoot;
            self.impact_overshoot *= self.impact_decay;

            *sample += sum * overshoot_env;
            self.sample += 1;
        }
    }

    /// LCG PRNG → approximate standard normal via Box-Muller-like transform.
    /// Uses two uniform samples to produce one normal sample. Fast, no branching.
    #[inline]
    fn lcg_normal(&mut self) -> f64 {
        // LCG step (Numerical Recipes constants)
        self.jitter_state = self.jitter_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let u1 = (self.jitter_state >> 1) as f64 / (u32::MAX as f64 / 2.0); // (0, 1)
        self.jitter_state = self.jitter_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let u2 = (self.jitter_state >> 1) as f64 / (u32::MAX as f64 / 2.0);
        // Box-Muller: only use one of the two outputs for simplicity
        let r = (-2.0 * u1.max(1e-30).ln()).sqrt();
        r * (2.0 * std::f64::consts::PI * u2).cos()
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

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, 44100.0, 42);
        let mut buf = vec![0.0f64; 44100];
        reed.render(&mut buf);

        // With 0.04% jitter, frequency should still be within ~1 Hz of 440
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
        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, sr, 42);

        let half_sec = (sr * 0.5) as usize;
        let mut buf = vec![0.0f64; half_sec];
        reed.render(&mut buf);

        let peak = buf[buf.len() - 200..].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(peak < 0.1, "expected decay, got peak {peak}");
        assert!(peak > 0.01, "decayed too much, got peak {peak}");
    }

    #[test]
    fn test_onset_ramp_shapes_attack() {
        // With a 20ms ramp (bass ring-up), the onset should build up smoothly.
        // First few samples should be near zero, reaching ~full amplitude after ramp.
        let sr = 44100.0;
        let ramp = 0.020; // 20ms — typical bass ring-up from voice.rs
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, ramp, sr, 42);
        let n = (sr * 0.050) as usize;
        let mut buf = vec![0.0f64; n];
        reed.render(&mut buf);

        // First sample should be near zero (raised cosine starts at 0)
        assert!(buf[0].abs() < 0.01,
            "First sample should be near zero, got {:.6}", buf[0]);

        // At half the ramp time, the ramp should be at ~0.5
        let mid_ramp = (ramp * 0.5 * sr) as usize;
        let mid_peak = buf[mid_ramp.saturating_sub(5)..mid_ramp + 5].iter()
            .map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(mid_peak < 0.8,
            "Mid-ramp peak should be < 0.8, got {mid_peak:.4}");

        // Well after ramp, amplitude should be ~1.0
        let late_start = (sr * 0.030) as usize;
        let late_peak = buf[late_start..late_start + 200].iter()
            .map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(late_peak > 0.85,
            "Post-ramp peak should be ~1.0, got {late_peak:.4}");
    }

    #[test]
    fn test_onset_ramp_ff_vs_pp() {
        // ff (short ramp) should reach full amplitude faster than pp (long ramp)
        let sr = 44100.0;
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        // At 440 Hz, ring-up = 3/440 = 6.8ms. ff dwell = 0.5ms, pp dwell = 2.5ms.
        // voice.rs passes max(dwell, ring_up), so both get 6.8ms here.
        // To test ff vs pp, use high frequency where ring_up < dwell.
        let hi_freq = 5000.0; // ring_up = 3/5000 = 0.6ms
        let mut reed_ff = ModalReed::new(hi_freq, &ratios, &amps, &decays, 0.001, sr, 42);
        let mut reed_pp = ModalReed::new(hi_freq, &ratios, &amps, &decays, 0.005, sr, 42);

        let n = (sr * 0.010) as usize; // 10ms window
        let mut buf_ff = vec![0.0f64; n];
        let mut buf_pp = vec![0.0f64; n];
        reed_ff.render(&mut buf_ff);
        reed_pp.render(&mut buf_pp);

        // At 2ms, ff should be louder than pp
        let t2ms = (sr * 0.002) as usize;
        let ff_energy: f64 = buf_ff[..t2ms].iter().map(|x| x * x).sum();
        let pp_energy: f64 = buf_pp[..t2ms].iter().map(|x| x * x).sum();
        assert!(ff_energy > pp_energy * 1.5,
            "ff should be louder than pp at 2ms: ff={ff_energy:.6}, pp={pp_energy:.6}");
    }

    #[test]
    fn test_onset_zero_dwell_is_instant() {
        // dwell_time=0.0 should produce full amplitude from sample 0
        let sr = 44100.0;
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, sr, 42);
        let mut buf = vec![0.0f64; 100];
        reed.render(&mut buf);

        // With dwell=0, the only ramp is from mode_ratio^0.25 which for mode 0
        // (ratio=1.0) gives ramp=0 → immediate amplitude
        let early_peak = buf[..10].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(early_peak > 0.05,
            "Zero dwell should give immediate amplitude, got {early_peak:.6}");
    }

    #[test]
    fn test_jitter_breaks_phase_coherence() {
        // Two reeds with the same parameters but different seeds should produce
        // different output — the jitter decorrelates them.
        let sr = 44100.0;
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        amps[1] = 0.3;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed_a = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, sr, 100);
        let mut reed_b = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, sr, 200);

        let n = (sr * 0.5) as usize;
        let mut buf_a = vec![0.0f64; n];
        let mut buf_b = vec![0.0f64; n];
        reed_a.render(&mut buf_a);
        reed_b.render(&mut buf_b);

        // Compute RMS difference in the last 0.3s (after jitter has had time to diverge)
        let late_start = (sr * 0.2) as usize;
        let mut diff_sq = 0.0;
        let mut sig_sq = 0.0;
        for i in late_start..n {
            diff_sq += (buf_a[i] - buf_b[i]).powi(2);
            sig_sq += buf_a[i].powi(2);
        }
        let rms_diff = (diff_sq / (n - late_start) as f64).sqrt();
        let rms_sig = (sig_sq / (n - late_start) as f64).sqrt();

        // With 0.04% jitter over 20ms correlation, outputs should measurably differ
        // but not be wildly different
        let relative_diff = rms_diff / rms_sig.max(1e-10);
        assert!(relative_diff > 0.001,
            "Jitter should cause measurable difference: relative_diff={relative_diff:.6}");
        assert!(relative_diff < 0.5,
            "Jitter should be subtle, not overwhelming: relative_diff={relative_diff:.4}");
    }

    #[test]
    fn test_jitter_deterministic_with_same_seed() {
        // Same seed → same output (deterministic PRNG)
        let sr = 44100.0;
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed_a = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, sr, 42);
        let mut reed_b = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, sr, 42);

        let n = (sr * 0.2) as usize;
        let mut buf_a = vec![0.0f64; n];
        let mut buf_b = vec![0.0f64; n];
        reed_a.render(&mut buf_a);
        reed_b.render(&mut buf_b);

        assert_eq!(buf_a, buf_b, "Same seed should produce identical output");
    }

    #[test]
    fn test_jitter_preserves_frequency() {
        // Even with jitter, the average frequency over 1 second should be very
        // close to the nominal frequency (OU process is zero-mean).
        let sr = 44100.0;
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, sr, 77);
        let n = sr as usize; // 1 second
        let mut buf = vec![0.0f64; n];
        reed.render(&mut buf);

        let mut crossings = 0u32;
        for i in 1..buf.len() {
            if buf[i - 1] < 0.0 && buf[i] >= 0.0 {
                crossings += 1;
            }
        }
        // 0.04% jitter → frequency within ~0.2 Hz of 440
        assert!((crossings as f64 - 440.0).abs() < 1.0,
            "Average frequency should be ~440 Hz, got {crossings} crossings");
    }

}
