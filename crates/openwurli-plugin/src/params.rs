use nih_plug::prelude::*;

#[derive(Params)]
pub struct OpenWurliParams {
    /// Volume pot: attenuator between preamp and power amp (real circuit topology).
    /// At low settings, signal drops into the power amp's crossover distortion region.
    #[id = "volume"]
    pub volume: FloatParam,

    // R-11 "REED BAR VOLUME" is not a parameter: it is a factory service
    // trimmer inside the chassis, and in series with the volume pot it only
    // acts as a second level control. The engine holds it at its factory
    // default (tables::R11_REED_BAR_VOLUME_DEFAULT); see OUTPUT_ALIGNMENT_DB
    // in lib.rs for the level the plugin presents.
    /// Tremolo modulation depth (0 = off, 1 = full).
    #[id = "trem_depth"]
    pub tremolo_depth: FloatParam,

    /// Speaker cabinet character: 0 = bypass (flat), 1 = authentic (HPF+LPF).
    #[id = "speaker"]
    pub speaker_character: FloatParam,

    /// MLP per-note corrections: on = apply learned corrections, off = raw
    /// physics only. Default OFF since 2026-08-20: the corrections are
    /// trained on single-velocity reference material and
    /// measurably mute the harmonic series when extrapolating to expressive
    /// velocities (Sensor A/B: H2 −0.9 dB .. H5 −3.5 dB at C4 ff), while the
    /// in-distribution benefit is ~1.5 dB at one reference note. Raw physics
    /// wins by ear and by ruler; flip ON to A/B the learned fit.
    #[id = "mlp"]
    pub mlp_enabled: BoolParam,
}

impl Default for OpenWurliParams {
    fn default() -> Self {
        Self {
            volume: FloatParam::new("Volume", 0.80, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),

            tremolo_depth: FloatParam::new(
                "Tremolo Depth",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            speaker_character: FloatParam::new(
                "Speaker Character",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            mlp_enabled: BoolParam::new("MLP Corrections", false),
        }
    }
}
