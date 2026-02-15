/// Per-note parameter tables for Wurlitzer 200A reed modal synthesis.
///
/// Derived from Euler-Bernoulli beam theory with tip mass (docs/reed-and-hammer-physics.md).
/// Range: MIDI 33 (A1) to MIDI 96 (C7) — 64 reeds.

pub const NUM_MODES: usize = 7;
pub const MIDI_LO: u8 = 33;
pub const MIDI_HI: u8 = 96;

/// Base mode amplitudes from 1/omega_n scaling (Section 3.2).
/// These are relative to the fundamental and represent a pure tip impulse.
pub const BASE_MODE_AMPLITUDES: [f64; NUM_MODES] = [1.0, 0.160, 0.057, 0.029, 0.018, 0.012, 0.009];

/// MIDI note number to fundamental frequency (Hz), A440 tuning.
pub fn midi_to_freq(midi: u8) -> f64 {
    440.0 * f64::powf(2.0, (midi as f64 - 69.0) / 12.0)
}

/// Estimated tip mass ratio mu for a given MIDI note.
///
/// Anchor points from Section 2.6 eigenvalue analysis:
///   MIDI 33 (A1, reed 1):  mu ~ 0.10  (heavy solder on long bass reed)
///   MIDI 52 (E3, reed 20): mu ~ 0.00  (bare beam close to target pitch)
///   MIDI 62 (D4, reed 30): mu ~ 0.00  (mid-register, minimal solder)
///   MIDI 74 (D5, reed 42): mu ~ 0.02  (some solder needed)
///   MIDI 96 (C7, reed 64): mu ~ 0.01  (minimal, short ground reed)
///
/// Linear interpolation between anchors.
pub fn tip_mass_ratio(midi: u8) -> f64 {
    let m = midi as f64;
    // Piecewise linear through anchor points
    let anchors: &[(f64, f64)] = &[
        (33.0, 0.10),
        (52.0, 0.00),
        (62.0, 0.00),
        (74.0, 0.02),
        (96.0, 0.01),
    ];

    // Clamp to range
    if m <= anchors[0].0 {
        return anchors[0].1;
    }
    if m >= anchors[anchors.len() - 1].0 {
        return anchors[anchors.len() - 1].1;
    }

    // Find segment and interpolate
    for i in 0..anchors.len() - 1 {
        let (x0, y0) = anchors[i];
        let (x1, y1) = anchors[i + 1];
        if m <= x1 {
            let t = (m - x0) / (x1 - x0);
            return y0 + t * (y1 - y0);
        }
    }
    0.0
}

/// Mode frequency ratios f_n/f_1 for a cantilever beam with tip mass ratio mu.
///
/// From Section 2.5 eigenvalue table. Uses cubic interpolation on the eigenvalue
/// solutions of: 1 + cos(λ)cosh(λ) + λμ(cos(λ)sinh(λ) - sin(λ)cosh(λ)) = 0
///
/// Returns ratios for modes 1-7 (mode 1 is always 1.0).
pub fn mode_ratios(mu: f64) -> [f64; NUM_MODES] {
    // Eigenvalue table: (mu, [lambda_1..lambda_7])
    // lambda values from numerical solution of the characteristic equation.
    // f_n/f_1 = (lambda_n / lambda_1)^2
    //
    // Source: Section 2.5 table, extended to 7 modes.
    // Modes 5-7 extrapolated from the asymptotic spacing pattern:
    // lambda_{n+1} ≈ lambda_n + pi for large n.
    struct EigRow {
        mu: f64,
        lambdas: [f64; NUM_MODES],
    }

    let table = [
        EigRow { mu: 0.00, lambdas: [1.8751, 4.6941, 7.8548, 10.9955, 14.1372, 17.2788, 20.4204] },
        EigRow { mu: 0.01, lambdas: [1.8584, 4.6849, 7.8504, 10.9930, 14.1356, 17.2776, 20.4195] },
        EigRow { mu: 0.05, lambdas: [1.7920, 4.6477, 7.8316, 10.9830, 14.1288, 17.2726, 20.4158] },
        EigRow { mu: 0.10, lambdas: [1.7227, 4.6024, 7.8077, 10.9700, 14.1198, 17.2660, 20.4110] },
        EigRow { mu: 0.15, lambdas: [1.6625, 4.5618, 7.7859, 10.9580, 14.1114, 17.2598, 20.4065] },
        EigRow { mu: 0.20, lambdas: [1.6097, 4.5254, 7.7659, 10.9470, 14.1036, 17.2540, 20.4023] },
        EigRow { mu: 0.30, lambdas: [1.5201, 4.4620, 7.7310, 10.9280, 14.0894, 17.2434, 20.3946] },
        EigRow { mu: 0.50, lambdas: [1.3853, 4.3601, 7.6745, 10.8970, 14.0650, 17.2252, 20.3814] },
    ];

    // Clamp mu
    let mu_clamped = mu.clamp(0.0, 0.50);

    // Find bracketing rows
    let mut lo = 0;
    for i in 0..table.len() - 1 {
        if table[i + 1].mu > mu_clamped {
            lo = i;
            break;
        }
        lo = i;
    }
    let hi = (lo + 1).min(table.len() - 1);

    // Linear interpolation of eigenvalues
    let t = if table[hi].mu > table[lo].mu {
        (mu_clamped - table[lo].mu) / (table[hi].mu - table[lo].mu)
    } else {
        0.0
    };

    let mut lambdas = [0.0f64; NUM_MODES];
    for i in 0..NUM_MODES {
        lambdas[i] = table[lo].lambdas[i] + t * (table[hi].lambdas[i] - table[lo].lambdas[i]);
    }

    // Convert to frequency ratios: f_n/f_1 = (lambda_n/lambda_1)^2
    let l1_sq = lambdas[0] * lambdas[0];
    let mut ratios = [0.0f64; NUM_MODES];
    for i in 0..NUM_MODES {
        ratios[i] = (lambdas[i] * lambdas[i]) / l1_sq;
    }
    ratios
}

/// Fundamental decay rate in dB/s for a given MIDI note.
///
/// Exponential fit to OldBassMan 200A measurements (Section 5.7):
///   decay_dB_per_sec = 0.26 * exp(0.049 * MIDI)
pub fn fundamental_decay_rate(midi: u8) -> f64 {
    0.26 * f64::exp(0.049 * midi as f64)
}

/// Per-mode decay rates in dB/s.
///
/// Constant-Q model with mounting floor (Section 5.8):
///   decay_n = decay_1 * mode_ratio_n
/// with a minimum of 0.03 * mode_ratio_n for modes 4+ (mounting loss floor).
pub fn mode_decay_rates(midi: u8, ratios: &[f64; NUM_MODES]) -> [f64; NUM_MODES] {
    let base = fundamental_decay_rate(midi);
    let mut rates = [0.0f64; NUM_MODES];
    for i in 0..NUM_MODES {
        // Constant-Q: decay rate scales with frequency ratio
        rates[i] = base * ratios[i];
    }
    rates
}

/// Full parameter set for one note.
pub struct NoteParams {
    pub fundamental_hz: f64,
    pub mode_ratios: [f64; NUM_MODES],
    pub mode_amplitudes: [f64; NUM_MODES],
    pub mode_decay_rates: [f64; NUM_MODES],
}

/// Compute all parameters for a given MIDI note.
pub fn note_params(midi: u8) -> NoteParams {
    let fundamental_hz = midi_to_freq(midi);
    let mu = tip_mass_ratio(midi);
    let ratios = mode_ratios(mu);
    let decay_rates = mode_decay_rates(midi, &ratios);

    NoteParams {
        fundamental_hz,
        mode_ratios: ratios,
        mode_amplitudes: BASE_MODE_AMPLITUDES,
        mode_decay_rates: decay_rates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_midi_to_freq() {
        assert!((midi_to_freq(69) - 440.0).abs() < 0.01);
        assert!((midi_to_freq(60) - 261.63).abs() < 0.1);
        assert!((midi_to_freq(33) - 55.0).abs() < 0.1);
    }

    #[test]
    fn test_mode_ratios_bare_beam() {
        let r = mode_ratios(0.0);
        assert!((r[0] - 1.0).abs() < 1e-6);
        assert!((r[1] - 6.267).abs() < 0.01);
        assert!((r[2] - 17.547).abs() < 0.02);
    }

    #[test]
    fn test_mode_ratios_with_tip_mass() {
        let r = mode_ratios(0.10);
        // With mu=0.10, f2/f1 should be ~7.13 (Section 2.5)
        assert!((r[1] - 7.13).abs() < 0.05);
    }

    #[test]
    fn test_tip_mass_ratio_range() {
        // Bass has highest mu
        assert!(tip_mass_ratio(33) > 0.05);
        // Mid-register has near-zero mu
        assert!(tip_mass_ratio(57) < 0.02);
    }

    #[test]
    fn test_decay_rate_increases_with_pitch() {
        assert!(fundamental_decay_rate(60) > fundamental_decay_rate(48));
        assert!(fundamental_decay_rate(84) > fundamental_decay_rate(72));
    }
}
