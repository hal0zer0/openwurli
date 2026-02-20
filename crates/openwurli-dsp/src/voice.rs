//! Single voice: reed + hammer + pickup + decay.
//!
//! Signal flow: modal_oscillator -> pickup_hpf -> output
//! Attack noise mixed in during first ~15 ms.
#![allow(clippy::needless_range_loop)]

use crate::hammer::{AttackNoise, dwell_attenuation, onset_ramp_time};
use crate::mlp_correction::MlpCorrections;
use crate::pickup::Pickup;
use crate::reed::ModalReed;
use crate::tables::{self, NUM_MODES};
use crate::variation;

// velocity_scurve is in tables.rs (shared with output_scale's velocity-aware proxy)

pub struct Voice {
    reed: ModalReed,
    pickup: Pickup,
    noise: AttackNoise,
    post_pickup_gain: f64,
    sample_rate: f64,
    midi_note: u8,
}

impl Voice {
    /// Initialize a voice for a given note and velocity.
    ///
    /// - `midi_note`: MIDI note number (33-96)
    /// - `velocity`: 0.0 (pp) to 1.0 (ff)
    /// - `sample_rate`: audio sample rate
    /// - `noise_seed`: RNG seed for attack noise (decorrelates simultaneous notes)
    pub fn note_on(
        midi_note: u8,
        velocity: f64,
        sample_rate: f64,
        noise_seed: u32,
        mlp_enabled: bool,
    ) -> Self {
        let params = tables::note_params(midi_note);

        let detuned_fundamental = params.fundamental_hz * variation::freq_detune(midi_note);

        let dwell = dwell_attenuation(velocity, detuned_fundamental, &params.mode_ratios);
        let t_dwell = onset_ramp_time(velocity, detuned_fundamental);
        let amp_offsets = variation::mode_amplitude_offsets(midi_note);

        let mut amplitudes = [0.0f64; NUM_MODES];
        for i in 0..NUM_MODES {
            amplitudes[i] = params.mode_amplitudes[i] * dwell[i] * amp_offsets[i];
        }

        // Sigmoid → power-law velocity curve (physical hammer force — pre-pickup).
        // S-curve models progressive felt stiffness: pp barely deflects the reed,
        // bark onset at ~mf, ff saturates. Then power-law applies register-dependent
        // dynamic range scaling.
        // output_scale is applied POST-pickup to decouple volume from nonlinearity.
        let vel_exp = tables::velocity_exponent(midi_note);
        let vel_scale = tables::velocity_scurve(velocity).powf(vel_exp);
        for a in &mut amplitudes {
            *a *= vel_scale;
        }

        // MLP v2 per-note corrections (zero per-sample cost — runs once at note-on).
        // Adjusts mode frequencies, decay rates, and pickup displacement scale
        // based on learned residuals vs OBM recordings.
        let corrections = if mlp_enabled {
            MlpCorrections::infer(midi_note, velocity)
        } else {
            MlpCorrections::identity()
        };

        // Apply frequency corrections to modes 1-5 (mode 0 = fundamental, never corrected)
        let mut corrected_ratios = params.mode_ratios;
        for m in 1..NUM_MODES.min(6) {
            let cents = corrections.freq_offsets_cents[m - 1];
            corrected_ratios[m] *= f64::powf(2.0, cents / 1200.0);
        }

        // Apply decay corrections to modes 1-5
        let mut corrected_decay = params.mode_decay_rates;
        for m in 1..NUM_MODES.min(6) {
            let ratio = corrections.decay_offsets[m - 1];
            corrected_decay[m] /= ratio;
        }

        // Apply displacement scale correction (from H2/H1 ratio matching)
        let corrected_ds = tables::pickup_displacement_scale(midi_note) * corrections.ds_correction;

        let reed = ModalReed::new(
            detuned_fundamental,
            &corrected_ratios,
            &amplitudes,
            &corrected_decay,
            t_dwell,
            velocity,
            sample_rate,
            noise_seed,
        );

        let mut pickup = Pickup::new(sample_rate);
        pickup.set_displacement_scale(corrected_ds);
        let noise = AttackNoise::new(velocity, detuned_fundamental, sample_rate, noise_seed);

        // Post-pickup gain: technician voicing (gap adjustment) affects volume
        // without changing the nonlinear displacement fraction y.
        let post_pickup_gain = tables::output_scale(midi_note, velocity);

        Self {
            reed,
            pickup,
            noise,
            post_pickup_gain,
            sample_rate,
            midi_note,
        }
    }

    /// Override the pickup displacement scale.
    pub fn set_displacement_scale(&mut self, scale: f64) {
        self.pickup.set_displacement_scale(scale);
    }

    /// Start the damper (called on note_off).
    /// Activates progressive damping — higher modes die first.
    pub fn note_off(&mut self) {
        self.reed.start_damper(self.midi_note, self.sample_rate);
    }

    /// Render samples into the output buffer.
    /// Buffer is cleared first, then filled with the voice output.
    pub fn render(&mut self, output: &mut [f64]) {
        for s in output.iter_mut() {
            *s = 0.0;
        }

        self.reed.render(output);

        if !self.noise.is_done() {
            self.noise.render(output);
        }

        self.pickup.process(output);

        // Apply post-pickup voicing gain (technician gap/level adjustment).
        // This affects volume without changing bark character.
        let gain = self.post_pickup_gain;
        for s in output.iter_mut() {
            *s *= gain;
        }
    }

    /// Check if the voice has decayed to silence.
    /// Also returns true after 10 seconds of release (safety timeout).
    pub fn is_silent(&self) -> bool {
        if self.reed.is_damping() && self.reed.release_seconds(self.sample_rate) > 10.0 {
            return true;
        }
        self.reed.is_silent(-80.0)
    }

    /// Render a complete note of given duration to a Vec.
    pub fn render_note(
        midi_note: u8,
        velocity: f64,
        duration_secs: f64,
        sample_rate: f64,
    ) -> Vec<f64> {
        Self::render_note_with_scale(midi_note, velocity, duration_secs, sample_rate, None)
    }

    /// Render a complete note with optional displacement scale override.
    pub fn render_note_with_scale(
        midi_note: u8,
        velocity: f64,
        duration_secs: f64,
        sample_rate: f64,
        displacement_scale: Option<f64>,
    ) -> Vec<f64> {
        let noise_seed = (midi_note as u32).wrapping_mul(2654435761);
        let mut voice = Voice::note_on(midi_note, velocity, sample_rate, noise_seed, false);
        if let Some(scale) = displacement_scale {
            voice.set_displacement_scale(scale);
        }
        let num_samples = (duration_secs * sample_rate) as usize;
        let mut output = vec![0.0f64; num_samples];

        let chunk_size = 1024;
        let mut offset = 0;
        while offset < num_samples {
            let end = (offset + chunk_size).min(num_samples);
            voice.render(&mut output[offset..end]);
            offset = end;
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_note_produces_audio() {
        let output = Voice::render_note(60, 0.8, 0.5, 44100.0);
        let peak = output.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(peak > 0.0, "no audio produced");
    }

    #[test]
    fn test_higher_velocity_is_louder() {
        let soft = Voice::render_note(60, 0.3, 0.1, 44100.0);
        let loud = Voice::render_note(60, 1.0, 0.1, 44100.0);

        let peak_soft = soft.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        let peak_loud = loud.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(
            peak_loud > peak_soft,
            "loud ({peak_loud}) should exceed soft ({peak_soft})"
        );
    }

    #[test]
    fn test_deterministic() {
        let a = Voice::render_note(60, 0.8, 0.1, 44100.0);
        let b = Voice::render_note(60, 0.8, 0.1, 44100.0);
        assert_eq!(a, b, "same note should produce identical output");
    }

    #[test]
    fn test_different_notes_differ() {
        let a = Voice::render_note(60, 0.8, 0.1, 44100.0);
        let b = Voice::render_note(72, 0.8, 0.1, 44100.0);
        assert_ne!(a, b);
    }
}
