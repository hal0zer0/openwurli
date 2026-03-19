//! Melange-generated DK preamp adapter with shadow pump cancellation.
//!
//! Uses pot-as-rebuild: `set_pot_0()` re-stamps G and rebuilds A/S/K.
//! The circuit settles at the compile-time nominal (100K) and tremolo
//! modulation sweeps smoothly from there — no abrupt pot jumps.

use crate::gen_preamp::{self, CircuitState};
use crate::preamp::PreampModel;
use std::sync::OnceLock;

/// Cached state settled at the compile-time nominal pot value (100K).
/// Tremolo starts from here and sweeps smoothly.
static SETTLED_STATE: OnceLock<CircuitState> = OnceLock::new();

fn compute_settled_state() -> CircuitState {
    let mut s = CircuitState::default();
    // Settle at compile-time nominal (100K, already baked into G matrix).
    // No pot change needed — just let the circuit reach equilibrium.
    for _ in 0..176_400 {
        gen_preamp::process_sample(0.0, &mut s);
    }
    s
}

fn init_state(sample_rate: f64) -> CircuitState {
    let cached = SETTLED_STATE.get_or_init(compute_settled_state);
    let mut state = cached.clone();
    if (sample_rate - gen_preamp::SAMPLE_RATE).abs() > 0.5 {
        state.set_sample_rate(sample_rate);
    }
    state
}

pub struct DkPreamp {
    main: CircuitState,
    shadow: CircuitState,
    sample_rate: f64,
}

impl DkPreamp {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            main: init_state(sample_rate),
            shadow: init_state(sample_rate),
            sample_rate,
        }
    }
}

impl PreampModel for DkPreamp {
    fn process_sample(&mut self, input: f64) -> f64 {
        let main_out = gen_preamp::process_sample(input, &mut self.main)[0];
        let pump = gen_preamp::process_sample(0.0, &mut self.shadow)[0];
        let result = main_out - pump;
        if !result.is_finite() {
            self.reset();
            return 0.0;
        }
        result
    }

    fn set_ldr_resistance(&mut self, r_ldr_path: f64) {
        let r = r_ldr_path.max(1000.0);
        self.main.set_pot_0(r);
        self.shadow.set_pot_0(r);
    }

    fn reset(&mut self) {
        self.main = init_state(self.sample_rate);
        self.shadow = init_state(self.sample_rate);
    }
}
