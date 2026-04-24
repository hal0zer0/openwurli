// OpenWurli — Wurlitzer 200A virtual instrument plugin (CLAP + VST3).

use nih_plug::midi::control_change;
use nih_plug::prelude::*;
use openwurli_dsp::dk_preamp::DkPreamp;
use openwurli_dsp::oversampler::Oversampler;
use openwurli_dsp::power_amp::PowerAmp;
use openwurli_dsp::preamp::PreampModel;
use openwurli_dsp::speaker::Speaker;
use openwurli_dsp::tables;
use openwurli_dsp::tremolo::Tremolo;
use openwurli_dsp::voice::Voice;
use std::num::NonZeroU32;
use std::sync::Arc;

mod params;
use params::OpenWurliParams;

const MAX_VOICES: usize = 64;
const MAX_BLOCK_SIZE: usize = 8192;

// Signal path: reed → pickup (1/(1-y) nonlinearity + HPF) → preamp (DK method)
// → volume pot (attenuator) → power amp (VAS gain + crossover + clip) → speaker.
// The pickup's 1/(1-y) nonlinearity is the primary source of Wurlitzer bark.

// ── Voice management ────────────────────────────────────────────────────────

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
    // Voice being faded out from stealing (5ms linear crossfade)
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

// ── Plugin ──────────────────────────────────────────────────────────────────

struct OpenWurli {
    params: Arc<OpenWurliParams>,

    // Voice management
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

    /// Sustain pedal state (CC64 >= 0.5 = held).
    sustain_held: bool,
}

impl Default for OpenWurli {
    fn default() -> Self {
        let sr = 44100.0;
        let os_sr = sr * 2.0;
        Self {
            params: Arc::new(OpenWurliParams::default()),
            voices: (0..MAX_VOICES).map(|_| VoiceSlot::default()).collect(),
            age_counter: 0,
            preamp: DkPreamp::new(os_sr),
            tremolo: Tremolo::new(0.5, os_sr),
            oversampler: Oversampler::new(),
            power_amp: PowerAmp::new(),
            speaker: Speaker::new(sr),
            voice_buf: vec![0.0; MAX_BLOCK_SIZE],
            sum_buf: vec![0.0; MAX_BLOCK_SIZE],
            up_buf: vec![0.0; MAX_BLOCK_SIZE * 2],
            out_buf: vec![0.0; MAX_BLOCK_SIZE],
            sample_rate: sr,
            os_sample_rate: os_sr,
            oversample: true,
            sustain_held: false,
        }
    }
}

impl OpenWurli {
    /// Defensively pin each smoother's current and target value to its param's current value.
    /// The framework normally does this via `update_smoother(sr, true)` in `activate()` before
    /// `initialize()` runs, but we call it explicitly inside `initialize()` too so any wrapper
    /// / DAW path that skips that handshake still lands on the correct starting value. Without
    /// this, the per-sample `smoothed.next()` consumer in `render_subblock` returns 0.0 forever
    /// while `.value()` returns the correct default — tremolo / volume stuck silent. See
    /// `test_tremolo_smoother_does_not_pin_depth_to_zero` for the repro.
    fn reset_param_smoothers_to_current(&self) {
        self.params
            .volume
            .smoothed
            .reset(self.params.volume.value());
        self.params
            .tremolo_depth
            .smoothed
            .reset(self.params.tremolo_depth.value());
        self.params
            .speaker_character
            .smoothed
            .reset(self.params.speaker_character.value());
    }

    fn note_on(&mut self, note: u8, velocity: f32, mlp_enabled: bool) {
        let note = note.clamp(tables::MIDI_LO, tables::MIDI_HI);

        // Release any sustained voice for this note — real 200A has one reed per pitch,
        // so re-striking a sustained note damps the old vibration before the new attack.
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

        // If stealing an active voice, crossfade it out over 5ms
        if slot.state != VoiceState::Free {
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
            mlp_enabled,
        ));
        slot.state = VoiceState::Held;
        slot.midi_note = note;
        slot.age = self.age_counter;
    }

    fn note_off(&mut self, note: u8) {
        let note = note.clamp(tables::MIDI_LO, tables::MIDI_HI);
        // Release the oldest held voice matching this note
        let oldest_idx = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, s)| s.state == VoiceState::Held && s.midi_note == note)
            .min_by_key(|(_, s)| s.age)
            .map(|(i, _)| i);
        if let Some(idx) = oldest_idx {
            if self.sustain_held {
                // Pedal down: reed rings undamped until pedal release
                self.voices[idx].state = VoiceState::Sustained;
            } else {
                self.voices[idx].state = VoiceState::Releasing;
                if let Some(ref mut voice) = self.voices[idx].voice {
                    voice.note_off();
                }
            }
        }
    }

    /// Release all sustained voices (called when sustain pedal goes up).
    fn release_sustained(&mut self) {
        for slot in &mut self.voices {
            if slot.state == VoiceState::Sustained {
                slot.state = VoiceState::Releasing;
                if let Some(ref mut voice) = slot.voice {
                    voice.note_off();
                }
            }
        }
    }

    /// Find a voice slot: prefer Free, then oldest Releasing, Sustained, then Held.
    fn allocate_voice(&self) -> usize {
        let mut best_idx = 0;
        let mut best_priority = u64::MAX;

        for (i, slot) in self.voices.iter().enumerate() {
            // Priority: Free (immediate) > oldest Releasing > oldest Sustained > oldest Held.
            // Sustained voices already had their key released — less disruptive to steal
            // than a Held voice the player is still pressing.
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

    /// Render a sub-block of audio: voices -> preamp -> output buffer.
    fn render_subblock(&mut self, offset: usize, len: usize) {
        // Sum all active voices
        self.sum_buf[..len].fill(0.0);
        for slot in &mut self.voices {
            if slot.state == VoiceState::Free && slot.steal_voice.is_none() {
                continue;
            }

            // Render the main voice
            if let Some(ref mut voice) = slot.voice {
                voice.render(&mut self.voice_buf[..len]);
                for i in 0..len {
                    self.sum_buf[i] += self.voice_buf[i];
                }
            }

            // Render the stealing voice with linear fade-out
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
            // Kill the offending voice(s) immediately — don't wait for the
            // 10-second is_silent() timeout which would cause sustained xruns.
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
            // Upsample to 2x rate
            self.oversampler
                .upsample_2x(&self.sum_buf[..len], &mut self.up_buf[..len * 2]);

            // Process through preamp at oversampled rate.
            // Advance tremolo smoothers once per base-rate sample (physical: continuous
            // pot rotation). Each base-rate sample produces 2 oversampled preamp steps.
            for i in 0..len {
                let depth = self.params.tremolo_depth.smoothed.next() as f64;
                self.tremolo.set_depth(depth);

                for j in 0..2 {
                    let idx = i * 2 + j;
                    let r_ldr = self.tremolo.process();
                    self.preamp.set_ldr_resistance(r_ldr);
                    self.up_buf[idx] = self.preamp.process_sample(self.up_buf[idx]);
                }
            }

            // Downsample back to base rate
            self.oversampler.downsample_2x(
                &self.up_buf[..len * 2],
                &mut self.out_buf[offset..offset + len],
            );
        } else {
            // At >= 88.2 kHz: preamp runs at native rate, no oversampling needed
            for i in 0..len {
                let depth = self.params.tremolo_depth.smoothed.next() as f64;
                self.tremolo.set_depth(depth);

                let r_ldr = self.tremolo.process();
                self.preamp.set_ldr_resistance(r_ldr);
                self.out_buf[offset + i] = self.preamp.process_sample(self.sum_buf[i]);
            }
        }
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
}

impl Plugin for OpenWurli {
    const NAME: &'static str = "OpenWurli";
    const VENDOR: &'static str = "OpenWurli";
    const URL: &'static str = "https://github.com/hal0zer0/openwurli";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate as f64;
        // Skip 2x oversampling at 88.2kHz+ — Nyquist is already above preamp BW
        self.oversample = self.sample_rate < 88200.0;
        self.os_sample_rate = if self.oversample {
            self.sample_rate * 2.0
        } else {
            self.sample_rate
        };

        // Reinitialize DSP modules at correct sample rate
        self.preamp = DkPreamp::new(self.os_sample_rate);
        self.tremolo = Tremolo::new(
            self.params.tremolo_depth.value() as f64,
            self.os_sample_rate,
        );
        self.oversampler = Oversampler::new();
        self.power_amp = PowerAmp::new();
        self.speaker = Speaker::new(self.sample_rate);

        // Ensure buffers are large enough
        let max_samples = buffer_config.max_buffer_size as usize;
        if self.sum_buf.len() < max_samples {
            self.voice_buf.resize(max_samples, 0.0);
            self.sum_buf.resize(max_samples, 0.0);
            self.up_buf.resize(max_samples * 2, 0.0);
            self.out_buf.resize(max_samples, 0.0);
        }

        self.reset_param_smoothers_to_current();

        true
    }

    fn reset(&mut self) {
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
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let num_samples = buffer.samples();

        // Tremolo rate/depth: set once per buffer from .value() as a baseline
        // (ensures correct state even before smoothers are host-initialized),
        // then smoothed per-sample inside render_subblock() for automation.
        let trem_depth = self.params.tremolo_depth.value() as f64;
        self.tremolo.set_depth(trem_depth);

        // Event-splitting process loop: split at each MIDI event for sample-accuracy
        let mut next_event = context.next_event();
        let mut block_start: usize = 0;
        let mlp_on = self.params.mlp_enabled.value();

        while block_start < num_samples {
            // Process all events at or before current position
            loop {
                match next_event {
                    Some(ref event) if (event.timing() as usize) <= block_start => {
                        match event {
                            NoteEvent::NoteOn { note, velocity, .. } => {
                                self.note_on(*note, *velocity, mlp_on);
                            }
                            NoteEvent::NoteOff { note, .. } => {
                                self.note_off(*note);
                            }
                            NoteEvent::MidiCC { cc, value, .. }
                                if *cc == control_change::DAMPER_PEDAL =>
                            {
                                let pedal_down = *value >= 0.5;
                                if self.sustain_held && !pedal_down {
                                    self.release_sustained();
                                }
                                self.sustain_held = pedal_down;
                            }
                            _ => {}
                        }
                        next_event = context.next_event();
                    }
                    _ => break,
                }
            }

            // Find next event boundary (or end of buffer)
            let block_end = match next_event {
                Some(ref event) => (event.timing() as usize).min(num_samples),
                None => num_samples,
            };
            let block_len = block_end - block_start;

            if block_len > 0 {
                self.render_subblock(block_start, block_len);
            }

            block_start = block_end;
        }

        // Drain any remaining events
        while let Some(event) = next_event {
            match event {
                NoteEvent::NoteOn { note, velocity, .. } => self.note_on(note, velocity, mlp_on),
                NoteEvent::NoteOff { note, .. } => self.note_off(note),
                NoteEvent::MidiCC { cc, value, .. } if cc == control_change::DAMPER_PEDAL => {
                    let pedal_down = value >= 0.5;
                    if self.sustain_held && !pedal_down {
                        self.release_sustained();
                    }
                    self.sustain_held = pedal_down;
                }
                _ => {}
            }
            next_event = context.next_event();
        }

        // Signal chain: preamp -> volume pot -> power amp (gain + crossover + clip) -> speaker
        // Matches real 200A topology: volume pot sits between preamp and power amp.
        // Power amp has internal voltage gain (VAS/driver stages).

        for (i, mut channel_samples) in buffer.iter_samples().enumerate() {
            let volume = self.params.volume.smoothed.next() as f64;
            // Speaker character smoothed per-sample to prevent biquad coefficient
            // discontinuities (HPF 20→95 Hz, LPF 20k→5.5k Hz) that cause clicks.
            let speaker_char = self.params.speaker_character.smoothed.next() as f64;
            self.speaker.set_character(speaker_char);
            // Volume pot attenuates before power amp (3K audio taper: vol² approximation)
            let attenuated = self.out_buf[i] * volume * volume;
            // Power amp: VAS gain → crossover distortion → rail clip
            let amplified = self.power_amp.process(attenuated);
            let shaped = self.speaker.process(amplified);
            // Post-speaker gain: maps physical SPL to DAW-friendly levels.
            // Applied after all analog stages — no circuit model distortion.
            let sample = (shaped * tables::POST_SPEAKER_GAIN) as f32;
            // NaN guard: non-finite samples crash PipeWire/JACK audio engines.
            // If any stage diverged, output silence and reset stateful stages.
            let sample = if sample.is_finite() {
                sample
            } else {
                self.preamp.reset();
                self.oversampler.reset();
                self.power_amp.reset();
                self.speaker.reset();
                0.0f32
            };
            for s in channel_samples.iter_mut() {
                *s = sample;
            }
        }

        self.cleanup_voices();

        ProcessStatus::Normal
    }
}

impl ClapPlugin for OpenWurli {
    const CLAP_ID: &'static str = "com.openwurli.wurlitzer-200a";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Wurlitzer 200A electric piano — analog circuit simulation");
    const CLAP_MANUAL_URL: Option<&'static str> =
        Some("https://github.com/hal0zer0/openwurli/tree/main/docs");
    const CLAP_SUPPORT_URL: Option<&'static str> =
        Some("https://github.com/hal0zer0/openwurli/issues");
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Custom("electric-piano"),
    ];
}

impl Vst3Plugin for OpenWurli {
    const VST3_CLASS_ID: [u8; 16] = *b"OpenWurli200AVST";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(OpenWurli);
nih_export_vst3!(OpenWurli);

#[cfg(test)]
mod tests {
    use super::*;

    fn make_plugin() -> OpenWurli {
        OpenWurli::default()
    }

    #[test]
    fn test_plugin_instantiates() {
        let plugin = make_plugin();
        assert_eq!(plugin.voices.len(), MAX_VOICES);
        assert_eq!(plugin.sample_rate, 44100.0);
    }

    #[test]
    fn test_params_have_correct_defaults() {
        let params = OpenWurliParams::default();
        assert!((params.volume.default_plain_value() - 0.50).abs() < 0.01);
        assert!((params.tremolo_depth.default_plain_value() - 0.5).abs() < 0.01);
        assert!((params.speaker_character.default_plain_value()).abs() < 0.01);
        assert!(params.mlp_enabled.default_plain_value());
    }

    #[test]
    fn test_note_on_allocates_voice() {
        let mut plugin = make_plugin();
        plugin.note_on(60, 0.8, true);
        let active = plugin
            .voices
            .iter()
            .filter(|s| s.state == VoiceState::Held)
            .count();
        assert_eq!(active, 1);
        assert_eq!(
            plugin
                .voices
                .iter()
                .find(|s| s.state == VoiceState::Held)
                .unwrap()
                .midi_note,
            60
        );
    }

    #[test]
    fn test_note_off_releases_voice() {
        let mut plugin = make_plugin();
        plugin.note_on(60, 0.8, true);
        plugin.note_off(60);
        let releasing = plugin
            .voices
            .iter()
            .filter(|s| s.state == VoiceState::Releasing)
            .count();
        assert_eq!(releasing, 1);
    }

    #[test]
    fn test_polyphony_up_to_max_voices() {
        let mut plugin = make_plugin();
        for note in 48..48 + MAX_VOICES as u8 {
            plugin.note_on(note, 0.8, true);
        }
        let active = plugin
            .voices
            .iter()
            .filter(|s| s.state == VoiceState::Held)
            .count();
        assert_eq!(active, MAX_VOICES);
    }

    #[test]
    fn test_voice_stealing_when_full() {
        let mut plugin = make_plugin();
        // Fill all voices
        for note in 48..48 + MAX_VOICES as u8 {
            plugin.note_on(note, 0.8, true);
        }
        // One more should steal the oldest
        plugin.note_on(96, 0.8, true);
        let active = plugin
            .voices
            .iter()
            .filter(|s| s.state == VoiceState::Held)
            .count();
        assert_eq!(active, MAX_VOICES);
        // The stolen slot should have a steal_voice for crossfade
        let stolen_slot = plugin.voices.iter().find(|s| s.midi_note == 96).unwrap();
        assert!(stolen_slot.steal_voice.is_some());
    }

    #[test]
    fn test_note_clamps_to_valid_range() {
        let mut plugin = make_plugin();
        // Notes below MIDI_LO should be clamped, not panic
        plugin.note_on(0, 0.8, true);
        plugin.note_on(127, 0.8, true);
        let active = plugin
            .voices
            .iter()
            .filter(|s| s.state == VoiceState::Held)
            .count();
        assert_eq!(active, 2);
    }

    #[test]
    fn test_render_subblock_produces_output() {
        let mut plugin = make_plugin();
        plugin.note_on(60, 0.8, true);
        let len = 256;
        plugin.render_subblock(0, len);
        // After a note-on, output buffer should have non-zero samples
        let energy: f64 = plugin.out_buf[..len].iter().map(|s| s * s).sum();
        assert!(
            energy > 0.0,
            "render_subblock produced silence after note-on"
        );
    }

    #[test]
    fn test_tremolo_smoother_does_not_pin_depth_to_zero() {
        // Regression test: with tremolo_depth default = 0.5, rendering a 4s note should
        // produce audible amplitude modulation (~6 Hz Twin-T oscillator → ~6 dB swing
        // through the preamp, amplified by the power amp). Before the smoother fix, the
        // per-sample `smoothed.next()` consumer returned 0.0 and set the depth to 0 on
        // every sample, wiping out the modulation.
        let mut plugin = make_plugin();
        // The DAW framework calls `update_smoother(sr, true)` on every param before
        // `initialize()` runs. We mirror that here so the test path matches the real
        // DAW handshake, then rely on the defensive reset inside `initialize()` (via
        // `reset_param_smoothers_to_current`) as the belt-and-suspenders guard.
        plugin.reset_param_smoothers_to_current();
        plugin.note_on(60, 0.9, true);

        let sr = plugin.sample_rate as usize;
        let total = sr * 4;
        let block = 256;
        let mut samples = Vec::with_capacity(total);

        for start in (0..total).step_by(block) {
            let len = (total - start).min(block);
            plugin.render_subblock(0, len);
            for i in 0..len {
                let vol = plugin.params.volume.smoothed.next() as f64;
                let speaker_char = plugin.params.speaker_character.smoothed.next() as f64;
                plugin.speaker.set_character(speaker_char);
                let attenuated = plugin.out_buf[i] * vol * vol;
                let amplified = plugin.power_amp.process(attenuated);
                let shaped = plugin.speaker.process(amplified);
                let out = shaped * tables::POST_SPEAKER_GAIN;
                samples.push(out);
            }
        }

        // RMS envelope over 20 ms windows, ignoring the first 0.5 s (attack overshoot).
        let win = sr / 50;
        let skip_wins = 25;
        let n_wins = samples.len() / win;
        let mut env_db = Vec::with_capacity(n_wins);
        for i in skip_wins..n_wins {
            let s = i * win;
            let e = s + win;
            let rms = (samples[s..e].iter().map(|x| x * x).sum::<f64>() / win as f64).sqrt();
            env_db.push(20.0 * (rms + 1e-12).log10());
        }
        let env_min = env_db.iter().cloned().fold(f64::INFINITY, f64::min);
        let env_max = env_db.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let swing = env_max - env_min;
        assert!(
            swing > 3.0,
            "Tremolo should produce > 3 dB RMS swing at default depth 0.5: got {swing:.2} dB (range {env_min:+.2} to {env_max:+.2} dBFS)"
        );
    }

    #[test]
    fn test_reset_clears_all_voices() {
        let mut plugin = make_plugin();
        plugin.note_on(60, 0.8, true);
        plugin.note_on(72, 0.8, true);
        plugin.reset();
        let active = plugin
            .voices
            .iter()
            .filter(|s| s.state != VoiceState::Free)
            .count();
        assert_eq!(active, 0);
    }

    #[test]
    fn test_render_no_notes_is_near_silent() {
        let mut plugin = make_plugin();
        let len = 512;
        plugin.render_subblock(0, len);
        let peak: f64 = plugin.out_buf[..len]
            .iter()
            .map(|s| s.abs())
            .fold(0.0, f64::max);
        // With no notes, output should be near-silent (preamp idle noise only).
        // Threshold accounts for small DC offset when nih-plug smoothers are
        // uninitialized in test context (host normally sets them before process).
        assert!(peak < 0.03, "idle output peak {peak} too high");
    }

    #[test]
    fn test_volume_zero_and_back_no_nan() {
        // Regression test: sweeping volume to zero and back caused NaN output
        // that crashed PipeWire audio engines. The logarithmic smoother produced
        // non-finite values transitioning to/from 0.0 (log(0) = -inf).
        let mut plugin = make_plugin();
        plugin.note_on(60, 0.8, true);

        let len = 256;

        let render_and_check = |plugin: &mut OpenWurli, vol: f64, label: &str| {
            plugin.render_subblock(0, len);
            for i in 0..len {
                let attenuated = plugin.out_buf[i] * vol * vol;
                let amplified = plugin.power_amp.process(attenuated);
                let shaped = plugin.speaker.process(amplified);
                let sample = (shaped * tables::POST_SPEAKER_GAIN) as f32;
                assert!(sample.is_finite(), "{label} produced NaN at sample {i}");
            }
        };

        render_and_check(&mut plugin, 0.50, "Normal volume");
        render_and_check(&mut plugin, 0.0, "Zero volume");
        // Volume back up — the transition that triggered the crash
        render_and_check(&mut plugin, 0.50, "Volume restore");
    }

    #[test]
    fn test_output_nan_guard() {
        // Verify the NaN guard catches non-finite power amp / speaker output.
        // If a NaN somehow reaches the output stage, it should be replaced with
        // silence and the stateful stages should be reset.
        let mut plugin = make_plugin();
        plugin.note_on(60, 0.8, true);
        let len = 256;
        plugin.render_subblock(0, len);

        // Inject a NaN into the output buffer (simulates upstream divergence)
        plugin.out_buf[128] = f64::NAN;

        // Process through the output chain — the NaN guard should catch it
        for i in 0..len {
            let vol = 0.50;
            let attenuated = plugin.out_buf[i] * vol * vol;
            let amplified = plugin.power_amp.process(attenuated);
            let shaped = plugin.speaker.process(amplified);
            let sample = (shaped * tables::POST_SPEAKER_GAIN) as f32;
            let safe = if sample.is_finite() { sample } else { 0.0f32 };
            assert!(safe.is_finite(), "NaN guard failed at sample {i}");
        }
    }

    #[test]
    fn test_higher_velocity_louder() {
        let mut plugin_soft = make_plugin();
        let mut plugin_loud = make_plugin();
        let len = 2048;

        plugin_soft.note_on(60, 0.3, true);
        plugin_soft.render_subblock(0, len);
        let energy_soft: f64 = plugin_soft.out_buf[..len].iter().map(|s| s * s).sum();

        plugin_loud.note_on(60, 1.0, true);
        plugin_loud.render_subblock(0, len);
        let energy_loud: f64 = plugin_loud.out_buf[..len].iter().map(|s| s * s).sum();

        assert!(
            energy_loud > energy_soft,
            "ff ({energy_loud}) should be louder than pp ({energy_soft})"
        );
    }

    // ── Sustain pedal tests ──────────────────────────────────────────────

    #[test]
    fn test_sustain_pedal_defers_note_off() {
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        plugin.note_on(60, 0.8, true);
        plugin.note_off(60);
        // Voice should be Sustained, not Releasing
        let slot = plugin.voices.iter().find(|s| s.midi_note == 60).unwrap();
        assert_eq!(slot.state, VoiceState::Sustained);
    }

    #[test]
    fn test_sustain_pedal_release_triggers_damping() {
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        plugin.note_on(60, 0.8, true);
        plugin.note_on(64, 0.8, true);
        plugin.note_off(60);
        plugin.note_off(64);
        // Both should be Sustained
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Sustained)
                .count(),
            2
        );
        // Pedal up: release all sustained voices
        plugin.sustain_held = false;
        plugin.release_sustained();
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Releasing)
                .count(),
            2
        );
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Sustained)
                .count(),
            0
        );
    }

    #[test]
    fn test_sustain_held_voices_still_render() {
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        plugin.note_on(60, 0.8, true);
        plugin.note_off(60);
        // Sustained voice should produce audio
        let len = 256;
        plugin.render_subblock(0, len);
        let energy: f64 = plugin.out_buf[..len].iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "sustained voice should produce output");
    }

    #[test]
    fn test_no_sustain_normal_note_off() {
        let mut plugin = make_plugin();
        // sustain_held is false by default
        plugin.note_on(60, 0.8, true);
        plugin.note_off(60);
        let slot = plugin.voices.iter().find(|s| s.midi_note == 60).unwrap();
        assert_eq!(slot.state, VoiceState::Releasing);
    }

    #[test]
    fn test_voice_stealing_prefers_sustained_over_held() {
        let mut plugin = make_plugin();
        // Fill all voices with notes in the valid range
        for i in 0..MAX_VOICES {
            let note = tables::MIDI_LO + (i as u8 % (tables::MIDI_HI - tables::MIDI_LO + 1));
            plugin.note_on(note, 0.8, true);
        }
        // Sustain pedal down, then release a voice — it becomes Sustained
        let sustained_note = plugin.voices[0].midi_note;
        plugin.sustain_held = true;
        plugin.note_off(sustained_note);
        assert_eq!(plugin.voices[0].state, VoiceState::Sustained);
        plugin.sustain_held = false;
        // New note should steal the sustained voice (lower priority than Held)
        let new_note = tables::MIDI_LO;
        plugin.note_on(new_note, 0.8, true);
        // The sustained slot (index 0) should have been stolen
        assert_eq!(plugin.voices[0].state, VoiceState::Held);
        assert!(plugin.voices[0].steal_voice.is_some());
    }

    #[test]
    fn test_reset_clears_sustain_state() {
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        plugin.note_on(60, 0.8, true);
        plugin.note_off(60);
        plugin.reset();
        assert!(!plugin.sustain_held);
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state != VoiceState::Free)
                .count(),
            0
        );
    }

    #[test]
    fn test_reattack_releases_sustained_same_note() {
        // Real 200A has one reed per pitch — re-striking a sustained note
        // should release the old voice, not accumulate duplicates.
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        plugin.note_on(60, 0.8, true);
        plugin.note_off(60); // → Sustained
        plugin.note_on(60, 0.8, true); // re-attack same note
        // Should have exactly one Held (new) and one Releasing (old), no Sustained
        let sustained = plugin
            .voices
            .iter()
            .filter(|s| s.state == VoiceState::Sustained && s.midi_note == 60)
            .count();
        let held = plugin
            .voices
            .iter()
            .filter(|s| s.state == VoiceState::Held && s.midi_note == 60)
            .count();
        let releasing = plugin
            .voices
            .iter()
            .filter(|s| s.state == VoiceState::Releasing && s.midi_note == 60)
            .count();
        assert_eq!(sustained, 0, "old sustained voice should be released");
        assert_eq!(held, 1, "new voice should be Held");
        assert_eq!(releasing, 1, "old voice should be Releasing");
    }

    #[test]
    fn test_pedal_down_before_notes() {
        // Pedal pressed before playing — standard legato technique.
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        // Play and release several notes while pedal is down
        for note in [60, 64, 67] {
            plugin.note_on(note, 0.8, true);
        }
        for note in [60, 64, 67] {
            plugin.note_off(note);
        }
        // All three should be Sustained
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Sustained)
                .count(),
            3
        );
        // All should produce audio
        let len = 256;
        plugin.render_subblock(0, len);
        let energy: f64 = plugin.out_buf[..len].iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "sustained chord should produce audio");

        // Pedal up releases all three
        plugin.sustain_held = false;
        plugin.release_sustained();
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Releasing)
                .count(),
            3
        );
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Sustained)
                .count(),
            0
        );
    }

    #[test]
    fn test_pedal_up_only_releases_sustained_not_held() {
        // Player holds some keys while releasing pedal — only the released
        // notes should damp, held notes should continue ringing.
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        plugin.note_on(60, 0.8, true); // C4
        plugin.note_on(64, 0.8, true); // E4
        plugin.note_on(67, 0.8, true); // G4
        plugin.note_off(60); // C4 → Sustained
        plugin.note_off(64); // E4 → Sustained
        // G4 still held (key down)

        // Pedal up
        plugin.sustain_held = false;
        plugin.release_sustained();

        // C4 and E4 should be Releasing, G4 should remain Held
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Releasing)
                .count(),
            2
        );
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Held)
                .count(),
            1
        );
        let g4 = plugin
            .voices
            .iter()
            .find(|s| s.midi_note == 67 && s.state == VoiceState::Held);
        assert!(g4.is_some(), "G4 should still be Held");
    }

    #[test]
    fn test_note_held_through_pedal_cycle() {
        // Key held while pedal goes down and back up — key was never released,
        // so the pedal cycle shouldn't affect it.
        let mut plugin = make_plugin();
        plugin.note_on(60, 0.8, true);
        plugin.sustain_held = true; // pedal down while key held
        plugin.sustain_held = false; // pedal up while key still held
        plugin.release_sustained(); // nothing to release
        // Voice should still be Held
        let slot = plugin.voices.iter().find(|s| s.midi_note == 60).unwrap();
        assert_eq!(slot.state, VoiceState::Held);
    }

    #[test]
    fn test_rapid_pedal_toggle() {
        // Quick pedal pumping — common technique for partial sustain effect.
        let mut plugin = make_plugin();

        // Play a chord
        for note in [60, 64, 67] {
            plugin.note_on(note, 0.8, true);
        }
        // Release all keys
        for note in [60, 64, 67] {
            plugin.note_off(note);
        }
        // All should be Releasing (no pedal)
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Releasing)
                .count(),
            3
        );

        // New chord with pedal
        plugin.sustain_held = true;
        for note in [65, 69, 72] {
            plugin.note_on(note, 0.8, true);
        }
        // Release new chord
        for note in [65, 69, 72] {
            plugin.note_off(note);
        }
        // New chord should be Sustained
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Sustained)
                .count(),
            3
        );

        // Quick pedal up-down (catch-release technique)
        plugin.sustain_held = false;
        plugin.release_sustained();
        plugin.sustain_held = true;
        // Old sustained notes now Releasing, pedal back down for new ones
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Sustained)
                .count(),
            0
        );
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Releasing)
                .count(),
            6
        );
    }

    #[test]
    fn test_cc64_threshold_boundary() {
        // MIDI spec: CC64 >= 64 = on, < 64 = off.
        // nih-plug normalizes to [0,1] by dividing by 127.
        let mut plugin = make_plugin();
        plugin.note_on(60, 0.8, true);

        // Value 63/127 = 0.496 — should be OFF
        let pedal_down = 63.0 / 127.0 >= 0.5;
        assert!(!pedal_down, "63/127 should be pedal OFF");

        // Value 64/127 = 0.504 — should be ON
        let pedal_down = 64.0 / 127.0 >= 0.5;
        assert!(pedal_down, "64/127 should be pedal ON");

        // Actually apply: set pedal at exactly 64/127
        plugin.sustain_held = 64.0_f32 / 127.0 >= 0.5;
        assert!(plugin.sustain_held);
        plugin.note_off(60);
        let slot = plugin.voices.iter().find(|s| s.midi_note == 60).unwrap();
        assert_eq!(slot.state, VoiceState::Sustained);
    }

    #[test]
    fn test_sustained_voice_has_no_damper() {
        // A sustained voice should NOT have its damper active — the reed
        // rings freely, same as a Held voice.
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        plugin.note_on(60, 0.8, true);
        plugin.note_off(60); // → Sustained (no damper)

        // Render a block to advance time
        let len = 256;
        plugin.render_subblock(0, len);

        // Compare energy: sustained voice vs a freshly held voice
        let sustained_energy: f64 = plugin.out_buf[..len].iter().map(|s| s * s).sum();

        let mut plugin2 = make_plugin();
        plugin2.note_on(60, 0.8, true);
        // Don't release — stays Held
        plugin2.render_subblock(0, len);
        let held_energy: f64 = plugin2.out_buf[..len].iter().map(|s| s * s).sum();

        // Should be very similar (both undamped)
        let ratio = sustained_energy / held_energy;
        assert!(
            (0.95..=1.05).contains(&ratio),
            "sustained/held energy ratio {ratio:.3} — expected ~1.0 (both undamped)"
        );
    }

    #[test]
    fn test_damper_activates_after_pedal_release() {
        // After pedal up, sustained voices should start damping (energy decreases).
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        plugin.note_on(60, 0.8, true);
        plugin.note_off(60); // → Sustained

        // Render while sustained (undamped)
        let len = 1024;
        plugin.render_subblock(0, len);
        let sustained_energy: f64 = plugin.out_buf[..len].iter().map(|s| s * s).sum();

        // Release pedal → damper engages
        plugin.sustain_held = false;
        plugin.release_sustained();

        // Render a bit for damper to take effect (skip transient)
        plugin.render_subblock(0, len);
        // Now measure: damping should reduce energy
        plugin.render_subblock(0, len);
        let damped_energy: f64 = plugin.out_buf[..len].iter().map(|s| s * s).sum();

        assert!(
            damped_energy < sustained_energy,
            "energy should decrease after damper: sustained={sustained_energy:.6} damped={damped_energy:.6}"
        );
    }

    #[test]
    fn test_note_off_for_nonexistent_note_is_noop() {
        // MIDI controller sends note_off for a note that was never played.
        // Should be harmless.
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        plugin.note_on(60, 0.8, true);
        plugin.note_off(72); // G5 was never played
        // C4 should still be Held, nothing Sustained
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Held)
                .count(),
            1
        );
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Sustained)
                .count(),
            0
        );
    }

    #[test]
    fn test_all_voices_sustained_then_steal() {
        // Extreme case: all 64 voices sustained, new note must steal.
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        // Fill and sustain all voices
        for i in 0..MAX_VOICES {
            let note = tables::MIDI_LO + (i as u8 % (tables::MIDI_HI - tables::MIDI_LO + 1));
            plugin.note_on(note, 0.8, true);
            plugin.note_off(note);
        }
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Sustained)
                .count(),
            MAX_VOICES
        );
        // New note should steal oldest sustained (not panic or fail)
        plugin.note_on(60, 0.8, true);
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Held)
                .count(),
            1
        );
        // Stolen voice should have crossfade
        let held = plugin
            .voices
            .iter()
            .find(|s| s.state == VoiceState::Held)
            .unwrap();
        assert!(held.steal_voice.is_some());
    }

    #[test]
    fn test_double_note_off_with_pedal() {
        // Some controllers send duplicate note-offs. With pedal down,
        // first note_off → Sustained, second note_off → no match (already Sustained, not Held).
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        plugin.note_on(60, 0.8, true);
        plugin.note_off(60); // → Sustained
        plugin.note_off(60); // no Held voice for note 60 — should be no-op
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Sustained)
                .count(),
            1
        );
        // Voice should still be valid and producin audio
        let len = 256;
        plugin.render_subblock(0, len);
        let energy: f64 = plugin.out_buf[..len].iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "voice should still be producing audio");
    }

    #[test]
    fn test_clamped_note_sustain_roundtrip() {
        // Note 0 → clamped to MIDI_LO on both note_on and note_off.
        // Verify the sustain path also works with clamped notes.
        let mut plugin = make_plugin();
        plugin.sustain_held = true;
        plugin.note_on(0, 0.8, true); // clamped to MIDI_LO
        plugin.note_off(0); // clamped to MIDI_LO — should find the voice
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Sustained)
                .count(),
            1
        );
        // Pedal up
        plugin.sustain_held = false;
        plugin.release_sustained();
        assert_eq!(
            plugin
                .voices
                .iter()
                .filter(|s| s.state == VoiceState::Releasing)
                .count(),
            1
        );
    }

    // ── Reinitialization tests ───────────────────────────────────────────

    /// Simulate what initialize() + reset() does, without needing InitContext.
    fn reinit_at_sample_rate(plugin: &mut OpenWurli, sr: f64) {
        plugin.sample_rate = sr;
        plugin.oversample = sr < 88200.0;
        plugin.os_sample_rate = if plugin.oversample { sr * 2.0 } else { sr };
        plugin.preamp = DkPreamp::new(plugin.os_sample_rate);
        plugin.tremolo = Tremolo::new(0.5, plugin.os_sample_rate);
        plugin.oversampler = Oversampler::new();
        plugin.power_amp = PowerAmp::new();
        plugin.speaker = Speaker::new(sr);
        plugin.reset();
    }

    #[test]
    fn test_sound_after_sample_rate_change() {
        let mut plugin = make_plugin();

        // Verify sound at initial 44100 Hz
        plugin.note_on(60, 0.8, true);
        let len = 512;
        plugin.render_subblock(0, len);
        let energy_before: f64 = plugin.out_buf[..len].iter().map(|s| s * s).sum();
        assert!(energy_before > 0.0, "no sound before reinit");

        // Reinitialize at 48000 Hz (same oversample path)
        reinit_at_sample_rate(&mut plugin, 48000.0);
        plugin.note_on(60, 0.8, true);
        plugin.render_subblock(0, len);
        let energy_48k: f64 = plugin.out_buf[..len].iter().map(|s| s * s).sum();
        assert!(energy_48k > 0.0, "silence after reinit to 48kHz");

        // Reinitialize at 96000 Hz (non-oversampled path)
        reinit_at_sample_rate(&mut plugin, 96000.0);
        plugin.note_on(60, 0.8, true);
        plugin.render_subblock(0, len);
        let energy_96k: f64 = plugin.out_buf[..len].iter().map(|s| s * s).sum();
        assert!(energy_96k > 0.0, "silence after reinit to 96kHz");

        // Back to 44100 Hz
        reinit_at_sample_rate(&mut plugin, 44100.0);
        plugin.note_on(60, 0.8, true);
        plugin.render_subblock(0, len);
        let energy_back: f64 = plugin.out_buf[..len].iter().map(|s| s * s).sum();
        assert!(energy_back > 0.0, "silence after reinit back to 44.1kHz");
    }

    #[test]
    fn test_sound_after_buffer_size_change() {
        let mut plugin = make_plugin();
        plugin.note_on(60, 0.8, true);
        let len = 256;
        plugin.render_subblock(0, len);
        let energy_before: f64 = plugin.out_buf[..len].iter().map(|s| s * s).sum();
        assert!(energy_before > 0.0, "no sound before reinit");

        // Simulate reinit at same SR but different buffer (reset clears voices)
        reinit_at_sample_rate(&mut plugin, 44100.0);
        plugin.note_on(60, 0.8, true);
        let small_len = 64;
        plugin.render_subblock(0, small_len);
        let energy_small: f64 = plugin.out_buf[..small_len].iter().map(|s| s * s).sum();
        assert!(
            energy_small > 0.0,
            "silence after reinit with smaller buffer"
        );
    }
}
