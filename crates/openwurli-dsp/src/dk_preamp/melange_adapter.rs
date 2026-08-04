//! Melange-generated DK preamp adapter with shadow pump cancellation.
//!
//! LDR resistance is declared as `.runtime R` in the netlist (plugin-
//! driven, not a user knob). `set_runtime_R_r_ldr` marks matrices dirty;
//! the next `process_sample` does a lazy rebuild before the NR solve.
//! max_iter=200 for convergence across the full R_ldr range (1K-1M).

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
    noise_enabled: bool,
    thermal_gain: f64,
}

impl DkPreamp {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            main: init_state(sample_rate),
            shadow: init_state(sample_rate),
            sample_rate,
            noise_enabled: false,
            thermal_gain: 1.0,
        }
    }

    /// Enable/disable authentic Johnson-Nyquist thermal noise on the preamp
    /// resistors. Only the `main` state draws noise — the `shadow` state
    /// stays noiseless so pump subtraction cancels R_ldr DC drift without
    /// also cancelling the noise we just added.
    pub fn set_noise_enabled(&mut self, on: bool) {
        self.noise_enabled = on;
        self.main.set_noise_enabled(on);
    }

    /// Scale thermal noise amplitude. `1.0` = physics-honest full noise:
    /// every preamp resistor contributes its `sqrt(4·k_B·T·R·BW)` density,
    /// matching what ngspice `.NOISE` reports for the same netlist (~8 µV
    /// RMS at the preamp output). That lands near −86 dBFS at DAW default
    /// gain staging — the same place a clean DI of a real 200A sits.
    ///
    /// The plugin's `noise_gain` param wraps this via `set_noise_gain` and
    /// defaults to `1.0×` (asserted in `openwurli-plugin/src/lib.rs`). See
    /// `openwurli-plugin/src/params.rs` for the authoritative dBFS figures
    /// (that doc is the single source of truth). Raise above `1.0` to
    /// exaggerate the noise for "vintage hiss"; `30×` ≈ −56 dBFS.
    pub fn set_thermal_gain(&mut self, gain: f64) {
        self.thermal_gain = gain;
        self.main.set_thermal_gain(gain);
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
        self.main.set_runtime_R_r_ldr(r_ldr_path);
        self.shadow.set_runtime_R_r_ldr(r_ldr_path);
    }

    fn reset(&mut self) {
        self.main = init_state(self.sample_rate);
        self.shadow = init_state(self.sample_rate);
        self.main.set_noise_enabled(self.noise_enabled);
        self.main.set_thermal_gain(self.thermal_gain);
    }
}
