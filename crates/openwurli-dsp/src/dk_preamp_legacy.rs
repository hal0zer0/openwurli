//! DK (Discretization-Kernel) preamp — full coupled 2-stage BJT circuit solver.
//!
//! Solves the complete Wurlitzer 200A preamp as a 9-node MNA system with
//! trapezoidal discretization and Newton-Raphson on the 2×2 nonlinear kernel.
//!
//! ## Drawn-topology revision (2026-09-13)
//!
//! This solver was rebuilt against the pixel-instrumented re-read of schematic
//! #203720-S-3, cross-confirmed by an independent SPICE deck and a hand
//! derivation. The netlist spec is `spice/melange/wurli-preamp.cir`; the
//! acceptance targets are in `spice/testbench/REVISION-NOTES.md`. Changes vs
//! the pre-revision solver:
//!
//! | Element | Was | Drawn |
//! |---|---|---|
//! | R-2 1MEG | 2MEG base1→Vcc (core bias) | polarizing feed on the **pickup** side of Cin — folded into the input companion, NOT a core node |
//! | R-3 470K | base1→GND | **base1→emit2b** — DC feedback from TR-2's emitter divider, and TR-1's *only* bias path |
//! | C-7 22µF | across R-7 (270) only | **emit2→GND**, spanning R-7+R-8: stage 2 is a high-gain CE stage |
//! | C-6 4.7µF | absent | **NEW**, coll2→node_c6 series coupling |
//! | R-9 / R-10 | coll2→out / out→fb | **node_c6→out / node_c6→fb** |
//! | C-2 220pF | absent | **RESTORED** at base1→GND (real 200A part) |
//! | Vcc | 15.0 V | **14.5 V** (service-manual text; the drawing marks +15V) |
//!
//! Consequence for voicing: stage 1's voltage gain collapses (TR-2's r_π loads
//! the 150K collector to ~3K) while stage 2 becomes the gain stage. The
//! pre-revision "stage 1 asymmetric clipping is the H2 source" story does not
//! survive this topology — see the KNOWN TRAP note on [`VBE_MAX`] below.
//!
//! Input coupling: the drawn input network is a two-port — R1 (22K) in series
//! from the source to the R-1/C-1 junction, R2 (1MEG) shunting that junction to
//! AC ground (the +150V polarizing line is an AC ground), then Cin (0.022µF) to
//! base1. Seen from base1 this is exactly a Thévenin source `input · R2/(R1+R2)`
//! behind `Cin + (R1‖R2)`, so the existing bilinear companion is reused with
//! `R_IN_EFF = R1‖R2` and the input pre-scaled by `K_IN_DIV`. This is an exact
//! equivalent of the two-port, not an approximation.
//!
//! See docs/research/dk-preamp-derivation.md for the full mathematical derivation.
#![allow(clippy::needless_range_loop)]

use crate::preamp::PreampModel;

// ── Circuit constants ───────────────────────────────────────────────────────

/// Supply rail. Service manual TEXT specifies "+14.5 volts regulated" (p.64);
/// the schematic MARKS "+15V". 14.5 V is what closes the TR-2 collector KCL
/// imbalance implied by the drawing's own DC annotations (+10.4% → +1.6%).
const VCC: f64 = 14.5;

// Resistors (ohms)
const R1: f64 = 22_000.0; // Source → R-1/C-1 junction (input two-port)
const R2: f64 = 1_000_000.0; // Polarizing feed, R-1/C-1 junction → AC ground
const R3: f64 = 470_000.0; // base1 → emit2b (DC feedback; TR-1's only bias path)
const RE1: f64 = 33_000.0; // emit1 to GND
const RC1: f64 = 150_000.0; // coll1 to Vcc
const RE2A: f64 = 270.0; // emit2 to emit2b
const RE2B: f64 = 820.0; // emit2b to GND
const RC2: f64 = 1_800.0; // coll2 to Vcc
const R9: f64 = 6_800.0; // node_c6 to out (OUTSIDE the feedback loop)
const R10: f64 = 56_000.0; // node_c6 to fb (feedback tapped at the collector)

/// Output load at OUT.
///
/// Not structurally required (R-9 alone gives `out` a nonzero diagonal); it is
/// here so the Rust and SPICE gain figures are like-for-like — 100K is the load
/// the re-baselined `tb_preamp_ac` bench measures into. The real load is the
/// R-11 25K trimmer into the 10K volume pot (10–35 kΩ, ≈−1.7 dB vs 100K at
/// mid-trim); it is flat, so level calibration absorbs the difference. The
/// volume path is deliberately decoupled from drive — see the 2026-04-26
/// drive/volume decoupling.
const RLOAD: f64 = 100_000.0;

// Input two-port reduction (see module docs): Thévenin as seen from base1.
const R_IN_EFF: f64 = R1 * R2 / (R1 + R2); // 21.526 kΩ
const K_IN_DIV: f64 = R2 / (R1 + R2); // 0.978474

// Capacitors (farads)
const CIN: f64 = 0.022e-6; // Input coupling cap (in series with R_IN_EFF)
const C2: f64 = 220.0e-12; // C-2 at base1 → GND (restored in the revision)
const C3: f64 = 100.0e-12; // Miller, Stage 1 (coll1 ↔ base1)
const C4: f64 = 100.0e-12; // Miller, Stage 2 (coll2 ↔ coll1)
const CE1: f64 = 4.7e-6; // Feedback coupling (emit1 ↔ fb)
const CE2: f64 = 22.0e-6; // Stage 2 emitter bypass (emit2 → GND, spans R-7+R-8)
const C6: f64 = 4.7e-6; // Output coupling (coll2 ↔ node_c6)

// ── BJT model (2N5089 card, forward-active) ─────────────────────────────────
//
// The pre-revision kernel was a bare transconductance: Ic = Is·(exp(Vbe/Vt)−1)
// with NO base current, i.e. β = ∞. That is structurally unable to reproduce
// the drawn topology's DC point: with no base current there is no drop across
// R-3, so base1 ≡ emit2b, and the arbiter table puts them 59 mV apart.
//
// Two terms were added, both still explicit functions of Vbe alone (no new
// kernel dimension, no implicit inner solve):
//   • base current — ideal (Ic/BF) plus the Gummel-Poon low-current
//     recombination term ISE·(exp(Vbe/(NE·Vt))−1), which dominates β at TR-1's
//     58 µA operating point;
//   • high injection — the GP normalised base charge qb from IKF, which is
//     what drops TR-2's effective β from ~850 to ~700 at its 3.3 mA.
//
// Progression of worst-node DC error vs the arbiter table: β=∞ 137 mV →
// +base current 27 mV → +high injection 2.70 mV. The remaining ~2.7 mV would
// need the full GP card (VAF/VAR via q1, RE/RB/RC parasitics); that is the
// melange-generated solver's job (`--features melange-preamp`), not this one.
const IS: f64 = 3.03e-14; // Saturation current
// Thermal voltage. ngspice/melange use kT/q at 27 °C = 0.025852 and 25 °C
// would be 0.02569; this card (IS/BF/NF) was fitted against the deck WITH
// 0.026, and moving VT alone shifts TR-1's collector 6 mV off the
// tb_preamp_dc anchor (±5 mV test). Change them together or not at all.
const VT: f64 = 0.026;
const NF: f64 = 1.005; // Forward emission coefficient
const BF: f64 = 1434.0; // Ideal forward beta
const ISE: f64 = 2.88e-15; // B-E leakage saturation current
const NE: f64 = 1.262; // B-E leakage emission coefficient
const IKF: f64 = 0.01358; // Forward knee current (high injection)

const VTF: f64 = NF * VT; // Forward ideality-scaled thermal voltage
const NE_VT: f64 = NE * VT;
const IS_OVER_IKF: f64 = IS / IKF;

/// Max Vbe clamp — prevents exp overflow while allowing full operating range.
/// Real 2N5089 Vbe never exceeds ~0.8V; 0.85V gives ample margin.
///
/// KNOWN TRAP (§6.3 caveat): this file carries no `satLimit`/`cutoffLimit`
/// Stage-1 asymmetry constants — the collector rail limits are left to emerge
/// from the topology and the NR solve, which is why none appear here. Under the
/// drawn topology stage 1's gain is ~8 (not ~420), so TR-1's 5.3:1 headroom
/// asymmetry is unreachable: TR-2 clips first and is near-symmetric (1.21:1).
/// That conclusion follows structurally from the confirmed A1/A2 inversion but
/// has NOT been directly probed. Do not remove clipping behaviour on the
/// strength of it without a measurement.
const VBE_MAX: f64 = 0.85;

// Node indices
const BASE1: usize = 0;
const EMIT1: usize = 1;
const COLL1: usize = 2;
const EMIT2: usize = 3;
const EMIT2B: usize = 4;
const COLL2: usize = 5;
const NODE_C6: usize = 6;
const OUT: usize = 7;
const FB: usize = 8;

const N: usize = 9; // number of nodes

// ── Nonlinear incidence maps ────────────────────────────────────────────────
//
// N_v rows extract the controlling voltages: vbe1 = v[BASE1] − v[EMIT1],
// vbe2 = v[COLL1] − v[EMIT2] (TR-2's base IS TR-1's collector — direct coupled).
const NV: [[(usize, f64); 2]; 2] = [[(BASE1, 1.0), (EMIT1, -1.0)], [(COLL1, 1.0), (EMIT2, -1.0)]];

/// Collector-current injection: Ic flows from the collector node into the
/// device and out of the emitter node.
const NIC: [[(usize, f64); 2]; 2] = [[(EMIT1, 1.0), (COLL1, -1.0)], [(EMIT2, 1.0), (COLL2, -1.0)]];

/// Base-current injection: Ib flows from the base node into the device and out
/// of the emitter node. TR-2's base node is COLL1.
const NIB: [[(usize, f64); 2]; 2] = [[(EMIT1, 1.0), (BASE1, -1.0)], [(EMIT2, 1.0), (COLL1, -1.0)]];

// ── N×N matrix type aliases ─────────────────────────────────────────────────

type MatN = [[f64; N]; N];
type VecN = [f64; N];

fn mat_zero() -> MatN {
    [[0.0; N]; N]
}
fn vec_zero() -> VecN {
    [0.0; N]
}

/// Matrix-vector multiply: y = A * x
fn mat_vec_mul(a: &MatN, x: &VecN) -> VecN {
    let mut y = vec_zero();
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
fn mat_add(a: &MatN, b: &MatN) -> MatN {
    let mut c = mat_zero();
    for i in 0..N {
        for j in 0..N {
            c[i][j] = a[i][j] + b[i][j];
        }
    }
    c
}

/// Matrix subtract: C = A - B
fn mat_sub(a: &MatN, b: &MatN) -> MatN {
    let mut c = mat_zero();
    for i in 0..N {
        for j in 0..N {
            c[i][j] = a[i][j] - b[i][j];
        }
    }
    c
}

/// Scale matrix: B = scalar * A
fn mat_scale(scalar: f64, a: &MatN) -> MatN {
    let mut b = mat_zero();
    for i in 0..N {
        for j in 0..N {
            b[i][j] = scalar * a[i][j];
        }
    }
    b
}

/// Gauss-Jordan inverse of an N×N matrix. Panics if singular.
fn mat_inverse(m: &MatN) -> MatN {
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

    let mut inv = mat_zero();
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
    s_base: MatN,       // inv(A_base) where A_base = 2C/T + G_base (no R_ldr)
    a_neg_base: MatN,   // 2C/T - G_base (no R_ldr)
    k_c: [[f64; 2]; 2], // DK kernel for collector currents (R_ldr-independent)
    k_b: [[f64; 2]; 2], // DK kernel for base currents (R_ldr-independent)
    two_w: VecN,        // 2 * w

    // ── Precomputed S·N columns (node-update projections) ──
    s_nic: [VecN; 2], // S * NIC[:,j]
    s_nib: [VecN; 2], // S * NIB[:,j]

    // ── SM projection vectors for R_ldr ──
    s_fb_col: VecN, // S_base[:,FB] — column FB of S_base
    #[cfg_attr(not(test), allow(dead_code))]
    s_fb_row: VecN, // S_base[FB,:] — row FB of S_base (used in tests)
    s_fb_fb: f64,   // S_base[FB][FB] — SM denominator scalar
    nv_sfb: [f64; 2], // N_v * s_fb_col: NL voltage extraction at FB col
    sfb_nic: [f64; 2], // s_fb_row * N_ic: collector-current injection at FB row
    sfb_nib: [f64; 2], // s_fb_row * N_ib: base-current injection at FB row

    // ── DC operating point ──
    v_dc: VecN,      // DC node voltages at current R_ldr
    g_dc_base: MatN, // G_dc without R_ldr or g_cin (for DC solve)

    // ── Cin-R1/R2 companion (shared constants) ──
    g_cin: f64,
    c_cin: f64,
    gc_1pc: f64,

    // ── Per-instance mutable state ──
    //
    // RETIRED 2026-09-14: the shadow state. It ran a second full solve with
    // zero input each sample, purely to produce the tremolo bias pump so it
    // could be subtracted from the main output.
    //
    // That pump was an artifact of the pre-revision topology, where the output
    // was DC-coupled through R-10 into the LDR leg. C-6 eliminates it at
    // source: node_c6, out and fb all sit at 0 V DC, so modulating R_ldr moves
    // no bias (`tb_pump_emit`, and `test_acceptance_pump_guard_c6` here).
    //
    // Retired on a null test rather than on the argument above: renders built
    // with and without the subtraction, across five notes at static shunt plus
    // three tremolo-sweeping cases, differ by **exactly one 24-bit LSB
    // (-138.47 dBFS)** — i.e. the float-domain difference is below half an LSB
    // and the outputs are bit-identical after quantization. The acceptance bar
    // was -80 dBFS. It was subtracting nothing, for ~50% of the solver's CPU.
    main: DkState,

    // ── Shared R_ldr tracking ──
    r_ldr: f64,
    g_ldr: f64,      // 1/r_ldr (current conductance)
    g_ldr_prev: f64, // g_ldr from previous timestep
}

/// Per-instance mutable state for the DK solver.
/// The solver's fixed matrices and R_ldr live here, separate from mutable state.
#[derive(Clone)]
struct DkState {
    j_cin: f64,
    cin_rhs_prev: f64,
    v: VecN,        // Absolute node voltages
    i_c: [f64; 2],  // Absolute collector currents
    i_b: [f64; 2],  // Absolute base currents
    v_nl: [f64; 2], // Full Vbe (for NR warm start)
}

impl DkState {
    /// Initialize state at a DC operating point.
    fn at_dc(g_cin: f64, v_nl_dc: [f64; 2], v_dc: VecN) -> Self {
        let d0 = bjt(v_nl_dc[0]);
        let d1 = bjt(v_nl_dc[1]);
        Self {
            j_cin: g_cin * v_dc[BASE1],
            cin_rhs_prev: g_cin * v_dc[BASE1],
            v: v_dc,
            i_c: [d0.0, d1.0],
            i_b: [d0.1, d1.1],
            v_nl: v_nl_dc,
        }
    }
}

impl DkPreamp {
    /// No-op in the legacy preamp — authentic circuit noise is a
    /// melange-only feature. The hand-written MNA solver has no noise
    /// model. Method mirrored here so the plugin compiles regardless of
    /// which preamp is selected (default = this; `--features melange-preamp`
    /// = melange solver).
    pub fn set_noise_enabled(&mut self, _on: bool) {}

    /// No-op in the legacy preamp — see `set_noise_enabled`.
    pub fn set_thermal_gain(&mut self, _gain: f64) {}

    pub fn new(sample_rate: f64) -> Self {
        let t = 1.0 / sample_rate;
        let two_over_t = 2.0 / t;

        // ── Cin companion model parameters ──
        // Thévenin of the R1/R2/Cin input two-port seen from base1: the series
        // resistance is R1‖R2 and the drive is pre-scaled by K_IN_DIV.
        let alpha_cin = 2.0 * R_IN_EFF * CIN * sample_rate;
        let g_cin = (2.0 * CIN * sample_rate) / (1.0 + alpha_cin);
        let c_cin = (1.0 - alpha_cin) / (1.0 + alpha_cin);
        let gc_1pc = g_cin * (1.0 + c_cin);

        // ── Stamp G_base matrix (without R_ldr, WITH g_cin) ──
        //
        // NOTE: base1 has NO resistive path to a supply here. R-2 moved to the
        // pickup side of Cin (input companion), so R-3 to emit2b is TR-1's
        // ONLY DC bias path — exactly as drawn.
        let mut g_base = mat_zero();
        let mut w = vec_zero();

        stamp_resistor(&mut g_base, BASE1, EMIT2B, R3);
        g_base[EMIT1][EMIT1] += 1.0 / RE1;
        g_base[COLL1][COLL1] += 1.0 / RC1;
        w[COLL1] += VCC / RC1;
        stamp_resistor(&mut g_base, EMIT2, EMIT2B, RE2A);
        g_base[EMIT2B][EMIT2B] += 1.0 / RE2B;
        g_base[COLL2][COLL2] += 1.0 / RC2;
        w[COLL2] += VCC / RC2;
        stamp_resistor(&mut g_base, NODE_C6, OUT, R9);
        stamp_resistor(&mut g_base, NODE_C6, FB, R10);
        g_base[OUT][OUT] += 1.0 / RLOAD;

        // G_dc_base: no R_ldr, no g_cin (for DC solves)
        let g_dc_base = g_base;

        // Add g_cin to g_base (for transient matrices)
        g_base[BASE1][BASE1] += g_cin;

        // ── Stamp C matrix ──
        let mut c = mat_zero();
        stamp_capacitor(&mut c, COLL1, BASE1, C3);
        stamp_capacitor(&mut c, COLL2, COLL1, C4);
        stamp_capacitor(&mut c, EMIT1, FB, CE1);
        stamp_capacitor(&mut c, COLL2, NODE_C6, C6);
        stamp_capacitor_to_gnd(&mut c, EMIT2, CE2);
        stamp_capacitor_to_gnd(&mut c, BASE1, C2);
        let two_c_over_t = mat_scale(two_over_t, &c);

        let two_w: VecN = core::array::from_fn(|i| 2.0 * w[i]);

        // ── Build FIXED transient matrices (no R_ldr) ──
        let a_base = mat_add(&two_c_over_t, &g_base);
        let a_neg_base = mat_sub(&two_c_over_t, &g_base);
        let s_base = mat_inverse(&a_base);
        let k_c = compute_k(&s_base, &NIC);
        let k_b = compute_k(&s_base, &NIB);

        // Extract SM projection vectors
        let s_fb_col: VecN = core::array::from_fn(|i| s_base[i][FB]);
        let s_fb_row: VecN = core::array::from_fn(|i| s_base[FB][i]);
        let s_fb_fb = s_base[FB][FB];

        // Pre-compute NL extraction/injection vectors for K correction
        let nv_sfb =
            core::array::from_fn(|i| NV[i].iter().map(|&(r, a)| a * s_fb_col[r]).sum::<f64>());
        let sfb_nic =
            core::array::from_fn(|j| NIC[j].iter().map(|&(cn, b)| b * s_fb_row[cn]).sum::<f64>());
        let sfb_nib =
            core::array::from_fn(|j| NIB[j].iter().map(|&(cn, b)| b * s_fb_row[cn]).sum::<f64>());

        // Pre-compute S·N columns for the node update
        let s_nic: [VecN; 2] = core::array::from_fn(|j| s_times_ni(&s_base, &NIC[j]));
        let s_nib: [VecN; 2] = core::array::from_fn(|j| s_times_ni(&s_base, &NIB[j]));

        // ── DC solve at initial R_ldr ──
        let r_ldr_init = 1_000_000.0;
        let (v_nl_dc, v_dc) = Self::full_dc_solve(&g_dc_base, &w, r_ldr_init);

        // Solver state starts at the DC operating point
        let init_state = DkState::at_dc(g_cin, v_nl_dc, v_dc);

        Self {
            s_base,
            a_neg_base,
            k_c,
            k_b,
            two_w,

            s_nic,
            s_nib,

            s_fb_col,
            s_fb_row,
            s_fb_fb,
            nv_sfb,
            sfb_nic,
            sfb_nib,

            v_dc,
            g_dc_base,

            g_cin,
            c_cin,
            gc_1pc,

            main: init_state,

            r_ldr: r_ldr_init,
            g_ldr: 1.0 / r_ldr_init,
            g_ldr_prev: 1.0 / r_ldr_init,
        }
    }

    /// Full DC solve: find quiescent operating point at a given R_ldr.
    /// Returns (v_nl_dc, v_dc).
    fn full_dc_solve(g_dc_base: &MatN, w: &VecN, r_ldr: f64) -> ([f64; 2], VecN) {
        let mut g_full = *g_dc_base;
        g_full[FB][FB] += 1.0 / r_ldr;
        let s_dc = mat_inverse(&g_full);
        let k_c_dc = compute_k(&s_dc, &NIC);
        let k_b_dc = compute_k(&s_dc, &NIB);
        let sv = mat_vec_mul(&s_dc, w);
        let p_dc = [sv[BASE1] - sv[EMIT1], sv[COLL1] - sv[EMIT2]];

        let mut v_nl = [0.56, 0.66];
        for _iter in 0..200 {
            let d0 = bjt(v_nl[0]);
            let d1 = bjt(v_nl[1]);
            let (ic0, ib0, gc0, gb0) = d0;
            let (ic1, ib1, gc1, gb1) = d1;
            let f = [
                v_nl[0]
                    - p_dc[0]
                    - k_c_dc[0][0] * ic0
                    - k_c_dc[0][1] * ic1
                    - k_b_dc[0][0] * ib0
                    - k_b_dc[0][1] * ib1,
                v_nl[1]
                    - p_dc[1]
                    - k_c_dc[1][0] * ic0
                    - k_c_dc[1][1] * ic1
                    - k_b_dc[1][0] * ib0
                    - k_b_dc[1][1] * ib1,
            ];
            if f[0].abs() < 1e-13 && f[1].abs() < 1e-13 {
                break;
            }

            let j00 = 1.0 - k_c_dc[0][0] * gc0 - k_b_dc[0][0] * gb0;
            let j01 = -k_c_dc[0][1] * gc1 - k_b_dc[0][1] * gb1;
            let j10 = -k_c_dc[1][0] * gc0 - k_b_dc[1][0] * gb0;
            let j11 = 1.0 - k_c_dc[1][1] * gc1 - k_b_dc[1][1] * gb1;
            let det = j00 * j11 - j01 * j10;
            let inv_det = 1.0 / det;
            let dv0 = inv_det * (j11 * f[0] - j01 * f[1]);
            let dv1 = inv_det * (j00 * f[1] - j10 * f[0]);
            let max_step = 2.0 * VT;
            v_nl[0] -= dv0.clamp(-max_step, max_step);
            v_nl[1] -= dv1.clamp(-max_step, max_step);
        }

        let d0 = bjt(v_nl[0]);
        let d1 = bjt(v_nl[1]);
        let ic = [d0.0, d1.0];
        let ib = [d0.1, d1.1];
        let mut dc_rhs = *w;
        for j in 0..2 {
            for &(node, coeff) in &NIC[j] {
                dc_rhs[node] += coeff * ic[j];
            }
            for &(node, coeff) in &NIB[j] {
                dc_rhs[node] += coeff * ib[j];
            }
        }
        let v_dc = mat_vec_mul(&s_dc, &dc_rhs);

        (v_nl, v_dc)
    }

    /// Get w = two_w / 2 (the original DC source vector).
    fn two_w_half(&self) -> VecN {
        core::array::from_fn(|i| self.two_w[i] * 0.5)
    }
}

/// Compute S * N_i for one nonlinear current column.
fn s_times_ni(s: &MatN, ni: &[(usize, f64); 2]) -> VecN {
    core::array::from_fn(|i| ni.iter().map(|&(c, b)| b * s[i][c]).sum())
}

/// Compute K = N_v * S * N_i from an N×N matrix S and an incidence map.
fn compute_k(s: &MatN, ni: &[[(usize, f64); 2]; 2]) -> [[f64; 2]; 2] {
    let mut k = [[0.0; 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            let mut acc = 0.0;
            for &(r, a) in &NV[i] {
                for &(c, b) in &ni[j] {
                    acc += a * b * s[r][c];
                }
            }
            k[i][j] = acc;
        }
    }
    k
}

/// Core DK trapezoidal step — free function for borrow-checker compatibility.
///
/// Called with the immutable config and the instance's own mutable state.
/// different mutable state. Making this a free function (not a method) allows
/// Rust's borrow checker to split borrows at the field level: config fields
/// borrowed immutably, state field borrowed mutably, in the same call.
///
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn dk_step(
    a_neg_base: &MatN,
    two_w: &VecN,
    s_base: &MatN,
    s_nic: &[VecN; 2],
    s_nib: &[VecN; 2],
    s_fb_col: &VecN,
    s_fb_fb: f64,
    g_ldr: f64,
    g_ldr_prev: f64,
    k_c: &[[f64; 2]; 2],
    k_b: &[[f64; 2]; 2],
    nv_sfb: &[f64; 2],
    sfb_nic: &[f64; 2],
    sfb_nib: &[f64; 2],
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

    // Cin companion. The drive is the Thévenin source of the R1/R2/Cin
    // two-port, i.e. the raw input scaled by the R2/(R1+R2) divider.
    let vin_eff = input * K_IN_DIV;
    let cin_rhs_now = g_cin * vin_eff + state.j_cin;
    rhs[BASE1] += cin_rhs_now + state.cin_rhs_prev;

    // Previous NL currents (collector and base)
    for j in 0..2 {
        for &(node, coeff) in &NIC[j] {
            rhs[node] += coeff * state.i_c[j];
        }
        for &(node, coeff) in &NIB[j] {
            rhs[node] += coeff * state.i_b[j];
        }
    }

    // DC sources (2w)
    for i in 0..N {
        rhs[i] += two_w[i];
    }

    // 2. v_pred_base = S_base * rhs (without R_ldr on LHS)
    let v_pred_base = mat_vec_mul(s_base, &rhs);

    // 3. SM correction for current R_ldr
    let sm_k = g_ldr / (1.0 + s_fb_fb * g_ldr);
    let sm_vpred = sm_k * v_pred_base[FB];
    let mut v_pred = vec_zero();
    for i in 0..N {
        v_pred[i] = v_pred_base[i] - sm_vpred * s_fb_col[i];
    }

    // 4. Predicted NL voltages
    let p = [v_pred[BASE1] - v_pred[EMIT1], v_pred[COLL1] - v_pred[EMIT2]];

    // 5. NR solve on 2x2 system with R_ldr-corrected kernels
    let mut kc = [[0.0f64; 2]; 2];
    let mut kb = [[0.0f64; 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            kc[i][j] = k_c[i][j] - sm_k * nv_sfb[i] * sfb_nic[j];
            kb[i][j] = k_b[i][j] - sm_k * nv_sfb[i] * sfb_nib[j];
        }
    }

    let mut v_nl = state.v_nl;

    for _iter in 0..6 {
        let (ic0, ib0, gc0, gb0) = bjt(v_nl[0]);
        let (ic1, ib1, gc1, gb1) = bjt(v_nl[1]);

        let f0 = v_nl[0] - p[0] - kc[0][0] * ic0 - kc[0][1] * ic1 - kb[0][0] * ib0 - kb[0][1] * ib1;
        let f1 = v_nl[1] - p[1] - kc[1][0] * ic0 - kc[1][1] * ic1 - kb[1][0] * ib0 - kb[1][1] * ib1;

        if f0.abs() < 1e-9 && f1.abs() < 1e-9 {
            break;
        }

        let j00 = 1.0 - kc[0][0] * gc0 - kb[0][0] * gb0;
        let j01 = -kc[0][1] * gc1 - kb[0][1] * gb1;
        let j10 = -kc[1][0] * gc0 - kb[1][0] * gb0;
        let j11 = 1.0 - kc[1][1] * gc1 - kb[1][1] * gb1;

        let det = j00 * j11 - j01 * j10;
        if det.abs() < 1e-30 {
            break;
        }
        let inv_det = 1.0 / det;

        v_nl[0] -= inv_det * (j11 * f0 - j01 * f1);
        v_nl[1] -= inv_det * (j00 * f1 - j10 * f0);
    }

    // 6. Final NL currents at the converged Vbe
    let (ic0, ib0, _, _) = bjt(v_nl[0]);
    let (ic1, ib1, _, _) = bjt(v_nl[1]);
    let ic_new = [ic0, ic1];
    let ib_new = [ib0, ib1];

    // 7. Node voltage update
    let sfb_dot = sfb_nic[0] * ic_new[0]
        + sfb_nic[1] * ic_new[1]
        + sfb_nib[0] * ib_new[0]
        + sfb_nib[1] * ib_new[1];
    for i in 0..N {
        let s_ni_i = ic_new[0] * s_nic[0][i]
            + ic_new[1] * s_nic[1][i]
            + ib_new[0] * s_nib[0][i]
            + ib_new[1] * s_nib[1][i];
        state.v[i] = v_pred[i] + s_ni_i - sm_k * sfb_dot * s_fb_col[i];
    }

    // 8. Cin companion update
    state.cin_rhs_prev = cin_rhs_now;
    let dv_cin = vin_eff - state.v[BASE1];
    state.j_cin = -gc_1pc * dv_cin - c_cin * state.j_cin;

    // 9. State update
    state.i_c = ic_new;
    state.i_b = ib_new;
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
            &self.s_nic,
            &self.s_nib,
            &self.s_fb_col,
            self.s_fb_fb,
            self.g_ldr,
            self.g_ldr_prev,
            &self.k_c,
            &self.k_b,
            &self.nv_sfb,
            &self.sfb_nic,
            &self.sfb_nib,
            self.g_cin,
            self.gc_1pc,
            self.c_cin,
            &mut self.main,
            input,
        );

        // Update shared R_ldr tracking (after the step consumed g_ldr_prev)
        self.g_ldr_prev = self.g_ldr;

        let result = main_out;

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
        let (v_nl_dc, v_dc) = Self::full_dc_solve(&self.g_dc_base, &w, self.r_ldr);

        self.v_dc = v_dc;
        self.g_ldr = 1.0 / self.r_ldr;
        self.g_ldr_prev = self.g_ldr;

        // Reset to the DC operating point
        let state = DkState::at_dc(self.g_cin, v_nl_dc, v_dc);
        self.main = state;
    }
}

// ── Resistor/capacitor stamp helpers ────────────────────────────────────────

fn stamp_resistor(g: &mut MatN, i: usize, j: usize, r: f64) {
    let cond = 1.0 / r;
    g[i][i] += cond;
    g[j][j] += cond;
    g[i][j] -= cond;
    g[j][i] -= cond;
}

fn stamp_capacitor(c: &mut MatN, i: usize, j: usize, cap: f64) {
    c[i][i] += cap;
    c[j][j] += cap;
    c[i][j] -= cap;
    c[j][i] -= cap;
}

/// Stamp a capacitor from node `i` to ground (ground row/col is eliminated).
fn stamp_capacitor_to_gnd(c: &mut MatN, i: usize, cap: f64) {
    c[i][i] += cap;
}

// ── BJT model ───────────────────────────────────────────────────────────────

/// 2N5089 forward-active model: returns `(ic, ib, dic/dvbe, dib/dvbe)`.
///
/// Ic = Is·(exp(Vbe/(NF·Vt)) − 1) / qb, with the Gummel-Poon normalised base
/// charge qb = ½·(1 + √(1 + 4·q2)), q2 = (Is/IKF)·exp(Vbe/(NF·Vt)).
/// Ib = Icc/BF + Ise·(exp(Vbe/(NE·Vt)) − 1).
///
/// Vbe is clamped to [-1.0, VBE_MAX] to prevent exp overflow. No artificial
/// saturation limiting — the circuit topology and NR solver naturally
/// constrain the operating point. Early effect (VAF/VAR) is deliberately NOT
/// modelled: it would make the device a function of Vbc as well as Vbe, which
/// costs the kernel a dimension, and the DC point already lands inside the
/// arbiter tolerance without it (worst node 2.70 mV).
#[inline]
fn bjt(vbe: f64) -> (f64, f64, f64, f64) {
    let v = vbe.clamp(-1.0, VBE_MAX);
    let ef = (v / VTF).exp();
    let icc = IS * (ef - 1.0);

    // High injection (Gummel-Poon qb)
    let q2 = IS_OVER_IKF * ef;
    let root = (1.0 + 4.0 * q2).sqrt();
    let qb = 0.5 * (1.0 + root);
    let ic = icc / qb;

    // Base current: ideal + low-current recombination
    let ee = (v / NE_VT).exp();
    let ib = icc / BF + ISE * (ee - 1.0);

    // Derivatives
    let dicc = IS * ef / VTF;
    let dq2 = q2 / VTF;
    let dqb = dq2 / root;
    let gic = (dicc * qb - icc * dqb) / (qb * qb);
    let gib = dicc / BF + ISE * ee / NE_VT;

    (ic, ib, gic, gib)
}

/// BJT transconductance: dIc/dVbe. Only used in small-signal transfer tests.
#[cfg(test)]
#[inline]
fn bjt_gm(vbe: f64) -> f64 {
    bjt(vbe).2
}

/// Base-current transconductance: dIb/dVbe. Only used in small-signal tests.
#[cfg(test)]
#[inline]
fn bjt_gb(vbe: f64) -> f64 {
    bjt(vbe).3
}
// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    // ── Shared helpers ───────────────────────────────────────────────────────

    /// Build G_dc matrix independently from DkPreamp (for stamp verification).
    /// Does NOT include Cin-R1 companion or R_ldr.
    fn build_g_dc() -> MatN {
        // DRAWN TOPOLOGY: R-2 is gone from the core (it lives on the pickup
        // side of Cin, inside the input companion); R-3 returns to emit2b, not
        // ground; R-9/R-10 hang off node_c6, not coll2/out; OUT carries RLOAD.
        let mut g = mat_zero();
        stamp_resistor(&mut g, BASE1, EMIT2B, R3);
        g[EMIT1][EMIT1] += 1.0 / RE1;
        g[COLL1][COLL1] += 1.0 / RC1;
        stamp_resistor(&mut g, EMIT2, EMIT2B, RE2A);
        g[EMIT2B][EMIT2B] += 1.0 / RE2B;
        g[COLL2][COLL2] += 1.0 / RC2;
        stamp_resistor(&mut g, NODE_C6, OUT, R9);
        stamp_resistor(&mut g, NODE_C6, FB, R10);
        g[OUT][OUT] += 1.0 / RLOAD;
        g
    }

    /// Build C matrix independently from DkPreamp (for stamp verification).
    fn build_c_matrix() -> MatN {
        // DRAWN TOPOLOGY: CE2 spans R-7+R-8 (emit2 → GND, not emit2 ↔ emit2b);
        // C-6 and C-2 are new.
        let mut c = mat_zero();
        stamp_capacitor(&mut c, COLL1, BASE1, C3);
        stamp_capacitor(&mut c, COLL2, COLL1, C4);
        stamp_capacitor(&mut c, EMIT1, FB, CE1);
        stamp_capacitor(&mut c, COLL2, NODE_C6, C6);
        stamp_capacitor_to_gnd(&mut c, EMIT2, CE2);
        stamp_capacitor_to_gnd(&mut c, BASE1, C2);
        c
    }

    /// Build DC source vector independently from DkPreamp.
    fn build_w_vec() -> VecN {
        // DRAWN TOPOLOGY: no VCC/R2 term at base1 — R-2 is not a core element.
        let mut w = vec_zero();
        w[COLL1] += VCC / RC1;
        w[COLL2] += VCC / RC2;
        w
    }

    /// 8×8 matrix multiply: C = A * B
    fn mat_mul_nxn(a: &MatN, b: &MatN) -> MatN {
        let mut c = mat_zero();
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
    fn small_signal_gain_db(v_nl: [f64; 2], r_ldr: f64, freq_hz: f64) -> f64 {
        // DRAWN TOPOLOGY: the device now has a base current as well as a
        // collector current, so the linearised stamp carries gb = dIb/dVbe
        // alongside gm = dIc/dVbe. Without gb the base node would float.
        let (gm1, gm2) = (bjt_gm(v_nl[0]), bjt_gm(v_nl[1]));
        let (gb1, gb2) = (bjt_gb(v_nl[0]), bjt_gb(v_nl[1]));
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

        // Base-current VCCS: Ib = gb*(V_base - V_emit), enters emit, leaves base
        g_lin[EMIT1][BASE1] += gb1;
        g_lin[EMIT1][EMIT1] -= gb1;
        g_lin[BASE1][BASE1] -= gb1;
        g_lin[BASE1][EMIT1] += gb1;
        g_lin[EMIT2][COLL1] += gb2;
        g_lin[EMIT2][EMIT2] -= gb2;
        g_lin[COLL1][COLL1] -= gb2;
        g_lin[COLL1][EMIT2] += gb2;

        // Cin-R1 input admittance: Y = jωCin / (1 + jωR1Cin)
        // Input two-port: Thevenin R1||R2 behind Cin, drive scaled by K_IN_DIV
        let jwrc = c_mul(jw, (R_IN_EFF * CIN, 0.0));
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
        b[BASE1] = c_mul(y_cin, (K_IN_DIV, 0.0));

        let v = complex_solve(&a_cpx, &b);
        20.0 * c_abs(v[OUT]).log10()
    }

    /// Find -3dB bandwidth by binary search on the transfer function.
    fn find_bandwidth(v_nl: [f64; 2], r_ldr: f64) -> f64 {
        let ref_gain = small_signal_gain_db(v_nl, r_ldr, 1000.0);
        let target = ref_gain - 3.0;
        let mut lo: f64 = 1000.0;
        let mut hi: f64 = 200_000.0;
        for _ in 0..60 {
            let mid = (lo * hi).sqrt();
            if small_signal_gain_db(v_nl, r_ldr, mid) > target {
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
        // ACCEPTANCE #1 — drawn topology, BF-1434 card, Vcc = 14.5 V.
        //
        // Targets are the independent SPICE cross-check deck's DC table for the
        // revised netlist (spice/melange/wurli-preamp.cir); tolerance +/-5 mV.
        //
        // PRE-REVISION EXPECTATIONS REPLACED: this test previously asserted
        // base1=2.854 / emit1=2.297 / coll1=4.556 / emit2a=3.897 / coll2=8.551
        // against an ideal-BJT (BF=100000) solve of the OLD topology at 15 V,
        // with 0.1-1.0 V tolerances. Every one of those numbers is void: R-2
        // left the base node, R-3 moved to emit2b, and the rail is 14.5 V.
        //
        // Measured at the time of writing (88.2 kHz and 176.4 kHz, identical):
        //   base1 2.6516 (+0.59 mV)   emit1  2.0907 (-2.25 mV)
        //   coll1 4.2698 (-0.20 mV)   emit2a 3.6001 (-1.93 mV)
        //   emit2b 2.7083 (-1.71 mV)  coll2  8.5637 (+2.70 mV)
        let sr = 88200.0;
        let preamp = DkPreamp::new(sr);
        let v = preamp.v_dc; // At init, dv=0, so v_dc IS the operating point

        let targets = [
            (BASE1, 2.651, "TR-1 base"),
            (EMIT1, 2.093, "TR-1 emitter"),
            (COLL1, 4.270, "TR-1 collector"),
            (EMIT2, 3.602, "TR-2 emitter (emit2a)"),
            (EMIT2B, 2.710, "R-7/R-8 junction (emit2b)"),
            (COLL2, 8.561, "TR-2 collector"),
        ];
        for (node, want, what) in targets {
            let err_mv = (v[node] - want) * 1000.0;
            assert!(
                err_mv.abs() < 5.0,
                "{what}: {:.4} V, want {want:.3} V (err {err_mv:+.2} mV, tol +/-5 mV)",
                v[node]
            );
        }

        // Vbe sanity — TR-1 runs at ~58 uA, TR-2 at ~3.3 mA, so Vbe2 > Vbe1.
        let vbe1 = v[BASE1] - v[EMIT1];
        let vbe2 = v[COLL1] - v[EMIT2];
        assert!(vbe1 > 0.50 && vbe1 < 0.62, "Vbe1 = {vbe1:.3}V");
        assert!(vbe2 > 0.60 && vbe2 < 0.72, "Vbe2 = {vbe2:.3}V");
        assert!(vbe2 > vbe1, "Vbe2 must exceed Vbe1 (50x the current)");

        // R-3 is TR-1's ONLY DC bias path: base1 must sit BELOW emit2b by
        // exactly I_B1 * R3. With the pre-revision beta=infinity kernel this
        // drop was structurally zero (base1 == emit2b) — see the BJT block.
        let drop = v[EMIT2B] - v[BASE1];
        assert!(
            drop > 0.030 && drop < 0.090,
            "R-3 drop = {drop:.4} V, want ~0.059 V (I_B1 * 470K). \
             Zero here means the base current vanished from the kernel."
        );
    }

    #[test]
    fn test_dc_output_node_is_zero() {
        // C-6 REGRESSION GUARD (structural half of ACCEPTANCE #4).
        // C-6 series-couples TR-2's collector to the R-9/R-10 node, so
        // node_c6 / out / fb carry NO DC and no DC flows into the LDR leg.
        // Before the revision the output was DC-coupled into R-10 and the
        // whole shadow-pump apparatus existed to cancel the resulting pump.
        let preamp = DkPreamp::new(88200.0);
        let v = preamp.v_dc;
        for (node, name) in [(NODE_C6, "node_c6"), (OUT, "out"), (FB, "fb")] {
            assert!(
                v[node].abs() < 1e-9,
                "{name} must sit at 0 V DC behind C-6, got {:.6} V",
                v[node]
            );
        }
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
        // DRAWN TOPOLOGY (2026-09-13). Changes vs the pre-revision expectations:
        //   base1   : was 1/R2 + 1/R3 (R-2 to Vcc, R-3 to gnd) -> now 1/R3 only.
        //             R-2 is not a core element; R-3 is the only path.
        //   emit2b  : gains 1/R3 — R-3 now returns HERE (DC feedback).
        //   coll2   : loses 1/R9 — R-9 hangs off node_c6 behind C-6.
        //   node_c6 : new node, 1/R9 + 1/R10.
        //   out     : was 1/R9 + 1/R10 -> now 1/R9 + 1/RLOAD (R-10 moved to node_c6).
        let g = build_g_dc();
        let eps = 1e-12;

        let want = [
            (BASE1, 1.0 / R3, "base1: R3 to emit2b (ONLY bias path)"),
            (EMIT1, 1.0 / RE1, "emit1: Re1 to GND"),
            (COLL1, 1.0 / RC1, "coll1: Rc1 to Vcc"),
            (EMIT2, 1.0 / RE2A, "emit2: Re2a to emit2b"),
            (
                EMIT2B,
                1.0 / RE2A + 1.0 / RE2B + 1.0 / R3,
                "emit2b: Re2a + Re2b + R3 return",
            ),
            (COLL2, 1.0 / RC2, "coll2: Rc2 to Vcc (R9 moved behind C6)"),
            (NODE_C6, 1.0 / R9 + 1.0 / R10, "node_c6: R9 + R10"),
            (OUT, 1.0 / R9 + 1.0 / RLOAD, "out: R9 + load"),
            (FB, 1.0 / R10, "fb: R10 (no R_ldr yet)"),
        ];
        for (node, expect, what) in want {
            assert!(
                (g[node][node] - expect).abs() < eps,
                "G[{node}][{node}] = {:.6e}, want {expect:.6e} ({what})",
                g[node][node]
            );
        }
    }

    #[test]
    fn test_l1_g_off_diagonal_stamps() {
        // DRAWN TOPOLOGY: R-3 now couples base1<->emit2b (was base1->gnd, no
        // off-diagonal at all); R-9/R-10 couple node_c6<->out and
        // node_c6<->fb (were coll2<->out and out<->fb).
        let g = build_g_dc();
        let eps = 1e-12;

        let pairs = [
            (BASE1, EMIT2B, R3, "R3 DC feedback"),
            (EMIT2, EMIT2B, RE2A, "Re2a"),
            (NODE_C6, OUT, R9, "R9"),
            (NODE_C6, FB, R10, "R10"),
        ];
        for (i, j, r, what) in pairs {
            assert!(
                (g[i][j] - (-1.0 / r)).abs() < eps,
                "G[{i}][{j}] = {:.3e}, want {:.3e} ({what})",
                g[i][j],
                -1.0 / r
            );
            assert!((g[j][i] - (-1.0 / r)).abs() < eps, "G[{j}][{i}] ({what})");
        }

        // All other off-diagonals must be zero
        let mut connected = Vec::new();
        for (i, j, _, _) in pairs {
            connected.push((i, j));
            connected.push((j, i));
        }
        for i in 0..N {
            for j in 0..N {
                if i == j || connected.contains(&(i, j)) {
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
        // DRAWN TOPOLOGY: C-2 (220pF) restored at base1->gnd; CE2 spans
        // R-7+R-8 so it is emit2->gnd (emit2b now carries NO capacitance);
        // C-6 (4.7uF) couples coll2<->node_c6.
        let c = build_c_matrix();
        let eps = 1e-15;

        assert!(
            (c[BASE1][BASE1] - (C3 + C2)).abs() < eps,
            "C[base1] = C3 + C2"
        );
        assert!((c[EMIT1][EMIT1] - CE1).abs() < eps, "C[emit1] = Ce1");
        assert!(
            (c[COLL1][COLL1] - (C3 + C4)).abs() < eps,
            "C[coll1] = C3 + C4"
        );
        assert!((c[EMIT2][EMIT2] - CE2).abs() < eps, "C[emit2] = Ce2 to gnd");
        assert!(
            c[EMIT2B][EMIT2B].abs() < eps,
            "C[emit2b] must be ZERO — Ce2 spans R-7+R-8 to ground now, got {:.3e}",
            c[EMIT2B][EMIT2B]
        );
        assert!(
            (c[COLL2][COLL2] - (C4 + C6)).abs() < eps,
            "C[coll2] = C4 + C6"
        );
        assert!((c[NODE_C6][NODE_C6] - C6).abs() < eps, "C[node_c6] = C6");
        assert!(c[OUT][OUT].abs() < eps, "C[out] should be zero");
        assert!((c[FB][FB] - CE1).abs() < eps, "C[fb] = Ce1");

        // Off-diagonal entries (caps create negative off-diags)
        assert!((c[BASE1][COLL1] - (-C3)).abs() < eps, "C3: base1<->coll1");
        assert!((c[COLL1][BASE1] - (-C3)).abs() < eps, "C3: coll1<->base1");
        assert!((c[COLL2][COLL1] - (-C4)).abs() < eps, "C4: coll2<->coll1");
        assert!((c[COLL1][COLL2] - (-C4)).abs() < eps, "C4: coll1<->coll2");
        assert!((c[EMIT1][FB] - (-CE1)).abs() < eps, "Ce1: emit1<->fb");
        assert!((c[FB][EMIT1] - (-CE1)).abs() < eps, "Ce1: fb<->emit1");
        assert!(
            (c[COLL2][NODE_C6] - (-C6)).abs() < eps,
            "C6: coll2<->node_c6"
        );
        assert!(
            (c[NODE_C6][COLL2] - (-C6)).abs() < eps,
            "C6: node_c6<->coll2"
        );
        assert!(
            c[EMIT2][EMIT2B].abs() < eps,
            "Ce2 must NOT couple emit2<->emit2b any more"
        );
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

        // DRAWN TOPOLOGY: base1 has NO supply term — R-2 left the core.
        assert!(
            w[BASE1].abs() < eps,
            "w[base1] must be ZERO (R-2 is on the pickup side of Cin), got {:.3e}",
            w[BASE1]
        );
        assert!((w[COLL1] - VCC / RC1).abs() < eps, "w[coll1] = Vcc/Rc1");
        assert!((w[COLL2] - VCC / RC2).abs() < eps, "w[coll2] = Vcc/Rc2");

        // All other entries zero
        for i in 0..N {
            if i == COLL1 || i == COLL2 {
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

        let alpha_cin = 2.0 * R_IN_EFF * CIN * sr;
        let g_cin = (2.0 * CIN * sr) / (1.0 + alpha_cin);
        g[BASE1][BASE1] += g_cin;
        // NO R_ldr — S_base is built without it
        let a = mat_add(&two_c_t, &g);

        let preamp = DkPreamp::new(sr);
        let product = mat_mul_nxn(&preamp.s_base, &a);

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

        let alpha_cin = 2.0 * R_IN_EFF * CIN * sr;
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
        // DRAWN TOPOLOGY: the kernel is now TWO matrices — collector-current
        // and base-current incidence — because the BJT model gained a base
        // current (β=∞ could not produce the drop across R-3; see the BJT
        // constants block). Both must match their direct products.
        let k_c_full = compute_k(&preamp.s_base, &NIC);
        let k_b_full = compute_k(&preamp.s_base, &NIB);

        for i in 0..2 {
            for j in 0..2 {
                assert!(
                    (preamp.k_c[i][j] - k_c_full[i][j]).abs() < 1e-10,
                    "K_c[{}][{}] mismatch: stored={:.6e}, computed={:.6e}",
                    i,
                    j,
                    preamp.k_c[i][j],
                    k_c_full[i][j]
                );
                assert!(
                    (preamp.k_b[i][j] - k_b_full[i][j]).abs() < 1e-10,
                    "K_b[{}][{}] mismatch: stored={:.6e}, computed={:.6e}",
                    i,
                    j,
                    preamp.k_b[i][j],
                    k_b_full[i][j]
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

        let alpha_cin = 2.0 * R_IN_EFF * CIN * sr;
        let g_cin = (2.0 * CIN * sr) / (1.0 + alpha_cin);
        g_base[BASE1][BASE1] += g_cin;

        let preamp = DkPreamp::new(sr);

        for &r_ldr in &[1_000_000.0, 224_000.0, 50_000.0, 19_000.0] {
            // Brute-force K
            let mut g_full = g_base;
            g_full[FB][FB] += 1.0 / r_ldr;
            let s_full = mat_inverse(&mat_add(&two_c_t, &g_full));
            let k_c_bf = compute_k(&s_full, &NIC);
            let k_b_bf = compute_k(&s_full, &NIB);

            // SM-corrected K — now checked for BOTH incidence maps.
            let g_ldr = 1.0 / r_ldr;
            let sm_k = g_ldr / (1.0 + preamp.s_fb_fb * g_ldr);

            for i in 0..2 {
                for j in 0..2 {
                    let k_c_sm = preamp.k_c[i][j] - sm_k * preamp.nv_sfb[i] * preamp.sfb_nic[j];
                    let k_b_sm = preamp.k_b[i][j] - sm_k * preamp.nv_sfb[i] * preamp.sfb_nib[j];
                    let ec = (k_c_sm - k_c_bf[i][j]).abs();
                    let eb = (k_b_sm - k_b_bf[i][j]).abs();
                    assert!(
                        ec < 1e-6 * k_c_bf[i][j].abs().max(1e-6),
                        "K_c_eff[{i}][{j}] at R_ldr={r_ldr:.0}: sm={k_c_sm:.6e}, bf={:.6e}",
                        k_c_bf[i][j]
                    );
                    assert!(
                        eb < 1e-6 * k_b_bf[i][j].abs().max(1e-6),
                        "K_b_eff[{i}][{j}] at R_ldr={r_ldr:.0}: sm={k_b_sm:.6e}, bf={:.6e}",
                        k_b_bf[i][j]
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

        let alpha_cin = 2.0 * R_IN_EFF * CIN * sr;
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
    fn vnl_from_preamp(preamp: &DkPreamp) -> [f64; 2] {
        preamp.main.v_nl
    }

    #[test]
    fn test_l4_midband_gain_no_tremolo() {
        let preamp = DkPreamp::new(88200.0);
        let vnl = vnl_from_preamp(&preamp);

        let gain = small_signal_gain_db(vnl, 1_000_000.0, 1000.0);
        assert!(
            gain > 3.0 && gain < 12.0,
            "SS gain @ 1kHz (no trem) = {gain:.1} dB, want ~6 dB"
        );
    }

    #[test]
    fn test_l4_midband_gain_tremolo_bright() {
        let preamp = DkPreamp::new(88200.0);
        let vnl = vnl_from_preamp(&preamp);

        let gain = small_signal_gain_db(vnl, 19_000.0, 1000.0);
        assert!(
            gain > 8.0 && gain < 18.0,
            "SS gain @ 1kHz (trem bright) = {gain:.1} dB, want ~12 dB"
        );
    }

    #[test]
    fn test_l4_tremolo_gain_range() {
        let preamp = DkPreamp::new(88200.0);
        let vnl = vnl_from_preamp(&preamp);

        let gain_lo = small_signal_gain_db(vnl, 1_000_000.0, 1000.0);
        let gain_hi = small_signal_gain_db(vnl, 19_000.0, 1000.0);
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
        let vnl = vnl_from_preamp(&preamp);

        let bw_no_trem = find_bandwidth(vnl, 1_000_000.0);
        assert!(
            bw_no_trem > 8_000.0,
            "BW (no trem) = {bw_no_trem:.0} Hz, want > 8 kHz"
        );

        let bw_trem = find_bandwidth(vnl, 19_000.0);
        assert!(
            bw_trem > 8_000.0,
            "BW (trem bright) = {bw_trem:.0} Hz, want > 8 kHz (decoupled gives ~5.2 kHz)"
        );
    }

    #[test]
    fn test_l4_bandwidth_independent_of_rldr() {
        let preamp = DkPreamp::new(88200.0);
        let vnl = vnl_from_preamp(&preamp);

        let bw_1m = find_bandwidth(vnl, 1_000_000.0);
        let bw_19k = find_bandwidth(vnl, 19_000.0);

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
        let vnl = vnl_from_preamp(&preamp);

        let gain_1m = 10f64.powf(small_signal_gain_db(vnl, 1_000_000.0, 1000.0) / 20.0);
        let bw_1m = find_bandwidth(vnl, 1_000_000.0);
        let gbw_1m = gain_1m * bw_1m;

        let gain_19k = 10f64.powf(small_signal_gain_db(vnl, 19_000.0, 1000.0) / 20.0);
        let bw_19k = find_bandwidth(vnl, 19_000.0);
        let gbw_19k = gain_19k * bw_19k;

        assert!(
            gbw_19k > gbw_1m * 1.2,
            "GBW should scale with gain: 1M={gbw_1m:.0}, 19K={gbw_19k:.0}"
        );
    }

    #[test]
    fn test_l4_frequency_response_shape() {
        let preamp = DkPreamp::new(88200.0);
        let vnl = vnl_from_preamp(&preamp);

        let gain_100 = small_signal_gain_db(vnl, 1_000_000.0, 100.0);
        let gain_1k = small_signal_gain_db(vnl, 1_000_000.0, 1000.0);
        let gain_10k = small_signal_gain_db(vnl, 1_000_000.0, 10000.0);

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
        let vnl_a = vnl_from_preamp(&p1);
        let vnl_b = vnl_from_preamp(&p2);

        for &freq in &[100.0, 1000.0, 5000.0, 10000.0] {
            let g1 = small_signal_gain_db(vnl_a, 1_000_000.0, freq);
            let g2 = small_signal_gain_db(vnl_b, 1_000_000.0, freq);
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
        let alpha = 2.0 * R_IN_EFF * CIN * sr;
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

    /// Idle pump level under tremolo R_ldr modulation, with zero input.
    ///
    /// This used to verify that shadow subtraction cancelled the pump. The
    /// shadow was retired 2026-09-14; the test now verifies the thing that
    /// actually matters — that C-6 leaves no pump to cancel in the first place.
    #[test]
    fn test_idle_pump_level() {
        use crate::tremolo::Tremolo;

        let os_sr = 88200.0;
        let mut preamp = DkPreamp::new(os_sr);
        let mut tremolo = Tremolo::new(1.0, os_sr);

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

    #[test]
    fn test_rldr_transition_no_click() {
        // Regression test: R_ldr transitions (tremolo starting/stopping) must
        // not produce per-sample discontinuities that downstream gain (~49×)
        // would amplify into audible clicks.
        //
        // This test verifies continuity through the full cycle: modulation →
        // constant → modulation. (It formerly leaned on the shadow solver
        // running unconditionally; with the shadow retired, continuity rests on
        // the Sherman-Morrison R_ldr path alone, which is what it should have
        // been testing all along.)
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);

        // 1. Run with tremolo modulation for 500ms
        let n_settle = (sr * 0.5) as usize;
        for i in 0..n_settle {
            let phase = 2.0 * std::f64::consts::PI * 5.63 * i as f64 / sr;
            let lfo = phase.sin().max(0.0);
            let r_ldr = 18_000.0 + 50.0 + (1_000_000.0 - 50.0) * (1.0 - lfo);
            preamp.set_ldr_resistance(r_ldr);
            preamp.process_sample(0.0);
        }

        // 2. Transition to constant R_ldr (simulating depth → 0)
        preamp.set_ldr_resistance(1_068_000.0);
        let mut prev = preamp.process_sample(0.0);
        for j in 0..200 {
            let out = preamp.process_sample(0.0);
            let delta = (out - prev).abs();
            assert!(
                delta < 1e-4,
                "Modulation→constant transient at sample {j}: delta={delta:.2e}"
            );
            prev = out;
        }

        // 3. Run at constant R_ldr for 300ms (longer than Ce1 tau)
        let n_constant = (sr * 0.3) as usize;
        for _ in 0..n_constant {
            prev = preamp.process_sample(0.0);
        }

        // 4. Resume modulation (simulating depth → nonzero)
        for j in 0..200 {
            let phase = 2.0 * std::f64::consts::PI * 5.63 * j as f64 / sr;
            let lfo = phase.sin().max(0.0);
            let r_ldr = 18_000.0 + 50.0 + (1_000_000.0 - 50.0) * (1.0 - lfo);
            preamp.set_ldr_resistance(r_ldr);
            let out = preamp.process_sample(0.0);
            let delta = (out - prev).abs();
            assert!(
                delta < 1e-4,
                "Constant→modulation transient at sample {j}: delta={delta:.2e}"
            );
            prev = out;
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Drawn-topology acceptance tests (2026-09 revision)
    // ══════════════════════════════════════════════════════════════════════════

    /// Time-domain gain in dB at a frequency, measured through the real solver.
    fn measure_gain_db(preamp: &mut DkPreamp, freq: f64, amp: f64, sr: f64) -> f64 {
        preamp.reset();
        let n_settle = (sr * 0.4) as usize;
        let n_measure = (sr * 0.2) as usize;
        for i in 0..n_settle {
            let t = i as f64 / sr;
            preamp.process_sample(amp * (2.0 * PI * freq * t).sin());
        }
        let mut peak = 0.0f64;
        for i in 0..n_measure {
            let t = (n_settle + i) as f64 / sr;
            let out = preamp.process_sample(amp * (2.0 * PI * freq * t).sin());
            peak = peak.max(out.abs());
        }
        20.0 * (peak / amp).log10()
    }

    #[test]
    fn test_acceptance_ac_gain_1khz() {
        // ACCEPTANCE #2 — 1 kHz small-signal gain at OUT, R_ldr = 12K, into the
        // 100K load (RLOAD) that the re-baselined tb_preamp_ac bench measures
        // through. Target 15.54 dB +/-0.3.
        //
        // Measured at the time of writing: 15.56 dB time-domain (88.2 kHz),
        // 15.65 dB from the continuous-time linearised model — i.e. +0.02 dB
        // against the bench figure.
        //
        // PRE-REVISION EXPECTATION REPLACED: `test_gain_no_tremolo` asserted a
        // 3-12 dB window "want ~6 dB" at R_ldr = 1M. Under the drawn topology
        // the same 1M shunt gives ~7.0 dB and the gain-vs-shunt curve as a
        // whole sits ~1.9 dB higher, so the old window is not a valid check.
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);
        preamp.set_ldr_resistance(12_000.0);
        let gain_db = measure_gain_db(&mut preamp, 1000.0, 0.001, sr);
        assert!(
            (gain_db - 15.54).abs() < 0.3,
            "1 kHz gain at OUT (R_ldr=12K, 100K load) = {gain_db:.2} dB, want 15.54 +/-0.3"
        );
    }

    #[test]
    fn test_acceptance_hf_bandwidth() {
        // ACCEPTANCE #3 — HF -3 dB re 1 kHz, target 16.8 kHz +/-15%
        // (14.28-19.32 kHz). Integrator-dependent, so both figures are stated.
        //
        // Measured at the time of writing, R_ldr = 12K:
        //   continuous-time linearised model : 17.96 kHz
        //   trapezoidal solver @ 88.2 kHz    : ~15.2 kHz
        // The gap is bilinear frequency warping, not a topology error: the
        // bilinear map sends an analog 17.96 kHz to (fs/pi)*atan(pi*f/fs) =
        // 16.0 kHz at fs = 88.2 kHz, and the remaining difference is the
        // measurement's peak-detect bias. Both sit inside the +/-15% band, so
        // the trapezoidal integrator is kept (see the module docs).
        let sr = 88200.0;
        let preamp = DkPreamp::new(sr);
        let bw_lin = find_bandwidth(preamp.main.v_nl, 12_000.0);
        assert!(
            bw_lin > 14_280.0 && bw_lin < 19_320.0,
            "linearised -3 dB BW = {bw_lin:.0} Hz, want 16800 +/-15%"
        );

        // Time-domain check through the actual discretised solver.
        let g1k = {
            let mut p = DkPreamp::new(sr);
            p.set_ldr_resistance(12_000.0);
            measure_gain_db(&mut p, 1000.0, 0.001, sr)
        };
        let (mut lo, mut hi) = (5_000.0f64, 30_000.0f64);
        for _ in 0..12 {
            let mid = (lo * hi).sqrt();
            let mut p = DkPreamp::new(sr);
            p.set_ldr_resistance(12_000.0);
            if measure_gain_db(&mut p, mid, 0.001, sr) - g1k > -3.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let bw_td = (lo * hi).sqrt();
        assert!(
            bw_td > 14_280.0 && bw_td < 19_320.0,
            "trapezoidal -3 dB BW @88.2kHz = {bw_td:.0} Hz, want 16800 +/-15% \
             (bilinear warp accounts for the offset from the {bw_lin:.0} Hz continuous figure)"
        );
    }

    #[test]
    fn test_acceptance_pump_guard_c6() {
        // ACCEPTANCE #4 — C-6 pump guard. Mirror of the tb_pump_emit bench,
        // which is now a C-6 regression guard: cycling the LDR 19K<->1M at
        // 5.63 Hz with ZERO input must not move TR-1's bias.
        //
        // Before the revision the output was DC-coupled through R-10 into the
        // LDR leg, so LDR modulation pumped the operating point; the retired
        // shadow-subtraction machinery existed to cancel that pump. With C-6 in
        // place the pump is eliminated at source. Any failure here means C-6
        // has gone missing from the C matrix again.
        //
        // This probes emit1/coll1 DIRECTLY, so it was never maskable by the
        // shadow subtraction — which is why it is the guard that survives it.
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);
        preamp.set_ldr_resistance(19_000.0);
        preamp.reset();

        // Settle first: Ce1 x R_ldr has a multi-second time constant.
        for _ in 0..(sr as usize) {
            preamp.process_sample(0.0);
        }

        let (mut e_lo, mut e_hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut c_lo, mut c_hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let n = (sr * 2.0) as usize;
        for i in 0..n {
            let t = i as f64 / sr;
            // 19K <-> 1M cycled at the tremolo rate (log-swept, as the LDR moves)
            let phase = 0.5 - 0.5 * (2.0 * PI * 5.63 * t).cos();
            let r = 19_000.0f64 * (1_000_000.0f64 / 19_000.0).powf(phase);
            preamp.set_ldr_resistance(r);
            preamp.process_sample(0.0);
            let (e, c) = (preamp.main.v[EMIT1], preamp.main.v[COLL1]);
            e_lo = e_lo.min(e);
            e_hi = e_hi.max(e);
            c_lo = c_lo.min(c);
            c_hi = c_hi.max(c);
        }
        let emit_pp_mv = (e_hi - e_lo) * 1000.0;
        let coll_pp_mv = (c_hi - c_lo) * 1000.0;
        assert!(
            emit_pp_mv < 1.0,
            "emit1 DC excursion = {emit_pp_mv:.4} mV pp under LDR cycling, want < 1 mV \
             (C-6 missing from the C matrix?)"
        );
        assert!(
            coll_pp_mv < 1.0,
            "coll1 DC excursion = {coll_pp_mv:.4} mV pp under LDR cycling, want < 1 mV"
        );
    }

    #[test]
    fn test_numerics_no_nyquist_mode() {
        // INTEGRATOR GUARD — the reason this solver is still trapezoidal.
        //
        // The melange deck for this same topology is trapezoidal-UNSTABLE
        // (spectral radius 1.1394, dominant sign -1: a Nyquist-marginal z=-1
        // mode) and is author-pinned to backward Euler in
        // spice/melange/wurli-preamp.cir. That pin is for the CODEGEN path;
        // the deck header states the shipping hand solver "treats stability
        // separately", and notes BE's HF damping understates the ~16.7 kHz
        // corner. This solver keeps TRAPEZOIDAL because (a) it measures inside
        // the HF acceptance band and BE would pull it down, and (b) no z=-1
        // mode is observable here — which is what this test pins.
        //
        // A z=-1 mode shows up as sample-alternating ringing that does not
        // decay. We excite with an impulse AND modulate R_ldr (the element
        // that makes the deck's companion matrices time-varying, i.e. the
        // actual source of the deck's instability), then measure the energy
        // in the Nyquist bin of the tail.
        let sr = 88200.0;
        let mut preamp = DkPreamp::new(sr);
        preamp.set_ldr_resistance(19_000.0);
        preamp.reset();

        for _ in 0..(sr as usize) {
            preamp.process_sample(0.0);
        }
        preamp.process_sample(0.05); // impulse

        let n = (sr * 2.0) as usize;
        let mut tail = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f64 / sr;
            let phase = 0.5 - 0.5 * (2.0 * PI * 5.63 * t).cos();
            preamp.set_ldr_resistance(19_000.0 * (1_000_000.0f64 / 19_000.0).powf(phase));
            tail.push(preamp.process_sample(0.0));
        }

        // Nyquist-bin energy over the LAST half second, normalised by the
        // broadband RMS of the same window. A live z=-1 mode would dominate.
        let start = tail.len() - (sr * 0.5) as usize;
        let w = &tail[start..];
        let mut alt = 0.0f64;
        let mut sq = 0.0f64;
        for (i, &x) in w.iter().enumerate() {
            alt += if i % 2 == 0 { x } else { -x };
            sq += x * x;
        }
        let alt_rms = alt.abs() / w.len() as f64;
        let rms = (sq / w.len() as f64).sqrt();
        assert!(
            alt_rms < 1e-9,
            "Nyquist-bin (z=-1) component in tail = {alt_rms:.3e} (broadband rms {rms:.3e}) \
             — trapezoidal z=-1 mode is live; switch the integrator and re-measure HF"
        );
        assert!(
            w.iter().all(|x| x.is_finite()),
            "non-finite sample in tail — NR diverged"
        );
    }
}
