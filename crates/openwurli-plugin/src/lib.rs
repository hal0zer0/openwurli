// OpenWurli — Wurlitzer 200A virtual instrument plugin (CLAP + VST3).
//
// This file is the nih-plug shell: parameter declarations, MIDI event splitting,
// and host-buffer fan-out. All synthesis (voices, signal chain, smoothing, NaN
// guard) lives in `openwurli_dsp::engine::WurliEngine` so any other host —
// oomox/Vurli, custom DAW integrations, headless CLI tools — can wrap the same
// engine without copying glue.

use nih_plug::midi::control_change;
use nih_plug::prelude::*;
use openwurli_dsp::engine::WurliEngine;
use std::num::NonZeroU32;
use std::sync::Arc;

mod params;
use params::OpenWurliParams;

struct OpenWurli {
    params: Arc<OpenWurliParams>,
    engine: WurliEngine,
}

impl Default for OpenWurli {
    fn default() -> Self {
        Self {
            params: Arc::new(OpenWurliParams::default()),
            engine: WurliEngine::new(44_100.0),
        }
    }
}

impl OpenWurli {
    /// Push the current host-side param values into the engine. Engine
    /// smooths the audio-rate ones internally so block-rate refresh is
    /// click-free.
    fn sync_params(&mut self) {
        self.engine
            .set_volume(self.params.volume.value() as f64);
        self.engine
            .set_tremolo_depth(self.params.tremolo_depth.value() as f64);
        self.engine
            .set_speaker_character(self.params.speaker_character.value() as f64);
        self.engine.set_mlp_enabled(self.params.mlp_enabled.value());
        self.engine.set_di_limiter(self.params.di_limiter.value());
        self.engine
            .set_noise_enabled(self.params.noise_enable.value());
        self.engine
            .set_noise_gain(self.params.noise_gain.value() as f64);
    }

    fn handle_event(&mut self, event: &NoteEvent<()>) {
        match event {
            NoteEvent::NoteOn { note, velocity, .. } => {
                self.engine.note_on(*note, *velocity);
            }
            NoteEvent::NoteOff { note, .. } => {
                self.engine.note_off(*note);
            }
            NoteEvent::MidiCC { cc, value, .. } if *cc == control_change::DAMPER_PEDAL => {
                self.engine.set_sustain(*value >= 0.5);
            }
            _ => {}
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
        self.engine
            .set_sample_rate(buffer_config.sample_rate as f64);
        self.engine
            .ensure_buffer_capacity(buffer_config.max_buffer_size as usize);
        self.sync_params();
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.sync_params();

        let num_samples = buffer.samples();
        if num_samples == 0 {
            return ProcessStatus::Normal;
        }

        // Render mono engine output into channel 0, then fan out to the
        // remaining channels. Engine handles the entire signal chain; we
        // only do MIDI event splitting and stereo duplication here.
        let channels = buffer.as_slice();
        let mut block_start: usize = 0;
        let mut next_event = context.next_event();

        while block_start < num_samples {
            // Drain events at or before the current sub-block boundary.
            while let Some(ref event) = next_event {
                if (event.timing() as usize) > block_start {
                    break;
                }
                self.handle_event(event);
                next_event = context.next_event();
            }

            let block_end = match next_event {
                Some(ref event) => (event.timing() as usize).min(num_samples),
                None => num_samples,
            };
            let len = block_end - block_start;
            if len > 0 {
                let (left_chan, _rest) = channels.split_at_mut(1);
                self.engine
                    .render(&mut left_chan[0][block_start..block_end]);
            }
            block_start = block_end;
        }

        // Drain any trailing events (sample-accurate sustain on the last sample).
        while let Some(event) = next_event {
            self.handle_event(&event);
            next_event = context.next_event();
        }

        // Fan out channel 0 to channels 1..N.
        if channels.len() > 1 {
            let (first, rest) = channels.split_at_mut(1);
            for chan in rest.iter_mut() {
                chan[..num_samples].copy_from_slice(&first[0][..num_samples]);
            }
        }

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

    #[test]
    fn test_plugin_instantiates() {
        let plugin = OpenWurli::default();
        // Engine is wired up; sample rate is the default 44.1k until
        // initialize() is called by the host.
        assert_eq!(plugin.engine.active_voice_count(), 0);
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
    fn test_di_limiter_default_is_on() {
        let params = OpenWurliParams::default();
        assert!(
            params.di_limiter.default_plain_value(),
            "DI limiter must default ON for DAW-safe out-of-the-box behavior"
        );
    }

    #[test]
    fn test_noise_enable_default_is_off() {
        // Phase 5 bit-identical default: existing users hear no change
        // until they explicitly opt in to the authentic-noise character.
        let params = OpenWurliParams::default();
        assert!(
            !params.noise_enable.default_plain_value(),
            "noise_enable must default OFF"
        );
    }

    #[test]
    fn test_noise_gain_default_is_physics_honest() {
        // 1.0× = ngspice-validated thermal level. Users who want louder
        // hiss can crank toward 30×.
        let params = OpenWurliParams::default();
        assert!(
            (params.noise_gain.default_plain_value() - 1.0).abs() < 0.01,
            "noise_gain must default to 1.0×"
        );
    }
}
