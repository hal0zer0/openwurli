//! PreampModel trait — swappable preamp implementations.

/// PreampModel trait — swappable implementations for A/B testing.
pub trait PreampModel {
    fn process_sample(&mut self, input: f64) -> f64;
    fn set_ldr_resistance(&mut self, r_ldr_path: f64);
    fn reset(&mut self);
}
