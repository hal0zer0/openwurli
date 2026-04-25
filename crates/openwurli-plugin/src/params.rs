use nih_plug::prelude::*;
use std::sync::Arc;

#[derive(Params)]
pub struct OpenWurliParams {
    /// Volume pot: attenuator between preamp and power amp (real circuit topology).
    /// At low settings, signal drops into the power amp's crossover distortion region.
    #[id = "volume"]
    pub volume: FloatParam,

    /// Tremolo modulation depth (0 = off, 1 = full).
    #[id = "trem_depth"]
    pub tremolo_depth: FloatParam,

    /// Speaker cabinet character: 0 = bypass (flat), 1 = authentic (HPF+LPF).
    #[id = "speaker"]
    pub speaker_character: FloatParam,

    /// MLP per-note corrections: on = apply learned corrections, off = raw physics only.
    #[id = "mlp"]
    pub mlp_enabled: BoolParam,

    /// DI output limiter: soft-limits the final output to −1 dBFS to prevent
    /// DAW peak-protect muting on loud polyphonic chords. Not a circuit-level
    /// effect — it models the ceiling that any mic preamp / A-D converter /
    /// DI interface imposes on a physical 200A recording. Default ON for
    /// new sessions; turn OFF for raw un-limited output (the physical analog
    /// chain's rail clipping at ±22 V is preserved in both cases — the
    /// limiter only catches peaks that would otherwise exceed 0 dBFS in the
    /// DAW's digital buffer).
    #[id = "di_limiter"]
    pub di_limiter: BoolParam,

    /// Authentic preamp hiss: Johnson-Nyquist thermal noise injected on
    /// every preamp resistor through melange's nonlinear MNA solver. The
    /// noise is shaped by the full two-stage feedback transfer function
    /// and amplified by the bias-dependent BJT gain curve — physically
    /// correct, unlike static noise-sample convolution. Default OFF for
    /// new sessions so existing users hear no change; flip ON for the
    /// character of a real 200A preamp at idle.
    #[id = "noise_enable"]
    pub noise_enable: BoolParam,

    /// Noise intensity multiplier on the Johnson-Nyquist thermal sources.
    /// `1.0×` is physics-honest — every preamp resistor contributes the
    /// `sqrt(4·k_B·T·R·BW)` voltage density that ngspice `.NOISE`
    /// reports for the same netlist (~8 µV RMS at the preamp output).
    /// That level lands near −86 dBFS at DAW default gain staging, which
    /// is the same place a clean DI of a real 200A sits — typically
    /// inaudible at normal listening levels. Raise above `1.0×` to
    /// exaggerate the noise for audible "vintage hiss" character; the
    /// shape (spectrum, modulation by tremolo loop gain) stays correct.
    /// `30×` is the practical ceiling — chain output ≈ −56 dBFS, clearly
    /// audible without dominating quiet passages.
    #[id = "noise_gain"]
    pub noise_gain: FloatParam,
}

impl Default for OpenWurliParams {
    fn default() -> Self {
        Self {
            volume: FloatParam::new("Volume", 0.50, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(5.0))
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),

            tremolo_depth: FloatParam::new(
                "Tremolo Depth",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            speaker_character: FloatParam::new(
                "Speaker Character",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            mlp_enabled: BoolParam::new("MLP Corrections", true),

            di_limiter: BoolParam::new("DI Limiter", true),

            noise_enable: BoolParam::new("Authentic Noise", false),

            noise_gain: FloatParam::new(
                "Noise Level",
                1.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 30.0,
                },
            )
            .with_unit("×")
            .with_value_to_string(Arc::new(|v| format!("{v:.1}")))
            .with_string_to_value(Arc::new(|s| s.trim().trim_end_matches('×').parse().ok())),
        }
    }
}
