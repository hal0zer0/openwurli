/// OpenWurli DSP library — Wurlitzer 200A signal chain modules.
///
/// Pure DSP math with no audio framework dependencies.

// Reed/voice synthesis (from reed-renderer)
pub mod filters;
pub mod hammer;
pub mod pickup;
pub mod reed;
pub mod tables;
pub mod variation;
pub mod voice;

// Preamp circuit simulation
pub mod bjt_stage;
pub mod oversampler;
pub mod preamp;
pub mod tremolo;

// Output stage
pub mod power_amp;
pub mod speaker;
