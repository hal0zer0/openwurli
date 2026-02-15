use nih_plug::prelude::*;

#[derive(Params)]
pub struct OpenWurliParams {
    /// Master volume: post-everything output level.
    #[id = "volume"]
    pub volume: FloatParam,

    /// Post-preamp output gain (linear multiplier).
    #[id = "gain"]
    pub preamp_gain: FloatParam,

    /// Tremolo LFO rate in Hz.
    #[id = "trem_rate"]
    pub tremolo_rate: FloatParam,

    /// Tremolo modulation depth (0 = off, 1 = full).
    #[id = "trem_depth"]
    pub tremolo_depth: FloatParam,

    /// Speaker cabinet character: 0 = bypass (flat), 1 = authentic (HPF+LPF).
    #[id = "speaker"]
    pub speaker_character: FloatParam,
}

impl Default for OpenWurliParams {
    fn default() -> Self {
        Self {
            volume: FloatParam::new(
                "Volume",
                0.05,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 1.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(5.0))
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            preamp_gain: FloatParam::new(
                "Preamp Gain",
                40.0,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 200.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(5.0)),

            tremolo_rate: FloatParam::new(
                "Tremolo Rate",
                5.5,
                FloatRange::Linear {
                    min: 0.1,
                    max: 15.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" Hz")
            .with_step_size(0.1),

            tremolo_depth: FloatParam::new(
                "Tremolo Depth",
                0.5,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            speaker_character: FloatParam::new(
                "Speaker Character",
                1.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }
}
