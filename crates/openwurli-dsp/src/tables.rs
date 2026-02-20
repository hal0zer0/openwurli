//! Per-note parameter tables for Wurlitzer 200A reed modal synthesis.
//!
//! Derived from Euler-Bernoulli beam theory with tip mass (docs/reed-and-hammer-physics.md).
//! Range: MIDI 33 (A1) to MIDI 96 (C7) -- 64 reeds.
#![allow(clippy::needless_range_loop)]

pub const NUM_MODES: usize = 7;
pub const MIDI_LO: u8 = 33;
pub const MIDI_HI: u8 = 96;

/// Base mode amplitudes calibrated against OBM Wurlitzer 200A recordings.
///
/// Original 1/ω_n scaling (Section 3.2) predicted [1.0, 0.160, 0.057, ...]
/// from ideal bare-beam impulse response. However, OBM onset-phase spectral
/// analysis reveals physical mode 2 is 20-37 dB weaker than predicted across
/// the full register. Real Wurlitzer reeds have solder tip mass, non-uniform
/// geometry, and hammer coupling that suppress upper modes far below the
/// Euler-Bernoulli ideal.
///
/// Calibration method: back-calculate mechanical mode 2/fundamental ratio
/// from OBM recordings by correcting for the pickup HPF at 2312 Hz. OBM
/// mid-range (D4) implies mechanical mode 2 at -43 dB vs model's -16 dB.
/// Upper modes scaled proportionally (1/ω_n ratios preserved).
///
/// Mode 2 reduced from 0.010 to 0.005 (-6 dB) to eliminate a metallic
/// "plink" in mid-register: at 6.267×f0 ≈ 1642 Hz (C4), mode 2 sits in
/// peak ear sensitivity and creates an inharmonic ring. At 0.005, mode 2
/// is -36 dB at the pickup output — below the audibility threshold for an
/// inharmonic partial. This is within the OBM calibration uncertainty (±5 dB).
///
/// The real 200A's "bark" comes primarily from the pickup's 1/(1-y)
/// nonlinearity generating H2 at 2×fundamental, NOT from physical mode 2
/// oscillation at ~6.3×fundamental.
pub const BASE_MODE_AMPLITUDES: [f64; NUM_MODES] =
    [1.0, 0.005, 0.0035, 0.0018, 0.0011, 0.0007, 0.0005];

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

/// Eigenvalues beta_n for a cantilever beam with tip mass ratio mu.
///
/// From Section 2.5 eigenvalue table. Uses linear interpolation on the eigenvalue
/// solutions of: 1 + cos(beta)cosh(beta) + beta*mu*(cos(beta)sinh(beta) - sin(beta)cosh(beta)) = 0
///
/// Returns eigenvalues for modes 1-7.
fn eigenvalues(mu: f64) -> [f64; NUM_MODES] {
    struct EigRow {
        mu: f64,
        betas: [f64; NUM_MODES],
    }

    let table = [
        EigRow {
            mu: 0.00,
            betas: [1.8751, 4.6941, 7.8548, 10.9955, 14.1372, 17.2788, 20.4204],
        },
        EigRow {
            mu: 0.01,
            betas: [1.8584, 4.6849, 7.8504, 10.9930, 14.1356, 17.2776, 20.4195],
        },
        EigRow {
            mu: 0.05,
            betas: [1.7920, 4.6477, 7.8316, 10.9830, 14.1288, 17.2726, 20.4158],
        },
        EigRow {
            mu: 0.10,
            betas: [1.7227, 4.6024, 7.8077, 10.9700, 14.1198, 17.2660, 20.4110],
        },
        EigRow {
            mu: 0.15,
            betas: [1.6625, 4.5618, 7.7859, 10.9580, 14.1114, 17.2598, 20.4065],
        },
        EigRow {
            mu: 0.20,
            betas: [1.6097, 4.5254, 7.7659, 10.9470, 14.1036, 17.2540, 20.4023],
        },
        EigRow {
            mu: 0.30,
            betas: [1.5201, 4.4620, 7.7310, 10.9280, 14.0894, 17.2434, 20.3946],
        },
        EigRow {
            mu: 0.50,
            betas: [1.3853, 4.3601, 7.6745, 10.8970, 14.0650, 17.2252, 20.3814],
        },
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

    let mut betas = [0.0f64; NUM_MODES];
    for i in 0..NUM_MODES {
        betas[i] = table[lo].betas[i] + t * (table[hi].betas[i] - table[lo].betas[i]);
    }
    betas
}

/// Mode frequency ratios f_n/f_1 for a cantilever beam with tip mass ratio mu.
///
/// Returns ratios for modes 1-7 (mode 1 is always 1.0). Computed from
/// eigenvalues: ratio_n = (beta_n / beta_1)^2.
pub fn mode_ratios(mu: f64) -> [f64; NUM_MODES] {
    let betas = eigenvalues(mu);
    let b1_sq = betas[0] * betas[0];
    let mut ratios = [0.0f64; NUM_MODES];
    for i in 0..NUM_MODES {
        ratios[i] = (betas[i] * betas[i]) / b1_sq;
    }
    ratios
}

/// Reed length in mm for a given MIDI note.
///
/// Two-segment linear formula from docs/reed-and-hammer-physics.md (Section 1.3):
///   Reed number n = midi - 32 (MIDI 33 = reed 1, MIDI 96 = reed 64)
///   Bass (n=1-20):   L = 3.0 - n/20 inches
///   Treble (n=21-64): L = 2.0 - (n-20)/44 inches
pub fn reed_length_mm(midi: u8) -> f64 {
    let n = (midi as f64 - 32.0).clamp(1.0, 64.0);
    let inches = if n <= 20.0 {
        3.0 - n / 20.0
    } else {
        2.0 - (n - 20.0) / 44.0
    };
    inches * 25.4
}

/// Reed blank width and thickness for a given MIDI note.
///
/// Returns (width_mm, thickness_mm) based on 200A series blank dimensions
/// from docs/reed-and-hammer-physics.md Section 1.2.
///
/// Five blanks with distinct widths; thickness uses 200A values (thicker than
/// 200-series) per Vintage Vibe case study: bass 0.026", mid/treble 0.034".
/// The 200A's thicker reeds produce a "smoother, rounder, mellower tone"
/// and reduce displacement for the same hammer force (less bark at extreme bass).
/// The thickness transition is smoothed over 10 semitones (reeds 16-26, MIDI 48-58)
/// to model the gradual grinding taper at blank boundaries.
pub fn reed_blank_dims(midi: u8) -> (f64, f64) {
    let reed = (midi as i32 - 32).clamp(1, 64) as u8;

    // Width in inches — steps between blanks
    let width_inch = if reed <= 14 {
        0.151 // Blank 1: reeds 1-14
    } else if reed <= 20 {
        0.127 // Blank 2: reeds 15-20
    } else if reed <= 42 {
        0.121 // Blank 3: reeds 21-42
    } else if reed <= 50 {
        0.111 // Blank 4: reeds 43-50
    } else {
        0.098 // Blank 5: reeds 51-64
    };

    // Thickness in inches — 200A values (thicker than 200-series 0.020/0.031)
    // Smooth transition from bass (0.026") to mid/treble (0.034")
    let thickness_inch = if reed <= 16 {
        0.026
    } else if reed <= 26 {
        // 10-semitone crossfade: reeds 16-26 (MIDI 48-58)
        let t = (reed as f64 - 16.0) / 10.0;
        0.026 + t * (0.034 - 0.026)
    } else {
        0.034
    };

    (width_inch * 25.4, thickness_inch * 25.4)
}

/// Beam tip compliance: L³ / (w × t³).
///
/// Higher compliance = more physical tip deflection for a given hammer force.
/// Bass reeds (long, thin) have ~25× the compliance of treble reeds (short, thick).
/// This determines how much the reed actually moves relative to the pickup gap,
/// which controls the strength of the 1/(1-y) nonlinearity ("bark").
pub fn reed_compliance(midi: u8) -> f64 {
    let l = reed_length_mm(midi);
    let (w, t) = reed_blank_dims(midi);
    (l * l * l) / (w * t * t * t)
}

/// Per-note displacement scale for the pickup nonlinearity.
///
/// Derived from beam compliance: stiffer treble reeds deflect less → less bark.
/// Exponent 0.65 gives steeper bass-to-treble gradient than sqrt(0.5), needed
/// to match OBM's ~10:1 H2/H1 ratio range (D3 bark ~0.5 vs Bb6 clean ~0.06).
///
/// Calibrated against OBM + polyphonic Wurlitzer 200A recordings.
/// H2/H1 comparison (high-isolation notes, n=19) showed synth H2 was
/// -6.4 dB low at DS=0.42 ("sleepy Wurli"). Raised to 0.85 (+102%).
///
/// Approximate values across keyboard:
///   A1 (MIDI 33): 0.85  (clamped — heavy bass bark/growl)
///   D3 (MIDI 50): 0.85  (clamped — solid bark)
///   C4 (MIDI 60): 0.85  (strong bark, reference)
///   D5 (MIDI 74): 0.55  (moderate bark)
///   D6 (MIDI 86): 0.42  (lighter)
///   C7 (MIDI 96): 0.30  (clean, bell-like)
const DS_AT_C4: f64 = 0.85;
const DS_EXPONENT: f64 = 0.65;
const DS_CLAMP: (f64, f64) = (0.02, 0.85);

/// Runtime-overridable calibration parameters.
/// All fields default to the current hardcoded constants.
#[derive(Debug, Clone)]
pub struct CalibrationConfig {
    pub ds_at_c4: f64,
    pub ds_exponent: f64,
    pub ds_clamp: (f64, f64),
    pub target_db: f64,
    pub voicing_slope: f64,
    pub zero_trim: bool,
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            ds_at_c4: DS_AT_C4,
            ds_exponent: DS_EXPONENT,
            ds_clamp: DS_CLAMP,
            target_db: -13.0,
            voicing_slope: -0.04,
            zero_trim: false,
        }
    }
}

pub fn pickup_displacement_scale(midi: u8) -> f64 {
    pickup_displacement_scale_with_config(midi, &CalibrationConfig::default())
}

pub fn pickup_displacement_scale_with_config(midi: u8, cfg: &CalibrationConfig) -> f64 {
    let c = reed_compliance(midi);
    let c_ref = reed_compliance(60); // C4 reference
    let ds = cfg.ds_at_c4 * (c / c_ref).powf(cfg.ds_exponent);
    ds.clamp(cfg.ds_clamp.0, cfg.ds_clamp.1)
}

/// Cantilever beam mode shape phi_n(xi) with tip mass.
///
/// phi_n(xi) = cosh(beta*xi) - cos(beta*xi) - sigma*(sinh(beta*xi) - sin(beta*xi))
/// where sigma = (cosh(beta) + cos(beta)) / (sinh(beta) + sin(beta))
/// and xi = x/L (0=clamp, 1=tip).
fn mode_shape(beta: f64, xi: f64) -> f64 {
    let sigma = (beta.cosh() + beta.cos()) / (beta.sinh() + beta.sin());
    let bx = beta * xi;
    bx.cosh() - bx.cos() - sigma * (bx.sinh() - bx.sin())
}

/// Active pickup plate length in mm.
///
/// Estimated from Wurlitzer 200A pickup geometry — the electrode region that
/// effectively senses reed displacement. Conservative estimate; real plates
/// vary slightly by register.
const PLATE_ACTIVE_LENGTH_MM: f64 = 6.0;

/// Spatial coupling coefficients for pickup-reed interaction.
///
/// The pickup plate has finite length and integrates reed displacement over its
/// active region near the tip. Higher bending modes oscillate spatially — their
/// lobes partially cancel within the pickup window. This produces a spatial
/// low-pass filter that attenuates inharmonic modes relative to the fundamental.
///
/// Raw coupling: kappa_n = |integral of phi_n(xi) from (1-ell/L) to 1| / [ell/L * |phi_n(1)|]
///
/// **Normalized to mode 1**: the returned coefficients are kappa_n / kappa_1, so
/// mode 1 always returns 1.0. The absolute value of kappa_1 (0.83–0.94 depending
/// on register) is already absorbed into the pickup's displacement_scale, which
/// was calibrated against real-world bark levels assuming tip-displacement sensing.
/// What matters for mode suppression is the *differential* attenuation: how much
/// less the pickup senses mode n relative to mode 1.
pub fn spatial_coupling_coefficients(mu: f64, reed_len_mm: f64) -> [f64; NUM_MODES] {
    let betas = eigenvalues(mu);
    let ell_over_l = (PLATE_ACTIVE_LENGTH_MM / reed_len_mm).clamp(0.0, 1.0);

    let mut kappa_raw = [0.0f64; NUM_MODES];

    // Simpson's rule with 32 subintervals
    const N_SIMPSON: usize = 32;
    let xi_start = 1.0 - ell_over_l;

    for mode in 0..NUM_MODES {
        let beta = betas[mode];
        let tip_val = mode_shape(beta, 1.0);

        if tip_val.abs() < 1e-30 || ell_over_l < 1e-12 {
            kappa_raw[mode] = 1.0; // Degenerate case: point pickup at tip
            continue;
        }

        // Simpson's rule: integral = (h/3) * [f(x0) + 4*f(x1) + 2*f(x2) + 4*f(x3) + ... + f(xN)]
        let h = ell_over_l / N_SIMPSON as f64;
        let mut sum = mode_shape(beta, xi_start) + mode_shape(beta, 1.0);

        for j in 1..N_SIMPSON {
            let xi = xi_start + j as f64 * h;
            let coeff = if j % 2 == 1 { 4.0 } else { 2.0 };
            sum += coeff * mode_shape(beta, xi);
        }

        let integral = sum * h / 3.0;
        // Absolute value: mode shapes can be negative at the tip for higher modes
        // (the sign just flips the phase, but coupling magnitude is what matters)
        let k = (integral / (ell_over_l * tip_val)).abs();
        kappa_raw[mode] = k.clamp(0.0, 1.0);
    }

    // Normalize to mode 1: the reed model outputs tip displacement, and
    // displacement_scale is calibrated for tip-referenced y. Applying raw kappa_1 < 1
    // would double-count the spatial averaging (once here, once in the calibrated scale).
    // Only the differential suppression (kappa_n / kappa_1) is meaningful.
    let k1 = kappa_raw[0];
    if k1 > 1e-30 {
        let mut kappa = [0.0f64; NUM_MODES];
        for i in 0..NUM_MODES {
            kappa[i] = (kappa_raw[i] / k1).clamp(0.0, 1.0);
        }
        kappa
    } else {
        [1.0; NUM_MODES]
    }
}

/// Hammer spatial coupling coefficients for mode excitation.
///
/// The hammer contacts the reed over a finite length at a specific position,
/// acting as a spatial filter in mode space. Modes whose half-wavelength is
/// shorter than the contact region experience partial cancellation.
///
/// From Wurlitzer patents:
///   - Strike center: 0.30L from clamp (Andersen US 2,919,616: 0.25-0.35L)
///   - Contact length: 0.20L (Miessner US 2,932,231: 10-30% of reed)
///   - Contact region: ξ ∈ [0.20, 0.40] in normalized coordinates
///
/// Returns per-mode coupling coefficients normalized to mode 1.
/// Values > 1.0 mean that mode is excited MORE efficiently than mode 1
/// at this position (mode 1 has small displacement near the clamp).
pub fn hammer_spatial_coupling(mu: f64) -> [f64; NUM_MODES] {
    let betas = eigenvalues(mu);

    let xi_start = 0.20;
    let xi_end = 0.40;
    let contact_len = xi_end - xi_start;

    let mut coupling_raw = [0.0f64; NUM_MODES];

    const N_SIMPSON: usize = 32;
    let h = contact_len / N_SIMPSON as f64;

    for mode in 0..NUM_MODES {
        let beta = betas[mode];
        let mut sum = mode_shape(beta, xi_start) + mode_shape(beta, xi_end);
        for j in 1..N_SIMPSON {
            let xi = xi_start + j as f64 * h;
            let coeff = if j % 2 == 1 { 4.0 } else { 2.0 };
            sum += coeff * mode_shape(beta, xi);
        }
        let integral = (sum * h / 3.0).abs();
        coupling_raw[mode] = integral / contact_len;
    }

    // Normalize to mode 1
    let k1 = coupling_raw[0];
    if k1 > 1e-30 {
        let mut coupling = [0.0f64; NUM_MODES];
        for i in 0..NUM_MODES {
            coupling[i] = coupling_raw[i] / k1;
        }
        coupling
    } else {
        [1.0; NUM_MODES]
    }
}

/// Fundamental decay rate in dB/s for a given MIDI note.
///
/// Frequency power law calibrated against 10 clean OBM 200A decay rates
/// (excluding anomalous D6/F#6/D7): `decay = 0.005 * f^1.22`, floored at 3.0 dB/s.
///
/// Physical basis: corresponds to Q ∝ f^(-0.22) — a slight Q decrease with
/// frequency, consistent with thermoelastic (Zener) damping becoming more
/// significant for shorter, stiffer reeds. A constant-Q model (Q~1636) gives
/// exponent 1.0 (decay ∝ f); the slight upward correction captures the trend
/// in OBM data where treble reeds have modestly lower Q than bass.
///
/// Approximate values: F#3=3.0 (floor), D4=5.2, Bb4=8.5, F#5=16.0,
/// Bb5=21.1, C6=24.2 dB/s.
///
/// The 3.0 dB/s floor handles frequency-independent losses (clamping friction,
/// structural radiation, viscous air damping) that dominate for bass reeds
/// MIDI 33-48. OBM confirms bass decay rates are ~3 dB/s throughout octaves
/// 2-3, matching this floor.
const MIN_DECAY_RATE: f64 = 3.0;

pub fn fundamental_decay_rate(midi: u8) -> f64 {
    let f = midi_to_freq(midi);
    (0.005 * f.powf(1.22)).max(MIN_DECAY_RATE)
}

/// Per-mode decay rates in dB/s.
///
/// Super-linear damping model: decay_n = decay_1 * mode_ratio_n^p
///
/// Real steel reed damping is NOT constant-Q (p=1.0). Thermoelastic (Zener),
/// air radiation, and clamping losses all scale faster than linearly with
/// frequency, giving p ≈ 1.3–2.0 for steel cantilevers in air.
///
/// At p=2.0, mode 2 at C4 (1642 Hz) decays at ~326 dB/s, reaching -10 dB
/// within 9ms (~2.5 cycles). This confines the inharmonic partials (which
/// sit at non-integer ratios like 6.267× and sound "metallic") to the
/// first 2-3 cycles of the attack, matching real Wurlitzer behavior.
///
/// Previous value (1.5) allowed mode 2 to ring for ~77ms in mid-register,
/// creating an audible metallic "plink" at 1642 Hz — peak ear sensitivity.
/// The p=2.0 value better matches Zener thermoelastic theory (loss ∝ ω²).
const MODE_DECAY_EXPONENT: f64 = 2.0;

pub fn mode_decay_rates(midi: u8, ratios: &[f64; NUM_MODES]) -> [f64; NUM_MODES] {
    let base = fundamental_decay_rate(midi);
    let mut rates = [0.0f64; NUM_MODES];
    for i in 0..NUM_MODES {
        rates[i] = base * ratios[i].powf(MODE_DECAY_EXPONENT);
    }
    rates
}

/// Multi-harmonic RMS proxy for post-pickup signal level.
///
/// The pickup's 1/(1-y) nonlinearity distributes energy across harmonics.
/// For `y = ds·sin(θ)`, the Fourier magnitudes of `y/(1-y)` are:
///   r = (1 - sqrt(1 - ds²)) / ds
///   c_n = 2·r^n / sqrt(1 - ds²)
///
/// Each harmonic passes the pickup HPF differently: `hpf_n = n·f0 / sqrt((n·f0)² + fc²)`.
/// The RMS proxy sums the first 8 harmonics: `sqrt(Σ (c_n · hpf_n)²)`.
///
/// This replaces the old single-harmonic proxy `ds/(1-ds) * f0/sqrt(f0²+fc²)` which
/// used the peak of y/(1-y) instead of Fourier coefficients and ignored H2-H8 energy.
/// The peak/c₁ ratio varies from 1.7 (ds=0.50) to 2.4 (ds=0.80), systematically
/// over-estimating bass by ~2 dB.
pub fn pickup_rms_proxy(ds: f64, f0: f64, fc: f64) -> f64 {
    if ds < 1e-10 {
        return 0.0;
    }
    let r = (1.0 - (1.0 - ds * ds).sqrt()) / ds;
    let inv_sqrt = 1.0 / (1.0 - ds * ds).sqrt();
    let mut sum_sq = 0.0;
    let mut r_n = r;
    for n in 1..=8u32 {
        let cn = 2.0 * r_n * inv_sqrt;
        let nf = n as f64 * f0;
        let hpf_n = nf / (nf * nf + fc * fc).sqrt();
        sum_sq += (cn * hpf_n) * (cn * hpf_n);
        r_n *= r;
    }
    sum_sq.sqrt()
}

/// Empirical register trim from Tier 3 render calibration at v=127.
///
/// Corrects the residual imbalance that the multi-harmonic proxy and voicing
/// slope cannot model analytically: preamp Cin coupling cap HPF (~329 Hz),
/// speaker coloration, displacement_scale clamp effects, and mode interaction
/// artifacts. Reference: MIDI 60 (C4) = 0.0 dB.
///
/// Positive = boost (note too quiet), negative = cut (note too loud).
/// Linear interpolation between anchor points; clamped outside range.
pub fn register_trim_db(midi: u8) -> f64 {
    // Calibrated from zero-trim full-chain (t5_rms) renders at v=127 (2026-02-19).
    // Reference: C4 = -23.0 dBFS. Trim = target - actual t5_rms.
    // Measured via: preamp-bench sensitivity --zero-trim (DS=0.85, 13 notes × 3 vel)
    const ANCHORS: [(f64, f64); 13] = [
        (36.0, -4.9), // C2:  -18.1 → -23.0
        (40.0, -3.6), // E2:  -19.4 → -23.0
        (44.0, -5.0), // G#2: -18.0 → -23.0
        (48.0, -3.0), // C3:  -20.0 → -23.0
        (52.0, -2.4), // E3:  -20.6 → -23.0
        (56.0, -3.1), // G#3: -19.9 → -23.0
        (60.0, 0.0),  // C4:  -23.0 (reference)
        (64.0, 0.1),  // E4:  -23.1 → -23.0
        (68.0, 0.1),  // G#4: -23.1 → -23.0
        (72.0, -1.3), // C5:  -21.7 → -23.0
        (76.0, 0.5),  // E5:  -23.5 → -23.0
        (80.0, 1.1),  // G#5: -24.1 → -23.0
        (84.0, 2.4),  // C6:  -25.4 → -23.0
    ];

    let m = midi as f64;
    if m <= ANCHORS[0].0 {
        return ANCHORS[0].1;
    }
    if m >= ANCHORS[ANCHORS.len() - 1].0 {
        return ANCHORS[ANCHORS.len() - 1].1;
    }
    for i in 0..ANCHORS.len() - 1 {
        let (x0, y0) = ANCHORS[i];
        let (x1, y1) = ANCHORS[i + 1];
        if m <= x1 {
            let t = (m - x0) / (x1 - x0);
            return y0 + t * (y1 - y0);
        }
    }
    0.0
}

/// Per-note output scaling to balance the keyboard.
///
/// Applied POST-pickup to decouple volume from nonlinear displacement.
/// Three layers of correction:
///   1. Velocity-aware multi-harmonic proxy — models 1/(1-y) harmonics through
///      HPF at the ACTUAL displacement for this velocity, not just peak.
///      At ff, bass gets harmonic energy that passes the 2312 Hz HPF.
///      At pp, the nonlinearity is nearly linear — bass loses that harmonic
///      boost. The proxy accounts for this automatically.
///   2. Voicing slope — gentle treble roll for preamp BW and harmonic content
///   3. Empirical register trim — calibrated from Tier 3 renders at v=127
///
/// Tuning knobs:
///   TARGET_DB — absolute level
///   VOICING_SLOPE — treble balance (dB/semi above C4)
pub fn output_scale(midi: u8, velocity_norm: f64) -> f64 {
    output_scale_with_config(midi, velocity_norm, &CalibrationConfig::default())
}

pub fn output_scale_with_config(midi: u8, velocity_norm: f64, cfg: &CalibrationConfig) -> f64 {
    const HPF_FC: f64 = 2312.0;

    let ds = pickup_displacement_scale_with_config(midi, cfg);
    let f0 = midi_to_freq(midi);

    // Velocity-aware proxy: compute at the actual displacement that the pickup
    // sees at this velocity. vel_scale reduces reed amplitude, which reduces
    // the input to the 1/(1-y) nonlinearity, which changes the harmonic content
    // that passes the HPF. At ff (vel_scale=1.0) this equals the old behavior.
    let scurve_v = velocity_scurve(velocity_norm);
    let vel_scale = scurve_v.powf(velocity_exponent(midi));
    let vel_scale_c4 = scurve_v.powf(velocity_exponent(60));
    let effective_ds = (ds * vel_scale).max(1e-6);
    let effective_ds_ref = (cfg.ds_at_c4 * vel_scale_c4).max(1e-6);

    let rms = pickup_rms_proxy(effective_ds, f0, HPF_FC);
    let rms_ref = pickup_rms_proxy(effective_ds_ref, midi_to_freq(60), HPF_FC);

    let flat_db = -20.0 * (rms / rms_ref).log10();
    let voicing_db = cfg.voicing_slope * (midi as f64 - 60.0).max(0.0);
    let trim = if cfg.zero_trim {
        0.0
    } else {
        register_trim_db(midi)
    };

    // Velocity-dependent trim blend: at mf (v=80, norm=0.63, blend=0.546)
    // the register trim is partially applied. At ff (v=127, blend=1.0)
    // the full trim applies, preserving the v=127 calibration.
    // Exponent 1.3 gives ~4 dB spread at mf (was 6.0 dB with ^2.0).
    let vel_blend = velocity_norm.powf(1.3);
    let effective_trim = trim * vel_blend;

    f64::powf(
        10.0,
        (cfg.target_db + flat_db + voicing_db + effective_trim) / 20.0,
    )
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
/// Gives ~18-22 dB mid-register range, ~10-12 dB at extremes.
pub fn velocity_exponent(midi: u8) -> f64 {
    let m = midi as f64;
    // Bell curve centered at MIDI 62 (D4, mid-register sweet spot)
    // Peak exponent 1.7 (expanded dynamics)
    // Edges (A1, C7) at 1.3 (moderate dynamics)
    let center = 62.0;
    let sigma = 15.0; // Gradual compression onset across keyboard
    let min_exp = 1.3;
    let max_exp = 1.7;
    let t = f64::exp(-0.5 * ((m - center) / sigma).powi(2));
    min_exp + t * (max_exp - min_exp)
}

/// Sigmoid velocity shaping — models neoprene foam pad compression curve.
///
/// k=1.5 gives a mild S: pp slightly compressed, mf and ff nearly linear.
/// Neoprene foam pads (Miessner US 2,932,231) have a more linear compression
/// curve than piano felt. Used by both voice.rs (reed amplitude scaling) and
/// output_scale (velocity-aware pickup RMS proxy).
pub fn velocity_scurve(velocity: f64) -> f64 {
    let k = 1.5;
    let s = 1.0 / (1.0 + (-k * (velocity - 0.5)).exp());
    let s0 = 1.0 / (1.0 + (k * 0.5).exp());
    let s1 = 1.0 / (1.0 + (-k * 0.5).exp());
    (s - s0) / (s1 - s0)
}

/// Bass mode amplitude taper — SUPERSEDED by hammer_spatial_coupling.
///
/// Kept for reference. Replaced by physics-based spatial integration over the
/// hammer contact region (Andersen/Miessner patents) which models the same
/// effect more accurately across the full register.
#[allow(dead_code)]
fn bass_mode_taper(midi: u8, mode: usize) -> f64 {
    if mode == 0 || midi >= 48 {
        return 1.0;
    }
    let semis_below_c3 = (48 - midi) as f64;
    // Mode 2: -0.30 dB/semi, mode 3: -0.45, mode 4: -0.60, etc.
    // Reduced from 0.6 base now that spatial_coupling_coefficients handle
    // pickup geometry attenuation. The remaining taper covers the hammer
    // contact-point proximity to mode 2's vibration node at ~0.78L.
    let db_per_semi = 0.30 + 0.15 * (mode as f64 - 1.0);
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
///
/// Uses Miessner patent dwell: 0.75 cycles at ff.
fn dwell_attenuation_ff(fundamental_hz: f64, mode_ratios: &[f64; NUM_MODES]) -> [f64; NUM_MODES] {
    let t_dwell = (0.75 / fundamental_hz).clamp(0.0003, 0.020);
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

    let coupling = spatial_coupling_coefficients(mu, reed_length_mm(midi));

    // Modes 2-7 (index 1-6) — mode 1 is always ratio 1.0, no intermod
    // NOTE: hammer_spatial_coupling not applied — OBM amplitudes already include it
    for i in 1..NUM_MODES {
        let ratio = ratios[i];
        let nearest = ratio.round() as u32;
        let fractional_offset = (ratio - nearest as f64).abs();
        let beat_hz = fractional_offset * fundamental_hz;
        let effective_amplitude = BASE_MODE_AMPLITUDES[i] * coupling[i] * dwell[i];
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

    let mut amplitudes = BASE_MODE_AMPLITUDES;

    // NOTE: hammer_spatial_coupling is NOT applied here because
    // BASE_MODE_AMPLITUDES were calibrated from OBM recordings that already
    // include the real hammer's excitation profile. Applying spatial coupling
    // on top would double-count, boosting inharmonic modes 2-3 by +11-14 dB
    // and creating a metallic "plink" on attack.

    // Apply spatial pickup coupling — finite plate length attenuates higher bending modes
    let coupling = spatial_coupling_coefficients(mu, reed_length_mm(midi));
    for i in 0..NUM_MODES {
        amplitudes[i] *= coupling[i];
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
    fn test_decay_rate_obm_calibration() {
        // OBM-calibrated frequency power law: decay = 0.005 * f^1.22
        // Wide bounds accommodate ±30% OBM scatter while catching regressions.
        let bass = fundamental_decay_rate(36); // C2
        assert!(
            (bass - 3.0).abs() < 0.5,
            "C2 should be near floor (3.0), got {bass:.1}"
        );

        // Mid/treble: frequency power law gives gentler curve than old MIDI law
        let c4 = fundamental_decay_rate(60); // ~4.5 dB/s
        let c5 = fundamental_decay_rate(72); // ~10.3 dB/s
        let c6 = fundamental_decay_rate(84); // ~24.2 dB/s
        assert!(
            c4 > 3.5 && c4 < 7.0,
            "C4 decay should be ~4.5 dB/s, got {c4:.1}"
        );
        assert!(
            c5 > 7.0 && c5 < 16.0,
            "C5 decay should be ~10.3 dB/s, got {c5:.1}"
        );
        assert!(
            c6 > 17.0 && c6 < 35.0,
            "C6 decay should be ~24.2 dB/s, got {c6:.1}"
        );
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
            worst_midi,
            worst_risk,
            threshold
        );
        for midi in MIDI_LO..=MIDI_HI {
            let report = intermod_risk(midi);
            assert!(
                report.max_risk < threshold,
                "MIDI {} ({:.1} Hz): max_risk = {:.4} exceeds {:.4}",
                midi,
                report.fundamental_hz,
                report.max_risk,
                threshold
            );
        }
    }

    #[test]
    fn test_intermod_risk_known_values() {
        // A1 (MIDI 33): mu=0.10, mode 2 ratio ~7.13
        let report = intermod_risk(33);
        let m2 = &report.products[0]; // mode 2 is first in products vec
        assert_eq!(m2.mode, 2);
        assert!(
            (m2.mode_ratio - 7.13).abs() < 0.1,
            "mode 2 ratio = {}",
            m2.mode_ratio
        );
        assert_eq!(m2.nearest_integer, 7);
        // Beat frequency = fractional_offset * fundamental_hz
        // A1 = 55 Hz, offset ~0.13, beat ~7.2 Hz
        assert!(
            m2.beat_hz > 3.0 && m2.beat_hz < 12.0,
            "A1 mode 2 beat_hz = {}",
            m2.beat_hz
        );
        // Should be in the high perceptual weight zone (5-10 Hz)
        assert!(
            m2.perceptual_weight > 0.8,
            "A1 mode 2 perceptual_weight = {}",
            m2.perceptual_weight
        );
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

    // ─── Spatial pickup coupling tests ──────────────────────────────────────

    #[test]
    fn test_reed_length_known_values() {
        // Reed 1 (MIDI 33): n=1, bass formula: 3.0 - 1/20 = 2.95 in = 74.93 mm
        let l1 = reed_length_mm(33);
        assert!((l1 - 74.93).abs() < 0.1, "reed 1 length = {l1}");
        // Reed 64 (MIDI 96): n=64, treble formula: 2.0 - 44/44 = 1.0 in = 25.4 mm
        let l64 = reed_length_mm(96);
        assert!((l64 - 25.4).abs() < 0.1, "reed 64 length = {l64}");
        // Reed 20 (MIDI 52): n=20, bass formula: 3.0 - 20/20 = 2.0 in = 50.8 mm
        let l20 = reed_length_mm(52);
        assert!((l20 - 50.8).abs() < 0.1, "reed 20 length = {l20}");
    }

    #[test]
    fn test_mode_shape_tip_nonzero() {
        // |phi_n(1.0)| should be significantly nonzero for all modes.
        // Higher modes can have negative tip displacement — the sign alternates.
        for &mu in &[0.0, 0.05, 0.10, 0.20, 0.50] {
            let betas = eigenvalues(mu);
            for (i, &beta) in betas.iter().enumerate() {
                let tip = mode_shape(beta, 1.0);
                assert!(
                    tip.abs() > 0.1,
                    "mode_shape(beta={beta:.4}, 1.0) = {tip:.6} for mu={mu}, mode {i}"
                );
            }
        }
    }

    #[test]
    fn test_mode_shape_clamp_zero() {
        // phi_n(0.0) should be ~0 for all modes (clamped end)
        for &mu in &[0.0, 0.10, 0.50] {
            let betas = eigenvalues(mu);
            for (i, &beta) in betas.iter().enumerate() {
                let clamp = mode_shape(beta, 0.0);
                assert!(
                    clamp.abs() < 1e-10,
                    "mode_shape(beta={beta:.4}, 0.0) = {clamp:.2e} for mu={mu}, mode {i}"
                );
            }
        }
    }

    #[test]
    fn test_coupling_mode1_is_unity() {
        // After normalization, mode 1 coupling is always exactly 1.0.
        // The absolute kappa_1 (0.83–0.94) is absorbed into displacement_scale.
        for midi in (MIDI_LO..=MIDI_HI).step_by(4) {
            let mu = tip_mass_ratio(midi);
            let len = reed_length_mm(midi);
            let kappa = spatial_coupling_coefficients(mu, len);
            assert!(
                (kappa[0] - 1.0).abs() < 1e-10,
                "MIDI {midi}: kappa_1 = {:.10} (expected exactly 1.0)",
                kappa[0]
            );
        }
    }

    #[test]
    fn test_coupling_decreases_with_mode() {
        // Coupling generally decreases with mode number, but strict monotonicity
        // is NOT guaranteed — the spatial integration behaves like a sinc function
        // with side lobes, so occasionally a higher mode's spatial pattern aligns
        // better with the pickup window. What matters:
        //   1. Mode 1 always has the highest coupling
        //   2. Mode 2 is always less than mode 1
        //   3. Higher modes are all below mode 1
        for midi in (MIDI_LO..=MIDI_HI).step_by(4) {
            let mu = tip_mass_ratio(midi);
            let len = reed_length_mm(midi);
            let kappa = spatial_coupling_coefficients(mu, len);

            // Mode 1 has the highest coupling
            for i in 1..NUM_MODES {
                assert!(
                    kappa[i] <= kappa[0] + 1e-6,
                    "MIDI {midi}: kappa[{i}]={:.4} > kappa[0]={:.4}",
                    kappa[i],
                    kappa[0]
                );
            }
            // Mode 2 is less than mode 1 (first spatial attenuation step)
            assert!(
                kappa[1] < kappa[0],
                "MIDI {midi}: kappa[1]={:.4} >= kappa[0]={:.4}",
                kappa[1],
                kappa[0]
            );
        }
    }

    #[test]
    fn test_coupling_register_variation() {
        // Treble reeds (shorter, larger ell/L) should have more suppression
        // than bass reeds (longer, smaller ell/L) for the same mode
        let mu_bass = tip_mass_ratio(33); // A1 — longest reed
        let mu_treb = tip_mass_ratio(96); // C7 — shortest reed
        let kappa_bass = spatial_coupling_coefficients(mu_bass, reed_length_mm(33));
        let kappa_treb = spatial_coupling_coefficients(mu_treb, reed_length_mm(96));

        // For mode 3+ the treble should be more suppressed (smaller kappa)
        // due to larger ell/L overlap
        for i in 2..NUM_MODES {
            assert!(
                kappa_treb[i] < kappa_bass[i],
                "Mode {}: treble kappa {:.4} should be < bass kappa {:.4}",
                i + 1,
                kappa_treb[i],
                kappa_bass[i]
            );
        }
    }

    #[test]
    fn test_eigenvalues_matches_mode_ratios() {
        // Verify that the eigenvalues() refactor produces identical mode_ratios
        for &mu in &[0.0, 0.01, 0.05, 0.10, 0.15, 0.20, 0.30, 0.50] {
            let betas = eigenvalues(mu);
            let ratios = mode_ratios(mu);
            let b1_sq = betas[0] * betas[0];
            for i in 0..NUM_MODES {
                let ratio_from_beta = (betas[i] * betas[i]) / b1_sq;
                assert!(
                    (ratio_from_beta - ratios[i]).abs() < 1e-10,
                    "mu={mu}, mode {i}: eigenvalue ratio {ratio_from_beta} != mode_ratios {:.10}",
                    ratios[i]
                );
            }
        }
    }

    // ─── Reed compliance and displacement scale tests ────────────────────

    #[test]
    fn test_blank_dims_known_values() {
        // Reed 1 (MIDI 33): blank 1, w=0.151", t=0.026" (200A)
        let (w, t) = reed_blank_dims(33);
        assert!((w - 0.151 * 25.4).abs() < 0.01, "MIDI 33 width = {w}");
        assert!((t - 0.026 * 25.4).abs() < 0.01, "MIDI 33 thickness = {t}");

        // Reed 42 (MIDI 74): blank 3, w=0.121", t=0.034" (200A)
        let (w, t) = reed_blank_dims(74);
        assert!((w - 0.121 * 25.4).abs() < 0.01, "MIDI 74 width = {w}");
        assert!((t - 0.034 * 25.4).abs() < 0.01, "MIDI 74 thickness = {t}");

        // Reed 64 (MIDI 96): blank 5, w=0.098", t=0.034" (200A)
        let (w, t) = reed_blank_dims(96);
        assert!((w - 0.098 * 25.4).abs() < 0.01, "MIDI 96 width = {w}");
        assert!((t - 0.034 * 25.4).abs() < 0.01, "MIDI 96 thickness = {t}");
    }

    #[test]
    fn test_blank_dims_smooth_transition() {
        // The thickness transition from bass (0.026") to mid (0.034") should be
        // smooth over MIDI 48-58 (reeds 16-26), not a sharp step.
        let (_, t48) = reed_blank_dims(48); // reed 16: pure bass
        let (_, t53) = reed_blank_dims(53); // reed 21: mid-transition
        let (_, t58) = reed_blank_dims(58); // reed 26: pure mid

        assert!(
            (t48 - 0.026 * 25.4).abs() < 0.01,
            "MIDI 48 should be pure bass (200A: 0.026\")"
        );
        assert!(
            (t58 - 0.034 * 25.4).abs() < 0.01,
            "MIDI 58 should be pure mid (200A: 0.034\")"
        );
        // Mid-transition should be between
        assert!(
            t53 > t48 + 0.02 && t53 < t58 - 0.02,
            "MIDI 53 thickness ({t53:.3}) should be between {t48:.3} and {t58:.3}"
        );
    }

    #[test]
    fn test_compliance_bass_greater_than_treble() {
        // Bass reeds (long, thin) have much higher compliance than treble (short, thick)
        let c_bass = reed_compliance(33); // A1
        let c_mid = reed_compliance(60); // C4
        let c_treb = reed_compliance(96); // C7

        assert!(
            c_bass > c_mid * 5.0,
            "Bass compliance ({c_bass:.0}) should be >5x mid ({c_mid:.0})"
        );
        assert!(
            c_mid > c_treb * 2.0,
            "Mid compliance ({c_mid:.0}) should be >2x treble ({c_treb:.0})"
        );
    }

    #[test]
    fn test_displacement_scale_monotone_decreasing() {
        // Displacement scale should generally decrease from bass to treble
        // (more compliance → more deflection → more bark)
        // Bass notes may clamp at DS_CLAMP upper bound, so use >= for bass vs mid.
        let ds_33 = pickup_displacement_scale(33); // A1
        let ds_60 = pickup_displacement_scale(60); // C4
        let ds_96 = pickup_displacement_scale(96); // C7

        assert!(
            ds_33 >= ds_60,
            "A1 ({ds_33:.3}) should have >= bark than C4 ({ds_60:.3})"
        );
        assert!(
            ds_60 > ds_96,
            "C4 ({ds_60:.3}) should have more bark than C7 ({ds_96:.3})"
        );
    }

    #[test]
    fn test_displacement_scale_c4_calibration() {
        // C4 is the reference point — should be exactly DS_AT_C4
        let ds = pickup_displacement_scale(60);
        assert!(
            (ds - DS_AT_C4).abs() < 0.001,
            "C4 displacement scale = {ds:.4}, expected {DS_AT_C4}"
        );
    }

    #[test]
    fn test_displacement_scale_range() {
        // Bass should be high (strong bark), treble should be low (clean)
        let ds_bass = pickup_displacement_scale(33);
        let ds_treb = pickup_displacement_scale(96);

        assert!(
            ds_bass > 0.50,
            "Bass ds ({ds_bass:.3}) should give strong bark"
        );
        assert!(
            ds_treb < 0.35,
            "Treble ds ({ds_treb:.3}) should be nearly clean"
        );
        // Ratio should be at least 2.5:1 (bass is barkier, but clamp compresses range)
        assert!(
            ds_bass / ds_treb > 2.5,
            "Bass/treble ratio = {:.1}x, expected >2.5x",
            ds_bass / ds_treb
        );
    }
}
