//! Melange-generated DK preamp adapter with shadow pump cancellation.
//!
//! Uses SM (Sherman-Morrison) pot correction with max_iter=200 for NR
//! convergence across the full R_ldr range (1K-1M). No per-sample matrix
//! rebuild — SM handles the per-sample delta_g smoothly.

use crate::gen_preamp::{self, CircuitState};
use crate::preamp::PreampModel;
use std::sync::OnceLock;

static SETTLED_STATE: OnceLock<CircuitState> = OnceLock::new();

fn compute_settled_state() -> CircuitState {
    let mut s = CircuitState::default();
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
        self.main.pot_0_resistance = r;
        self.shadow.pot_0_resistance = r;
    }

    fn reset(&mut self) {
        self.main = init_state(self.sample_rate);
        self.shadow = init_state(self.sample_rate);
    }
}
