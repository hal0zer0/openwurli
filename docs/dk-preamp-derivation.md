# DK Method Preamp Derivation

> **See also:** [Preamp Circuit](preamp-circuit.md) (circuit analysis and component values), [DK Preamp Testing](dk-preamp-testing.md) (validation test pyramid)

## 1. Circuit Overview

Two-stage direct-coupled NPN CE amplifier (TR-1/TR-2, 2N5089) with:
- **Inner feedback**: C-3 (100pF) Miller cap on Stage 1, C-4 (100pF) Miller cap on Stage 2
- **Outer feedback**: R-10 (56K) from output through Ce1 (4.7µF) to TR-1 emitter
- **Tremolo**: LDR shunts feedback junction to ground (variable 19K–1M)

## 2. Node Numbering

| Idx | Node     | Description                        |
|-----|----------|------------------------------------|
| 0   | base1    | TR-1 base                          |
| 1   | emit1    | TR-1 emitter                       |
| 2   | coll1    | TR-1 collector = TR-2 base         |
| 3   | emit2    | TR-2 emitter (Ce2 top)             |
| 4   | emit2b   | Re2a/Re2b junction (Ce2 bottom)    |
| 5   | coll2    | TR-2 collector                     |
| 6   | out      | Output (after R-9)                 |
| 7   | fb       | Feedback junction (tremolo shunt)  |

Vcc = 15V and GND = 0V are fixed references (not state nodes).

## 3. Capacitor State Variables

| Cap | Value   | Nodes  | Role                       |
|-----|---------|--------|----------------------------|
| C3  | 100pF   | 2 ↔ 0 | Miller feedback, Stage 1   |
| C4  | 100pF   | 5 ↔ 2 | Miller feedback, Stage 2   |
| Ce1 | 4.7µF   | 1 ↔ 7 | Outer feedback coupling    |
| Ce2 | 22µF    | 3 ↔ 4 | Stage 2 emitter bypass     |

Cin (0.022µF) in series with R-1 (22K) is modeled as a bilinear companion element
(see Section 8.1). This avoids adding a 9th node while correctly blocking DC and
loading the base with R-1's impedance at audio frequencies.

## 4. Conductance Matrix G (8×8)

Each resistor R connecting nodes i and j stamps:
```
G[i,i] += 1/R
G[j,j] += 1/R
G[i,j] -= 1/R
G[j,i] -= 1/R
```

Resistors to Vcc stamp only the diagonal (the Vcc current becomes a DC source in **w**):
```
G[i,i] += 1/R     // conductance to Vcc
w[i]   += Vcc/R    // Norton equivalent current source
```

### Component stamps:

| Component | Value | Nodes      | G stamp                              | w stamp      |
|-----------|-------|------------|--------------------------------------|--------------|
| R-1+Cin   | 22K+22nF | ext→0  | G[0,0] += g_cin (companion, see §8.1) |              |
| R-2       | 2M    | 0 ↔ Vcc   | G[0,0] += 1/2M                       | w[0] += 15/2M |
| R-3       | 470K  | 0 ↔ GND   | G[0,0] += 1/470K                     |              |
| Re1       | 33K   | 1 ↔ GND   | G[1,1] += 1/33K                      |              |
| Rc1       | 150K  | 2 ↔ Vcc   | G[2,2] += 1/150K                     | w[2] += 15/150K |
| Re2a      | 270   | 3 ↔ 4     | G[3,3]+=1/270, G[4,4]+=1/270, G[3,4]-=1/270, G[4,3]-=1/270 | |
| Re2b      | 820   | 4 ↔ GND   | G[4,4] += 1/820                      |              |
| Rc2       | 1.8K  | 5 ↔ Vcc   | G[5,5] += 1/1.8K                     | w[5] += 15/1.8K |
| R-9       | 6.8K  | 5 ↔ 6     | G[5,5]+=1/6.8K, G[6,6]+=1/6.8K, G[5,6]-=1/6.8K, G[6,5]-=1/6.8K | |
| R-10      | 56K   | 6 ↔ 7     | G[6,6]+=1/56K, G[7,7]+=1/56K, G[6,7]-=1/56K, G[7,6]-=1/56K | |
| Ce1       | 4.7µF | 1 ↔ 7     | (capacitor — stamped in C, not G)    |              |
| R_ldr     | var.  | 7 ↔ GND   | G[7,7] += 1/R_ldr                    |              |

**Note:** R_ldr is the only variable element. All other G entries are constant.

## 5. Capacitance Matrix C (8×8)

Each capacitor C connecting nodes i and j stamps:
```
C[i,i] += C
C[j,j] += C
C[i,j] -= C
C[j,i] -= C
```

| Cap | Value   | Stamp                            |
|-----|---------|----------------------------------|
| C3  | 100pF   | C[2,2]+=, C[0,0]+=, C[2,0]-=, C[0,2]-= |
| C4  | 100pF   | C[5,5]+=, C[2,2]+=, C[5,2]-=, C[2,5]-= |
| Ce1 | 4.7µF   | C[1,1]+=, C[7,7]+=, C[1,7]-=, C[7,1]-= |
| Ce2 | 22µF    | C[3,3]+=, C[4,4]+=, C[3,4]-=, C[4,3]-= |

## 6. Nonlinear Elements

### BJT model (forward-active, beta → ∞)

```
Ic = Is * (exp(Vbe / Vt) - 1)
```

Is = 3.03e-14 A (2N5089), Vt = 0.026V (thermal voltage at 25°C).

Beta = 1434 → base current = Ic/1434 = 0.07% of Ic → negligible.

### N_v matrix (2×8) — extracts Vbe from node voltages

```
N_v[0,:] = [+1, -1,  0,  0,  0,  0,  0,  0]   // Vbe1 = v[0] - v[1]
N_v[1,:] = [ 0,  0, +1, -1,  0,  0,  0,  0]   // Vbe2 = v[2] - v[3]
```

### N_i matrix (8×2) — injects Ic into circuit

NPN: Ic flows *into* collector, *out of* emitter. In MNA, current *entering* a node is positive. So Ic *enters* emitter (positive) and *leaves* collector (negative):

```
N_i[:,0] = [ 0, +1, -1,  0,  0,  0,  0,  0]^T   // TR-1: +emit1, -coll1
N_i[:,1] = [ 0,  0,  0, +1,  0, -1,  0,  0]^T   // TR-2: +emit2, -coll2
```

## 7. DC Source Vector w

```
w[0] = Vcc / R2 = 15 / 2M   = 7.5µA
w[2] = Vcc / Rc1 = 15 / 150K = 100µA
w[5] = Vcc / Rc2 = 15 / 1.8K = 8.333mA
```

All other entries zero.

## 8. MNA System

Continuous-time (internal nodes only):
```
C · dv/dt + G · v = I_cin(t) · e_0 + N_i · i_NL(N_v · v) + w
```

Where `I_cin(t)` is the current from the series Cin-R1 network into node 0 (base1),
and `e_0 = [1, 0, ..., 0]^T`.

### 8.1. Cin-R1 Companion Model

The series R-C input coupling (R1=22K, Cin=0.022µF) has admittance:
```
Y(s) = sCin / (1 + sR1Cin)
```

Bilinear discretization (s → 2/T · (1−z⁻¹)/(1+z⁻¹)) gives a companion element:
```
I_cin[n] = g_cin · (Vin[n] − V0[n]) + J_cin[n−1]
```

Where:
- `α = 2·R1·Cin·fs`
- `g_cin = 2·Cin·fs / (1 + α)` — companion conductance (stamped into G[0,0])
- `c_cin = (1 − α) / (1 + α)` — history coefficient
- `J_cin[n] = −g_cin·(1+c_cin)·(Vin[n]−V0[n]) − c_cin·J_cin[n−1]` — history update

The companion conductance `g_cin` is included in G (and thus in both A and A_neg).
The trapezoidal average of the companion source appears in the per-sample RHS
as `cin_rhs[n] + cin_rhs[n−1]`, where `cin_rhs[n] = g_cin·Vin[n] + J_cin[n−1]`.

## 9. Trapezoidal Discretization

Replace `dv/dt ≈ (2/T)(v[n] - v[n-1]) - dv/dt[n-1]`:

```
(2C/T + G) · v[n] = (2C/T - G) · v[n-1] + (cin_rhs[n] + cin_rhs[n-1])·e_0
                     + N_i·(i_NL[n] + i_NL[n-1]) + 2w
```

Where `cin_rhs[n] = g_cin·Vin[n] + J_cin[n-1]` is the companion source at step n.

Define:
- `A = 2C/T + G` (8×8, constant for fixed R_ldr; G includes g_cin)
- `A_neg = 2C/T - G` (8×8, history weighting; **same G** as A)
- `S = A^{-1}` (precomputed)

**Critical:** Both A and A_neg use the same G matrix (including g_cin).
The `-g_cin·V0[n-1]` in A_neg and the `+cin_rhs[n-1]` in the source together
form the trapezoidal average of the companion element. Omitting g_cin from
A_neg would break the trapezoidal symmetry.

Per-sample:
```
v[n] = S · [A_neg · v[n-1] + (cin_rhs[n] + cin_rhs[n-1])·e_0 + N_i·i_NL[n-1] + 2w]
       + S · N_i · i_NL[n]
```

Let `v_pred = S · rhs_known` (the prediction from history + known sources).

## 10. The 2×2 Nonlinear System

Extract the nonlinear voltages:
```
v_NL = N_v · v[n] = N_v · (v_pred + S · N_i · i_NL[n])
     = N_v · v_pred + (N_v · S · N_i) · i_NL(v_NL)
     = p + K · i_NL(v_NL)
```

Where:
- **p** = N_v · v_pred (2×1, predicted Vbe values)
- **K** = N_v · S · N_i (2×2, the DK kernel)

### K matrix entries (from N_v/N_i sparsity)

N_v row 0: [+1, -1, 0, 0, 0, 0, 0, 0] → picks S[0,:] - S[1,:]
N_v row 1: [0, 0, +1, -1, 0, 0, 0, 0] → picks S[2,:] - S[3,:]

N_i col 0: [0, +1, -1, 0, 0, 0, 0, 0]^T → picks column (S[:,1] - S[:,2])
N_i col 1: [0, 0, 0, +1, 0, -1, 0, 0]^T → picks column (S[:,3] - S[:,5])

```
K[0,0] = S[0,1] - S[0,2] - S[1,1] + S[1,2]
K[0,1] = S[0,3] - S[0,5] - S[1,3] + S[1,5]
K[1,0] = S[2,1] - S[2,2] - S[3,1] + S[3,2]
K[1,1] = S[2,3] - S[2,5] - S[3,3] + S[3,5]
```

### Newton-Raphson on F(v_NL) = 0

Residual: `F(v_NL) = v_NL - p - K · i_NL(v_NL)`

Jacobian:
```
J_F = I_2 - K · diag(gm1, gm2)
```

where `gm_k = dIc/dVbe = (Is/Vt) · exp(Vbe_k / Vt)`.

Update: `v_NL_new = v_NL - J_F^{-1} · F(v_NL)`

2×2 inverse via Cramer's rule:
```
det = J[0,0]·J[1,1] - J[0,1]·J[1,0]
J_inv = (1/det) · [[J[1,1], -J[0,1]], [-J[1,0], J[0,0]]]
```

Cost: 12 multiplies + 1 divide per NR iteration.

## 11. Explicit R_ldr with Per-Sample Sherman-Morrison

### Design rationale

R_ldr is the only time-varying element. The naive approach — stamping R_ldr into G and recomputing S when R_ldr changes — causes a subtle problem: Ce1's trapezoidal companion model stores history that depends on the previous G matrix. If G changes (because R_ldr changed), the Ce1 companion's history source is inconsistent with the new system matrix, producing 5.5 Hz artifacts (the tremolo rate). Keeping R_ldr out of G entirely avoids this.

### Approach: Explicit R_ldr

G_base and S_base are computed **without** R_ldr. The history matrix A_neg_base = (2C/T - G_base) is therefore **constant** — it never changes when R_ldr varies.

Pre-computed Sherman-Morrison projection vectors (one-time at init):
- `s_fb_col = S_base[:, FB]` — column FB (=7) of S_base
- `s_fb_row = S_base[FB, :]` — row FB of S_base
- `s_fb_fb = S_base[FB, FB]` — scalar pivot
- `nv_sfb[k] = N_v[k,:] · s_fb_col` — 2-element vector (projected NL extraction)
- `sfb_ni[k] = s_fb_row · N_i[:,k]` — 2-element vector (projected NL injection)

### Per-sample correction

Each sample, compute the SM correction factor:
```
g_ldr = 1 / R_ldr
alpha = g_ldr / (1 + g_ldr * s_fb_fb)
```

Apply SM correction to v_pred:
```
v_pred_fb = dot(s_fb_row, rhs)              // predicted v[FB] without R_ldr
v_pred[i] -= alpha * s_fb_col[i] * v_pred_fb   // rank-1 correction to all nodes
```

Compute K_eff per-sample from precomputed K_base plus SM correction terms:
```
K_eff[i][j] = K_base[i][j] - alpha * nv_sfb[i] * sfb_ni[j]
```

### Backward step (trapezoidal consistency)

In the history/RHS computation, the R_ldr contribution uses the **previous** sample's conductance to maintain trapezoidal integrator consistency:
```
rhs[FB] -= g_ldr_prev * v[FB]
```

This ensures the trapezoidal rule sees the same G for both the forward and backward terms within each integration step.

### Cost

The per-sample SM overhead is simple scalar and dot-product operations — approximately 30 FLOPs per sample (not per R_ldr change). The benefit is that A_neg_base is constant, eliminating the Ce1 companion model consistency problem entirely.

## 12. DC Initialization

At DC, all capacitor currents are zero (`dv/dt = 0`), and `u = 0`:
```
G · v_dc = N_i · i_NL(N_v · v_dc) + w
```

Solve with NR on the full 8D system (one-time cost at init):
1. Start from estimated quiescent point (TR-1: B=2.45V, E=1.95V, C=4.1V, etc.)
2. Iterate: `v_dc_new = inv(G - N_i · J_NL · N_v) · (N_i · [i_NL - J_NL · v_NL] + w)`
3. Converge to `< 1e-12` residual (typically 5–8 iterations)

Or use the same DK reduction: at DC, `S_dc = inv(G)`, `K_dc = N_v · S_dc · N_i`, and solve the 2×2 system `v_NL = p_dc + K_dc · i_NL(v_NL)` with `p_dc = N_v · S_dc · w`.

Target: TR-1: E=1.95V, B=2.45V, C=4.1V; TR-2: E=3.4V, B=4.1V, C=8.8V.

## 13. BJT Saturation Safety

**Note:** The tanh saturation approach described below was designed but not implemented. The deployed code (`dk_preamp.rs`) uses Vbe clamping to [-1.0, 0.85] to prevent exp() overflow, without explicit Ic saturation limiting.

### Designed (not deployed) tanh limiting:

Soft-clip Ic at physical maximum:
```
Ic_max_1 = (Vcc - 0.2) / Rc1 = 14.8 / 150K = 98.7µA
Ic_max_2 = (Vcc - 0.2) / Rc2 = 14.8 / 1.8K = 8.22mA
```

Using tanh limiting:
```
Ic_clipped = Ic_max · tanh(Ic_raw / Ic_max)
gm_eff = gm_raw · sech²(Ic_raw / Ic_max)     // = gm_raw · (1 - tanh²(...))
```

**Critical (if implemented):** NR Jacobian must use `gm_eff`, not raw `gm`. Using raw gm with clipped Ic causes NR overshoot near saturation.

## 14. Per-Sample Algorithm

```
function process_sample(Vin):
    // 1. Companion source for this step
    cin_rhs_now = g_cin · Vin + J_cin

    // 2. History term (trapezoidal)
    rhs = A_neg · v + (cin_rhs_now + cin_rhs_prev) · e_0
          + N_i · i_nl_prev + 2·w
    rhs[FB] -= g_ldr_prev · v[FB]   // explicit R_ldr backward term (trapezoidal consistency)
    v_pred = S · rhs                 // S = S_base (no R_ldr); SM correction applied after

    // 3. Predicted NL voltages
    p = N_v · v_pred     // [Vbe1_pred, Vbe2_pred]

    // 4. NR solve (up to 6 iterations, warm-started from previous v_nl)
    v_nl = v_nl_prev     // warm start
    for iter in 0..6:
        ic = [bjt_ic(v_nl[0]), bjt_ic(v_nl[1])]
        gm = [bjt_gm(v_nl[0]), bjt_gm(v_nl[1])]
        F = v_nl - p - K · ic
        if |F| < 1e-9: break
        J = I_2 - K · diag(gm)
        det = J[0,0]·J[1,1] - J[0,1]·J[1,0]
        dv = [(J[1,1]·F[0] - J[0,1]·F[1])/det,
              (J[0,0]·F[1] - J[1,0]·F[0])/det]
        v_nl -= dv

    // 5. Update state
    i_nl_new = [bjt_ic(v_nl[0]), bjt_ic(v_nl[1])]
    v = v_pred + S · N_i · i_nl_new
    i_nl_prev = i_nl_new
    v_nl_prev = v_nl

    // 6. Update Cin-R1 companion
    cin_rhs_prev = cin_rhs_now
    dv_cin = Vin - v[0]
    J_cin = -g_cin·(1+c_cin)·dv_cin - c_cin·J_cin

    // 7. Output: v[6] (output node), filtered through 4th-order Bessel HPF at 40 Hz
    //    (Q=0.5219, Q=0.8055) — chosen over Butterworth to eliminate bass onset ringing.
    //    Provides -23 dB at 22 Hz.
    return bessel_hpf(v[6] - v_out_dc)
```

## 15. Computational Cost

| Operation | FLOPs | Frequency |
|-----------|-------|-----------|
| A_neg · v (8×8 · 8) | 120 | per sample |
| S · rhs (8×8 · 8) | 120 | per sample |
| N_v · v_pred (2×8 · 8) | 30 | per sample |
| NR iteration (×3 avg) | 3×40 = 120 | per sample |
| S · N_i · i_nl (8×2 · 2) | 30 | per sample |
| Sherman-Morrison (explicit R_ldr) | ~30 | per sample (scalar ops) |
| **Total** | **~450** | per sample |

At 88.2 kHz (2x oversampled): ~37 MFLOP/s. Negligible on modern CPUs.
