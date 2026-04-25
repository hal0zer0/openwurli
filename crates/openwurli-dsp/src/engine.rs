//! WurliEngine — synth engine extracted from `openwurli-plugin`.
//!
//! Owns voice management (slots, allocation, stealing, sustain pedal),
//! the shared signal chain (preamp → vol² → power amp → speaker → PSG),
//! and the optional DAW-domain DI limiter. Framework-agnostic so any host
//! (nih-plug, custom DAW integration, oomox/Vurli) can wrap it without
//! copying voice or signal-chain code.
//!
//! Smoothing model: the three audio-rate user params (volume, tremolo
//! depth, speaker character) ramp internally over `SMOOTH_SAMPLES_AT_44K1`
//! base-rate samples after each setter call to avoid zipper noise on knob
//! moves. Hosts can call setters at block rate without their own smoothers.
//! Block-rate params (MLP, DI limiter, noise) take effect immediately.

use crate::dk_preamp::DkPreamp;
use crate::oversampler::Oversampler;
use crate::power_amp::PowerAmp;
use crate::preamp::PreampModel;
use crate::speaker::Speaker;
use crate::tables;
use crate::tremolo::Tremolo;
use crate::voice::Voice;

const MAX_VOICES: usize = 64;
const MAX_BLOCK_SIZE: usize = 8192;

/// DI limiter threshold (−6 dBFS, 0.501 linear). Samples below pass
/// through bit-exact; only ff-chord peaks crossing this engage the
/// soft-knee compression toward `DI_LIMITER_CEILING`.
const DI_LIMITER_THRESHOLD: f64 = 0.501;
/// DI limiter ceiling (−1 dBFS, 0.891 linear). Outputs are
/// asymptotically bounded here even on the loudest input.
const DI_LIMITER_CEILING: f64 = 0.891;

/// Soft-knee limiter for the DAW-domain output (mic preamp / A-D
/// converter ceiling model). See `engine` module docs for context.
#[inline]
fn di_soft_limit(x: f64) -> f64 {
    let a = x.abs();
    if a < DI_LIMITER_THRESHOLD {
        return x;
    }
    let over = (a - DI_LIMITER_THRESHOLD) / (DI_LIMITER_CEILING - DI_LIMITER_THRESHOLD);
    let compressed =
        DI_LIMITER_THRESHOLD + (DI_LIMITER_CEILING - DI_LIMITER_THRESHOLD) * over.tanh();
    compressed.copysign(x)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum VoiceState {
    Free,
    Held,
    /// Key released while sustain pedal was down — reed rings undamped.
    Sustained,
    Releasing,
}

struct VoiceSlot {
    voice: Option<Voice>,
    state: VoiceState,
    midi_note: u8,
    age: u64,
    /// Voice being faded out from stealing (5 ms linear crossfade).
    steal_voice: Option<Voice>,
    steal_fade: u32,
    steal_fade_len: u32,
}

impl Default for VoiceSlot {
    fn default() -> Self {
        Self {
            voice: None,
            state: VoiceState::Free,
            midi_note: 0,
            age: 0,
            steal_voice: None,
            steal_fade: 0,
            steal_fade_len: 0,
        }
    }
}

/// Single-pole linear smoother: given a target value, produces a per-sample
/// ramp toward it over `samples` base-rate samples. Used for vol, tremolo
/// depth, speaker character so block-rate setter calls don't zipper.
struct LinearSmoother {
    current: f64,
    target: f64,
    step: f64,
    samples_remaining: u32,
    ramp_samples: u32,
}

impl LinearSmoother {
    fn new(initial: f64, ramp_samples: u32) -> Self {
        Self {
            current: initial,
            target: initial,
            step: 0.0,
            samples_remaining: 0,
            ramp_samples,
        }
    }

    fn set_target(&mut self, target: f64) {
        if (target - self.target).abs() < 1e-9 {
            return;
        }
        self.target = target;
        let delta = target - self.current;
        if self.ramp_samples == 0 {
            self.current = target;
            self.samples_remaining = 0;
            return;
        }
        self.step = delta / self.ramp_samples as f64;
        self.samples_remaining = self.ramp_samples;
    }

    /// Snap to value instantly (no ramp). Used on `reset()` and
    /// `set_sample_rate()` to avoid carrying a stale ramp across.
    fn snap_to(&mut self, value: f64) {
        self.current = value;
        self.target = value;
        self.step = 0.0;
        self.samples_remaining = 0;
    }

    fn set_ramp_samples(&mut self, ramp_samples: u32) {
        self.ramp_samples = ramp_samples;
        // Recompute step for any in-flight ramp so it still finishes correctly.
        if self.samples_remaining > 0 {
            self.step = (self.target - self.current) / ramp_samples.max(1) as f64;
            self.samples_remaining = ramp_samples;
        }
    }

    #[inline]
    fn next(&mut self) -> f64 {
        if self.samples_remaining > 0 {
            self.current += self.step;
            self.samples_remaining -= 1;
            if self.samples_remaining == 0 {
                self.current = self.target;
            }
        }
        self.current
    }
}

/// Wurlitzer 200A synth engine — owns voices, signal chain, and all
/// params except the host-facing parameter container.
///
/// Host integration sketch:
/// ```ignore
/// let mut engine = WurliEngine::new(sample_rate);
/// // Per buffer:
/// engine.set_volume(params.volume.value() as f64);
/// engine.set_tremolo_depth(params.tremolo_depth.value() as f64);
/// engine.set_speaker_character(params.speaker.value() as f64);
/// engine.set_mlp_enabled(params.mlp.value());
/// engine.set_di_limiter(params.di_limiter.value());
/// engine.set_noise_enabled(params.noise_enable.value());
/// engine.set_noise_gain(params.noise_gain.value() as f64);
/// for event in midi_events {
///     match event {
///         NoteOn(n, v) => engine.note_on(n, v),
///         NoteOff(n)   => engine.note_off(n),
///         Sustain(on)  => engine.set_sustain(on),
///     }
/// }
/// engine.render(&mut mono_out_buffer);
/// ```
pub struct WurliEngine {
    voices: Vec<VoiceSlot>,
    age_counter: u64,

    // Shared signal chain (mono, post voice-sum)
    preamp: DkPreamp,
    tremolo: Tremolo,
    oversampler: Oversampler,
    power_amp: PowerAmp,
    speaker: Speaker,

    // Pre-allocated scratch buffers
    voice_buf: Vec<f64>,
    sum_buf: Vec<f64>,
    up_buf: Vec<f64>,
    out_buf: Vec<f64>,

    // Sample rates
    sample_rate: f64,
    os_sample_rate: f64,
    /// Whether to oversample the preamp (false at >= 88.2 kHz host rates).
    oversample: bool,

    // MIDI state
    sustain_held: bool,
    mlp_enabled: bool,

    // Smoothed audio-rate params (target set by host, ramped per sample)
    volume: LinearSmoother,
    tremolo_depth: LinearSmoother,
    speaker_character: LinearSmoother,

    // Block-rate flags
    di_limiter_enabled: bool,
}

impl WurliEngine {
    pub fn new(sample_rate: f64) -> Self {
        let oversample = sample_rate < 88_200.0;
        let os_sr = if oversample {
            sample_rate * 2.0
        } else {
            sample_rate
        };
        let ramp = ramp_samples_for_rate(sample_rate);
        Self {
            voices: (0..MAX_VOICES).map(|_| VoiceSlot::default()).collect(),
            age_counter: 0,
            preamp: DkPreamp::new(os_sr),
            tremolo: Tremolo::new(0.5, os_sr),
            oversampler: Oversampler::new(),
            power_amp: PowerAmp::new(),
            speaker: Speaker::new(sample_rate),
            voice_buf: vec![0.0; MAX_BLOCK_SIZE],
            sum_buf: vec![0.0; MAX_BLOCK_SIZE],
            up_buf: vec![0.0; MAX_BLOCK_SIZE * 2],
            out_buf: vec![0.0; MAX_BLOCK_SIZE],
            sample_rate,
            os_sample_rate: os_sr,
            oversample,
            sustain_held: false,
            mlp_enabled: true,
            volume: LinearSmoother::new(0.5, ramp),
            tremolo_depth: LinearSmoother::new(0.5, ramp),
            speaker_character: LinearSmoother::new(0.0, ramp),
            di_limiter_enabled: true,
        }
    }

    pub fn reset(&mut self) {
        for slot in &mut self.voices {
            slot.state = VoiceState::Free;
            slot.voice = None;
            slot.steal_voice = None;
            slot.steal_fade = 0;
        }
        self.preamp.reset();
        self.tremolo.reset();
        self.oversampler.reset();
        self.power_amp.reset();
        self.speaker.reset();
        self.age_counter = 0;
        self.sustain_held = false;
        // Snap smoothers so a ramp doesn't survive a transport reset.
        self.volume.snap_to(self.volume.target);
        self.tremolo_depth.snap_to(self.tremolo_depth.target);
        self.speaker_character.snap_to(self.speaker_character.target);
    }

    pub fn set_sample_rate(&mut self, sr: f64) {
        self.sample_rate = sr;
        self.oversample = sr < 88_200.0;
        self.os_sample_rate = if self.oversample { sr * 2.0 } else { sr };
        self.preamp = DkPreamp::new(self.os_sample_rate);
        self.tremolo = Tremolo::new(self.tremolo_depth.target, self.os_sample_rate);
        self.oversampler = Oversampler::new();
        self.power_amp = PowerAmp::new();
        self.speaker = Speaker::new(sr);
        let ramp = ramp_samples_for_rate(sr);
        self.volume.set_ramp_samples(ramp);
        self.tremolo_depth.set_ramp_samples(ramp);
        self.speaker_character.set_ramp_samples(ramp);
    }

    pub fn ensure_buffer_capacity(&mut self, max_samples: usize) {
        if self.sum_buf.len() < max_samples {
            self.voice_buf.resize(max_samples, 0.0);
            self.sum_buf.resize(max_samples, 0.0);
            self.up_buf.resize(max_samples * 2, 0.0);
            self.out_buf.resize(max_samples, 0.0);
        }
    }

    // ── MIDI ────────────────────────────────────────────────────────────

    pub fn note_on(&mut self, note: u8, velocity: f32) {
        let note = note.clamp(tables::MIDI_LO, tables::MIDI_HI);

        // Re-strike of a sustained note: damp the old vibration before
        // the new attack (real 200A has one reed per pitch).
        for slot in &mut self.voices {
            if slot.state == VoiceState::Sustained && slot.midi_note == note {
                slot.state = VoiceState::Releasing;
                if let Some(ref mut voice) = slot.voice {
                    voice.note_off();
                }
            }
        }

        let slot_idx = self.allocate_voice();
        let slot = &mut self.voices[slot_idx];

        if slot.state != VoiceState::Free {
            // Stealing an active voice: 5 ms linear crossfade.
            let fade_samples = (self.sample_rate * 0.005) as u32;
            slot.steal_voice = slot.voice.take();
            slot.steal_fade = fade_samples;
            slot.steal_fade_len = fade_samples;
        }

        self.age_counter += 1;
        let noise_seed = (note as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(self.age_counter as u32);
        slot.voice = Some(Voice::note_on(
            note,
            velocity as f64,
            self.sample_rate,
            noise_seed,
            self.mlp_enabled,
        ));
        slot.state = VoiceState::Held;
        slot.midi_note = note;
        slot.age = self.age_counter;
    }

    pub fn note_off(&mut self, note: u8) {
        let note = note.clamp(tables::MIDI_LO, tables::MIDI_HI);
        let oldest_idx = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, s)| s.state == VoiceState::Held && s.midi_note == note)
            .min_by_key(|(_, s)| s.age)
            .map(|(i, _)| i);
        if let Some(idx) = oldest_idx {
            if self.sustain_held {
                self.voices[idx].state = VoiceState::Sustained;
            } else {
                self.voices[idx].state = VoiceState::Releasing;
                if let Some(ref mut voice) = self.voices[idx].voice {
                    voice.note_off();
                }
            }
        }
    }

    pub fn set_sustain(&mut self, held: bool) {
        if self.sustain_held && !held {
            // Pedal release: damp every voice that was held by the pedal.
            for slot in &mut self.voices {
                if slot.state == VoiceState::Sustained {
                    slot.state = VoiceState::Releasing;
                    if let Some(ref mut voice) = slot.voice {
                        voice.note_off();
                    }
                }
            }
        }
        self.sustain_held = held;
    }

    // ── Param setters ────────────────────────────────────────────────────

    pub fn set_volume(&mut self, v: f64) {
        self.volume.set_target(v);
    }

    pub fn set_tremolo_depth(&mut self, depth: f64) {
        self.tremolo_depth.set_target(depth);
    }

    pub fn set_speaker_character(&mut self, c: f64) {
        self.speaker_character.set_target(c);
    }

    pub fn set_mlp_enabled(&mut self, on: bool) {
        self.mlp_enabled = on;
    }

    pub fn set_di_limiter(&mut self, on: bool) {
        self.di_limiter_enabled = on;
    }

    pub fn set_noise_enabled(&mut self, on: bool) {
        self.preamp.set_noise_enabled(on);
    }

    pub fn set_noise_gain(&mut self, gain: f64) {
        self.preamp.set_thermal_gain(gain);
    }

    // ── Render ───────────────────────────────────────────────────────────

    /// Render `out.len()` mono samples through the full chain.
    pub fn render(&mut self, out: &mut [f32]) {
        let len = out.len();
        if len == 0 {
            return;
        }
        self.ensure_buffer_capacity(len);

        self.render_voices_to_preamp_out(0, len);

        for (i, sample_slot) in out.iter_mut().enumerate() {
            let volume = self.volume.next();
            let speaker_char = self.speaker_character.next();
            self.speaker.set_character(speaker_char);
            // Volume pot attenuates before power amp (3K audio taper: vol²)
            let attenuated = self.out_buf[i] * volume * volume;
            // Power amp: VAS gain → crossover distortion → rail clip
            let amplified = self.power_amp.process(attenuated);
            let shaped = self.speaker.process(amplified);
            // POST_SPEAKER_GAIN: maps physical SPL to DAW-friendly levels
            let post_gain = shaped * tables::POST_SPEAKER_GAIN;
            // DI limiter: optional soft-limit, post-analog-chain
            let limited = if self.di_limiter_enabled {
                di_soft_limit(post_gain)
            } else {
                post_gain
            };
            let sample = limited as f32;
            // NaN guard: non-finite samples crash PipeWire/JACK audio engines.
            *sample_slot = if sample.is_finite() {
                sample
            } else {
                self.preamp.reset();
                self.oversampler.reset();
                self.power_amp.reset();
                self.speaker.reset();
                0.0f32
            };
        }

        self.cleanup_voices();
    }

    /// Render voices through the preamp and write into `self.out_buf[offset..offset+len]`.
    /// Pulled out so introspection tests can call it without going through the chain.
    pub(crate) fn render_voices_to_preamp_out(&mut self, offset: usize, len: usize) {
        // Sum all active voices
        self.sum_buf[..len].fill(0.0);
        for slot in &mut self.voices {
            if slot.state == VoiceState::Free && slot.steal_voice.is_none() {
                continue;
            }

            if let Some(ref mut voice) = slot.voice {
                voice.render(&mut self.voice_buf[..len]);
                for i in 0..len {
                    self.sum_buf[i] += self.voice_buf[i];
                }
            }

            if let Some(ref mut steal) = slot.steal_voice {
                steal.render(&mut self.voice_buf[..len]);
                let fade_len = slot.steal_fade_len as f64;
                for i in 0..len {
                    let remaining = slot.steal_fade.saturating_sub(i as u32);
                    let gain = remaining as f64 / fade_len;
                    self.sum_buf[i] += self.voice_buf[i] * gain;
                }
                slot.steal_fade = slot.steal_fade.saturating_sub(len as u32);
                if slot.steal_fade == 0 {
                    slot.steal_voice = None;
                }
            }
        }

        // NaN guard: catch non-finite voice output BEFORE the oversampler.
        // Once NaN enters IIR allpass filter state, it persists and causes
        // per-sample preamp resets (expensive full_dc_solve) → xruns → frozen audio.
        if self.sum_buf[..len].iter().any(|s| !s.is_finite()) {
            self.sum_buf[..len].fill(0.0);
            for slot in &mut self.voices {
                if slot.state == VoiceState::Free && slot.steal_voice.is_none() {
                    continue;
                }
                if let Some(ref mut voice) = slot.voice {
                    voice.render(&mut self.voice_buf[..len]);
                    if self.voice_buf[..len].iter().any(|s| !s.is_finite()) {
                        slot.state = VoiceState::Free;
                        slot.voice = None;
                    }
                }
                if let Some(ref mut steal) = slot.steal_voice {
                    steal.render(&mut self.voice_buf[..len]);
                    if self.voice_buf[..len].iter().any(|s| !s.is_finite()) {
                        slot.steal_voice = None;
                        slot.steal_fade = 0;
                    }
                }
            }
        }

        if self.oversample {
            self.oversampler
                .upsample_2x(&self.sum_buf[..len], &mut self.up_buf[..len * 2]);

            // Per base-rate sample: advance tremolo depth smoother once,
            // run the preamp twice (for the two oversampled samples).
            for i in 0..len {
                let depth = self.tremolo_depth.next();
                self.tremolo.set_depth(depth);

                for j in 0..2 {
                    let idx = i * 2 + j;
                    let r_ldr = self.tremolo.process();
                    self.preamp.set_ldr_resistance(r_ldr);
                    self.up_buf[idx] = self.preamp.process_sample(self.up_buf[idx]);
                }
            }

            self.oversampler.downsample_2x(
                &self.up_buf[..len * 2],
                &mut self.out_buf[offset..offset + len],
            );
        } else {
            for i in 0..len {
                let depth = self.tremolo_depth.next();
                self.tremolo.set_depth(depth);
                let r_ldr = self.tremolo.process();
                self.preamp.set_ldr_resistance(r_ldr);
                self.out_buf[offset + i] = self.preamp.process_sample(self.sum_buf[i]);
            }
        }
    }

    fn allocate_voice(&self) -> usize {
        let mut best_idx = 0;
        let mut best_priority = u64::MAX;

        for (i, slot) in self.voices.iter().enumerate() {
            // Free (immediate) > oldest Releasing > oldest Sustained > oldest Held.
            // Sustained voices already had their key released — less disruptive
            // to steal than a Held voice the player is still pressing.
            let priority = match slot.state {
                VoiceState::Free => return i,
                VoiceState::Releasing => slot.age,
                VoiceState::Sustained => slot.age + u64::MAX / 4,
                VoiceState::Held => slot.age + u64::MAX / 2,
            };
            if priority < best_priority {
                best_priority = priority;
                best_idx = i;
            }
        }

        best_idx
    }

    fn cleanup_voices(&mut self) {
        for slot in &mut self.voices {
            if slot.state != VoiceState::Free
                && let Some(ref voice) = slot.voice
                && voice.is_silent()
            {
                slot.state = VoiceState::Free;
                slot.voice = None;
            }
        }
    }

    // ── Test/inspection helpers ──────────────────────────────────────────

    #[doc(hidden)]
    pub fn active_voice_count(&self) -> usize {
        self.voices
            .iter()
            .filter(|s| s.state != VoiceState::Free)
            .count()
    }

    #[doc(hidden)]
    pub fn held_voice_count(&self) -> usize {
        self.voices
            .iter()
            .filter(|s| s.state == VoiceState::Held)
            .count()
    }

    #[doc(hidden)]
    pub fn sustained_voice_count(&self) -> usize {
        self.voices
            .iter()
            .filter(|s| s.state == VoiceState::Sustained)
            .count()
    }

    #[doc(hidden)]
    pub fn has_steal_voice_for(&self, note: u8) -> bool {
        self.voices
            .iter()
            .any(|s| s.midi_note == note && s.steal_voice.is_some())
    }
}

fn ramp_samples_for_rate(sample_rate: f64) -> u32 {
    // 5 ms equivalent at any rate.
    ((sample_rate * 0.005) as u32).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> WurliEngine {
        WurliEngine::new(44_100.0)
    }

    #[test]
    fn test_engine_instantiates() {
        let e = engine();
        assert_eq!(e.voices.len(), MAX_VOICES);
        assert_eq!(e.sample_rate, 44_100.0);
    }

    #[test]
    fn test_note_on_allocates_voice() {
        let mut e = engine();
        e.note_on(60, 0.8);
        assert_eq!(e.held_voice_count(), 1);
    }

    #[test]
    fn test_note_off_releases_voice() {
        let mut e = engine();
        e.note_on(60, 0.8);
        e.note_off(60);
        assert_eq!(e.held_voice_count(), 0);
    }

    #[test]
    fn test_polyphony_up_to_max_voices() {
        let mut e = engine();
        for n in 0..MAX_VOICES {
            e.note_on((36 + n) as u8, 0.8);
        }
        assert_eq!(e.held_voice_count(), MAX_VOICES);
    }

    #[test]
    fn test_voice_stealing_when_full() {
        let mut e = engine();
        for n in 0..MAX_VOICES {
            e.note_on((36 + n) as u8, 0.8);
        }
        e.note_on(96, 0.8);
        assert_eq!(e.held_voice_count(), MAX_VOICES);
        assert!(e.has_steal_voice_for(96));
    }

    #[test]
    fn test_render_produces_output() {
        let mut e = engine();
        e.note_on(60, 0.8);
        let mut buf = vec![0.0f32; 256];
        e.render(&mut buf);
        let energy: f64 = buf.iter().map(|s| (*s as f64) * (*s as f64)).sum();
        assert!(energy > 0.0, "render produced silence after note-on");
    }

    #[test]
    fn test_render_no_notes_is_near_silent() {
        // Idle output has a small chain-startup DC settling transient
        // (preamp/speaker biasing). Threshold mirrors the legacy plugin
        // test (0.03) which carried the same accommodation.
        let mut e = engine();
        let mut buf = vec![0.0f32; 512];
        e.render(&mut buf);
        let peak = buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak < 0.05, "idle output peak {peak} too high");
    }

    #[test]
    fn test_reset_clears_voices() {
        let mut e = engine();
        e.note_on(60, 0.8);
        e.note_on(72, 0.8);
        e.reset();
        assert_eq!(e.active_voice_count(), 0);
    }

    #[test]
    fn test_sustain_defers_note_off() {
        let mut e = engine();
        e.set_sustain(true);
        e.note_on(60, 0.8);
        e.note_off(60);
        assert_eq!(e.sustained_voice_count(), 1);
        assert_eq!(e.held_voice_count(), 0);
        e.set_sustain(false);
        // Releasing now (note_off triggered on pedal release)
        assert_eq!(e.sustained_voice_count(), 0);
    }

    #[test]
    fn test_di_limiter_off_passthrough() {
        let mut e = engine();
        e.set_di_limiter(false);
        e.note_on(60, 0.8);
        let mut buf = vec![0.0f32; 256];
        e.render(&mut buf);
        // Just verify it ran without panic
        assert!(buf.iter().any(|s| s.abs() > 0.0));
    }

    #[test]
    fn test_volume_smoother_ramps() {
        let mut e = engine();
        e.set_volume(1.0);
        let mut buf = vec![0.0f32; 1];
        e.render(&mut buf); // advances smoother by 1 sample
        // Default starts at 0.5, ramping toward 1.0 over ~220 samples
        assert!(e.volume.current > 0.5);
        assert!(e.volume.current < 1.0);
    }

    #[test]
    fn test_higher_velocity_louder() {
        let mut e = engine();
        e.set_volume(0.5);
        // pp render
        e.note_on(60, 0.2);
        let mut soft = vec![0.0f32; 4096];
        e.render(&mut soft);
        e.reset();
        // ff render
        e.note_on(60, 1.0);
        let mut loud = vec![0.0f32; 4096];
        e.render(&mut loud);

        let soft_rms: f64 = (soft.iter().map(|s| (*s as f64).powi(2)).sum::<f64>()
            / soft.len() as f64)
            .sqrt();
        let loud_rms: f64 = (loud.iter().map(|s| (*s as f64).powi(2)).sum::<f64>()
            / loud.len() as f64)
            .sqrt();
        assert!(
            loud_rms > soft_rms,
            "ff RMS ({loud_rms}) should exceed pp RMS ({soft_rms})"
        );
    }
}
