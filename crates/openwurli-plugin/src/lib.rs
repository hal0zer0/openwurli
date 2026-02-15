// OpenWurli — Wurlitzer 200A virtual instrument plugin (CLAP + VST3).

use nih_plug::prelude::*;
use openwurli_dsp::oversampler::Oversampler;
use openwurli_dsp::power_amp::PowerAmp;
use openwurli_dsp::preamp::{EbersMollPreamp, PreampModel};
use openwurli_dsp::speaker::Speaker;
use openwurli_dsp::tremolo::Tremolo;
use openwurli_dsp::tables;
use openwurli_dsp::voice::Voice;
use std::num::NonZeroU32;
use std::sync::Arc;

mod params;
use params::OpenWurliParams;

const MAX_VOICES: usize = 12;
const MAX_BLOCK_SIZE: usize = 8192;

// Note: PREAMP_INPUT_SCALE is no longer needed. The pickup model now includes
// DISPLACEMENT_SCALE (0.30) which converts reed displacement to physical y = x/d_0,
// applies the nonlinear 1/(1-y) capacitance model, and outputs calibrated millivolt
// signals that feed directly to the preamp. The nonlinear pickup is where the
// Wurlitzer bark comes from — not the preamp (which is a clean gain stage).

// ── Voice management ────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum VoiceState {
    Free,
    Held,
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
    preamp: EbersMollPreamp,
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
}

impl Default for OpenWurli {
    fn default() -> Self {
        let sr = 44100.0;
        let os_sr = sr * 2.0;
        Self {
            params: Arc::new(OpenWurliParams::default()),
            voices: (0..MAX_VOICES).map(|_| VoiceSlot::default()).collect(),
            age_counter: 0,
            preamp: EbersMollPreamp::new(os_sr),
            tremolo: Tremolo::new(5.5, 0.5, os_sr),
            oversampler: Oversampler::new(),
            power_amp: PowerAmp::new(),
            speaker: Speaker::new(sr),
            voice_buf: vec![0.0; MAX_BLOCK_SIZE],
            sum_buf: vec![0.0; MAX_BLOCK_SIZE],
            up_buf: vec![0.0; MAX_BLOCK_SIZE * 2],
            out_buf: vec![0.0; MAX_BLOCK_SIZE],
            sample_rate: sr,
            os_sample_rate: os_sr,
        }
    }
}

impl OpenWurli {
    fn note_on(&mut self, note: u8, velocity: f32) {
        let note = note.clamp(tables::MIDI_LO, tables::MIDI_HI);
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
        ));
        slot.state = VoiceState::Held;
        slot.midi_note = note;
        slot.age = self.age_counter;
    }

    fn note_off(&mut self, note: u8) {
        // Release the oldest held voice matching this note
        let mut oldest_age = u64::MAX;
        let mut oldest_idx = None;
        for (i, slot) in self.voices.iter().enumerate() {
            if slot.state == VoiceState::Held && slot.midi_note == note && slot.age < oldest_age {
                oldest_age = slot.age;
                oldest_idx = Some(i);
            }
        }
        if let Some(idx) = oldest_idx {
            self.voices[idx].state = VoiceState::Releasing;
            if let Some(ref mut voice) = self.voices[idx].voice {
                voice.note_off();
            }
        }
    }

    /// Find a voice slot: prefer Free, then oldest Releasing, then oldest Held.
    fn allocate_voice(&mut self) -> usize {
        // 1. Free slot
        for (i, slot) in self.voices.iter().enumerate() {
            if slot.state == VoiceState::Free {
                return i;
            }
        }

        // 2. Oldest releasing voice
        let mut oldest_age = u64::MAX;
        let mut oldest_idx = 0;
        for (i, slot) in self.voices.iter().enumerate() {
            if slot.state == VoiceState::Releasing && slot.age < oldest_age {
                oldest_age = slot.age;
                oldest_idx = i;
            }
        }
        if oldest_age < u64::MAX {
            return oldest_idx;
        }

        // 3. Oldest held voice (voice stealing)
        oldest_age = u64::MAX;
        oldest_idx = 0;
        for (i, slot) in self.voices.iter().enumerate() {
            if slot.age < oldest_age {
                oldest_age = slot.age;
                oldest_idx = i;
            }
        }
        oldest_idx
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

        // Upsample to 2x rate
        self.oversampler
            .upsample_2x(&self.sum_buf[..len], &mut self.up_buf[..len * 2]);

        // Process through preamp at oversampled rate (tremolo modulates LDR)
        // PERF: LDR resistance recomputed every oversampled sample; could be every 16
        for s in &mut self.up_buf[..len * 2] {
            let r_ldr = self.tremolo.process();
            self.preamp.set_ldr_resistance(r_ldr);
            *s = self.preamp.process_sample(*s);
        }

        // Downsample back to base rate
        self.oversampler
            .downsample_2x(&self.up_buf[..len * 2], &mut self.out_buf[offset..offset + len]);
    }

    fn cleanup_voices(&mut self) {
        for slot in &mut self.voices {
            if slot.state != VoiceState::Free {
                if let Some(ref voice) = slot.voice {
                    if voice.is_silent() {
                        slot.state = VoiceState::Free;
                        slot.voice = None;
                    }
                }
            }
        }
    }
}

impl Plugin for OpenWurli {
    const NAME: &'static str = "OpenWurli";
    const VENDOR: &'static str = "OpenWurli";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
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
        self.os_sample_rate = self.sample_rate * 2.0;

        // Reinitialize DSP modules at correct sample rate
        self.preamp = EbersMollPreamp::new(self.os_sample_rate);
        self.tremolo = Tremolo::new(
            self.params.tremolo_rate.value() as f64,
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
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let num_samples = buffer.samples();

        // Update tremolo params (per-buffer is fine; LFO is slow)
        let trem_rate = self.params.tremolo_rate.smoothed.next() as f64;
        let trem_depth = self.params.tremolo_depth.smoothed.next() as f64;
        self.tremolo.set_rate(trem_rate, self.os_sample_rate);
        self.tremolo.set_depth(trem_depth);

        // Event-splitting process loop: split at each MIDI event for sample-accuracy
        let mut next_event = context.next_event();
        let mut block_start: usize = 0;

        while block_start < num_samples {
            // Process all events at or before current position
            loop {
                match next_event {
                    Some(ref event) if (event.timing() as usize) <= block_start => {
                        match event {
                            NoteEvent::NoteOn { note, velocity, .. } => {
                                self.note_on(*note, *velocity);
                            }
                            NoteEvent::NoteOff { note, .. } => {
                                self.note_off(*note);
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
                NoteEvent::NoteOn { note, velocity, .. } => self.note_on(note, velocity),
                NoteEvent::NoteOff { note, .. } => self.note_off(note),
                _ => {}
            }
            next_event = context.next_event();
        }

        // Signal chain: preamp -> gain -> volume -> power amp -> speaker -> output
        // Speaker character: per-buffer is fine (50ms smoother, slow-changing knob)
        let speaker_char = self.params.speaker_character.smoothed.next() as f64;
        self.speaker.set_character(speaker_char);

        for (i, mut channel_samples) in buffer.iter_samples().enumerate() {
            // Per-sample smoothing prevents zipper noise on gain/volume changes
            let preamp_gain = self.params.preamp_gain.smoothed.next() as f64;
            let volume = self.params.volume.smoothed.next() as f64;
            // Volume attenuates BEFORE power amp (real circuit topology)
            let attenuated = self.out_buf[i] * preamp_gain * volume;
            let amplified = self.power_amp.process(attenuated);
            let shaped = self.speaker.process(amplified);
            let sample = shaped as f32;
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
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
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
