//! Deterministic per-note variation -- hash-based pseudo-random offsets.
//!
//! Each physical reed on a real Wurlitzer has slightly different tuning,
//! solder mass, and mounting characteristics. This module provides fixed
//! per-note offsets so note 60 always sounds the same, but differs
//! slightly from note 61.
#![allow(clippy::needless_range_loop)]

use crate::tables::NUM_MODES;

/// Simple deterministic hash: takes MIDI note + seed, returns 0.0..1.0.
fn hash_f64(midi: u8, seed: u32) -> f64 {
    let mut h: u32 = 2166136261;
    h ^= midi as u32;
    h = h.wrapping_mul(16777619);
    h ^= seed;
    h = h.wrapping_mul(16777619);
    h ^= h >> 16;
    h = h.wrapping_mul(2654435769);
    (h & 0x00FF_FFFF) as f64 / 16777216.0
}

/// Frequency detuning factor for a note: multiplier in range [1-max, 1+max].
/// max = 0.008 (+/-0.8%).
pub fn freq_detune(midi: u8) -> f64 {
    let r = hash_f64(midi, 0xDEAD) * 2.0 - 1.0;
    1.0 + r * 0.008
}

/// Per-mode amplitude variation factors: multipliers in range [1-max, 1+max].
/// max = 0.08 (+/-8%).
pub fn mode_amplitude_offsets(midi: u8) -> [f64; NUM_MODES] {
    let mut offsets = [0.0f64; NUM_MODES];
    for i in 0..NUM_MODES {
        let r = hash_f64(midi, 0xBEEF + i as u32) * 2.0 - 1.0;
        offsets[i] = 1.0 + r * 0.08;
    }
    offsets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic() {
        assert_eq!(freq_detune(60), freq_detune(60));
        assert_eq!(mode_amplitude_offsets(60), mode_amplitude_offsets(60));
    }

    #[test]
    fn test_different_notes_differ() {
        assert_ne!(freq_detune(60), freq_detune(61));
    }

    #[test]
    fn test_detune_range() {
        for midi in 33..=96 {
            let d = freq_detune(midi);
            assert!(
                d > 0.99 && d < 1.01,
                "detune out of range for MIDI {midi}: {d}"
            );
        }
    }

    #[test]
    fn test_amplitude_range() {
        for midi in 33..=96 {
            let offsets = mode_amplitude_offsets(midi);
            for (i, &o) in offsets.iter().enumerate() {
                assert!(
                    o > 0.90 && o < 1.10,
                    "amplitude offset out of range for MIDI {midi} mode {i}: {o}"
                );
            }
        }
    }
}
