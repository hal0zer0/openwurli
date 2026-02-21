//! DK (Discretization-Kernel) preamp — full coupled 2-stage BJT circuit solver.
//!
//! Solves the complete Wurlitzer 200A preamp as an 8-node MNA system with
//! trapezoidal discretization and Newton-Raphson on the 2×2 nonlinear kernel.
//!
//! Key advantage over EbersMollPreamp: C-3/C-4 Miller capacitors are modeled
//! as coupled state variables, giving correct bandwidth (~15.5 kHz independent
//! of R_ldr) and gain-bandwidth scaling (GBW ∝ gain, not constant).
//!
//! Input coupling: The series Cin (0.022µF) + R1 (22K) is modeled as a bilinear
//! companion element. This correctly blocks DC (preserving the R2/R3 bias point)
//! while loading the base at audio frequencies with the proper R1 impedance.
//!
//! See docs/dk-preamp-derivation.md for the full mathematical derivation.
#![allow(clippy::needless_range_loop)]

use crate::preamp::PreampModel;

// ── Circuit constants ───────────────────────────────────────────────────────

const VCC: f64 = 15.0;

// Resistors (ohms)
const R1: f64 = 22_000.0; // Input to base1 (in series with Cin)
const R2: f64 = 2_000_000.0; // base1 to Vcc (bias)
const R3: f64 = 470_000.0; // base1 to GND (bias)
const RE1: f64 = 33_000.0; // emit1 to GND
const RC1: f64 = 150_000.0; // coll1 to Vcc
const RE2A: f64 = 270.0; // emit2 to emit2b
const RE2B: f64 = 820.0; // emit2b to GND
const RC2: f64 = 1_800.0; // coll2 to Vcc
const R9: f64 = 6_800.0; // coll2 to out
const R10: f64 = 56_000.0; // out to fb

// Capacitors (farads)
const CIN: f64 = 0.022e-6; // Input coupling cap (in series with R1)
const C3: f64 = 100.0e-12; // Miller, Stage 1 (coll1 ↔ base1)
const C4: f64 = 100.0e-12; // Miller, Stage 2 (coll2 ↔ coll1)
const CE1: f64 = 4.7e-6; // Feedback coupling (emit1 ↔ fb)
const CE2: f64 = 22.0e-6; // Stage 2 emitter bypass (emit2 ↔ emit2b)

// BJT (2N5089, forward-active)
const IS: f64 = 3.03e-14; // Saturation current
const VT: f64 = 0.026; // Thermal voltage (25°C)
const IS_OVER_VT: f64 = IS / VT;

// Max Vbe clamp — prevents exp overflow while allowing full operating range.
// Real 2N5089 Vbe never exceeds ~0.8V; 0.85V gives ample margin.
const VBE_MAX: f64 = 0.85;

// Node indices
const BASE1: usize = 0;
const EMIT1: usize = 1;
const COLL1: usize = 2;
const EMIT2: usize = 3;
const EMIT2B: usize = 4;
const COLL2: usize = 5;
const OUT: usize = 6;
const FB: usize = 7;

const N: usize = 8; // number of nodes

// ── 8×8 matrix type aliases ─────────────────────────────────────────────────

type Mat8 = [[f64; N]; N];
type Vec8 = [f64; N];

fn mat8_zero() -> Mat8 {
    [[0.0; N]; N]
}
fn vec8_zero() -> Vec8 {
    [0.0; N]
}

/// Matrix-vector multiply: y = A * x
fn mat_vec_mul(a: &Mat8, x: &Vec8) -> Vec8 {
    let mut y = vec8_zero();
    for i in 0..N {
        let mut sum = 0.0;
        for j in 0..N {
            sum += a[i][j] * x[j];
        }
        y[i] = sum;
    }
    y
}

/// Matrix add: C = A + B
fn mat_add(a: &Mat8, b: &Mat8) -> Mat8 {
    let mut c = mat8_zero();
    for i in 0..N {
        for j in 0..N {
            c[i][j] = a[i][j] + b[i][j];
        }
    }
    c
}

/// Matrix subtract: C = A - B
fn mat_sub(a: &Mat8, b: &Mat8) -> Mat8 {
    let mut c = mat8_zero();
    for i in 0..N {
        for j in 0..N {
            c[i][j] = a[i][j] - b[i][j];
        }
    }
    c
}

/// Scale matrix: B = scalar * A
fn mat_scale(scalar: f64, a: &Mat8) -> Mat8 {
    let mut b = mat8_zero();
    for i in 0..N {
        for j in 0..N {
            b[i][j] = scalar * a[i][j];
        }
    }
    b
}

/// Gauss-Jordan inverse of an 8×8 matrix. Panics if singular.
fn mat_inverse(m: &Mat8) -> Mat8 {
    let mut aug = [[0.0f64; N * 2]; N];
    for i in 0..N {
        for j in 0..N {
            aug[i][j] = m[i][j];
            aug[i][N + j] = if i == j { 1.0 } else { 0.0 };
        }
    }

    for col in 0..N {
        let mut max_val = aug[col][col].abs();
        let mut max_row = col;
        for row in (col + 1)..N {
            if aug[row][col].abs() > max_val {
                max_val = aug[row][col].abs();
                max_row = row;
            }
        }
        assert!(max_val > 1e-30, "Singular matrix in Gauss-Jordan inverse");

        if max_row != col {
            aug.swap(col, max_row);
        }

        let pivot = aug[col][col];
        for j in 0..(N * 2) {
            aug[col][j] /= pivot;
        }

        for row in 0..N {
            if row != col {
                let factor = aug[row][col];
                for j in 0..(N * 2) {
                    aug[row][j] -= factor * aug[col][j];
                }
            }
        }
    }

    let mut inv = mat8_zero();
    for i in 0..N {
        for j in 0..N {
            inv[i][j] = aug[i][N + j];
        }
    }
    inv
}

// ── DK Preamp ───────────────────────────────────────────────────────────────

pub struct DkPreamp {
    // ── Explicit R_ldr approach ──
    //
    // R_ldr is NOT stamped into G. Instead, the R_ldr current (v_FB/R_ldr)
    // is handled as an explicit source term, corrected via Sherman-Morrison
    // on the FIXED S_base = inv(2C/T + G_base) matrix.
    //
    // Why: When R_ldr is in G, changing R_ldr changes A = 2C/T + G, which
    // creates a mismatch between the forward matrix (A with new R_ldr) and
    // the history stored in Ce1's companion model (computed with old R_ldr).
    // Ce1's companion conductance (g_c = 2*4.7µF/T = 829 S at 88.2 kHz)
    // dominates the MNA system, so even small matrix changes create massive
    // DC transients as Ce1 charge redistribution overwhelms the AC signal.
    //
    // With R_ldr explicit, S_base and A_neg_base are CONSTANT. The Ce1
    // companion is always self-consistent. R_ldr only affects the v_pred
    // via a scalar SM correction on v_FB, preserving the Ce1 charge state.

    // ── Fixed matrices (never change after construction) ──
    s_base: Mat8,     // inv(A_base) where A_base = 2C/T + G_base (no R_ldr)
    a_neg_base: Mat8, // 2C/T - G_base (no R_ldr)
    k: [[f64; 2]; 2], // DK kernel = N_v * S_base * N_i (R_ldr-independent)
    two_w: Vec8,      // 2 * w

    // ── SM projection vectors for R_ldr ──
    s_fb_col: Vec8, // S_base[:,FB] — column FB of S_base
    #[cfg_attr(not(test), allow(dead_code))]
    s_fb_row: Vec8, // S_base[FB,:] — row FB of S_base (used in tests)
    s_fb_fb: f64,   // S_base[FB][FB] — SM denominator scalar
    nv_sfb: [f64; 2], // N_v * s_fb_col: NL voltage extraction at FB col
    sfb_ni: [f64; 2], // s_fb_row * N_i: NL current injection at FB row

    // ── DC operating point ──
    v_dc: Vec8,      // DC node voltages at current R_ldr
    g_dc_base: Mat8, // G_dc without R_ldr or g_cin (for DC solve)

    // ── Cin-R1 companion (shared constants) ──
    g_cin: f64,
    c_cin: f64,
    gc_1pc: f64,

    // ── Per-instance mutable state ──
    //
    // The main state processes the audio signal. The shadow state runs in
    // parallel with zero input, producing only the tremolo pump. Subtracting
    // shadow output from main output cancels all pump harmonics without
    // any frequency-domain filtering — zero bass loss, zero phase distortion.
    //
    // Why: R_ldr modulation at 5.63 Hz creates a ~4.5V pp pump at the output
    // (confirmed by SPICE: Ce1 transient dynamics dominate, not DC shift).
    // The pump has harmonics spanning 28-200+ Hz that overlap bass fundamentals.
    // No HPF can separate them without cutting bass. Shadow subtraction is
    // frequency-independent and exact (for small audio signals ≪ operating point).
    main: DkState,
    shadow: DkState,

    // ── Shared R_ldr tracking ──
    r_ldr: f64,
    g_ldr: f64,      // 1/r_ldr (current conductance)
    g_ldr_prev: f64, // g_ldr from previous timestep

    // ── Shadow bypass (when tremolo depth ≈ 0, R_ldr is constant → shadow output is constant) ──
    shadow_bypass: bool,
    shadow_dc: f64, // Captured shadow output when transitioning to bypass
}

/// Per-instance mutable state for the DK solver.
/// Both main and shadow instances share the same fixed matrices and R_ldr.
#[derive(Clone)]
struct DkState {
    j_cin: f64,
    cin_rhs_prev: f64,
    v: Vec8,        // Absolute node voltages
    i_nl: [f64; 2], // Absolute NL currents
    v_nl: [f64; 2], // Full Vbe (for NR warm start)
}

impl DkPreamp {
    pub fn new(sample_rate: f64) -> Self {
        let t = 1.0 / sample_rate;
        let two_over_t = 2.0 / t;

        // ── Cin-R1 companion model parameters ──
        let alpha_cin = 2.0 * R1 * CIN * sample_rate;
        let g_cin = (2.0 * CIN * sample_rate) / (1.0 + alpha_cin);
        let c_cin = (1.0 - alpha_cin) / (1.0 + alpha_cin);
        let gc_1pc = g_cin * (1.0 + c_cin);

        // ── Stamp G_base matrix (without R_ldr, WITH g_cin) ──
        let mut g_base = mat8_zero();
        let mut w = vec8_zero();

        g_base[BASE1][BASE1] += 1.0 / R2;
        w[BASE1] += VCC / R2;
        g_base[BASE1][BASE1] += 1.0 / R3;
        g_base[EMIT1][EMIT1] += 1.0 / RE1;
        g_base[COLL1][COLL1] += 1.0 / RC1;
        w[COLL1] += VCC / RC1;
        stamp_resistor(&mut g_base, EMIT2, EMIT2B, RE2A);
        g_base[EMIT2B][EMIT2B] += 1.0 / RE2B;
        g_base[COLL2][COLL2] += 1.0 / RC2;
        w[COLL2] += VCC / RC2;
        stamp_resistor(&mut g_base, COLL2, OUT, R9);
        stamp_resistor(&mut g_base, OUT, FB, R10);

        // G_dc_base: no R_ldr, no g_cin (for DC solves)
        let g_dc_base = g_base;

        // Add g_cin to g_base (for transient matrices)
        g_base[BASE1][BASE1] += g_cin;

        // ── Stamp C matrix ──
        let mut c = mat8_zero();
        stamp_capacitor(&mut c, COLL1, BASE1, C3);
        stamp_capacitor(&mut c, COLL2, COLL1, C4);
        stamp_capacitor(&mut c, EMIT1, FB, CE1);
        stamp_capacitor(&mut c, EMIT2, EMIT2B, CE2);
        let two_c_over_t = mat_scale(two_over_t, &c);

        let mut two_w = vec8_zero();
        for i in 0..N {
            two_w[i] = 2.0 * w[i];
        }

        // ── Build FIXED transient matrices (no R_ldr) ──
        let a_base = mat_add(&two_c_over_t, &g_base);
        let a_neg_base = mat_sub(&two_c_over_t, &g_base);
        let s_base = mat_inverse(&a_base);
        let k = compute_k(&s_base);

        // Extract SM projection vectors
        let mut s_fb_col = vec8_zero();
        let mut s_fb_row = vec8_zero();
        for i in 0..N {
            s_fb_col[i] = s_base[i][FB]; // column FB
            s_fb_row[i] = s_base[FB][i]; // row FB
        }
        let s_fb_fb = s_base[FB][FB];

        // Pre-compute NL extraction/injection vectors for K correction
        let nv_sfb = [
            s_fb_col[BASE1] - s_fb_col[EMIT1], // N_v[0,:] . s_fb_col
            s_fb_col[COLL1] - s_fb_col[EMIT2], // N_v[1,:] . s_fb_col
        ];
        let sfb_ni = [
            s_fb_row[EMIT1] - s_fb_row[COLL1], // s_fb_row . N_i[:,0]
            s_fb_row[EMIT2] - s_fb_row[COLL2], // s_fb_row . N_i[:,1]
        ];

        // ── DC solve at initial R_ldr ──
        let r_ldr_init = 1_000_000.0;
        let (_, v_nl_dc, v_dc, _) = Self::full_dc_solve(&g_dc_base, &w, r_ldr_init);

        // Both main and shadow start at identical DC operating point
        let init_state = DkState {
            j_cin: g_cin * v_dc[BASE1],
            cin_rhs_prev: g_cin * v_dc[BASE1],
            v: v_dc,
            i_nl: [bjt_ic(v_nl_dc[0]), bjt_ic(v_nl_dc[1])],
            v_nl: v_nl_dc,
        };

        Self {
            s_base,
            a_neg_base,
            k,
            two_w,

            s_fb_col,
            s_fb_row,
            s_fb_fb,
            nv_sfb,
            sfb_ni,

            v_dc,
            g_dc_base,

            g_cin,
            c_cin,
            gc_1pc,

            shadow: init_state.clone(),
            main: init_state,

            r_ldr: r_ldr_init,
            g_ldr: 1.0 / r_ldr_init,
            g_ldr_prev: 1.0 / r_ldr_init,

            shadow_bypass: false,
            shadow_dc: v_dc[OUT],
        }
    }

    /// Full DC solve: find quiescent operating point at a given R_ldr.
    /// Returns (i_nl_dc, v_nl_dc, v_dc, dc_rhs).
    fn full_dc_solve(g_dc_base: &Mat8, w: &Vec8, r_ldr: f64) -> ([f64; 2], [f64; 2], Vec8, Vec8) {
        let mut g_full = *g_dc_base;
        g_full[FB][FB] += 1.0 / r_ldr;
        let s_dc = mat_inverse(&g_full);
        let k_dc = compute_k(&s_dc);
        let sv = mat_vec_mul(&s_dc, w);
        let p_dc = [sv[BASE1] - sv[EMIT1], sv[COLL1] - sv[EMIT2]];

        let mut v_nl = [0.56, 0.66];
        for _iter in 0..100 {
            let (ic0, gm0) = bjt_ic_gm(v_nl[0]);
            let (ic1, gm1) = bjt_ic_gm(v_nl[1]);
            let f = [
                v_nl[0] - p_dc[0] - k_dc[0][0] * ic0 - k_dc[0][1] * ic1,
                v_nl[1] - p_dc[1] - k_dc[1][0] * ic0 - k_dc[1][1] * ic1,
            ];
            if f[0].abs() < 1e-12 && f[1].abs() < 1e-12 {
                break;
            }

            let j00 = 1.0 - k_dc[0][0] * gm0;
            let j01 = -k_dc[0][1] * gm1;
            let j10 = -k_dc[1][0] * gm0;
            let j11 = 1.0 - k_dc[1][1] * gm1;
            let det = j00 * j11 - j01 * j10;
            let inv_det = 1.0 / det;
            let dv0 = inv_det * (j11 * f[0] - j01 * f[1]);
            let dv1 = inv_det * (j00 * f[1] - j10 * f[0]);
            let max_step = 2.0 * VT;
            v_nl[0] -= dv0.clamp(-max_step, max_step);
            v_nl[1] -= dv1.clamp(-max_step, max_step);
        }

        let ic = [bjt_ic(v_nl[0]), bjt_ic(v_nl[1])];
        let mut dc_rhs = *w;
        dc_rhs[EMIT1] += ic[0];
        dc_rhs[COLL1] -= ic[0];
        dc_rhs[EMIT2] += ic[1];
        dc_rhs[COLL2] -= ic[1];
        let v_dc = mat_vec_mul(&s_dc, &dc_rhs);

        (ic, v_nl, v_dc, dc_rhs)
    }

    /// Enable/disable shadow preamp bypass.
    /// When tremolo depth ≈ 0, R_ldr is constant so shadow output is constant DC.
    /// Bypassing saves ~50% of DK solver cost.
    pub fn set_shadow_bypass(&mut self, bypass: bool) {
        if bypass && !self.shadow_bypass {
            // Transitioning to bypass: capture current shadow output as constant
            self.shadow_dc = self.shadow.v[OUT];
            self.shadow_bypass = true;
        } else if !bypass && self.shadow_bypass {
            // Transitioning from bypass: re-sync shadow to current DC operating point
            let w = self.two_w_half();
            let (_, v_nl_dc, v_dc, _) = Self::full_dc_solve(&self.g_dc_base, &w, self.r_ldr);
            self.shadow = DkState {
                j_cin: self.g_cin * v_dc[BASE1],
                cin_rhs_prev: self.g_cin * v_dc[BASE1],
                v: v_dc,
                i_nl: [bjt_ic(v_nl_dc[0]), bjt_ic(v_nl_dc[1])],
                v_nl: v_nl_dc,
            };
            self.shadow_bypass = false;
        }
    }

    /// Get w = two_w / 2 (the original DC source vector).
    fn two_w_half(&self) -> Vec8 {
        let mut w = vec8_zero();
        for i in 0..N {
            w[i] = self.two_w[i] * 0.5;
        }
        w
    }
}

/// Compute K = N_v * S * N_i from an 8x8 matrix S.
fn compute_k(s: &Mat8) -> [[f64; 2]; 2] {
    [
        [
            s[BASE1][EMIT1] - s[BASE1][COLL1] - s[EMIT1][EMIT1] + s[EMIT1][COLL1],
            s[BASE1][EMIT2] - s[BASE1][COLL2] - s[EMIT1][EMIT2] + s[EMIT1][COLL2],
        ],
        [
            s[COLL1][EMIT1] - s[COLL1][COLL1] - s[EMIT2][EMIT1] + s[EMIT2][COLL1],
            s[COLL1][EMIT2] - s[COLL1][COLL2] - s[EMIT2][EMIT2] + s[EMIT2][COLL2],
        ],
    ]
}

/// Core DK trapezoidal step — free function for borrow-checker compatibility.
///
/// Both main and shadow instances call this with the same immutable config but
/// different mutable state. Making this a free function (not a method) allows
/// Rust's borrow checker to split borrows at the field level: config fields
/// borrowed immutably, state field borrowed mutably, in the same call.
///
/// The shadow runs with `input=0.0` to produce the pure pump signal.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn dk_step(
    a_neg_base: &Mat8,
    two_w: &Vec8,
    s_base: &Mat8,
    s_fb_col: &Vec8,
    s_fb_fb: f64,
    g_ldr: f64,
    g_ldr_prev: f64,
    k: &[[f64; 2]; 2],
    nv_sfb: &[f64; 2],
    sfb_ni: &[f64; 2],
    g_cin: f64,
    gc_1pc: f64,
    c_cin: f64,
    state: &mut DkState,
    input: f64,
) -> f64 {
    // 1. History: rhs = A_neg_base * v[n] + sources
    let mut rhs = mat_vec_mul(a_neg_base, &state.v);

    // Subtract previous R_ldr current (explicit, trapezoidal backward term)
    rhs[FB] -= g_ldr_prev * state.v[FB];

    // Cin-R1 companion
    let cin_rhs_now = g_cin * input + state.j_cin;
    rhs[BASE1] += cin_rhs_now + state.cin_rhs_prev;

    // Previous NL currents
    rhs[EMIT1] += state.i_nl[0];
    rhs[COLL1] -= state.i_nl[0];
    rhs[EMIT2] += state.i_nl[1];
    rhs[COLL2] -= state.i_nl[1];

    // DC sources (2w)
    for i in 0..N {
        rhs[i] += two_w[i];
    }

    // 2. v_pred_base = S_base * rhs (without R_ldr on LHS)
    let v_pred_base = mat_vec_mul(s_base, &rhs);

    // 3. SM correction for current R_ldr
    let sm_k = g_ldr / (1.0 + s_fb_fb * g_ldr);
    let sm_vpred = sm_k * v_pred_base[FB];
    let mut v_pred = vec8_zero();
    for i in 0..N {
        v_pred[i] = v_pred_base[i] - sm_vpred * s_fb_col[i];
    }

    // 4. Predicted NL voltages
    let p = [v_pred[BASE1] - v_pred[EMIT1], v_pred[COLL1] - v_pred[EMIT2]];

    // 5. NR solve on 2x2 system with R_ldr-corrected K
    let k00 = k[0][0] - sm_k * nv_sfb[0] * sfb_ni[0];
    let k01 = k[0][1] - sm_k * nv_sfb[0] * sfb_ni[1];
    let k10 = k[1][0] - sm_k * nv_sfb[1] * sfb_ni[0];
    let k11 = k[1][1] - sm_k * nv_sfb[1] * sfb_ni[1];

    let mut v_nl = state.v_nl;

    for _iter in 0..6 {
        let (ic0, gm0) = bjt_ic_gm(v_nl[0]);
        let (ic1, gm1) = bjt_ic_gm(v_nl[1]);

        let f0 = v_nl[0] - p[0] - k00 * ic0 - k01 * ic1;
        let f1 = v_nl[1] - p[1] - k10 * ic0 - k11 * ic1;

        if f0.abs() < 1e-9 && f1.abs() < 1e-9 {
            break;
        }

        let j00 = 1.0 - k00 * gm0;
        let j01 = -k01 * gm1;
        let j10 = -k10 * gm0;
        let j11 = 1.0 - k11 * gm1;

        let det = j00 * j11 - j01 * j10;
        if det.abs() < 1e-30 {
            break;
        }
        let inv_det = 1.0 / det;

        v_nl[0] -= inv_det * (j11 * f0 - j01 * f1);
        v_nl[1] -= inv_det * (j00 * f1 - j10 * f0);
    }

    // 6. Final NL currents
    let ic_new = [bjt_ic(v_nl[0]), bjt_ic(v_nl[1])];

    // 7. Node voltage update
    let sfb_ni_dot_ic = sfb_ni[0] * ic_new[0] + sfb_ni[1] * ic_new[1];
    for i in 0..N {
        let s_ni_i = ic_new[0] * (s_base[i][EMIT1] - s_base[i][COLL1])
            + ic_new[1] * (s_base[i][EMIT2] - s_base[i][COLL2]);
        state.v[i] = v_pred[i] + s_ni_i - sm_k * sfb_ni_dot_ic * s_fb_col[i];
    }

    // 8. Cin-R1 companion update
    state.cin_rhs_prev = cin_rhs_now;
    let dv_cin = input - state.v[BASE1];
    state.j_cin = -gc_1pc * dv_cin - c_cin * state.j_cin;

    // 9. State update
    state.i_nl = ic_new;
    state.v_nl = v_nl;

    state.v[OUT]
}

impl PreampModel for DkPreamp {
    fn process_sample(&mut self, input: f64) -> f64 {
        // Run main solver with audio input.
        // Field-level borrow splitting: config fields (&self.xxx) are immutable,
        // state field (&mut self.main) is mutable — different fields, no conflict.
        let main_out = dk_step(
            &self.a_neg_base,
            &self.two_w,
            &self.s_base,
            &self.s_fb_col,
            self.s_fb_fb,
            self.g_ldr,
            self.g_ldr_prev,
            &self.k,
            &self.nv_sfb,
            &self.sfb_ni,
            self.g_cin,
            self.gc_1pc,
            self.c_cin,
            &mut self.main,
            input,
        );

        // Run shadow solver with zero input — produces pure pump.
        // When shadow_bypass is active (tremolo off), R_ldr is constant so
        // shadow output is constant DC — skip the expensive DK solve.
        let pump = if self.shadow_bypass {
            self.shadow_dc
        } else {
            dk_step(
                &self.a_neg_base,
                &self.two_w,
                &self.s_base,
                &self.s_fb_col,
                self.s_fb_fb,
                self.g_ldr,
                self.g_ldr_prev,
                &self.k,
                &self.nv_sfb,
                &self.sfb_ni,
                self.g_cin,
                self.gc_1pc,
                self.c_cin,
                &mut self.shadow,
                0.0,
            )
        };

        // Update shared R_ldr tracking (after both steps used g_ldr_prev)
        self.g_ldr_prev = self.g_ldr;

        // Subtract pump: cancels all tremolo pump harmonics (28-200+ Hz)
        // without any frequency-domain filtering. Zero bass loss.
        let result = main_out - pump;

        // NaN guard: if NR diverged, reset state and return silence.
        // Branch never taken in normal operation.
        if !result.is_finite() {
            self.reset();
            return 0.0;
        }

        result
    }

    fn set_ldr_resistance(&mut self, r_ldr_path: f64) {
        let new_r = r_ldr_path.max(1000.0);
        if (new_r - self.r_ldr).abs() > 0.01 {
            self.r_ldr = new_r;
            self.g_ldr = 1.0 / new_r;
        }
    }

    fn reset(&mut self) {
        // Full DC solve at current R_ldr
        let w = self.two_w_half();
        let (_, v_nl_dc, v_dc, _) = Self::full_dc_solve(&self.g_dc_base, &w, self.r_ldr);

        self.v_dc = v_dc;
        self.g_ldr = 1.0 / self.r_ldr;
        self.g_ldr_prev = self.g_ldr;

        // Reset both main and shadow to identical DC operating point
        let state = DkState {
            j_cin: self.g_cin * v_dc[BASE1],
            cin_rhs_prev: self.g_cin * v_dc[BASE1],
            v: v_dc,
            i_nl: [bjt_ic(v_nl_dc[0]), bjt_ic(v_nl_dc[1])],
            v_nl: v_nl_dc,
        };
        self.shadow = state.clone();
        self.main = state;
        self.shadow_dc = v_dc[OUT];
        self.shadow_bypass = false;
    }
}

// ── Resistor/capacitor stamp helpers ────────────────────────────────────────

fn stamp_resistor(g: &mut Mat8, i: usize, j: usize, r: f64) {
    let cond = 1.0 / r;
    g[i][i] += cond;
    g[j][j] += cond;
    g[i][j] -= cond;
    g[j][i] -= cond;
}

fn stamp_capacitor(c: &mut Mat8, i: usize, j: usize, cap: f64) {
    c[i][i] += cap;
    c[j][j] += cap;
    c[i][j] -= cap;
    c[j][i] -= cap;
}

// ── BJT model ───────────────────────────────────────────────────────────────

/// BJT collector current: Ic = Is * (exp(Vbe/Vt) - 1).
/// Vbe clamped to [-1.0, VBE_MAX] to prevent exp overflow.
/// No artificial saturation limiting — the circuit topology and NR solver
/// naturally constrain the operating point.
#[inline]
fn bjt_ic(vbe: f64) -> f64 {
    let vbe_clamped = vbe.clamp(-1.0, VBE_MAX);
    IS * ((vbe_clamped / VT).exp() - 1.0)
}

/// BJT transconductance: gm = (Is/Vt) * exp(Vbe/Vt).
/// Only used in small-signal transfer function tests.
#[cfg(test)]
#[inline]
fn bjt_gm(vbe: f64) -> f64 {
    let vbe_clamped = vbe.clamp(-1.0, VBE_MAX);
    IS_OVER_VT * (vbe_clamped / VT).exp()
}

/// BJT collector current AND transconductance from a single exp().
/// Returns (ic, gm) = (Is*(e-1), Is/Vt*e) where e = exp(Vbe/Vt).
/// Saves one exp() per BJT per NR iteration in the hot loop.
#[inline]
fn bjt_ic_gm(vbe: f64) -> (f64, f64) {
    let vbe_clamped = vbe.clamp(-1.0, VBE_MAX);
    let e = (vbe_clamped / VT).exp();
    (IS * (e - 1.0), IS_OVER_VT * e)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    // ── Shared helpers ───────────────────────────────────────────────────────

    /// Build G_dc matrix independently from DkPreamp (for stamp verification).
    /// Does NOT include Cin-R1 companion or R_ldr.
    fn build_g_dc() -> Mat8 {
        let mut g = mat8_zero();
        g[BASE1][BASE1] += 1.0 / R2;
        g[BASE1][BASE1] += 1.0 / R3;
        g[EMIT1][EMIT1] += 1.0 / RE1;
        g[COLL1][COLL1] += 1.0 / RC1;
        stamp_resistor(&mut g, EMIT2, EMIT2B, RE2A);
        g[EMIT2B][EMIT2B] += 1.0 / RE2B;
        g[COLL2][COLL2] += 1.0 / RC2;
        stamp_resistor(&mut g, COLL2, OUT, R9);
        stamp_resistor(&mut g, OUT, FB, R10);
        g
    }

    /// Build C matrix independently from DkPreamp (for stamp verification).
    fn build_c_matrix() -> Mat8 {
        let mut c = mat8_zero();
        stamp_capacitor(&mut c, COLL1, BASE1, C3);
        stamp_capacitor(&mut c, COLL2, COLL1, C4);
        stamp_capacitor(&mut c, EMIT1, FB, CE1);
        stamp_capacitor(&mut c, EMIT2, EMIT2B, CE2);
        c
    }

    /// Build DC source vector independently from DkPreamp.
    fn build_w_vec() -> Vec8 {
        let mut w = vec8_zero();
        w[BASE1] += VCC / R2;
        w[COLL1] += VCC / RC1;
        w[COLL2] += VCC / RC2;
        w
    }

    /// 8×8 matrix multiply: C = A * B
    fn mat_mul_8x8(a: &Mat8, b: &Mat8) -> Mat8 {
        let mut c = mat8_zero();
        for i in 0..N {
            for j in 0..N {
                let mut sum = 0.0;
                for k in 0..N {
                    sum += a[i][k] * b[k][j];
                }
                c[i][j] = sum;
            }
        }
        c
    }

    // compute_k() is now a module-level function, reusable from tests

    // ── Complex arithmetic for Layer 4 ───────────────────────────────────────

    type C64 = (f64, f64);

    fn c_add(a: C64, b: C64) -> C64 {
        (a.0 + b.0, a.1 + b.1)
    }
    fn c_sub(a: C64, b: C64) -> C64 {
        (a.0 - b.0, a.1 - b.1)
    }
    fn c_mul(a: C64, b: C64) -> C64 {
        (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
    }
    fn c_div(a: C64, b: C64) -> C64 {
        let d = b.0 * b.0 + b.1 * b.1;
        ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
    }
    fn c_abs(a: C64) -> f64 {
        (a.0 * a.0 + a.1 * a.1).sqrt()
    }

    /// Solve complex 8×8 system A*x = b via Gauss-Jordan with partial pivoting.
    fn complex_solve(a: &[[C64; N]; N], b: &[C64; N]) -> [C64; N] {
        let mut aug = [[(0.0, 0.0); N + 1]; N];
        for i in 0..N {
            for j in 0..N {
                aug[i][j] = a[i][j];
            }
            aug[i][N] = b[i];
        }

        for col in 0..N {
            let mut max_abs = c_abs(aug[col][col]);
            let mut max_row = col;
            for row in (col + 1)..N {
                let abs = c_abs(aug[row][col]);
                if abs > max_abs {
                    max_abs = abs;
                    max_row = row;
                }
            }
            aug.swap(col, max_row);

            let pivot = aug[col][col];
            for j in 0..(N + 1) {
                aug[col][j] = c_div(aug[col][j], pivot);
            }

            for row in 0..N {
                if row != col {
                    let factor = aug[row][col];
                    for j in 0..(N + 1) {
                        let scaled = c_mul(factor, aug[col][j]);
                        aug[row][j] = c_sub(aug[row][j], scaled);
                    }
                }
            }
        }

        let mut x = [(0.0, 0.0); N];
        for i in 0..N {
            x[i] = aug[i][N];
        }
        x
    }

    /// Small-signal gain in dB at a given frequency.
    /// Uses the continuous-time linearized circuit model — no sample rate dependency.
    fn small_signal_gain_db(gm1: f64, gm2: f64, r_ldr: f64, freq_hz: f64) -> f64 {
        let omega = 2.0 * PI * freq_hz;
        let jw: C64 = (0.0, omega);

        // G_lin = G_dc + R_ldr + BJT VCCS stamps
        let mut g_lin = build_g_dc();
        g_lin[FB][FB] += 1.0 / r_ldr;

        // TR-1 VCCS: Ic1 = gm1*(V_base1 - V_emit1), enters emit1, leaves coll1
        g_lin[EMIT1][BASE1] += gm1;
        g_lin[EMIT1][EMIT1] -= gm1;
        g_lin[COLL1][BASE1] -= gm1;
        g_lin[COLL1][EMIT1] += gm1;

        // TR-2 VCCS: Ic2 = gm2*(V_coll1 - V_emit2), enters emit2, leaves coll2
        g_lin[EMIT2][COLL1] += gm2;
        g_lin[EMIT2][EMIT2] -= gm2;
        g_lin[COLL2][COLL1] -= gm2;
        g_lin[COLL2][EMIT2] += gm2;

        // Cin-R1 input admittance: Y = jωCin / (1 + jωR1Cin)
        let jwrc = c_mul(jw, (R1 * CIN, 0.0));
        let y_cin = c_div(c_mul(jw, (CIN, 0.0)), c_add((1.0, 0.0), jwrc));

        // Complex system matrix: A(jω) = jωC + G_lin + Y_cin at [0][0]
        let c_mat = build_c_matrix();
        let mut a_cpx = [[(0.0, 0.0); N]; N];
        for i in 0..N {
            for j in 0..N {
                a_cpx[i][j] = c_add(c_mul(jw, (c_mat[i][j], 0.0)), (g_lin[i][j], 0.0));
            }
        }
        a_cpx[BASE1][BASE1] = c_add(a_cpx[BASE1][BASE1], y_cin);

        // Solve: A * v = Y_cin * e_base1  (unit Vin)
        let mut b = [(0.0, 0.0); N];
        b[BASE1] = y_cin;

        let v = complex_solve(&a_cpx, &b);
        20.0 * c_abs(v[OUT]).log10()
    }

    /// Find -3dB bandwidth by binary search on the transfer function.
    fn find_bandwidth(gm1: f64, gm2: f64, r_ldr: f64) -> f64 {
        let ref_gain = small_signal_gain_db(gm1, gm2, r_ldr, 1000.0);
        let target = ref_gain - 3.0;
        let mut lo: f64 = 1000.0;
        let mut hi: f64 = 200_000.0;
        for _ in 0..60 {
            let mid = (lo * hi).sqrt();
            if small_signal_gain_db(gm1, gm2, r_ldr, mid) > target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo * hi).sqrt()
    }

    fn measure_gain(preamp: &mut DkPreamp, freq: f64, amplitude: f64, sr: f64) -> f64 {
        preamp.reset();
        let n_settle = (sr * 0.3) as usize;
        let n_measure = (sr * 0.2) as usize;

        for i in 0..n_settle {
            let t = i as f64 / sr;
            let input = amplitude * (2.0 * PI * freq * t).sin();
            preamp.process_sample(input);
        }

        let mut peak = 0.0f64;
        for i in 0..n_measure {
            let t = (n_settle + i) as f64 / sr;
            let input = amplitude * (2.0 * PI * freq * t).sin();
            let output = preamp.process_sample(input);
            peak = peak.max(output.abs());
        }

        peak / amplitude
    }

    #[test]
    fn test_dc_operating_point() {
        let sr = 88200.0;
        let preamp = DkPreamp::new(sr);
        let v = preamp.v_dc; // At init, dv=0, so v_dc IS the operating point

        // SPICE ground truth (ideal BJT, BF=100000, R_ldr=1M, no C20/D1/Rload):
        //   base1=2.854, emit1=2.297, coll1=4.556
        //   emit2a=3.897, emit2b=2.931, coll2=8.551
        //   out=8.496, fb=8.045
        assert!(
            (v[BASE1] - 2.854).abs() < 0.1,
            "TR-1 base: {:.3}V, want ~2.854V",
            v[BASE1]
        );
        assert!(
            (v[EMIT1] - 2.297).abs() < 0.1,
            "TR-1 emitter: {:.3}V, want ~2.297V",
            v[EMIT1]
        );
        assert!(
            (v[COLL1] - 4.556).abs() < 0.5,
            "TR-1 collector: {:.3}V, want ~4.556V",
            v[COLL1]
        );
        assert!(
            (v[EMIT2] - 3.897).abs() < 0.5,
            "TR-2 emitter: {:.3}V, want ~3.897V",
            v[EMIT2]
        );
        assert!(
            (v[COLL2] - 8.551).abs() < 1.0,
            "TR-2 collector: {:.3}V, want ~8.551V",
            v[COLL2]
        );

        let vbe1 = v[BASE1] - v[EMIT1];
        let vbe2 = v[COLL1] - v[EMIT2];
        assert!(
            vbe1 > 0.45 && vbe1 < 0.70,
            "Vbe1 = {vbe1:.3}V, want 0.5-0.65V"
        );
        assert!(
            vbe2 > 0.55 && vbe2 < 0.75,
            "Vbe2 = {vbe2:.3}V, want 0.6-0.7V"
        );
    }

    #[test]
    fn test_gain_no_tremolo() {
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);
        preamp.set_ldr_resistance(1_000_000.0);

        let gain = measure_gain(&mut preamp, 1000.0, 0.001, sr);
        let gain_db = 20.0 * gain.log10();

        assert!(
            gain_db > 3.0 && gain_db < 12.0,
            "Gain @ 1kHz no tremolo = {gain_db:.1} dB, want ~6 dB"
        );
    }

    #[test]
    fn test_gain_increases_with_tremolo() {
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);

        preamp.set_ldr_resistance(1_000_000.0);
        let gain_no_trem = measure_gain(&mut preamp, 1000.0, 0.001, sr);

        preamp.set_ldr_resistance(19_000.0);
        let gain_trem = measure_gain(&mut preamp, 1000.0, 0.001, sr);

        let no_trem_db = 20.0 * gain_no_trem.log10();
        let trem_db = 20.0 * gain_trem.log10();

        assert!(
            gain_trem > gain_no_trem * 1.2,
            "Tremolo bright gain ({trem_db:.1} dB) should exceed no-tremolo ({no_trem_db:.1} dB)"
        );
    }

    #[test]
    fn test_h2_dominates() {
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);
        preamp.set_ldr_resistance(1_000_000.0);

        let freq = 440.0;
        let n = (sr * 0.3) as usize;
        let mut output = vec![0.0f64; n];

        for i in 0..n {
            let t = i as f64 / sr;
            let input = 0.005 * (2.0 * PI * freq * t).sin();
            output[i] = preamp.process_sample(input);
        }

        let start = n * 3 / 4;
        let h2 = dft_magnitude(&output[start..], 2.0 * freq, sr);
        let h3 = dft_magnitude(&output[start..], 3.0 * freq, sr);

        if h3 > 1e-15 {
            assert!(h2 > h3, "H2 ({h2:.2e}) should dominate H3 ({h3:.2e})");
        }
    }

    #[test]
    fn test_stability() {
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);

        preamp.process_sample(0.01);

        // Run 2 seconds — Ce1×R_ldr has τ=4.7s, need long settling.
        let mut last = 0.0;
        for _ in 0..(sr * 2.0) as usize {
            last = preamp.process_sample(0.0);
        }

        // After 2s: output must be decaying, not growing. Allow 1e-3 for
        // the slow Ce1/R_ldr exponential tail (τ=4.7s → e^(-2/4.7)=0.65).
        assert!(
            last.abs() < 1e-3,
            "DK preamp should be stable after impulse, got {last}"
        );
    }

    #[test]
    fn test_bandwidth_rolloff() {
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);
        preamp.set_ldr_resistance(1_000_000.0);

        let gain_1k = measure_gain(&mut preamp, 1000.0, 0.001, sr);
        let gain_15k = measure_gain(&mut preamp, 15000.0, 0.001, sr);

        assert!(
            gain_15k < gain_1k,
            "Should roll off at HF: 1kHz={gain_1k:.2}x, 15kHz={gain_15k:.2}x"
        );
    }

    #[test]
    fn test_bandwidth_independent_of_rldr() {
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);

        preamp.set_ldr_resistance(1_000_000.0);
        let gain_1k_notrem = measure_gain(&mut preamp, 1000.0, 0.001, sr);
        let gain_10k_notrem = measure_gain(&mut preamp, 10000.0, 0.001, sr);
        let ratio_notrem = gain_10k_notrem / gain_1k_notrem;

        preamp.set_ldr_resistance(19_000.0);
        let gain_1k_trem = measure_gain(&mut preamp, 1000.0, 0.001, sr);
        let gain_10k_trem = measure_gain(&mut preamp, 10000.0, 0.001, sr);
        let ratio_trem = gain_10k_trem / gain_1k_trem;

        let ratio_notrem_db = 20.0 * ratio_notrem.log10();
        let ratio_trem_db = 20.0 * ratio_trem.log10();
        let delta = (ratio_notrem_db - ratio_trem_db).abs();

        assert!(
            delta < 6.0,
            "BW should be similar: no-trem 10k/1k = {ratio_notrem_db:.1} dB, \
             trem 10k/1k = {ratio_trem_db:.1} dB, delta = {delta:.1} dB (want < 6)"
        );
    }

    #[test]
    fn test_gbw_scales_with_gain() {
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);

        preamp.set_ldr_resistance(1_000_000.0);
        let gain_notrem = measure_gain(&mut preamp, 1000.0, 0.001, sr);
        let gain_notrem_10k = measure_gain(&mut preamp, 10000.0, 0.001, sr);
        let gbw_notrem = gain_notrem * 10000.0 * (gain_notrem_10k / gain_notrem);

        preamp.set_ldr_resistance(19_000.0);
        let gain_trem = measure_gain(&mut preamp, 1000.0, 0.001, sr);
        let gain_trem_10k = measure_gain(&mut preamp, 10000.0, 0.001, sr);
        let gbw_trem = gain_trem * 10000.0 * (gain_trem_10k / gain_trem);

        assert!(
            gbw_trem > gbw_notrem * 0.8,
            "GBW should scale with gain: no-trem GBW ~{gbw_notrem:.0}, trem GBW ~{gbw_trem:.0}"
        );
    }

    fn dft_magnitude(signal: &[f64], freq: f64, sr: f64) -> f64 {
        let n = signal.len() as f64;
        let mut re = 0.0;
        let mut im = 0.0;
        for (i, &s) in signal.iter().enumerate() {
            let phase = 2.0 * PI * freq * i as f64 / sr;
            re += s * phase.cos();
            im -= s * phase.sin();
        }
        ((re / n).powi(2) + (im / n).powi(2)).sqrt()
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Layer 1: Matrix Stamp Verification
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_l1_g_diagonal_stamps() {
        let g = build_g_dc();
        let eps = 1e-12;

        // Node 0 (base1): R2 to Vcc + R3 to GND
        assert!(
            (g[BASE1][BASE1] - (1.0 / R2 + 1.0 / R3)).abs() < eps,
            "G[0][0] = {:.6e}, want {:.6e}",
            g[BASE1][BASE1],
            1.0 / R2 + 1.0 / R3
        );

        // Node 1 (emit1): Re1 to GND
        assert!(
            (g[EMIT1][EMIT1] - 1.0 / RE1).abs() < eps,
            "G[1][1] = {:.6e}, want {:.6e}",
            g[EMIT1][EMIT1],
            1.0 / RE1
        );

        // Node 2 (coll1): Rc1 to Vcc
        assert!(
            (g[COLL1][COLL1] - 1.0 / RC1).abs() < eps,
            "G[2][2] = {:.6e}, want {:.6e}",
            g[COLL1][COLL1],
            1.0 / RC1
        );

        // Node 3 (emit2): Re2a to emit2b
        assert!(
            (g[EMIT2][EMIT2] - 1.0 / RE2A).abs() < eps,
            "G[3][3] = {:.6e}, want {:.6e}",
            g[EMIT2][EMIT2],
            1.0 / RE2A
        );

        // Node 4 (emit2b): Re2a from emit2 + Re2b to GND
        assert!(
            (g[EMIT2B][EMIT2B] - (1.0 / RE2A + 1.0 / RE2B)).abs() < eps,
            "G[4][4] = {:.6e}, want {:.6e}",
            g[EMIT2B][EMIT2B],
            1.0 / RE2A + 1.0 / RE2B
        );

        // Node 5 (coll2): Rc2 to Vcc + R9 to out
        assert!(
            (g[COLL2][COLL2] - (1.0 / RC2 + 1.0 / R9)).abs() < eps,
            "G[5][5] = {:.6e}, want {:.6e}",
            g[COLL2][COLL2],
            1.0 / RC2 + 1.0 / R9
        );

        // Node 6 (out): R9 from coll2 + R10 to fb
        assert!(
            (g[OUT][OUT] - (1.0 / R9 + 1.0 / R10)).abs() < eps,
            "G[6][6] = {:.6e}, want {:.6e}",
            g[OUT][OUT],
            1.0 / R9 + 1.0 / R10
        );

        // Node 7 (fb): R10 from out (no R_ldr yet)
        assert!(
            (g[FB][FB] - 1.0 / R10).abs() < eps,
            "G[7][7] = {:.6e}, want {:.6e}",
            g[FB][FB],
            1.0 / R10
        );
    }

    #[test]
    fn test_l1_g_off_diagonal_stamps() {
        let g = build_g_dc();
        let eps = 1e-12;

        // Re2a (270Ω) between emit2 (3) and emit2b (4)
        assert!((g[EMIT2][EMIT2B] - (-1.0 / RE2A)).abs() < eps);
        assert!((g[EMIT2B][EMIT2] - (-1.0 / RE2A)).abs() < eps);

        // R9 (6.8K) between coll2 (5) and out (6)
        assert!((g[COLL2][OUT] - (-1.0 / R9)).abs() < eps);
        assert!((g[OUT][COLL2] - (-1.0 / R9)).abs() < eps);

        // R10 (56K) between out (6) and fb (7)
        assert!((g[OUT][FB] - (-1.0 / R10)).abs() < eps);
        assert!((g[FB][OUT] - (-1.0 / R10)).abs() < eps);

        // All other off-diagonals must be zero
        let connected = [
            (EMIT2, EMIT2B),
            (EMIT2B, EMIT2),
            (COLL2, OUT),
            (OUT, COLL2),
            (OUT, FB),
            (FB, OUT),
        ];
        for i in 0..N {
            for j in 0..N {
                if i == j {
                    continue;
                }
                if connected.contains(&(i, j)) {
                    continue;
                }
                assert!(
                    g[i][j].abs() < eps,
                    "G[{}][{}] = {:.2e}, should be zero",
                    i,
                    j,
                    g[i][j]
                );
            }
        }
    }

    #[test]
    fn test_l1_c_matrix_stamps() {
        let c = build_c_matrix();
        let eps = 1e-15;

        // Diagonal entries
        assert!((c[BASE1][BASE1] - C3).abs() < eps, "C[0][0]");
        assert!((c[EMIT1][EMIT1] - CE1).abs() < eps, "C[1][1]");
        assert!(
            (c[COLL1][COLL1] - (C3 + C4)).abs() < eps,
            "C[2][2] should have C3+C4"
        );
        assert!((c[EMIT2][EMIT2] - CE2).abs() < eps, "C[3][3]");
        assert!((c[EMIT2B][EMIT2B] - CE2).abs() < eps, "C[4][4]");
        assert!((c[COLL2][COLL2] - C4).abs() < eps, "C[5][5]");
        assert!(
            c[OUT][OUT].abs() < eps,
            "C[6][6] should be zero (no cap at OUT)"
        );
        assert!((c[FB][FB] - CE1).abs() < eps, "C[7][7]");

        // Off-diagonal entries (caps create negative off-diags)
        assert!((c[BASE1][COLL1] - (-C3)).abs() < eps, "C3: base1↔coll1");
        assert!((c[COLL1][BASE1] - (-C3)).abs() < eps, "C3: coll1↔base1");
        assert!((c[COLL2][COLL1] - (-C4)).abs() < eps, "C4: coll2↔coll1");
        assert!((c[COLL1][COLL2] - (-C4)).abs() < eps, "C4: coll1↔coll2");
        assert!((c[EMIT1][FB] - (-CE1)).abs() < eps, "Ce1: emit1↔fb");
        assert!((c[FB][EMIT1] - (-CE1)).abs() < eps, "Ce1: fb↔emit1");
        assert!((c[EMIT2][EMIT2B] - (-CE2)).abs() < eps, "Ce2: emit2↔emit2b");
        assert!((c[EMIT2B][EMIT2] - (-CE2)).abs() < eps, "Ce2: emit2b↔emit2");
    }

    #[test]
    fn test_l1_c_matrix_symmetry() {
        let c = build_c_matrix();
        for i in 0..N {
            for j in 0..N {
                assert!(
                    (c[i][j] - c[j][i]).abs() < 1e-20,
                    "C not symmetric: C[{}][{}]={:.2e} != C[{}][{}]={:.2e}",
                    i,
                    j,
                    c[i][j],
                    j,
                    i,
                    c[j][i]
                );
            }
        }
    }

    #[test]
    fn test_l1_dc_source_vector() {
        let w = build_w_vec();
        let eps = 1e-12;

        assert!((w[BASE1] - VCC / R2).abs() < eps, "w[base1] = Vcc/R2");
        assert!((w[COLL1] - VCC / RC1).abs() < eps, "w[coll1] = Vcc/Rc1");
        assert!((w[COLL2] - VCC / RC2).abs() < eps, "w[coll2] = Vcc/Rc2");

        // All other entries zero
        for i in 0..N {
            if i == BASE1 || i == COLL1 || i == COLL2 {
                continue;
            }
            assert!(w[i].abs() < eps, "w[{}] = {:.2e}, should be zero", i, w[i]);
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Layer 2: Linear Algebra Identities
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_l2_s_base_inverse_identity() {
        // Verify S_base * A_base = I (no R_ldr in either)
        let sr = 88200.0;
        let t = 1.0 / sr;
        let two_over_t = 2.0 / t;

        let mut g = build_g_dc();
        let c = build_c_matrix();
        let two_c_t = mat_scale(two_over_t, &c);

        let alpha_cin = 2.0 * R1 * CIN * sr;
        let g_cin = (2.0 * CIN * sr) / (1.0 + alpha_cin);
        g[BASE1][BASE1] += g_cin;
        // NO R_ldr — S_base is built without it
        let a = mat_add(&two_c_t, &g);

        let preamp = DkPreamp::new(sr);
        let product = mat_mul_8x8(&preamp.s_base, &a);

        for i in 0..N {
            for j in 0..N {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (product[i][j] - expected).abs() < 1e-8,
                    "S_base*A_base[{}][{}] = {:.2e}, want {:.1}",
                    i,
                    j,
                    product[i][j],
                    expected
                );
            }
        }
    }

    #[test]
    fn test_l2_sm_gives_correct_s_eff() {
        // Verify SM update gives same result as brute-force S(R_ldr) for various R_ldr
        let sr = 88200.0;
        let t = 1.0 / sr;
        let two_over_t = 2.0 / t;

        let mut g_base = build_g_dc();
        let c = build_c_matrix();
        let two_c_t = mat_scale(two_over_t, &c);

        let alpha_cin = 2.0 * R1 * CIN * sr;
        let g_cin = (2.0 * CIN * sr) / (1.0 + alpha_cin);
        g_base[BASE1][BASE1] += g_cin;

        let preamp = DkPreamp::new(sr);

        for &r_ldr in &[1_000_000.0, 224_000.0, 50_000.0, 19_000.0] {
            // Brute-force: build S with R_ldr in G
            let mut g_full = g_base;
            g_full[FB][FB] += 1.0 / r_ldr;
            let a_full = mat_add(&two_c_t, &g_full);
            let s_expected = mat_inverse(&a_full);

            // SM: S_eff = S_base - sm_k * s_fb_col * s_fb_row^T
            let g_ldr = 1.0 / r_ldr;
            let sm_k = g_ldr / (1.0 + preamp.s_fb_fb * g_ldr);
            let mut s_sm = preamp.s_base;
            for i in 0..N {
                for j in 0..N {
                    s_sm[i][j] -= sm_k * preamp.s_fb_col[i] * preamp.s_fb_row[j];
                }
            }

            for i in 0..N {
                for j in 0..N {
                    let err = (s_sm[i][j] - s_expected[i][j]).abs();
                    let scale = s_expected[i][j].abs().max(1e-12);
                    assert!(
                        err < 1e-6 * scale + 1e-12,
                        "SM S_eff at R_ldr={r_ldr:.0}: [{i}][{j}] sm={:.6e}, bf={:.6e}",
                        s_sm[i][j],
                        s_expected[i][j]
                    );
                }
            }
        }
    }

    #[test]
    fn test_l2_k_matches_full_product() {
        // K = N_v * S_base * N_i should match direct computation
        let sr = 88200.0;
        let preamp = DkPreamp::new(sr);
        let k_full = compute_k(&preamp.s_base);

        for i in 0..2 {
            for j in 0..2 {
                assert!(
                    (preamp.k[i][j] - k_full[i][j]).abs() < 1e-10,
                    "K[{}][{}] mismatch: stored={:.6e}, computed={:.6e}",
                    i,
                    j,
                    preamp.k[i][j],
                    k_full[i][j]
                );
            }
        }
    }

    #[test]
    fn test_l2_k_eff_matches_brute_force() {
        // K_eff = K_base - sm_k * nv_sfb * sfb_ni^T should match
        // brute-force K = N_v * S(R_ldr) * N_i for various R_ldr values.
        let sr = 88200.0;
        let t = 1.0 / sr;
        let two_over_t = 2.0 / t;

        let mut g_base = build_g_dc();
        let c = build_c_matrix();
        let two_c_t = mat_scale(two_over_t, &c);

        let alpha_cin = 2.0 * R1 * CIN * sr;
        let g_cin = (2.0 * CIN * sr) / (1.0 + alpha_cin);
        g_base[BASE1][BASE1] += g_cin;

        let preamp = DkPreamp::new(sr);

        for &r_ldr in &[1_000_000.0, 224_000.0, 50_000.0, 19_000.0] {
            // Brute-force K
            let mut g_full = g_base;
            g_full[FB][FB] += 1.0 / r_ldr;
            let s_full = mat_inverse(&mat_add(&two_c_t, &g_full));
            let k_bf = compute_k(&s_full);

            // SM-corrected K
            let g_ldr = 1.0 / r_ldr;
            let sm_k = g_ldr / (1.0 + preamp.s_fb_fb * g_ldr);
            let k_sm = [
                [
                    preamp.k[0][0] - sm_k * preamp.nv_sfb[0] * preamp.sfb_ni[0],
                    preamp.k[0][1] - sm_k * preamp.nv_sfb[0] * preamp.sfb_ni[1],
                ],
                [
                    preamp.k[1][0] - sm_k * preamp.nv_sfb[1] * preamp.sfb_ni[0],
                    preamp.k[1][1] - sm_k * preamp.nv_sfb[1] * preamp.sfb_ni[1],
                ],
            ];

            for i in 0..2 {
                for j in 0..2 {
                    let err = (k_sm[i][j] - k_bf[i][j]).abs();
                    let scale = k_bf[i][j].abs().max(1e-6);
                    assert!(
                        err < 1e-6 * scale,
                        "K_eff[{i}][{j}] at R_ldr={r_ldr:.0}: sm={:.6e}, bf={:.6e}, err={err:.2e}",
                        k_sm[i][j],
                        k_bf[i][j]
                    );
                }
            }
        }
    }

    #[test]
    fn test_l2_a_neg_base_is_rldr_independent() {
        // a_neg_base has no R_ldr — verify it matches 2C/T - G_base
        let sr = 88200.0;
        let t = 1.0 / sr;
        let two_over_t = 2.0 / t;

        let mut g_base = build_g_dc();
        let c = build_c_matrix();
        let two_c_t = mat_scale(two_over_t, &c);

        let alpha_cin = 2.0 * R1 * CIN * sr;
        let g_cin = (2.0 * CIN * sr) / (1.0 + alpha_cin);
        g_base[BASE1][BASE1] += g_cin;

        // No R_ldr in a_neg_base
        let a_neg_expected = mat_sub(&two_c_t, &g_base);

        let preamp = DkPreamp::new(sr);

        for i in 0..N {
            for j in 0..N {
                assert!(
                    (preamp.a_neg_base[i][j] - a_neg_expected[i][j]).abs() < 1e-12,
                    "a_neg_base[{}][{}]: got={:.6e}, want={:.6e}",
                    i,
                    j,
                    preamp.a_neg_base[i][j],
                    a_neg_expected[i][j]
                );
            }
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Layer 3: DC Operating Point (additions to existing test)
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_l3_dc_independent_of_sample_rate() {
        let v_44k = DkPreamp::new(44100.0).v_dc;
        let v_88k = DkPreamp::new(88200.0).v_dc;
        let v_96k = DkPreamp::new(96000.0).v_dc;
        let v_192k = DkPreamp::new(192000.0).v_dc;

        for i in 0..N {
            assert!(
                (v_44k[i] - v_88k[i]).abs() < 1e-6,
                "DC v[{}] differs: 44.1k={:.6}, 88.2k={:.6}",
                i,
                v_44k[i],
                v_88k[i]
            );
            assert!(
                (v_44k[i] - v_96k[i]).abs() < 1e-6,
                "DC v[{}] differs: 44.1k={:.6}, 96k={:.6}",
                i,
                v_44k[i],
                v_96k[i]
            );
            assert!(
                (v_44k[i] - v_192k[i]).abs() < 1e-6,
                "DC v[{}] differs: 44.1k={:.6}, 192k={:.6}",
                i,
                v_44k[i],
                v_192k[i]
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Layer 4: Small-Signal Transfer Function
    // ══════════════════════════════════════════════════════════════════════════

    /// Get gm values from a DkPreamp's DC operating point.
    fn gm_from_preamp(preamp: &DkPreamp) -> (f64, f64) {
        (bjt_gm(preamp.main.v_nl[0]), bjt_gm(preamp.main.v_nl[1]))
    }

    #[test]
    fn test_l4_midband_gain_no_tremolo() {
        let preamp = DkPreamp::new(88200.0);
        let (gm1, gm2) = gm_from_preamp(&preamp);

        let gain = small_signal_gain_db(gm1, gm2, 1_000_000.0, 1000.0);
        assert!(
            gain > 3.0 && gain < 12.0,
            "SS gain @ 1kHz (no trem) = {gain:.1} dB, want ~6 dB"
        );
    }

    #[test]
    fn test_l4_midband_gain_tremolo_bright() {
        let preamp = DkPreamp::new(88200.0);
        let (gm1, gm2) = gm_from_preamp(&preamp);

        let gain = small_signal_gain_db(gm1, gm2, 19_000.0, 1000.0);
        assert!(
            gain > 8.0 && gain < 18.0,
            "SS gain @ 1kHz (trem bright) = {gain:.1} dB, want ~12 dB"
        );
    }

    #[test]
    fn test_l4_tremolo_gain_range() {
        let preamp = DkPreamp::new(88200.0);
        let (gm1, gm2) = gm_from_preamp(&preamp);

        let gain_lo = small_signal_gain_db(gm1, gm2, 1_000_000.0, 1000.0);
        let gain_hi = small_signal_gain_db(gm1, gm2, 19_000.0, 1000.0);
        let range = gain_hi - gain_lo;

        assert!(
            range > 3.0 && range < 10.0,
            "Tremolo range = {range:.1} dB, want ~6 dB"
        );
    }

    #[test]
    fn test_l4_bandwidth_not_collapsed() {
        // The decoupled model gives ~5.2 kHz at trem-bright due to missing
        // C-3/C-4 coupling. The DK model must do substantially better.
        let preamp = DkPreamp::new(88200.0);
        let (gm1, gm2) = gm_from_preamp(&preamp);

        let bw_no_trem = find_bandwidth(gm1, gm2, 1_000_000.0);
        assert!(
            bw_no_trem > 8_000.0,
            "BW (no trem) = {bw_no_trem:.0} Hz, want > 8 kHz"
        );

        let bw_trem = find_bandwidth(gm1, gm2, 19_000.0);
        assert!(
            bw_trem > 8_000.0,
            "BW (trem bright) = {bw_trem:.0} Hz, want > 8 kHz (decoupled gives ~5.2 kHz)"
        );
    }

    #[test]
    fn test_l4_bandwidth_independent_of_rldr() {
        let preamp = DkPreamp::new(88200.0);
        let (gm1, gm2) = gm_from_preamp(&preamp);

        let bw_1m = find_bandwidth(gm1, gm2, 1_000_000.0);
        let bw_19k = find_bandwidth(gm1, gm2, 19_000.0);

        let ratio = (bw_1m - bw_19k).abs() / bw_1m;
        assert!(
            ratio < 0.25,
            "BW should not depend on Rldr: 1M={bw_1m:.0} Hz, 19K={bw_19k:.0} Hz ({:.0}% diff)",
            ratio * 100.0
        );
    }

    #[test]
    fn test_l4_gbw_scales_with_gain() {
        let preamp = DkPreamp::new(88200.0);
        let (gm1, gm2) = gm_from_preamp(&preamp);

        let gain_1m = 10f64.powf(small_signal_gain_db(gm1, gm2, 1_000_000.0, 1000.0) / 20.0);
        let bw_1m = find_bandwidth(gm1, gm2, 1_000_000.0);
        let gbw_1m = gain_1m * bw_1m;

        let gain_19k = 10f64.powf(small_signal_gain_db(gm1, gm2, 19_000.0, 1000.0) / 20.0);
        let bw_19k = find_bandwidth(gm1, gm2, 19_000.0);
        let gbw_19k = gain_19k * bw_19k;

        assert!(
            gbw_19k > gbw_1m * 1.2,
            "GBW should scale with gain: 1M={gbw_1m:.0}, 19K={gbw_19k:.0}"
        );
    }

    #[test]
    fn test_l4_frequency_response_shape() {
        let preamp = DkPreamp::new(88200.0);
        let (gm1, gm2) = gm_from_preamp(&preamp);

        let gain_100 = small_signal_gain_db(gm1, gm2, 1_000_000.0, 100.0);
        let gain_1k = small_signal_gain_db(gm1, gm2, 1_000_000.0, 1000.0);
        let gain_10k = small_signal_gain_db(gm1, gm2, 1_000_000.0, 10000.0);

        // Midband relatively flat (100 Hz to 1 kHz within 3 dB)
        assert!(
            (gain_100 - gain_1k).abs() < 3.0,
            "100Hz={gain_100:.1} dB vs 1kHz={gain_1k:.1} dB, want < 3 dB diff"
        );

        // 10 kHz near midband (within 4 dB for ~15 kHz BW)
        assert!(
            (gain_10k - gain_1k).abs() < 4.0,
            "10kHz={gain_10k:.1} dB vs 1kHz={gain_1k:.1} dB, want < 4 dB diff"
        );
    }

    #[test]
    fn test_l4_independent_of_sample_rate() {
        // Transfer function is continuous-time — must not depend on fs
        let p1 = DkPreamp::new(44100.0);
        let p2 = DkPreamp::new(192000.0);
        let (gm1_a, gm2_a) = gm_from_preamp(&p1);
        let (gm1_b, gm2_b) = gm_from_preamp(&p2);

        for &freq in &[100.0, 1000.0, 5000.0, 10000.0] {
            let g1 = small_signal_gain_db(gm1_a, gm2_a, 1_000_000.0, freq);
            let g2 = small_signal_gain_db(gm1_b, gm2_b, 1_000_000.0, freq);
            assert!(
                (g1 - g2).abs() < 0.01,
                "SS gain at {} Hz depends on fs: 44.1k={g1:.3}, 192k={g2:.3}",
                freq
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Layer 5: Time-Domain Behavioral (existing tests below)
    // ══════════════════════════════════════════════════════════════════════════

    /// Step change in R_ldr should not disrupt the DC operating point.
    /// With explicit R_ldr (not in S matrix), Ce1 blocks DC and v[COLL2]
    /// stays near its quiescent value.
    #[test]
    fn test_diag_rldr_step_output_bounded() {
        // SPICE confirms: v[COLL2] shifts by ~5V on an R_ldr step from 1M to 50K.
        // This is correct physics — Ce1 holds v[FB] nearly constant, so changing
        // R_ldr swings the R10/R_ldr voltage divider at v[OUT] by several volts.
        // The test verifies that the OUTPUT SIGNAL (after DC coupling) stays bounded,
        // not that internal node voltages don't move.
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);
        preamp.set_ldr_resistance(1_000_000.0);
        preamp.reset();

        // Settle with no input, R_ldr=1M
        for _ in 0..((sr * 0.5) as usize) {
            preamp.process_sample(0.0);
        }

        // Step R_ldr from 1M to 50K
        preamp.set_ldr_resistance(50_000.0);

        // Run 1000 samples (11ms) and check output stays bounded
        let mut max_output = 0.0f64;
        for _ in 0..1000 {
            let out = preamp.process_sample(0.0);
            max_output = max_output.max(out.abs());
        }

        // SPICE confirms: stepping R_ldr from 1M to 50K causes v[OUT] to jump
        // ~4.3V (correct physics). Shadow subtraction cancels both the static
        // operating-point shift and the transient.
        eprintln!("Max output after R_ldr step: {max_output:.3}V");
        assert!(
            max_output < 10.0,
            "Output after R_ldr step unexpectedly large: {max_output:.3}V (want < 10.0V)"
        );

        // After settling (2s), output should be near zero (no input)
        for _ in 0..((sr * 2.0) as usize) {
            preamp.process_sample(0.0);
        }
        let settled_output = preamp.process_sample(0.0).abs();
        assert!(
            settled_output < 0.01,
            "Output should settle to ~0 with no input: {settled_output:.6}V"
        );
    }

    /// Dynamic tremolo modulation: verify audio-band gain matches static expectation.
    ///
    /// SPICE confirms the preamp's v[OUT] has a multi-volt 5.5 Hz operating-point
    /// swing during tremolo (correct physics: Ce1 holds FB constant while the
    /// R10/R_ldr divider swings the output DC). This test measures the 1 kHz
    /// audio signal amplitude using DFT, NOT broadband envelope, to isolate
    /// the actual gain modulation from the DC operating-point swing.
    #[test]
    fn test_diag_dynamic_rldr_modulation() {
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);

        // First: measure static gain at 1 kHz for two R_ldr values
        preamp.set_ldr_resistance(1_000_000.0);
        let gain_1m = measure_gain(&mut preamp, 1000.0, 0.001, sr);

        preamp.set_ldr_resistance(50_000.0);
        let gain_50k = measure_gain(&mut preamp, 1000.0, 0.001, sr);

        let static_range_db = 20.0 * (gain_50k / gain_1m).log10();
        eprintln!(
            "Static gain: R_ldr=1M -> {:.3}x, R_ldr=50K -> {:.3}x, range = {:.1} dB",
            gain_1m, gain_50k, static_range_db
        );

        // Dynamic R_ldr oscillation at 5.5 Hz while feeding 1 kHz sine.
        let mut preamp = DkPreamp::new(sr);
        preamp.set_ldr_resistance(1_000_000.0);
        preamp.reset();

        let freq = 1000.0;
        let amp = 0.001;
        let trem_rate = 5.5;

        // Settle for 10s with gradually increasing tremolo
        let n_settle = (sr * 10.0) as usize;
        for i in 0..n_settle {
            let t = i as f64 / sr;
            let ramp_time = 2.0 * sr;
            let depth = (i as f64 / ramp_time).min(1.0);
            let phase = 2.0 * PI * trem_rate * t;
            let log_base = 1_000_000.0f64.ln();
            let log_min = 50_000.0f64.ln();
            let log_r = log_base + depth * (log_min - log_base) * 0.5 * (1.0 - phase.cos());
            let r_ldr = log_r.exp();
            preamp.set_ldr_resistance(r_ldr);
            let input = amp * (2.0 * PI * freq * t).sin();
            preamp.process_sample(input);
        }

        // Oscillate R_ldr for 2 seconds and measure 1 kHz amplitude in
        // short windows aligned to tremolo quarter-cycles.
        // At trem_rate=5.5 Hz, period=16036 samples (at 88.2 kHz).
        // We measure in 10ms windows (882 samples = ~10 cycles of 1 kHz)
        // at the extremes of each tremolo cycle (R_ldr max and min).
        let trem_period = sr / trem_rate;
        let n_trem = (sr * 2.0) as usize;
        let mut outputs = Vec::with_capacity(n_trem);
        let mut rldr_values = Vec::with_capacity(n_trem);

        for i in 0..n_trem {
            let t = i as f64 / sr;
            let phase = 2.0 * PI * trem_rate * t;
            let log_min = 50_000.0f64.ln();
            let log_max = 1_000_000.0f64.ln();
            let log_mid = (log_min + log_max) / 2.0;
            let log_swing = (log_max - log_min) / 2.0;
            let r_ldr = (log_mid + log_swing * phase.sin()).exp();

            preamp.set_ldr_resistance(r_ldr);

            let t_abs = (n_settle + i) as f64 / sr;
            let input = amp * (2.0 * PI * freq * t_abs).sin();
            let out = preamp.process_sample(input);

            outputs.push(out);
            rldr_values.push(r_ldr);
        }

        // Measure 1 kHz amplitude via DFT in 10ms windows at tremolo extremes.
        // R_ldr is max (1M) at phase=pi/2 (quarter period) and min (50K) at
        // phase=3*pi/2 (three-quarter period).
        let window_ms = 10.0;
        let window_len = (sr * window_ms / 1000.0) as usize;
        let period_samples = trem_period as usize;
        let n_cycles = n_trem / period_samples;

        let mut gains_at_max_rldr = Vec::new();
        let mut gains_at_min_rldr = Vec::new();

        for cycle in 1..n_cycles {
            // skip first cycle
            // R_ldr max at quarter period
            let center_max = cycle * period_samples + period_samples / 4;
            let start = center_max.saturating_sub(window_len / 2);
            if start + window_len > outputs.len() {
                break;
            }
            let amp_at_max = dft_amplitude(&outputs[start..start + window_len], freq, sr);
            gains_at_max_rldr.push(amp_at_max / amp);

            // R_ldr min at three-quarter period
            let center_min = cycle * period_samples + 3 * period_samples / 4;
            let start = center_min.saturating_sub(window_len / 2);
            if start + window_len > outputs.len() {
                break;
            }
            let amp_at_min = dft_amplitude(&outputs[start..start + window_len], freq, sr);
            gains_at_min_rldr.push(amp_at_min / amp);
        }

        // Average gains across cycles
        let avg_gain_max_rldr =
            gains_at_max_rldr.iter().sum::<f64>() / gains_at_max_rldr.len() as f64;
        let avg_gain_min_rldr =
            gains_at_min_rldr.iter().sum::<f64>() / gains_at_min_rldr.len() as f64;

        let dynamic_range_db = 20.0 * (avg_gain_min_rldr / avg_gain_max_rldr).log10();

        eprintln!(
            "Dynamic 1kHz gain: R_ldr=1M -> {:.3}x, R_ldr=50K -> {:.3}x, range = {:.1} dB",
            avg_gain_max_rldr, avg_gain_min_rldr, dynamic_range_db
        );
        eprintln!(
            "  Static:          R_ldr=1M -> {:.3}x, R_ldr=50K -> {:.3}x, range = {:.1} dB",
            gain_1m, gain_50k, static_range_db
        );
        eprintln!("  ({} cycles measured)", gains_at_max_rldr.len());

        // Dynamic gain modulation should be close to static range (2.8 dB).
        // Allow generous margin for Ce1 time-constant effects reducing the
        // effective modulation depth (Ce1 tau ~2.4s vs tremolo period 0.18s).
        assert!(
            dynamic_range_db > 0.5,
            "Dynamic gain modulation too small: {dynamic_range_db:.1} dB (expected > 0.5 dB)"
        );
        assert!(
            dynamic_range_db < static_range_db + 6.0,
            "Dynamic modulation ({dynamic_range_db:.1} dB) far exceeds static range ({static_range_db:.1} dB)"
        );
    }

    /// DFT amplitude at a specific frequency with Hann windowing.
    /// Hann window suppresses spectral leakage from the multi-volt 5.5 Hz
    /// operating-point swing that would otherwise contaminate the 1 kHz bin.
    fn dft_amplitude(samples: &[f64], freq: f64, sr: f64) -> f64 {
        let n = samples.len();
        let nf = n as f64;
        let mut re = 0.0;
        let mut im = 0.0;
        let mut window_sum = 0.0;
        for (i, &s) in samples.iter().enumerate() {
            // Hann window: 0.5 * (1 - cos(2*pi*i/N))
            let w = 0.5 * (1.0 - (2.0 * PI * i as f64 / nf).cos());
            window_sum += w;
            let phase = 2.0 * PI * freq * i as f64 / sr;
            re += s * w * phase.cos();
            im += s * w * phase.sin();
        }
        2.0 * (re * re + im * im).sqrt() / window_sum
    }

    /// Verify that stepped R_ldr converges to fresh-start equilibrium.
    /// After stepping from 1M to 50K and settling for 5s, the circuit
    /// should converge to the same state as a fresh start at 50K.
    #[test]
    fn test_step_convergence() {
        let sr = 88200.0;

        // Fresh start at R_ldr=50K
        let mut fresh = DkPreamp::new(sr);
        fresh.set_ldr_resistance(50_000.0);
        fresh.reset();
        for _ in 0..((sr * 2.0) as usize) {
            fresh.process_sample(0.0);
        }

        // Step from 1M to 50K and settle
        let mut stepped = DkPreamp::new(sr);
        stepped.set_ldr_resistance(1_000_000.0);
        stepped.reset();
        for _ in 0..((sr * 2.0) as usize) {
            stepped.process_sample(0.0);
        }
        stepped.set_ldr_resistance(50_000.0);
        for _ in 0..((sr * 5.0) as usize) {
            stepped.process_sample(0.0);
        }

        // After 5s settling (2 tau), COLL2 should be converging toward fresh
        let delta_coll2 = (stepped.main.v[COLL2] - fresh.main.v[COLL2]).abs();
        assert!(
            delta_coll2 < 1.0,
            "After 5s settling, COLL2 delta={delta_coll2:.3}V (want < 1V)"
        );
    }

    /// Verify Sherman-Morrison S_eff matches brute-force matrix inverse.
    #[test]
    fn test_sm_vs_brute_force() {
        let sr = 88200.0;
        let preamp = DkPreamp::new(sr);

        let two_over_t = 2.0 * sr;
        let alpha = 2.0 * R1 * CIN * sr;
        let g_cin_ck = (2.0 * CIN * sr) / (1.0 + alpha);

        let r_ldr_test = 100_000.0;

        // Brute-force: build full S with R_ldr
        let mut g_full = build_g_dc();
        g_full[BASE1][BASE1] += g_cin_ck;
        g_full[FB][FB] += 1.0 / r_ldr_test;
        let c_mat = build_c_matrix();
        let two_c_t = mat_scale(two_over_t, &c_mat);
        let a_full = mat_add(&two_c_t, &g_full);
        let s_full = mat_inverse(&a_full);

        // SM: S_eff = S_base - sm_k * s_fb_col * s_fb_row^T
        let g_ldr = 1.0 / r_ldr_test;
        let sm_k = g_ldr / (1.0 + preamp.s_fb_fb * g_ldr);
        let mut s_sm = preamp.s_base;
        for i in 0..N {
            for j in 0..N {
                s_sm[i][j] -= sm_k * preamp.s_fb_col[i] * preamp.s_fb_row[j];
            }
        }

        let mut max_err = 0.0f64;
        for i in 0..N {
            for j in 0..N {
                max_err = max_err.max((s_sm[i][j] - s_full[i][j]).abs());
            }
        }

        assert!(
            max_err < 1e-7,
            "SM vs brute-force S_eff max error: {max_err:.2e} (want < 1e-7)"
        );
    }

    /// Verify shadow subtraction eliminates idle pump from tremolo R_ldr modulation.
    /// With zero input, main and shadow produce identical pump; subtraction cancels exactly.
    #[test]
    fn test_idle_pump_level() {
        use crate::tremolo::Tremolo;

        let os_sr = 88200.0;
        let mut preamp = DkPreamp::new(os_sr);
        let mut tremolo = Tremolo::new(5.63, 1.0, os_sr);
        tremolo.set_depth(1.0);

        // Run 2 seconds of zero input with cycling R_ldr
        let n = (os_sr * 2.0) as usize;
        let settle = (os_sr * 0.5) as usize; // 500ms settle time
        let mut peak_preamp = 0.0f64;
        let mut samples_preamp = Vec::new();

        for i in 0..n {
            let r_ldr = tremolo.process();
            preamp.set_ldr_resistance(r_ldr);
            let out = preamp.process_sample(0.0);
            if i >= settle {
                peak_preamp = peak_preamp.max(out.abs());
                if i % 10 == 0 {
                    samples_preamp.push(out);
                }
            }
        }

        let peak_db = if peak_preamp > 0.0 {
            20.0 * peak_preamp.log10()
        } else {
            -200.0
        };

        // Also measure at specific harmonic frequencies of 5.63 Hz
        let analysis_sr = os_sr / 10.0; // downsampled by 10x for the collection
        for harmonic in 1..=10 {
            let freq = 5.63 * harmonic as f64;
            let mag = dft_magnitude(&samples_preamp, freq, analysis_sr);
            let mag_db = if mag > 0.0 {
                20.0 * mag.log10()
            } else {
                -200.0
            };
            eprintln!(
                "  Idle pump harmonic {}: {:.1} Hz = {:.1} dB (amplitude {:.2e})",
                harmonic, freq, mag_db, mag
            );
        }

        eprintln!(
            "DK preamp idle pump: peak = {:.2e} ({:.1} dB)",
            peak_preamp, peak_db
        );

        // Shadow subtraction cancels pump exactly (both instances see same R_ldr,
        // produce identical pump, difference is zero). Residual is floating-point
        // noise only.
        assert!(
            peak_db < -100.0,
            "Shadow pump cancellation residual too large: {peak_db:.1} dB (want < -100 dB)"
        );
    }
}
