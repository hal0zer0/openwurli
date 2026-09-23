//! PreampModel trait — swappable preamp implementations for A/B testing.

pub trait PreampModel {
    fn process_sample(&mut self, input: f64) -> f64;
    fn set_ldr_resistance(&mut self, r_ldr_path: f64);
    fn reset(&mut self);

    /// Factor that turns this model's `out` sample into the preamp's
    /// open-circuit (Thévenin) output voltage at the R-9 terminal. The
    /// volume network downstream (`tables::volume_pot_gain`) then applies
    /// R-9 as the source resistance. A model that leaves `out` unloaded
    /// returns 1.0; one that stamps a bench load returns (R9 + RLOAD)/RLOAD.
    /// Exact for a resistive load because R-9 sits outside the feedback loop.
    fn open_circuit_output_factor(&self) -> f64 {
        1.0
    }
}
