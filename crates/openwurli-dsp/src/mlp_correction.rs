//! Per-note MLP v2 parameter corrections.
//!
//! A tiny neural network (2→16→16→11) runs ONCE at note-on to produce
//! per-note corrections to mode frequencies and decay rates. Zero per-sample
//! CPU cost.
//!
//! Trained on OBM recordings vs model output. See `ml/` for the
//! Python training pipeline.

#[allow(clippy::excessive_precision, clippy::unreadable_literal)]
#[path = "mlp_weights.rs"]
mod mlp_weights;

use mlp_weights::*;

const MIDI_MIN: f64 = 21.0;
const MIDI_MAX: f64 = 108.0;
/// Network outputs: [0:5] freq, [5:10] decay, [10] the retired displacement-
/// scale head. Output 10 is still produced by the trained weights but ignored
/// (retired 2026-09-27, see ml/compute_residuals.py TRAIN_DS); keep the count at
/// 11 so the weights load.
const N_OUTPUTS: usize = 11;

/// How many harmonic heads the network was actually TRAINED on.
///
/// Mirrors `MAX_RELIABLE_HARMONIC` in `ml/compute_residuals.py`: only H2 and H3
/// clear the SNR / anomaly filters on the reference set, so `freq_H4..H6` and
/// `decay_H4..H6` have **zero** training samples — verified against
/// `ml/ml_data/training_data.npz` (13/13 and 11/13 samples for H2/H3; 0/13 for
/// H4, H5 and H6).
///
/// An untrained head is not neutral. Its weights are whatever initialisation
/// and the shared hidden layers leave them, and the output then meets a clamp.
/// Measured before this fix, `decay_H4..H6` emitted **0.592** at MIDI 60 and sat
/// **railed at the 0.300 lower clamp** at MIDI 72 and 84 — a decay-rate
/// correction of 3.3x FASTER on three upper modes that nothing ever asked for.
/// That is the mechanism behind the muted upper-harmonic decay in the
/// corrections-on A/B (H4 -2.4 dB, H5 -3.2 dB at C4 v112).
///
/// Heads at or above this index now emit IDENTITY. If the pipeline ever trains
/// more harmonics, raise this to match `MAX_RELIABLE_HARMONIC` and regenerate.
const N_TRAINED_HARMONICS: usize = 2;
const N_FREQ: usize = 5;
const N_DECAY: usize = 5;
/// Training data MIDI range: the reference notes at their true pitch, D2-D6
/// (MIDI 38-86; the recordings' filenames run an octave high, see
/// `ml/extract_notes.py`). Outside this, corrections fade to identity.
const TRAIN_MIDI_LO: f64 = 38.0;
const TRAIN_MIDI_HI: f64 = 86.0;
/// Fade zone: corrections reach zero this many semitones outside training range.
const FADE_SEMITONES: f64 = 12.0;

/// Whether MLP corrections are active. Set to false to bypass.
pub const ENABLE_MLP: bool = true;

/// Corrections produced by the MLP at note-on.
pub struct MlpCorrections {
    /// Frequency offsets for H2-H6, in cents. Applied to modes 1-5.
    pub freq_offsets_cents: [f64; N_FREQ],
    /// Decay ratio offsets for H2-H6 (>1 = model decays too fast). Applied to modes 1-5.
    pub decay_offsets: [f64; N_DECAY],
}

impl MlpCorrections {
    /// Identity corrections (no change to any parameter).
    pub fn identity() -> Self {
        Self {
            freq_offsets_cents: [0.0; N_FREQ],
            decay_offsets: [1.0; N_DECAY],
        }
    }

    /// Run the MLP forward pass and produce corrections.
    ///
    /// Outside the training range (MIDI 38-86), corrections fade linearly
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

        // Unpack: [0:5] = freq, [5:10] = decay ([10], the retired ds head, is ignored)
        let mut freq_offsets_cents = [0.0f64; N_FREQ];
        let mut decay_offsets = [1.0f64; N_DECAY];

        // Untrained heads emit identity, not whatever the clamp catches.
        // See N_TRAINED_HARMONICS for why this is not a tuning choice.
        for h in 0..N_FREQ.min(N_TRAINED_HARMONICS) {
            freq_offsets_cents[h] = (raw[h] * fade).clamp(-100.0, 100.0);
        }
        for h in 0..N_DECAY.min(N_TRAINED_HARMONICS) {
            let raw_decay = raw[N_FREQ + h].clamp(0.3, 3.0);
            decay_offsets[h] = 1.0 + (raw_decay - 1.0) * fade;
        }

        Self {
            freq_offsets_cents,
            decay_offsets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_is_neutral() {
        let c = MlpCorrections::identity();
        for i in 0..N_FREQ {
            assert_eq!(c.freq_offsets_cents[i], 0.0);
        }
        for i in 0..N_DECAY {
            assert_eq!(c.decay_offsets[i], 1.0);
        }
    }

    #[test]
    fn test_infer_produces_corrections() {
        let c = MlpCorrections::infer(60, 0.8);
        let has_correction = c.freq_offsets_cents.iter().any(|&x| x.abs() > 0.01)
            || c.decay_offsets.iter().any(|&x| (x - 1.0).abs() > 0.01);
        assert!(has_correction, "MLP should produce non-trivial corrections");
    }

    #[test]
    fn test_different_notes_differ() {
        let c40 = MlpCorrections::infer(40, 0.8);
        let c80 = MlpCorrections::infer(80, 0.8);
        let differ = (0..N_FREQ)
            .any(|i| (c40.freq_offsets_cents[i] - c80.freq_offsets_cents[i]).abs() > 0.001)
            || (0..N_DECAY).any(|i| (c40.decay_offsets[i] - c80.decay_offsets[i]).abs() > 0.001);
        assert!(differ, "different notes should get different corrections");
    }

    #[test]
    fn test_corrections_within_bounds() {
        for midi in [33, 48, 60, 72, 84, 96] {
            for vel in [0.2, 0.5, 0.8, 1.0] {
                let c = MlpCorrections::infer(midi, vel);
                for i in 0..N_FREQ {
                    assert!(
                        c.freq_offsets_cents[i].abs() <= 100.0,
                        "freq clamp violated"
                    );
                }
                for i in 0..N_DECAY {
                    assert!(
                        (0.3..=3.0).contains(&c.decay_offsets[i]),
                        "decay clamp violated"
                    );
                }
            }
        }
    }
}
