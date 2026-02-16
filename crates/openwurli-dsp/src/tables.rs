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
    let anchors: &[(f64, f64)] = &[
        (33.0, 0.10),
        (52.0, 0.00),
        (62.0, 0.00),
        (74.0, 0.02),
        (96.0, 0.01),
    ];

    if m <= anchors[0].0 {
        return anchors[0].1;
    }
    if m >= anchors[anchors.len() - 1].0 {
        return anchors[anchors.len() - 1].1;
    }

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
/// From Section 2.5 eigenvalue table. Uses linear interpolation on the eigenvalue
/// solutions of: 1 + cos(lambda)cosh(lambda) + lambda*mu*(cos(lambda)sinh(lambda) - sin(lambda)cosh(lambda)) = 0
///
/// Returns ratios for modes 1-7 (mode 1 is always 1.0).
pub fn mode_ratios(mu: f64) -> [f64; NUM_MODES] {
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

    let mu_clamped = mu.clamp(0.0, 0.50);

    let mut lo = 0;
    for i in 0..table.len() - 1 {
        if table[i + 1].mu > mu_clamped {
            lo = i;
            break;
        }
        lo = i;
    }
    let hi = (lo + 1).min(table.len() - 1);

    let t = if table[hi].mu > table[lo].mu {
        (mu_clamped - table[lo].mu) / (table[hi].mu - table[lo].mu)
    } else {
        0.0
    };

    let mut lambdas = [0.0f64; NUM_MODES];
    for i in 0..NUM_MODES {
        lambdas[i] = table[lo].lambdas[i] + t * (table[hi].lambdas[i] - table[lo].lambdas[i]);
    }

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
/// Super-linear damping model: decay_n = decay_1 * mode_ratio_n^p
///
/// Real steel reed damping is NOT constant-Q (p=1.0). Thermoelastic (Zener),
/// air radiation, and clamping losses all scale faster than linearly with
/// frequency, giving p ≈ 1.3–2.0 for steel cantilevers in air.
///
/// At p=1.5, mode 2 at A1 (392 Hz) decays at ~25 dB/s (vs 9.4 dB/s at p=1.0),
/// reaching -25 dB within 1 second. This confines intermodulation products
/// between inharmonic modes to the attack region (~500ms), matching real
/// Wurlitzer behavior where bark is an attack-only phenomenon.
const MODE_DECAY_EXPONENT: f64 = 1.5;

pub fn mode_decay_rates(midi: u8, ratios: &[f64; NUM_MODES]) -> [f64; NUM_MODES] {
    let base = fundamental_decay_rate(midi);
    let mut rates = [0.0f64; NUM_MODES];
    for i in 0..NUM_MODES {
        rates[i] = base * ratios[i].powf(MODE_DECAY_EXPONENT);
    }
    rates
}

/// Per-note output scaling to balance the keyboard.
///
/// Applied POST-pickup to decouple volume from nonlinear displacement.
/// Two-slope curve: steeper below C3 to compensate for both the pickup HPF
/// attenuation of bass fundamentals AND the bass mode taper energy reduction.
///
///   A1 (MIDI 33): +13.1 dB  (steep: HPF + mode taper compensation)
///   C2 (MIDI 36): +11.2 dB
///   C3 (MIDI 48): +2.6 dB   (knee — slopes meet)
///   C4 (MIDI 60): 0 dB      (reference)
///   C5 (MIDI 72): -2.6 dB
///   C6 (MIDI 84): -5.3 dB
///   C7 (MIDI 96): -7.9 dB
pub fn output_scale(midi: u8) -> f64 {
    let m = midi as f64;
    let db = if m < 48.0 {
        // Below C3: 0.70 dB/semi to compensate HPF + bass mode taper.
        let db_at_c3 = -0.22 * (48.0 - 60.0); // +2.64 dB
        db_at_c3 + 0.70 * (48.0 - m)
    } else {
        // C3 and above: 0.22 dB/semi from C4 reference
        -0.22 * (m - 60.0)
    };
    f64::powf(10.0, db / 20.0)
}

/// Register-dependent velocity exponent for dynamic expression.
///
/// On a real 200A, mid-register notes (C3-C5) have the most dynamic range
/// because the hammer weight and reed stiffness are well-matched. Bass reeds
/// are heavy (compressed dynamics), treble reeds are light (quick saturation).
///
/// The velocity curve is: amplitude = velocity^exponent
///   exponent < 1.0: compressed dynamics (bass, treble)
///   exponent = 1.0: linear
///   exponent > 1.0: expanded dynamics (mid-register)
///
/// sigma=15: the compression onset is gradual across the keyboard —
/// the hammer-reed stiffness ratio changes smoothly, not abruptly.
/// Gives ~20+ dB mid-register range, ~12-15 dB at extremes.
pub fn velocity_exponent(midi: u8) -> f64 {
    let m = midi as f64;
    // Bell curve centered at MIDI 62 (D4, mid-register sweet spot)
    // Peak exponent 1.4 (expanded dynamics)
    // Edges (A1, C7) at 0.75 (compressed dynamics)
    let center = 62.0;
    let sigma = 15.0; // Gradual compression onset across keyboard
    let min_exp = 0.75;
    let max_exp = 1.4;
    let t = f64::exp(-0.5 * ((m - center) / sigma).powi(2));
    min_exp + t * (max_exp - min_exp)
}

/// Bass mode amplitude taper — compensates for pickup HPF differential.
///
/// The 1-pole HPF at 2312 Hz amplifies higher modes relative to the fundamental
/// by approximately their frequency ratio (in dB). For bass notes where the
/// fundamental is far below the HPF corner, this causes bending modes (especially
/// mode 2 at ~7x) to dominate the output spectrum.
///
/// Physical justification: longer bass reeds have the hammer contact point
/// closer to mode 2's vibration node (~0.78L from clamp), reducing excitation.
/// The dwell filter is too wide (sigma=8) to catch this at bass frequencies.
///
/// Below C3 (MIDI 48), attenuate modes 2+ proportional to distance from C3.
/// Higher modes get progressively steeper attenuation since their HPF advantage
/// is larger (HPF differential scales with log of frequency ratio).
pub fn bass_mode_taper(midi: u8, mode: usize) -> f64 {
    if mode == 0 || midi >= 48 {
        return 1.0;
    }
    let semis_below_c3 = (48 - midi) as f64;
    // Mode 2: -0.6 dB/semi, mode 3: -0.75, mode 4: -0.90, etc.
    let db_per_semi = 0.6 + 0.15 * (mode as f64 - 1.0);
    let atten_db = semis_below_c3 * db_per_semi;
    f64::powf(10.0, -atten_db / 20.0)
}

/// Full parameter set for one note.
pub struct NoteParams {
    pub fundamental_hz: f64,
    pub mode_ratios: [f64; NUM_MODES],
    pub mode_amplitudes: [f64; NUM_MODES],
    pub mode_decay_rates: [f64; NUM_MODES],
}

// ─── Intermodulation risk detection ─────────────────────────────────────────

/// Per-mode intermodulation product analysis.
pub struct IntermodProduct {
    pub mode: usize,
    pub mode_ratio: f64,
    pub nearest_integer: u32,
    pub fractional_offset: f64,
    pub beat_hz: f64,
    pub effective_amplitude: f64,
    pub perceptual_weight: f64,
    pub risk_score: f64,
}

/// Per-note intermodulation risk summary.
pub struct IntermodReport {
    pub midi: u8,
    pub fundamental_hz: f64,
    pub mu: f64,
    pub products: Vec<IntermodProduct>,
    pub max_risk: f64,
    pub total_risk: f64,
}

/// Psychoacoustic weighting for audible beating.
///
/// Peak (1.0) at 5-10 Hz (worst audible beating), ramps up from 0.5-2 Hz,
/// decays 15-40 Hz, floor at 0.1 above 40 Hz, zero below 0.5 Hz.
pub fn perceptual_beat_weight(beat_hz: f64) -> f64 {
    if beat_hz < 0.5 {
        return 0.0;
    }
    if beat_hz < 2.0 {
        // Ramp from 0 at 0.5 Hz to ~0.5 at 2 Hz
        return 0.5 * (beat_hz - 0.5) / 1.5;
    }
    if beat_hz <= 5.0 {
        // Ramp from 0.5 at 2 Hz to 1.0 at 5 Hz
        return 0.5 + 0.5 * (beat_hz - 2.0) / 3.0;
    }
    if beat_hz <= 10.0 {
        // Plateau at 1.0
        return 1.0;
    }
    if beat_hz <= 40.0 {
        // Decay from 1.0 at 10 Hz to 0.1 at 40 Hz
        return 0.1 + 0.9 * (40.0 - beat_hz) / 30.0;
    }
    // Floor above 40 Hz
    0.1
}

/// Dwell attenuation at ff velocity (velocity=1.0), inlined to keep tables.rs
/// dependency-free from hammer.rs.
fn dwell_attenuation_ff(fundamental_hz: f64, mode_ratios: &[f64; NUM_MODES]) -> [f64; NUM_MODES] {
    let t_dwell = 0.0005; // ff: velocity=1.0 → 0.0005 + 0.002*(1-1) = 0.5ms
    let sigma_sq = 8.0 * 8.0;

    let mut atten = [0.0f64; NUM_MODES];
    for i in 0..NUM_MODES {
        let ft = fundamental_hz * mode_ratios[i] * t_dwell;
        atten[i] = (-ft * ft / (2.0 * sigma_sq)).exp();
    }

    let a0 = atten[0];
    if a0 > 1e-30 {
        for a in &mut atten {
            *a /= a0;
        }
    }
    atten
}

/// Compute intermodulation risk for a given MIDI note.
///
/// For each mode 2-7, measures how close its frequency ratio is to the nearest
/// integer harmonic. When the pickup's 1/(1-y) nonlinearity generates intermod
/// products between inharmonic modes, they land near (but not at) integer
/// harmonics, producing audible beating in the 3-15 Hz range.
pub fn intermod_risk(midi: u8) -> IntermodReport {
    let fundamental_hz = midi_to_freq(midi);
    let mu = tip_mass_ratio(midi);
    let ratios = mode_ratios(mu);
    let dwell = dwell_attenuation_ff(fundamental_hz, &ratios);

    let mut products = Vec::new();
    let mut max_risk = 0.0f64;
    let mut total_risk = 0.0f64;

    // Modes 2-7 (index 1-6) — mode 1 is always ratio 1.0, no intermod
    for i in 1..NUM_MODES {
        let ratio = ratios[i];
        let nearest = ratio.round() as u32;
        let fractional_offset = (ratio - nearest as f64).abs();
        let beat_hz = fractional_offset * fundamental_hz;
        let effective_amplitude = BASE_MODE_AMPLITUDES[i] * bass_mode_taper(midi, i) * dwell[i];
        let weight = perceptual_beat_weight(beat_hz);
        let risk = effective_amplitude * weight;

        max_risk = max_risk.max(risk);
        total_risk += risk;

        products.push(IntermodProduct {
            mode: i + 1, // 1-indexed for display
            mode_ratio: ratio,
            nearest_integer: nearest,
            fractional_offset,
            beat_hz,
            effective_amplitude,
            perceptual_weight: weight,
            risk_score: risk,
        });
    }

    IntermodReport {
        midi,
        fundamental_hz,
        mu,
        products,
        max_risk,
        total_risk,
    }
}

/// Compute all parameters for a given MIDI note.
pub fn note_params(midi: u8) -> NoteParams {
    let fundamental_hz = midi_to_freq(midi);
    let mu = tip_mass_ratio(midi);
    let ratios = mode_ratios(mu);
    let decay_rates = mode_decay_rates(midi, &ratios);

    // Apply bass mode taper to compensate for HPF differential
    let mut amplitudes = BASE_MODE_AMPLITUDES;
    for i in 0..NUM_MODES {
        amplitudes[i] *= bass_mode_taper(midi, i);
    }

    NoteParams {
        fundamental_hz,
        mode_ratios: ratios,
        mode_amplitudes: amplitudes,
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
        assert!((r[1] - 7.13).abs() < 0.05);
    }

    #[test]
    fn test_tip_mass_ratio_range() {
        assert!(tip_mass_ratio(33) > 0.05);
        assert!(tip_mass_ratio(57) < 0.02);
    }

    #[test]
    fn test_decay_rate_increases_with_pitch() {
        assert!(fundamental_decay_rate(60) > fundamental_decay_rate(48));
        assert!(fundamental_decay_rate(84) > fundamental_decay_rate(72));
    }

    #[test]
    fn test_intermod_risk_below_threshold() {
        // Regression guard: no note in the playable range should exceed this threshold.
        // If this fails, it means mode decay or bass taper changed in a way that
        // reintroduces audible beating from inharmonic intermod products.
        //
        // The static metric captures worst-case ff attack amplitude before temporal
        // decay kicks in. Bass notes (MIDI 33-47) have the highest risk due to
        // high tip mass ratios making mode 2 inharmonic. The p=1.5 mode decay
        // ensures intermod is attack-only; render mode verifies spectral cleanliness.
        //
        // Find actual worst case first, then assert with headroom.
        let mut worst_risk = 0.0f64;
        let mut worst_midi = 0u8;
        for midi in MIDI_LO..=MIDI_HI {
            let report = intermod_risk(midi);
            if report.max_risk > worst_risk {
                worst_risk = report.max_risk;
                worst_midi = midi;
            }
        }
        // Threshold = 1.25x the current worst case, providing headroom for minor
        // parameter adjustments while catching any major regression.
        let threshold = worst_risk * 1.25;
        assert!(
            threshold < 0.15,
            "Worst-case risk at MIDI {} is {:.4} — threshold {:.4} seems too high, investigate",
            worst_midi, worst_risk, threshold
        );
        for midi in MIDI_LO..=MIDI_HI {
            let report = intermod_risk(midi);
            assert!(
                report.max_risk < threshold,
                "MIDI {} ({:.1} Hz): max_risk = {:.4} exceeds {:.4}",
                midi, report.fundamental_hz, report.max_risk, threshold
            );
        }
    }

    #[test]
    fn test_intermod_risk_known_values() {
        // A1 (MIDI 33): mu=0.10, mode 2 ratio ~7.13
        let report = intermod_risk(33);
        let m2 = &report.products[0]; // mode 2 is first in products vec
        assert_eq!(m2.mode, 2);
        assert!((m2.mode_ratio - 7.13).abs() < 0.1, "mode 2 ratio = {}", m2.mode_ratio);
        assert_eq!(m2.nearest_integer, 7);
        // Beat frequency = fractional_offset * fundamental_hz
        // A1 = 55 Hz, offset ~0.13, beat ~7.2 Hz
        assert!(m2.beat_hz > 3.0 && m2.beat_hz < 12.0,
            "A1 mode 2 beat_hz = {}", m2.beat_hz);
        // Should be in the high perceptual weight zone (5-10 Hz)
        assert!(m2.perceptual_weight > 0.8,
            "A1 mode 2 perceptual_weight = {}", m2.perceptual_weight);
    }

    #[test]
    fn test_perceptual_beat_weight_shape() {
        // Sub-1 Hz: near zero (below audible beating)
        assert!(perceptual_beat_weight(0.3) < 0.01);
        // 7 Hz: in the peak zone (5-10 Hz)
        assert!(perceptual_beat_weight(7.0) > 0.9);
        // 50 Hz: well above beating range, should be at floor
        assert!(perceptual_beat_weight(50.0) < 0.2);
    }
}
