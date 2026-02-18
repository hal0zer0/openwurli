//! Per-note MLP parameter corrections.
//!
//! A tiny neural network (2→8→8→22) runs ONCE at note-on to produce
//! per-note corrections to mode amplitudes, frequencies, decay rates,
//! and pickup displacement scale. Zero per-sample CPU cost.
//!
//! Trained on OBM recordings vs model output. See `ml/` for the
//! Python training pipeline.

#[allow(clippy::excessive_precision, clippy::unreadable_literal)]
#[path = "mlp_weights.rs"]
mod mlp_weights;

use mlp_weights::*;

const MIDI_MIN: f64 = 21.0;
const MIDI_MAX: f64 = 108.0;
const N_OUTPUTS: usize = 22;

/// Training data MIDI range. Outside this, corrections fade to identity.
const TRAIN_MIDI_LO: f64 = 65.0;
const TRAIN_MIDI_HI: f64 = 97.0;
/// Fade zone: corrections reach zero this many semitones outside training range.
const FADE_SEMITONES: f64 = 12.0;

/// Whether MLP corrections are active. Set to false to bypass.
pub const ENABLE_MLP: bool = true;

/// Corrections produced by the MLP at note-on.
pub struct MlpCorrections {
    /// Amplitude offsets for H2-H8, in dB (positive = model too quiet).
    pub amp_offsets_db: [f64; 7],
    /// Frequency offsets for H2-H8, in cents.
    pub freq_offsets_cents: [f64; 7],
    /// Decay ratio offsets for H2-H8 (>1 = model decays too fast).
    pub decay_offsets: [f64; 7],
    /// Displacement scale multiplier (1.0 = no change).
    pub ds_correction: f64,
}

impl MlpCorrections {
    /// Identity corrections (no change to any parameter).
    pub fn identity() -> Self {
        Self {
            amp_offsets_db: [0.0; 7],
            freq_offsets_cents: [0.0; 7],
            decay_offsets: [1.0; 7],
            ds_correction: 1.0,
        }
    }

    /// Run the MLP forward pass and produce corrections.
    ///
    /// Outside the training range (MIDI 65-97), corrections fade linearly
    /// to identity over 12 semitones to prevent wild extrapolation.
    pub fn infer(midi_note: u8, velocity: f64) -> Self {
        if !ENABLE_MLP {
            return Self::identity();
        }

        let midi = midi_note as f64;

        // Compute fade factor: 1.0 inside training range, 0.0 far outside
        let fade = if midi < TRAIN_MIDI_LO {
            ((midi - (TRAIN_MIDI_LO - FADE_SEMITONES)) / FADE_SEMITONES).clamp(0.0, 1.0)
        } else if midi > TRAIN_MIDI_HI {
            (((TRAIN_MIDI_HI + FADE_SEMITONES) - midi) / FADE_SEMITONES).clamp(0.0, 1.0)
        } else {
            1.0
        };

        if fade <= 0.0 {
            return Self::identity();
        }

        // Normalize inputs to [0, 1]
        let midi_norm = ((midi - MIDI_MIN) / (MIDI_MAX - MIDI_MIN)).clamp(0.0, 1.0);
        let vel_norm = velocity.clamp(0.0, 1.0);
        let input = [midi_norm, vel_norm];

        // Layer 1: affine + ReLU
        let mut h1 = [0.0f64; HIDDEN_SIZE];
        for i in 0..HIDDEN_SIZE {
            let mut sum = B1[i];
            for j in 0..2 {
                sum += W1[i][j] * input[j];
            }
            h1[i] = if sum > 0.0 { sum } else { 0.0 };
        }

        // Layer 2: affine + ReLU
        let mut h2 = [0.0f64; HIDDEN_SIZE];
        for i in 0..HIDDEN_SIZE {
            let mut sum = B2[i];
            for j in 0..HIDDEN_SIZE {
                sum += W2[i][j] * h1[j];
            }
            h2[i] = if sum > 0.0 { sum } else { 0.0 };
        }

        // Layer 3: affine (linear output) + denormalization
        let mut raw = [0.0f64; N_OUTPUTS];
        for i in 0..N_OUTPUTS {
            let mut sum = B3[i];
            for j in 0..HIDDEN_SIZE {
                sum += W3[i][j] * h2[j];
            }
            raw[i] = sum * TARGET_STDS[i] + TARGET_MEANS[i];
        }

        // Unpack, clamp, and apply fade toward identity outside training range
        let mut amp_offsets_db = [0.0f64; 7];
        let mut freq_offsets_cents = [0.0f64; 7];
        let mut decay_offsets = [1.0f64; 7];

        for h in 0..7 {
            // Fade: amp/freq offsets → 0.0, decay → 1.0
            amp_offsets_db[h] = (raw[h] * fade).clamp(-40.0, 40.0);
            freq_offsets_cents[h] = (raw[7 + h] * fade).clamp(-100.0, 100.0);
            let raw_decay = raw[14 + h].clamp(0.3, 3.0);
            decay_offsets[h] = 1.0 + (raw_decay - 1.0) * fade;
        }

        let raw_ds = raw[21].clamp(0.5, 2.5);
        let ds_correction = 1.0 + (raw_ds - 1.0) * fade;

        Self {
            amp_offsets_db,
            freq_offsets_cents,
            decay_offsets,
            ds_correction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_is_neutral() {
        let c = MlpCorrections::identity();
        for i in 0..7 {
            assert_eq!(c.amp_offsets_db[i], 0.0);
            assert_eq!(c.freq_offsets_cents[i], 0.0);
            assert_eq!(c.decay_offsets[i], 1.0);
        }
        assert_eq!(c.ds_correction, 1.0);
    }

    #[test]
    fn test_infer_produces_corrections() {
        let c = MlpCorrections::infer(60, 0.8);
        // MLP should produce non-trivial corrections for at least some targets
        let has_correction = c.amp_offsets_db.iter().any(|&x| x.abs() > 0.01)
            || c.freq_offsets_cents.iter().any(|&x| x.abs() > 0.01);
        assert!(has_correction, "MLP should produce non-trivial corrections");
    }

    #[test]
    fn test_different_notes_differ() {
        let c40 = MlpCorrections::infer(40, 0.8);
        let c80 = MlpCorrections::infer(80, 0.8);
        let differ = (0..7).any(|i| (c40.amp_offsets_db[i] - c80.amp_offsets_db[i]).abs() > 0.001);
        assert!(differ, "different notes should get different corrections");
    }

    #[test]
    fn test_corrections_within_bounds() {
        for midi in [33, 48, 60, 72, 84, 96] {
            for vel in [0.2, 0.5, 0.8, 1.0] {
                let c = MlpCorrections::infer(midi, vel);
                for i in 0..7 {
                    assert!(c.amp_offsets_db[i].abs() <= 40.0, "amp clamp violated");
                    assert!(
                        c.freq_offsets_cents[i].abs() <= 100.0,
                        "freq clamp violated"
                    );
                    assert!(
                        (0.3..=3.0).contains(&c.decay_offsets[i]),
                        "decay clamp violated"
                    );
                }
                assert!((0.5..=2.5).contains(&c.ds_correction), "ds clamp violated");
            }
        }
    }
}
