# DK Preamp Testing Strategy

Testing strategy for the DK (Discretization-Kernel) preamp implementation. The DK method derives the circuit model from explicit MNA matrices, which means the matrices ARE the model — we can test the model mathematically without running audio through it.

Five layers, bottom-up. Each layer catches a different class of bug, and each layer's passing increases confidence that higher layers will pass.

```
         /  Layer 5  \     Behavioral (slow, empirical)
        / Layer 4     \    Transfer function (fast, analytical)
       / Layer 3       \   DC operating point (one-shot NR)
      / Layer 2         \  Linear algebra identities
     / Layer 1           \ Matrix stamps (pure arithmetic)
```

## Layer 1: Matrix Stamp Verification

**What:** Assert every nonzero entry in G and C against hand-computed values.

**Why:** If a resistor is stamped at the wrong node pair, or a capacitor has the wrong sign, nothing downstream works. These tests are microsecond-fast and pinpoint the exact component that's wrong.

**Tests:**

```rust
#[test]
fn test_g_matrix_diagonal() {
    let dk = DkPreamp::new(44100.0);
    let g = dk.g_matrix();

    // Node 0 (base1): R1(22K) to input, R2(2M) to Vcc, R3(470K) to GND
    let expected_g00 = 1.0/22e3 + 1.0/2e6 + 1.0/470e3;
    assert_relative_eq!(g[0][0], expected_g00, epsilon = 1e-9);

    // Node 1 (emit1): Re1(33K) to GND
    // Ce1 coupling only appears in C matrix
    let expected_g11 = 1.0/33e3;
    assert_relative_eq!(g[1][1], expected_g11, epsilon = 1e-9);

    // Node 2 (coll1 = base2): Rc1(150K) to Vcc
    // Direct-coupled to TR-2 base — no resistor between nodes 2 and 3
    let expected_g22 = 1.0/150e3;
    assert_relative_eq!(g[2][2], expected_g22, epsilon = 1e-9);

    // ... every diagonal entry
}

#[test]
fn test_g_matrix_off_diagonal() {
    let dk = DkPreamp::new(44100.0);
    let g = dk.g_matrix();

    // No resistors connect any two circuit nodes directly
    // (all resistors go to Vcc, GND, or input — which are sources, not nodes)
    // So G should be diagonal (before Rldr is added)
    for i in 0..8 {
        for j in 0..8 {
            if i != j {
                assert_eq!(g[i][j], 0.0,
                    "G[{}][{}] should be zero (no resistor between nodes)", i, j);
            }
        }
    }
}

#[test]
fn test_g_matrix_rldr_stamp() {
    let dk = DkPreamp::new(44100.0);

    // Rldr adds 1/Rldr to G[7][7] only
    // With Rldr = 50K: G[7][7] should increase by 1/50e3
    // (R-10 already stamps G[6][6], G[7][7], G[6][7], G[7][6] for the
    //  out-to-fb connection, but Rldr is fb-to-GND)
}

#[test]
fn test_c_matrix_stamps() {
    let dk = DkPreamp::new(44100.0);
    let c = dk.c_matrix();

    // C-3 (100pF): nodes 2↔0 (coll1↔base1)
    assert_eq!(c[0][0],  100e-12);  // +C at (0,0)
    assert_eq!(c[2][2],  100e-12);  // +C at (2,2) (plus other caps at this node)
    assert_eq!(c[0][2], -100e-12);  // -C at (0,2)
    assert_eq!(c[2][0], -100e-12);  // -C at (2,0)

    // C-4 (100pF): nodes 5↔2 (coll2↔coll1)
    assert_eq!(c[5][5], 100e-12);
    assert_eq!(c[5][2], -100e-12);
    assert_eq!(c[2][5], -100e-12);
    // c[2][2] gets +100pF from both C-3 and C-4

    // Ce1 (4.7uF): nodes 1↔7 (emit1↔fb)
    assert_relative_eq!(c[1][1], 4.7e-6, epsilon = 1e-9);
    assert_relative_eq!(c[1][7], -4.7e-6, epsilon = 1e-9);

    // Ce2 (22uF): nodes 3↔4 (emit2↔emit2b)
    assert_relative_eq!(c[3][3], 22e-6, epsilon = 1e-9);
    assert_relative_eq!(c[3][4], -22e-6, epsilon = 1e-9);

    // C matrix must be symmetric
    for i in 0..8 {
        for j in 0..8 {
            assert_eq!(c[i][j], c[j][i],
                "C matrix must be symmetric: C[{}][{}] != C[{}][{}]", i, j, j, i);
        }
    }
}

#[test]
fn test_nv_ni_matrices() {
    let dk = DkPreamp::new(44100.0);
    let (n_v, n_i) = dk.incidence_matrices();

    // N_v extracts Vbe: row 0 = Vbe1 = v[0] - v[1], row 1 = Vbe2 = v[2] - v[3]
    assert_eq!(n_v[0][0],  1.0);
    assert_eq!(n_v[0][1], -1.0);
    assert_eq!(n_v[1][2],  1.0);
    assert_eq!(n_v[1][3], -1.0);
    // All other entries zero

    // N_i injects Ic: collector and emitter stamps
    // (sign convention must match BJT equation — verified by DC solve in Layer 3)
}

#[test]
fn test_dc_source_vector() {
    let dk = DkPreamp::new(44100.0);
    let w = dk.dc_source_vector();

    // R2 (2M) from base1 to Vcc: Norton current = 15V / 2M = 7.5uA into node 0
    // R3 (470K) from base1 to GND: no current source (GND side)
    // Rc1 (150K) from coll1 to Vcc: Norton current = 15V / 150K = 100uA into node 2
    // Rc2 (1.8K) from coll2 to Vcc: Norton current = 15V / 1.8K = 8.33mA into node 5
    assert_relative_eq!(w[0], 15.0 / 2e6, epsilon = 1e-9);
    assert_relative_eq!(w[2], 15.0 / 150e3, epsilon = 1e-9);
    assert_relative_eq!(w[5], 15.0 / 1.8e3, epsilon = 1e-6);
}
```

**Failure diagnosis:** If a stamp test fails, the bug is in the matrix construction — wrong component value, wrong node index, or missing stamp. Fix is always local.


## Layer 2: Linear Algebra Identities

**What:** Verify that matrix operations produce mathematically correct results, independent of circuit physics.

**Why:** Gauss-Jordan inversion, Sherman-Morrison updates, and K extraction involve index arithmetic that's easy to get wrong. These tests catch pure math bugs.

**Tests:**

```rust
#[test]
fn test_s_is_inverse_of_a_dk() {
    let dk = DkPreamp::new(44100.0);
    let (s, a_dk) = (dk.s_matrix(), dk.a_dk_matrix());

    let product = mat_mul_8x8(&s, &a_dk);
    for i in 0..8 {
        for j in 0..8 {
            let expected = if i == j { 1.0 } else { 0.0 };
            assert_relative_eq!(product[i][j], expected, epsilon = 1e-10,
                "S * A_dk should be identity at [{},{}]", i, j);
        }
    }
}

#[test]
fn test_sherman_morrison_matches_brute_force() {
    // For several Rldr values, verify that the rank-1 update gives
    // the same result as a full 8x8 re-inversion
    for &rldr in &[19e3, 50e3, 100e3, 500e3, 1e6] {
        let dk = DkPreamp::new(44100.0);
        dk.set_ldr_resistance(rldr);
        let s_sm = dk.s_matrix();  // Sherman-Morrison path

        let s_brute = invert_8x8(&dk.a_dk_matrix_with_rldr(rldr));

        for i in 0..8 {
            for j in 0..8 {
                assert_relative_eq!(s_sm[i][j], s_brute[i][j], epsilon = 1e-10,
                    "Sherman-Morrison mismatch at [{},{}] for Rldr={}", i, j, rldr);
            }
        }
    }
}

#[test]
fn test_k_matrix_matches_full_product() {
    // K = N_v * S * N_i — verify the sparse extraction matches the full product
    let dk = DkPreamp::new(44100.0);
    let (n_v, n_i, s) = (dk.n_v(), dk.n_i(), dk.s_matrix());

    let k_full = mat_mul_2x8_8x8_8x2(&n_v, &s, &n_i);
    let k_sparse = dk.k_matrix();

    for i in 0..2 {
        for j in 0..2 {
            assert_relative_eq!(k_sparse[i][j], k_full[i][j], epsilon = 1e-12);
        }
    }
}

#[test]
fn test_k_updates_with_rldr() {
    // K should change when Rldr changes (since S changes)
    let dk = DkPreamp::new(44100.0);

    dk.set_ldr_resistance(1e6);
    let k_no_trem = dk.k_matrix();

    dk.set_ldr_resistance(19e3);
    let k_trem = dk.k_matrix();

    // At least one entry should differ meaningfully
    let max_diff = (0..2).flat_map(|i| (0..2).map(move |j|
        (k_no_trem[i][j] - k_trem[i][j]).abs()
    )).fold(0.0f64, f64::max);

    assert!(max_diff > 1e-6, "K matrix must depend on Rldr");
}

#[test]
fn test_history_matrix_rldr_update() {
    // (2C/T - G) should only differ at [7][7] when Rldr changes
    let dk = DkPreamp::new(44100.0);

    dk.set_ldr_resistance(1e6);
    let a_neg_1 = dk.a_neg_matrix();

    dk.set_ldr_resistance(19e3);
    let a_neg_2 = dk.a_neg_matrix();

    for i in 0..8 {
        for j in 0..8 {
            if i == 7 && j == 7 {
                // Should differ by delta_g = 1/19K - 1/1M
                let expected_diff = 1.0/19e3 - 1.0/1e6;
                assert_relative_eq!(
                    a_neg_1[7][7] - a_neg_2[7][7], expected_diff, epsilon = 1e-10
                );
            } else {
                assert_eq!(a_neg_1[i][j], a_neg_2[i][j],
                    "Only [7][7] should change with Rldr");
            }
        }
    }
}
```

**Failure diagnosis:** If these fail but Layer 1 passes, the bug is in the linear algebra routines (Gauss-Jordan, Sherman-Morrison indexing, K extraction). The circuit model is fine.


## Layer 3: DC Operating Point

**What:** Verify quiescent node voltages against SPICE.

**Why:** This is the first test that exercises the NR solver. If it passes, then: G and C are stamped correctly, the inverse is right, the BJT equation has the correct sign convention, and the NR converges. It's a single integration test for Layers 1 and 2.

**Tests:**

```rust
#[test]
fn test_dc_operating_point() {
    let dk = DkPreamp::new(44100.0);
    let v = dk.dc_voltages();

    // TR-1: B=2.45V, E=1.95V, C=4.1V (SPICE reference)
    assert_relative_eq!(v[0], 2.45, epsilon = 0.10);  // base1
    assert_relative_eq!(v[1], 1.95, epsilon = 0.10);  // emit1
    assert_relative_eq!(v[2], 4.10, epsilon = 0.20);  // coll1 = base2

    // TR-2: E=3.4V, C=8.8V
    assert_relative_eq!(v[3], 3.40, epsilon = 0.15);  // emit2
    assert_relative_eq!(v[5], 8.80, epsilon = 0.30);  // coll2

    // Vbe sanity: both BJTs should be forward-biased
    let vbe1 = v[0] - v[1];
    let vbe2 = v[2] - v[3];
    assert!(vbe1 > 0.4 && vbe1 < 0.7, "TR-1 Vbe={} out of range", vbe1);
    assert!(vbe2 > 0.5 && vbe2 < 0.8, "TR-2 Vbe={} out of range", vbe2);
}

#[test]
fn test_dc_convergence() {
    // DC solve should converge in reasonable iterations
    let dk = DkPreamp::new(44100.0);
    let (v, iterations) = dk.dc_solve_with_diagnostics();

    assert!(iterations < 50, "DC solve took {} iterations (expected < 50)", iterations);

    // Residual should be small
    let residual = dk.dc_residual(&v);
    assert!(residual < 1e-10, "DC residual {} too large", residual);
}

#[test]
fn test_dc_point_independent_of_sample_rate() {
    // DC operating point is a static solution — should not depend on fs
    let v_44k = DkPreamp::new(44100.0).dc_voltages();
    let v_96k = DkPreamp::new(96000.0).dc_voltages();
    let v_192k = DkPreamp::new(192000.0).dc_voltages();

    for i in 0..8 {
        assert_relative_eq!(v_44k[i], v_96k[i], epsilon = 1e-6);
        assert_relative_eq!(v_44k[i], v_192k[i], epsilon = 1e-6);
    }
}
```

**Tolerance notes:** Wider epsilon than pure math tests because our Ebers-Moll is simplified (no Early effect, infinite beta). 50-100mV error on voltages is acceptable — the goal is "same operating region as SPICE," not exact match.

**Failure diagnosis:** If this fails but Layer 2 passes, the bug is in the NR solver (wrong Jacobian, sign convention error, convergence failure) or the BJT equation itself.


## Layer 4: Small-Signal Transfer Function

**What:** Evaluate the continuous-time transfer function H(jw) analytically at arbitrary frequencies. No time-domain simulation, no FFT, no sample rate dependency.

**Why:** This is the decisive test for the DK model. The linearized system has an analytical frequency response that can be compared directly to SPICE AC sweep data. It tests the coupled system behavior (C-3/C-4 Miller interaction) without any discretization artifacts.

**Method:**

Linearize the BJT elements at the DC operating point to get transconductances gm1, gm2. The small-signal conductance matrix is:

```
G_lin = G + N_i * diag(gm1, gm2) * N_v
```

The transfer function at angular frequency w:

```
H(jw) = C_out * (jwC + G_lin)^{-1} * B_in
```

where C_out extracts the output node voltage and B_in is the input excitation vector. This requires inverting a complex 8x8 matrix per frequency point — but that's trivial computation for a test.

**Tests:**

```rust
/// Helper: compute small-signal gain in dB at a given frequency
fn small_signal_gain_db(dk: &DkPreamp, freq_hz: f64) -> f64;

/// Helper: find -3dB frequency by binary search on small_signal_gain_db
fn find_bandwidth(dk: &DkPreamp) -> f64;

#[test]
fn test_midband_gain_no_tremolo() {
    let dk = DkPreamp::new(44100.0);
    dk.set_ldr_resistance(1e6);

    let gain = small_signal_gain_db(&dk, 1000.0);
    assert_relative_eq!(gain, 6.0, epsilon = 0.5);  // SPICE: 6.0 dB
}

#[test]
fn test_midband_gain_tremolo_bright() {
    let dk = DkPreamp::new(44100.0);
    dk.set_ldr_resistance(19e3);

    let gain = small_signal_gain_db(&dk, 1000.0);
    assert_relative_eq!(gain, 12.1, epsilon = 0.5);  // SPICE: 12.1 dB
}

#[test]
fn test_tremolo_gain_range() {
    let dk = DkPreamp::new(44100.0);

    dk.set_ldr_resistance(1e6);
    let gain_lo = small_signal_gain_db(&dk, 1000.0);

    dk.set_ldr_resistance(19e3);
    let gain_hi = small_signal_gain_db(&dk, 1000.0);

    let range = gain_hi - gain_lo;
    assert_relative_eq!(range, 6.1, epsilon = 0.5);  // SPICE: 6.1 dB
}

#[test]
fn test_bandwidth_extends_past_15khz() {
    // THE key test: bandwidth should be ~15.5 kHz regardless of Rldr.
    // The current EbersMollPreamp gives ~5.2 kHz at tremolo-bright
    // because it doesn't model the C-3/C-4 coupled Miller loop.
    let dk = DkPreamp::new(44100.0);

    dk.set_ldr_resistance(1e6);
    let bw_no_trem = find_bandwidth(&dk);
    assert!(bw_no_trem > 12e3, "BW (no trem) = {:.0} Hz, expected > 12 kHz", bw_no_trem);

    dk.set_ldr_resistance(19e3);
    let bw_trem = find_bandwidth(&dk);
    assert!(bw_trem > 12e3, "BW (trem bright) = {:.0} Hz, expected > 12 kHz", bw_trem);
}

#[test]
fn test_bandwidth_independent_of_rldr() {
    let dk = DkPreamp::new(44100.0);

    dk.set_ldr_resistance(1e6);
    let bw_1 = find_bandwidth(&dk);

    dk.set_ldr_resistance(19e3);
    let bw_2 = find_bandwidth(&dk);

    let ratio = (bw_1 - bw_2).abs() / bw_1;
    assert!(ratio < 0.20,
        "BW should not depend strongly on Rldr: {:.0} vs {:.0} Hz ({:.0}% diff)",
        bw_1, bw_2, ratio * 100.0);
}

#[test]
fn test_gbw_scales_with_gain() {
    let dk = DkPreamp::new(44100.0);

    dk.set_ldr_resistance(1e6);
    let gain_1 = 10f64.powf(small_signal_gain_db(&dk, 1000.0) / 20.0);
    let bw_1 = find_bandwidth(&dk);
    let gbw_1 = gain_1 * bw_1;

    dk.set_ldr_resistance(19e3);
    let gain_2 = 10f64.powf(small_signal_gain_db(&dk, 1000.0) / 20.0);
    let bw_2 = find_bandwidth(&dk);
    let gbw_2 = gain_2 * bw_2;

    assert!(gbw_2 > gbw_1 * 1.3,
        "GBW must increase with gain: {:.0} vs {:.0}", gbw_2, gbw_1);
}

#[test]
fn test_frequency_response_shape() {
    // Verify general shape matches SPICE AC sweep
    let dk = DkPreamp::new(44100.0);
    dk.set_ldr_resistance(1e6);

    let gain_100 = small_signal_gain_db(&dk, 100.0);
    let gain_1k  = small_signal_gain_db(&dk, 1000.0);
    let gain_10k = small_signal_gain_db(&dk, 10000.0);

    // Midband should be relatively flat (100 Hz to 1 kHz within 2 dB)
    assert!((gain_100 - gain_1k).abs() < 2.0);

    // 10 kHz should be close to midband (within 3 dB for 15 kHz BW)
    assert!((gain_10k - gain_1k).abs() < 3.0);
}

#[test]
fn test_transfer_function_independent_of_sample_rate() {
    // The small-signal transfer function is continuous-time (pre-discretization).
    // It should give identical results regardless of the sample rate used to
    // construct the DkPreamp (sample rate only affects the discretization).
    let dk_44k = DkPreamp::new(44100.0);
    let dk_96k = DkPreamp::new(96000.0);

    for &freq in &[100.0, 1000.0, 5000.0, 10000.0, 15000.0] {
        let g1 = small_signal_gain_db(&dk_44k, freq);
        let g2 = small_signal_gain_db(&dk_96k, freq);
        assert_relative_eq!(g1, g2, epsilon = 0.01,
            "Transfer function should not depend on fs at {} Hz", freq);
    }
}
```

**This is the decisive layer.** The `test_bandwidth_independent_of_rldr` test is the one the current EbersMollPreamp fundamentally cannot pass. If the DK model passes it, the coupled C-3/C-4 Miller loop is working correctly.

**Failure diagnosis:** If this fails but Layer 3 passes, the linearization or transfer function extraction has a bug. The DC point is right, but the small-signal behavior isn't — check the gm computation or the complex matrix inverse.


## Layer 5: Time-Domain Behavioral

**What:** Feed audio through the discrete-time DK model, measure output properties. The traditional approach — and the slowest.

**Why:** Catches discretization bugs, NR convergence failures, and state update errors that the continuous-time small-signal analysis (Layer 4) cannot see. Also verifies large-signal behavior (harmonic distortion, clipping) which the linearized model can't predict.

**Tests:**

```rust
#[test]
fn test_sine_gain_matches_small_signal() {
    // Run a low-level 1kHz sine (small-signal regime), measure RMS gain.
    // Should match Layer 4's analytical gain within 0.5 dB.
    // If not: discretization or NR convergence bug.
    let dk = DkPreamp::new(88200.0);  // 2x oversampled
    dk.set_ldr_resistance(1e6);

    let amplitude = 0.001;  // Small signal, linear regime
    let output = run_sine(&dk, 1000.0, amplitude, 0.1);  // 100ms
    let gain_db = 20.0 * (rms(&output) / (amplitude / std::f64::consts::SQRT_2)).log10();
    let expected = small_signal_gain_db(&dk, 1000.0);

    assert_relative_eq!(gain_db, expected, epsilon = 0.5);
}

#[test]
fn test_h2_dominates_at_moderate_drive() {
    // 440 Hz at moderate amplitude — should produce H2 > H3
    // (asymmetric Stage 1 clipping headroom: 2.05V vs 10.9V)
    let dk = DkPreamp::new(88200.0);
    dk.set_ldr_resistance(1e6);

    let output = run_sine(&dk, 440.0, 0.05, 0.2);
    let spectrum = fft_magnitudes(&output, 88200.0);

    let h2 = spectrum[880.0];
    let h3 = spectrum[1320.0];
    assert!(h2 > h3, "H2 ({:.1} dB) should exceed H3 ({:.1} dB)", h2, h3);
}

#[test]
fn test_stability_impulse_decay() {
    let dk = DkPreamp::new(88200.0);
    dk.set_ldr_resistance(1e6);

    // Single impulse
    let mut output = vec![0.0f64; 88200];  // 1 second
    output[0] = dk.process_sample(0.1);
    for i in 1..88200 {
        output[i] = dk.process_sample(0.0);
    }

    // Should decay to negligible
    let tail_rms = rms(&output[44100..]);
    assert!(tail_rms < 1e-6, "Impulse tail RMS = {}, expected < 1e-6", tail_rms);

    // Should not contain NaN or Inf
    assert!(output.iter().all(|x| x.is_finite()), "Output contains non-finite values");
}

#[test]
fn test_tremolo_modulates_gain() {
    // Sweep Rldr from 1M to 19K, verify gain increases
    let dk = DkPreamp::new(88200.0);

    dk.set_ldr_resistance(1e6);
    let out_lo = run_sine(&dk, 1000.0, 0.01, 0.05);
    let gain_lo = rms(&out_lo);

    dk.set_ldr_resistance(19e3);
    let out_hi = run_sine(&dk, 1000.0, 0.01, 0.05);
    let gain_hi = rms(&out_hi);

    let range_db = 20.0 * (gain_hi / gain_lo).log10();
    assert_relative_eq!(range_db, 6.1, epsilon = 1.0);
}

#[test]
fn test_no_dc_in_output() {
    // DC blocker should remove quiescent offset
    let dk = DkPreamp::new(88200.0);
    dk.set_ldr_resistance(1e6);

    // Run silence for 100ms, check output mean is near zero
    let output: Vec<f64> = (0..8820).map(|_| dk.process_sample(0.0)).collect();
    let dc = output[4410..].iter().sum::<f64>() / (8820.0 - 4410.0);
    assert!(dc.abs() < 1e-4, "DC offset = {}, expected ~0", dc);
}

#[test]
fn test_nr_convergence_under_signal() {
    // Run a moderate-level signal, verify NR converges every sample.
    // DkPreamp should expose iteration count diagnostics.
    let dk = DkPreamp::new(88200.0);
    dk.set_ldr_resistance(1e6);

    let mut max_iters = 0;
    for i in 0..88200 {
        let input = 0.05 * (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 88200.0).sin();
        let (_out, iters) = dk.process_sample_with_diagnostics(input);
        max_iters = max_iters.max(iters);
    }

    assert!(max_iters <= 8, "NR took {} iterations (expected <= 8)", max_iters);
}
```

**Failure diagnosis:** If these fail but Layer 4 passes, the bug is in the per-sample algorithm — trapezoidal state update, NR iteration loop, history term computation, or DC blocker. The circuit model and matrices are correct; the time-stepping is wrong.


## Test Execution Order

During development, run in layer order:

1. `cargo test dk_preamp::test_g_matrix` — did I stamp the matrices right?
2. `cargo test dk_preamp::test_s_is_inverse` — does the inversion work?
3. `cargo test dk_preamp::test_dc_operating_point` — does the NR find the right bias?
4. `cargo test dk_preamp::test_bandwidth` — does the coupled system have correct GBW?
5. `cargo test dk_preamp::test_sine` — does the time-domain implementation work?

If Layer N fails, don't bother with Layer N+1 — the bug is localized to Layer N's domain.


## Comparison Test: DK vs EbersMoll

One special test that runs both models and compares:

```rust
#[test]
fn test_dk_vs_ebers_moll_midband_gain() {
    // Both models should agree on midband gain (where the independent-stage
    // approximation is valid). They should disagree on bandwidth.
    let dk = DkPreamp::new(88200.0);
    let em = EbersMollPreamp::new(88200.0);

    for model in [&dk as &dyn PreampModel, &em as &dyn PreampModel] {
        model.set_ldr_resistance(1e6);
    }

    let dk_out = run_sine_model(&dk, 1000.0, 0.01, 0.05);
    let em_out = run_sine_model(&em, 1000.0, 0.01, 0.05);

    let dk_gain = 20.0 * (rms(&dk_out) / 0.01).log10();
    let em_gain = 20.0 * (rms(&em_out) / 0.01).log10();

    // Should agree within 1 dB at midband
    assert!((dk_gain - em_gain).abs() < 1.0,
        "DK ({:.1} dB) and EM ({:.1} dB) should agree at midband", dk_gain, em_gain);
}
```

This confirms the DK model is a refinement of the existing model, not a departure from it. Agreement at midband + disagreement at high frequencies = the C-3/C-4 coupling is the only meaningful difference.
