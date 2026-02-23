//! Modal reed oscillator — 7 damped sinusoidal modes.
//!
//! Each mode uses a quadrature oscillator (sin/cos pair rotated per sample)
//! instead of computing sin(phase) per sample. This eliminates 7 transcendental
//! calls per sample per voice — the dominant CPU cost in v0.1.x.
//!
//! Per-mode frequency jitter (Ornstein-Uhlenbeck process) breaks the perfect
//! phase coherence of digital oscillators. Real reeds have nonlinear frequency-
//! amplitude coupling (backbone curve), pickup loading, and micro-turbulence
//! that cause each mode's frequency to wander slightly. Without this, the
//! static spectral interference pattern sounds "metallic" and "resonant."
//!
//! Jitter is subsampled (every 16 samples) because the OU correlation time
//! (τ=20ms, ~882 samples at 44.1kHz) is far longer than the update interval.

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

/// Jitter subsample interval. OU τ=20ms spans ~882 samples at 44.1kHz.
/// Updating every 16 samples (0.36ms) is 55x below correlation time.
const JITTER_SUBSAMPLE: u64 = 16;

/// Quadrature renormalization interval. Every 1024 samples, correct the
/// radius of each (s,c) pair to prevent drift from accumulated FP error.
/// Cost: 7 sqrt + 7 div per 1024 samples = 0.014 transcendentals/sample.
const RENORM_INTERVAL: u64 = 1024;

/// Per-mode oscillator state — array-of-structs for sequential access.
///
/// 88 bytes/mode × 7 = 616 bytes. Fits in L1 cache.
struct Mode {
    /// Quadrature sine state (output signal).
    s: f64,
    /// Quadrature cosine state.
    c: f64,
    /// cos(base_phase_inc) — precomputed at note-on.
    cos_inc: f64,
    /// sin(base_phase_inc) — precomputed at note-on.
    sin_inc: f64,
    /// Base phase increment (for jitter delta scaling).
    phase_inc: f64,
    /// Initial amplitude for this mode.
    amplitude: f64,
    /// Per-sample multiplicative natural decay factor.
    decay_mult: f64,
    /// Current envelope level (starts at 1.0, decays each sample).
    envelope: f64,
    /// OU jitter drift state (fractional frequency deviation).
    jitter_drift: f64,
    /// Damper rate in nepers/sample (set on note_off).
    damper_rate: f64,
    /// Precomputed exp(-damper_rate) for post-ramp phase.
    damper_mult: f64,
}

pub struct ModalReed {
    modes: [Mode; NUM_MODES],
    sample: u64,
    // Onset ramp
    onset_ramp_samples: u64,
    onset_ramp_inc: f64,
    onset_shape_exp: f64,
    // Damper state
    damper_active: bool,
    damper_ramp_samples: f64,
    damper_release_count: f64,
    damper_ramp_done: bool,
    // Jitter PRNG and coefficients
    jitter_state: u32,
    jitter_revert: f64,
    jitter_diffusion: f64,
}

/// LCG PRNG → scaled uniform noise with unit variance.
/// Free function to avoid borrow conflicts when iterating `&mut modes`.
#[inline]
fn lcg_uniform_scaled(state: &mut u32) -> f64 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    let u = (*state >> 1) as f64 / (u32::MAX as f64 / 2.0);
    (u * 2.0 - 1.0) * SQRT_3
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

        // Ornstein-Uhlenbeck jitter coefficients
        let dt = 1.0 / sample_rate;
        let jitter_revert = (-dt / JITTER_TAU).exp();
        let jitter_diffusion = JITTER_SIGMA * (1.0 - jitter_revert * jitter_revert).sqrt();

        // Initialize jitter_drift from the OU stationary distribution N(0, JITTER_SIGMA).
        // Uses Box-Muller for the initial draw (one-time cost at note-on).
        let mut jitter_state = jitter_seed.max(1);
        let mut initial_drifts = [0.0f64; NUM_MODES];
        for d in &mut initial_drifts {
            jitter_state = jitter_state.wrapping_mul(1664525).wrapping_add(1013904223);
            let u1 = (jitter_state >> 1) as f64 / (u32::MAX as f64 / 2.0);
            jitter_state = jitter_state.wrapping_mul(1664525).wrapping_add(1013904223);
            let u2 = (jitter_state >> 1) as f64 / (u32::MAX as f64 / 2.0);
            let r = (-2.0 * u1.max(1e-30).ln()).sqrt();
            *d = JITTER_SIGMA * r * (two_pi * u2).cos();
        }

        // Build Mode structs with precomputed quadrature rotation coefficients
        let modes = std::array::from_fn(|i| {
            let freq = fundamental_hz * mode_ratios[i];
            let phase_inc = two_pi * freq / sample_rate;
            let alpha_nepers = decay_rates_db[i] / 8.686;
            let decay_per_sample = alpha_nepers / sample_rate;

            Mode {
                s: 0.0,                        // sin(0) = 0
                c: 1.0,                        // cos(0) = 1
                cos_inc: phase_inc.cos(),      // precomputed rotation
                sin_inc: phase_inc.sin(),      // precomputed rotation
                phase_inc,
                amplitude: amplitudes[i],
                decay_mult: (-decay_per_sample).exp(),
                envelope: 1.0,
                jitter_drift: initial_drifts[i],
                damper_rate: 0.0,
                damper_mult: 1.0,
            }
        });

        // Onset ramp
        let ramp_samps = (onset_time_s * sample_rate).round() as u64;
        let ramp_inc = if ramp_samps > 0 {
            std::f64::consts::PI / ramp_samps as f64
        } else {
            0.0
        };

        let onset_shape_exp = 1.0 + (1.0 - velocity);

        Self {
            modes,
            sample: 0,
            onset_ramp_samples: ramp_samps,
            onset_ramp_inc: ramp_inc,
            onset_shape_exp,
            damper_active: false,
            damper_ramp_samples: 0.0,
            damper_release_count: 0.0,
            damper_ramp_done: false,
            jitter_state,
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
        for (m, mode) in self.modes.iter_mut().enumerate() {
            let factor = (base_rate * 3.0_f64.powi(m as i32)).min(2000.0);
            mode.damper_rate = factor / sample_rate;
            mode.damper_mult = (-mode.damper_rate).exp();
        }

        let ramp_time = if midi_note < 48 {
            0.050
        } else if midi_note < 72 {
            0.025
        } else {
            0.008
        };

        self.damper_ramp_samples = ramp_time * sample_rate;
        self.damper_active = true;
        self.damper_release_count = 0.0;
        self.damper_ramp_done = false;
    }

    /// Render samples into the output buffer (additive, does NOT clear buffer).
    pub fn render(&mut self, output: &mut [f64]) {
        let revert = self.jitter_revert;
        let diffusion = self.jitter_diffusion;

        for sample in output.iter_mut() {
            let mut sum = 0.0f64;

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
                        for mode in &mut self.modes {
                            let inst_rate = mode.damper_rate * t / ramp;
                            mode.envelope *= (-inst_rate).exp();
                        }
                    }
                }
                if self.damper_ramp_done {
                    for mode in &mut self.modes {
                        mode.envelope *= mode.damper_mult;
                    }
                }
            }

            // Onset ramp: all modes ramp together during hammer contact.
            // Shape exponent: ff -> cosine^1 (standard), pp -> cosine^2 (softer)
            let onset = if self.sample < self.onset_ramp_samples {
                let n = self.sample as f64;
                let cosine = 0.5 * (1.0 - (n * self.onset_ramp_inc).cos());
                // Optimized powf: ff (exp~1.0) uses cosine, pp (exp~2.0) uses cosine²
                if self.onset_shape_exp <= 1.001 {
                    cosine
                } else if self.onset_shape_exp >= 1.999 {
                    cosine * cosine
                } else {
                    cosine.powf(self.onset_shape_exp)
                }
            } else {
                1.0
            };

            // Subsample jitter update: every 16 samples
            if self.sample & (JITTER_SUBSAMPLE - 1) == 0 {
                for mode in &mut self.modes {
                    let noise = lcg_uniform_scaled(&mut self.jitter_state);
                    mode.jitter_drift = revert * mode.jitter_drift + diffusion * noise;
                }
            }

            // Quadrature oscillator: 0 transcendentals per mode per sample
            for mode in &mut self.modes {
                sum += mode.amplitude * mode.s * onset * mode.envelope;

                // Jitter-corrected rotation via first-order Taylor approximation.
                // delta_phase = jitter_drift * phase_inc ~ 0.0004 * 0.063 = 2.5e-5 rad.
                // Taylor error ~ delta²/2 = 3e-10/sample. Over 1024 samples: ~3e-7 cumulative.
                let delta_phase = mode.jitter_drift * mode.phase_inc;
                let ci = mode.cos_inc - delta_phase * mode.sin_inc;
                let si = mode.sin_inc + delta_phase * mode.cos_inc;
                let s_new = mode.s * ci + mode.c * si;
                let c_new = mode.c * ci - mode.s * si;
                mode.s = s_new;
                mode.c = c_new;

                // Natural decay
                mode.envelope *= mode.decay_mult;
            }

            // Renormalize quadrature radius every 1024 samples
            if self.sample & (RENORM_INTERVAL - 1) == 0 && self.sample > 0 {
                for mode in &mut self.modes {
                    let r_sq = mode.s * mode.s + mode.c * mode.c;
                    let r_inv = 1.0 / r_sq.sqrt();
                    mode.s *= r_inv;
                    mode.c *= r_inv;
                }
            }

            *sample += sum;
            self.sample += 1;
        }
    }

    /// Check if the reed has decayed below a threshold (all modes).
    pub fn is_silent(&self, threshold_db: f64) -> bool {
        let threshold_linear = f64::powf(10.0, threshold_db / 20.0);
        self.modes
            .iter()
            .all(|mode| (mode.amplitude * mode.envelope).abs() <= threshold_linear)
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

        // With 0.04% jitter, frequency should still be within ~3 Hz of 440
        let mut crossings = 0u32;
        for i in 1..buf.len() {
            if buf[i - 1] < 0.0 && buf[i] >= 0.0 {
                crossings += 1;
            }
        }
        assert!(
            (crossings as f64 - 440.0).abs() < 3.0,
            "Average frequency should be ~440 Hz, got {crossings} crossings"
        );
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
        let sr = 44100.0;
        let ramp = 0.020;
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, ramp, 1.0, sr, 42);
        let n = (sr * 0.050) as usize;
        let mut buf = vec![0.0f64; n];
        reed.render(&mut buf);

        assert!(
            buf[0].abs() < 0.01,
            "First sample should be near zero, got {:.6}",
            buf[0]
        );

        let mid_ramp = (ramp * 0.5 * sr) as usize;
        let mid_peak = buf[mid_ramp.saturating_sub(5)..mid_ramp + 5]
            .iter()
            .map(|x| x.abs())
            .fold(0.0f64, f64::max);
        assert!(
            mid_peak < 0.8,
            "Mid-ramp peak should be < 0.8, got {mid_peak:.4}"
        );

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
        let sr = 44100.0;
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed_ff = ModalReed::new(440.0, &ratios, &amps, &decays, 0.001, 1.0, sr, 42);
        let mut reed_pp = ModalReed::new(440.0, &ratios, &amps, &decays, 0.005, 0.0, sr, 42);

        let n = (sr * 0.010) as usize;
        let mut buf_ff = vec![0.0f64; n];
        let mut buf_pp = vec![0.0f64; n];
        reed_ff.render(&mut buf_ff);
        reed_pp.render(&mut buf_pp);

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
        let sr = 44100.0;
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, 1.0, sr, 42);
        let mut buf = vec![0.0f64; 100];
        reed.render(&mut buf);

        let early_peak = buf[..10].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(
            early_peak > 0.05,
            "Zero dwell should give immediate amplitude, got {early_peak:.6}"
        );
    }

    #[test]
    fn test_jitter_breaks_phase_coherence() {
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

        let late_start = (sr * 0.2) as usize;
        let mut diff_sq = 0.0;
        let mut sig_sq = 0.0;
        for i in late_start..n {
            diff_sq += (buf_a[i] - buf_b[i]).powi(2);
            sig_sq += buf_a[i].powi(2);
        }
        let rms_diff = (diff_sq / (n - late_start) as f64).sqrt();
        let rms_sig = (sig_sq / (n - late_start) as f64).sqrt();

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
        let sr = 44100.0;
        let mut amps = [0.0f64; NUM_MODES];
        amps[0] = 1.0;
        let ratios = [1.0, 6.267, 17.547, 34.386, 56.842, 85.1, 119.3];
        let decays = [0.0f64; NUM_MODES];

        let mut reed = ModalReed::new(440.0, &ratios, &amps, &decays, 0.0, 1.0, sr, 77);
        let n = sr as usize;
        let mut buf = vec![0.0f64; n];
        reed.render(&mut buf);

        let mut crossings = 0u32;
        for i in 1..buf.len() {
            if buf[i - 1] < 0.0 && buf[i] >= 0.0 {
                crossings += 1;
            }
        }
        assert!(
            (crossings as f64 - 440.0).abs() < 3.0,
            "Average frequency should be ~440 Hz, got {crossings} crossings"
        );
    }
}
