//! Modal reed oscillator — 7 damped sinusoidal modes.
//!
//! Each mode: A_n * sin(phase_n) * exp(-alpha_n * t)
//! where alpha_n = decay_dB_per_sec / 8.686 (convert dB/s to nepers/s).
//!
//! Per-mode frequency jitter (Ornstein-Uhlenbeck process) breaks the perfect
//! phase coherence of digital oscillators. Real reeds have nonlinear frequency-
//! amplitude coupling (backbone curve), pickup loading, and micro-turbulence
//! that cause each mode's frequency to wander slightly. Without this, the
//! static spectral interference pattern sounds "metallic" and "resonant."
#![allow(clippy::needless_range_loop)]

use crate::tables::NUM_MODES;

/// RMS frequency jitter as fraction of mode frequency (~0.04% = 4 cents peak).
const JITTER_SIGMA: f64 = 0.0004;

/// OU correlation time in seconds (~20ms). Controls how fast modes drift
/// relative to each other — long enough for perceptible beating, short enough
/// to evolve within a note's sustain.
const JITTER_TAU: f64 = 0.020;

/// sqrt(3): scaling for uniform[-1,1] to achieve unit variance.
/// Uniform(-√3, √3) has variance 1.0, matching the Gaussian's variance.
const SQRT_3: f64 = 1.732_050_808_0;

pub struct ModalReed {
    phases: [f64; NUM_MODES],
    phase_incs: [f64; NUM_MODES],
    amplitudes: [f64; NUM_MODES],
    // Multiplicative decay: envelope[i] *= decay_mult[i] each sample.
    // Equivalent to exp(-α·n) but avoids per-sample transcendental.
    decay_mult: [f64; NUM_MODES],
    envelope: [f64; NUM_MODES],
    sample: u64,
    // Onset ramp: raised cosine during hammer contact period.
    // Models the finite hammer dwell — reed displacement builds up smoothly
    // rather than jumping to full amplitude. All modes ramp together.
    onset_ramp_samples: u64,
    onset_ramp_inc: f64,
    // Velocity-dependent ramp shape: pp uses softer (Hann-squared-like) onset,
    // ff uses standard raised cosine. Models progressive felt compression.
    onset_shape_exp: f64,
    // Damper state
    damper_active: bool,
    damper_rates: [f64; NUM_MODES],
    damper_ramp_samples: f64,
    damper_release_count: f64,
    // Multiplicative damper: post-ramp, envelope *= damper_mult each sample
    damper_mult: [f64; NUM_MODES],
    damper_ramp_done: bool,
    // Per-mode Ornstein-Uhlenbeck frequency jitter
    jitter_state: u32,
    jitter_drift: [f64; NUM_MODES],
    jitter_revert: f64,    // exp(-dt/tau): mean-reversion per sample
    jitter_diffusion: f64, // noise scaling per sample
}

impl ModalReed {
    /// Create a new modal reed oscillator.
    ///
    /// - `fundamental_hz`: fundamental frequency after detuning
    /// - `mode_ratios`: f_n / f_1 for each mode
    /// - `amplitudes`: initial amplitude for each mode (post dwell-filter, post variation)
    /// - `decay_rates_db`: decay rate in dB/s for each mode
    /// - `onset_time_s`: onset ramp duration in seconds (reed mechanical inertia)
    /// - `velocity`: 0.0 (pp) to 1.0 (ff), controls onset ramp shape
    /// - `sample_rate`: audio sample rate in Hz
    /// - `jitter_seed`: RNG seed for per-mode frequency jitter (decorrelates voices)
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        fundamental_hz: f64,
        mode_ratios: &[f64; NUM_MODES],
        amplitudes: &[f64; NUM_MODES],
        decay_rates_db: &[f64; NUM_MODES],
        onset_time_s: f64,
        velocity: f64,
        sample_rate: f64,
        jitter_seed: u32,
    ) -> Self {
        let two_pi = 2.0 * std::f64::consts::PI;
        let mut phase_incs = [0.0f64; NUM_MODES];
        let mut decay_mult = [0.0f64; NUM_MODES];

        for i in 0..NUM_MODES {
            let freq = fundamental_hz * mode_ratios[i];
            phase_incs[i] = two_pi * freq / sample_rate;
            let alpha_nepers = decay_rates_db[i] / 8.686;
            let decay_per_sample = alpha_nepers / sample_rate;
            // Precompute multiplicative decay factor: exp(-α/sr) applied each sample
            decay_mult[i] = (-decay_per_sample).exp();
        }

        // Ornstein-Uhlenbeck jitter coefficients:
        //   dx = -x/tau * dt + sigma * sqrt(2/tau) * dW
        // Discretized per sample:
        //   revert = exp(-dt/tau)
        //   diffusion = sigma * sqrt(1 - revert^2)  [exact discrete OU variance]
        let dt = 1.0 / sample_rate;
        let jitter_revert = (-dt / JITTER_TAU).exp();
        let jitter_diffusion = JITTER_SIGMA * (1.0 - jitter_revert * jitter_revert).sqrt();

        // Onset ramp: raised cosine over the onset period (reed mechanical inertia).
        // e(t) = 0.5 * (1 - cos(pi * t / T_onset)) for t < T_onset, then 1.0.
        //
        // All modes ramp up together over the same onset time. The hammer applies
        // force to ALL modes simultaneously — the dwell filter (Gaussian in
        // frequency domain) controls how much energy each mode receives, NOT when
        // it arrives. Mode-dependent ramp times would create a vibraphone-like
        // staggered onset where the fundamental appears before higher modes.
        let ramp_samps = (onset_time_s * sample_rate).round() as u64;
        let ramp_inc = if ramp_samps > 0 {
            std::f64::consts::PI / ramp_samps as f64
        } else {
            0.0
        };

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

        // Velocity-dependent ramp shape: pp gets a softer (Hann-squared-like)
        // onset, ff gets standard raised cosine. Exponent blends from 1.0 (ff)
        // to 2.0 (pp). Models progressive felt compression (Giordano & Milne 1998).
        let onset_shape_exp = 1.0 + (1.0 - velocity);

        Self {
            phases: [0.0; NUM_MODES],
            phase_incs,
            amplitudes: *amplitudes,
            decay_mult,
            envelope: [1.0; NUM_MODES],
            sample: 0,
            onset_ramp_samples: ramp_samps,
            onset_ramp_inc: ramp_inc,
            onset_shape_exp,
            damper_active: false,
            damper_rates: [0.0; NUM_MODES],
            damper_ramp_samples: 0.0,
            damper_release_count: 0.0,
            damper_mult: [1.0; NUM_MODES],
            damper_ramp_done: false,
            jitter_state,
            jitter_drift,
            jitter_revert,
            jitter_diffusion,
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
        self.damper_ramp_done = false;
        // Precompute post-ramp damper multipliers
        for m in 0..NUM_MODES {
            self.damper_mult[m] = (-self.damper_rates[m]).exp();
        }
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
                if !self.damper_ramp_done {
                    if t > ramp {
                        self.damper_ramp_done = true;
                    } else {
                        // During ramp: apply instantaneous rate (still needs exp per mode)
                        for m in 0..NUM_MODES {
                            let inst_rate = self.damper_rates[m] * t / ramp;
                            self.envelope[m] *= (-inst_rate).exp();
                        }
                    }
                }
                if self.damper_ramp_done {
                    // Post-ramp: precomputed multiplicative damper (no transcendentals)
                    for m in 0..NUM_MODES {
                        self.envelope[m] *= self.damper_mult[m];
                    }
                }
            }

            // Onset ramp: all modes ramp together during hammer contact.
            // Shape exponent: ff → cosine^1 (standard), pp → cosine^2 (softer)
            let onset = if self.sample < self.onset_ramp_samples {
                let cosine = 0.5 * (1.0 - (n * self.onset_ramp_inc).cos());
                cosine.powf(self.onset_shape_exp)
            } else {
                1.0
            };

            for i in 0..NUM_MODES {
                // Ornstein-Uhlenbeck jitter: mean-reverting random walk on frequency
                // drift[i] is the fractional frequency deviation (e.g. 0.0004 = 0.04%)
                let noise = self.lcg_uniform_scaled();
                self.jitter_drift[i] = revert * self.jitter_drift[i] + diffusion * noise;

                sum += self.amplitudes[i] * self.phases[i].sin() * onset * self.envelope[i];
                self.phases[i] += self.phase_incs[i] * (1.0 + self.jitter_drift[i]);
            }

            // Apply natural decay (multiplicative — no transcendentals)
            for i in 0..NUM_MODES {
                self.envelope[i] *= self.decay_mult[i];
            }

            if self.sample & 0x3FF == 0 {
                let two_pi = 2.0 * std::f64::consts::PI;
                for p in &mut self.phases {
                    *p %= two_pi;
                }
            }

            *sample += sum;
            self.sample += 1;
        }
    }

    /// LCG PRNG → scaled uniform noise with unit variance.
    /// Uniform(-√3, √3) has variance 1.0, matching the Gaussian's variance.
    /// The OU filter (τ=20ms) convolves ~880 past samples at 44.1kHz, so output
    /// converges to Gaussian via CLT regardless of input distribution.
    /// Saves 3 transcendentals/mode/sample vs Box-Muller (ln+sqrt+cos).
    #[inline]
    fn lcg_uniform_scaled(&mut self) -> f64 {
        self.jitter_state = self
            .jitter_state
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        let u = (self.jitter_state >> 1) as f64 / (u32::MAX as f64 / 2.0);
        (u * 2.0 - 1.0) * SQRT_3
    }

    /// Check if the reed has decayed below a threshold (all modes).
    pub fn is_silent(&self, threshold_db: f64) -> bool {
        let threshold_linear = f64::powf(10.0, threshold_db / 20.0);
        for i in 0..NUM_MODES {
            if (self.amplitudes[i] * self.envelope[i]).abs() > threshold_linear {
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

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, 1.0, 44100.0, 42);
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
        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, 1.0, sr, 42);

        let half_sec = (sr * 0.5) as usize;
        let mut buf = vec![0.0f64; half_sec];
        reed.render(&mut buf);

        let peak = buf[buf.len() - 200..]
            .iter()
            .map(|x| x.abs())
            .fold(0.0f64, f64::max);
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

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, ramp, 1.0, sr, 42);
        let n = (sr * 0.050) as usize;
        let mut buf = vec![0.0f64; n];
        reed.render(&mut buf);

        // First sample should be near zero (raised cosine starts at 0)
        assert!(
            buf[0].abs() < 0.01,
            "First sample should be near zero, got {:.6}",
            buf[0]
        );

        // At half the ramp time, the ramp should be at ~0.5
        let mid_ramp = (ramp * 0.5 * sr) as usize;
        let mid_peak = buf[mid_ramp.saturating_sub(5)..mid_ramp + 5]
            .iter()
            .map(|x| x.abs())
            .fold(0.0f64, f64::max);
        assert!(
            mid_peak < 0.8,
            "Mid-ramp peak should be < 0.8, got {mid_peak:.4}"
        );

        // Well after ramp, amplitude should be ~1.0
        let late_start = (sr * 0.030) as usize;
        let late_peak = buf[late_start..late_start + 200]
            .iter()
            .map(|x| x.abs())
            .fold(0.0f64, f64::max);
        assert!(
            late_peak > 0.85,
            "Post-ramp peak should be ~1.0, got {late_peak:.4}"
        );
    }

    #[test]
    fn test_onset_ramp_ff_vs_pp() {
        // ff (short dwell) should reach full amplitude faster than pp (long dwell).
        // All modes ramp together over the dwell time.
        let sr = 44100.0;
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed_ff = ModalReed::new(440.0, &ratios, &amps, &decays, 0.001, 1.0, sr, 42);
        let mut reed_pp = ModalReed::new(440.0, &ratios, &amps, &decays, 0.005, 0.0, sr, 42);

        let n = (sr * 0.010) as usize; // 10ms window
        let mut buf_ff = vec![0.0f64; n];
        let mut buf_pp = vec![0.0f64; n];
        reed_ff.render(&mut buf_ff);
        reed_pp.render(&mut buf_pp);

        // At 2ms, ff (1ms dwell, already past ramp) should be louder than pp (5ms dwell, still ramping)
        let t2ms = (sr * 0.002) as usize;
        let ff_energy: f64 = buf_ff[..t2ms].iter().map(|x| x * x).sum();
        let pp_energy: f64 = buf_pp[..t2ms].iter().map(|x| x * x).sum();
        assert!(
            ff_energy > pp_energy * 1.5,
            "ff should be louder than pp at 2ms: ff={ff_energy:.6}, pp={pp_energy:.6}"
        );
    }

    #[test]
    fn test_onset_zero_dwell_is_instant() {
        // dwell_time=0.0 should produce full amplitude from sample 0
        let sr = 44100.0;
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, 1.0, sr, 42);
        let mut buf = vec![0.0f64; 100];
        reed.render(&mut buf);

        // With dwell=0, all modes start at full amplitude immediately
        let early_peak = buf[..10].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(
            early_peak > 0.05,
            "Zero dwell should give immediate amplitude, got {early_peak:.6}"
        );
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

        let mut reed_a = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, 1.0, sr, 100);
        let mut reed_b = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, 1.0, sr, 200);

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
        assert!(
            relative_diff > 0.001,
            "Jitter should cause measurable difference: relative_diff={relative_diff:.6}"
        );
        assert!(
            relative_diff < 0.5,
            "Jitter should be subtle, not overwhelming: relative_diff={relative_diff:.4}"
        );
    }

    #[test]
    fn test_jitter_deterministic_with_same_seed() {
        // Same seed → same output (deterministic PRNG)
        let sr = 44100.0;
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed_a = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, 1.0, sr, 42);
        let mut reed_b = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, 1.0, sr, 42);

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

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, 1.0, sr, 77);
        let n = sr as usize; // 1 second
        let mut buf = vec![0.0f64; n];
        reed.render(&mut buf);

        let mut crossings = 0u32;
        for i in 1..buf.len() {
            if buf[i - 1] < 0.0 && buf[i] >= 0.0 {
                crossings += 1;
            }
        }
        // 0.04% jitter → frequency within ~2 Hz of 440
        assert!(
            (crossings as f64 - 440.0).abs() < 2.0,
            "Average frequency should be ~440 Hz, got {crossings} crossings"
        );
    }
}
