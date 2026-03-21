//! OpenWurli DSP library — Wurlitzer 200A signal chain modules.
//!
//! Pure DSP math with no audio framework dependencies.

// Reed/voice synthesis
pub mod filters;
pub mod hammer;
pub mod mlp_correction;
pub mod pickup;
pub mod reed;
pub mod tables;
pub mod variation;
pub mod voice;

// Preamp circuit simulation (melange-generated DK solver)
pub mod dk_preamp;
pub mod dk_preamp_legacy;
pub mod gen_preamp;
pub mod oversampler;
pub mod preamp;
pub mod tremolo;

// Output stage
#[cfg(feature = "melange-power-amp")]
pub mod gen_power_amp;
#[cfg(feature = "melange-tremolo")]
pub mod gen_tremolo;
pub mod power_amp;
pub mod speaker;
