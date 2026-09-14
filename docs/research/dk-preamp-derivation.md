# DK Method Preamp Derivation

> **See also:** [Preamp Circuit](preamp-circuit.md) (circuit analysis and component values), [DK Preamp Testing](../reference/dk-preamp-testing.md) (validation test pyramid)
>
> **Revised 2026-09-13** for the drawn-topology solver. This document describes
> `crates/openwurli-dsp/src/dk_preamp_legacy.rs` as it now ships: a 9-node MNA
> system with a base-current-carrying BJT kernel. Sections 1–13 are current.
> [Section 16](#16-pre-revision-material) preserves the pre-revision material
> that still teaches something, clearly marked as historical.

## 1. Circuit Overview

Two-stage direct-coupled NPN CE amplifier (TR-1/TR-2, 2N5089) with:

- **Inner feedback**: C-3 (100pF) Miller cap on Stage 1, C-4 (100pF) Miller cap on Stage 2
- **Outer feedback**: R-10 (56K) from the R-9/R-10 node through Ce1 (4.7µF) to TR-1's emitter
- **DC feedback**: R-3 (470K) from TR-1's base to the R-7/R-8 junction in TR-2's emitter leg
- **Tremolo**: LDR shunts the feedback junction to ground (variable 1K–1M)
- **Supply**: +14.5 V (service-manual text; the drawing marks +15 V)

The drawn topology inverts the stage split relative to the pre-revision model.
C-7 spans R-7+R-8, so TR-2's emitter is AC-grounded and TR-2 is a high-gain CE
stage; its r_π (~3 kΩ) then loads TR-1's 150 K collector down to ~3.9 kΩ.
Stage 1's voltage gain is ~8, not ~420, and stage 2's is ~137, not ~2.2.

## 2. Node Numbering

| Idx | Node     | Description                                  |
|-----|----------|----------------------------------------------|
| 0   | base1    | TR-1 base                                    |
| 1   | emit1    | TR-1 emitter                                 |
| 2   | coll1    | TR-1 collector = TR-2 base                   |
| 3   | emit2    | TR-2 emitter (Ce2 to ground)                 |
| 4   | emit2b   | R-7/R-8 junction (R-3's DC feedback return)  |
| 5   | coll2    | TR-2 collector                               |
| 6   | node_c6  | C-6 / R-9 / R-10 junction (0 V DC)           |
| 7   | out      | Output, after R-9                            |
| 8   | fb       | Feedback junction (tremolo shunt)            |

Vcc = 14.5 V and GND = 0 V are fixed references, not state nodes.

`node_c6` is the node the revision added. C-6 series-couples TR-2's collector to
it, so it — and `out` and `fb` behind it — sit at **exactly 0 V DC**. No DC
reaches R-10 or the LDR leg. This is the single most consequential structural
change in the revision (see [Section 11](#11-the-pump-situation)).

Note also that **R-9 is outside the feedback loop**: R-10 taps `node_c6`, which
is the collector's AC node, so R-9 is pure output series loss and does not
appear in the loop equation.

## 3. Capacitor State Variables

| Cap | Value   | Nodes   | Role                                      |
|-----|---------|---------|-------------------------------------------|
| C2  | 220pF   | 0 → GND | C-2 at the base (restored in the revision)|
| C3  | 100pF   | 2 ↔ 0   | Miller feedback, Stage 1                  |
| C4  | 100pF   | 5 ↔ 2   | Miller feedback, Stage 2                  |
| Ce1 | 4.7µF   | 1 ↔ 8   | Outer feedback coupling                   |
| Ce2 | 22µF    | 3 → GND | Stage 2 emitter bypass, spans R-7+R-8     |
| C6  | 4.7µF   | 5 ↔ 6   | Output coupling (new)                     |

Two of these stamp to ground only (`C[i,i] += C`), since the ground row and
column are eliminated: C-2 and Ce2.

**Ce2 is not a full bypass in the audio band.** re2 is only ~8.5 Ω, so 22 µF
does not swamp it until ~1 kHz. Stage 2's local gain therefore carries a zero at
6.6 Hz and a pole at 971 Hz, rising from ~4.6 at 20 Hz to ~178 at 10 kHz. This
is real in-band shaping inside the loop and the pre-revision model had no
equivalent.

Cin (0.022µF) in series with the R-1/R-2 input network is modelled as a bilinear
companion element (see [Section 8.1](#81-input-network-companion-a-two-port-reduction)),
which is what keeps the system at 9 nodes rather than 11.

## 4. Conductance Matrix G (9×9)

Each resistor R connecting nodes i and j stamps:

```
G[i,i] += 1/R
G[j,j] += 1/R
G[i,j] -= 1/R
G[j,i] -= 1/R
```

Resistors to Vcc stamp only the diagonal (the Vcc current becomes a DC source in **w**):

```
G[i,i] += 1/R      // conductance to Vcc
w[i]   += Vcc/R    // Norton equivalent current source
```

### Component stamps

| Component | Value | Nodes        | Note                                        |
|-----------|-------|--------------|---------------------------------------------|
| R-3       | 470K  | 0 ↔ 4        | DC feedback — TR-1's ONLY bias path         |
| Re1       | 33K   | 1 → GND      |                                             |
| Rc1       | 150K  | 2 → Vcc      | w[2] += Vcc/Rc1                             |
| Re2a      | 270   | 3 ↔ 4        |                                             |
| Re2b      | 820   | 4 → GND      |                                             |
| Rc2       | 1.8K  | 5 → Vcc      | w[5] += Vcc/Rc2                             |
| R-9       | 6.8K  | 6 ↔ 7        |                                             |
| R-10      | 56K   | 6 ↔ 8        | feedback tapped at the collector node       |
| RLOAD     | 100K  | 7 → GND      | see below                                   |
| R_ldr     | var   | 8 → GND      | NOT stamped — see [Section 10](#10-explicit-r_ldr-with-per-sample-sherman-morrison) |
| Cin/R-1/R-2 | —   | companion    | see [Section 8.1](#81-input-network-companion-a-two-port-reduction) |

**`base1` has no supply term.** R-2 is not a core element in the drawn topology,
so `w[0] == 0` and R-3 is the only DC path to the base. This is not a modelling
convenience — it is structural, and it is the strongest single argument for the
corrected reading: with R-3 returned to ground (the pre-revision reading) and
R-2 on the pickup side of Cin, TR-1 would have **no DC bias source at all**. The
pre-revision topology is not a working circuit.

### RLOAD rationale

The drawn netlist leaves `out` with only R-9 attached. A node with one
connection makes the nodal system singular, so a load is structurally required —
this is not a voicing choice. 100 K is the value the re-baselined `tb_preamp_ac`
bench measures into, so the Rust and SPICE gain figures are like-for-like.

The real chain here is the R-11 25 K trimmer into the 10 K volume pot. Modelling
that properly is separate work; the volume path is deliberately decoupled from
circuit drive (2026-04-26), so the load seen by the preamp is not the user's
volume setting. Sensitivity is low: dropping the load to 2.5 K moves the
R_shunt that yields 14 dB by 1.4%.

## 5. Capacitance Matrix C (9×9)

Same stamping rule as G, with capacitance in place of conductance. Grounded
capacitors (C-2, Ce2) stamp the diagonal only.

## 6. Nonlinear Elements

### BJT model

```
ef  = exp(Vbe / (NF·Vt))
Icc = Is · (ef − 1)
q2  = (Is/IKF) · ef
qb  = ½ · (1 + √(1 + 4·q2))          // Gummel-Poon normalised base charge
Ic  = Icc / qb
Ib  = Icc/BF + Ise · (exp(Vbe/(NE·Vt)) − 1)
```

with the 2N5089 card: Is = 3.03e-14, NF = 1.005, BF = 1434, Ise = 2.88e-15,
NE = 1.262, IKF = 0.01358, Vt = 0.026. Vbe is clamped to [−1.0, 0.85] to prevent
exp overflow.

Every term is an explicit function of Vbe alone, so the kernel keeps its
dimension and needs no implicit inner solve. The Early effect (VAF/VAR via the
GP `q1` factor) is deliberately **not** modelled: it would make the device a
function of Vbc as well as Vbe, costing the kernel a dimension, and the DC point
already lands inside tolerance without it.

### Why β → ∞ was structurally incompatible

The pre-revision kernel was a bare transconductance — `Ic = Is·(exp(Vbe/Vt) − 1)`
with **no base current at all**. Under the pre-revision topology (R-3 to ground,
R-2 feeding the base from Vcc) that was survivable, because the base was biased
by a resistive divider that does not need base current to work.

Under the drawn topology it is not survivable. R-3 is the only DC path to the
base, and the voltage across R-3 *is* `I_B1 · R-3`. With β = ∞ there is no base
current, so there is no drop across R-3, so `base1 ≡ emit2b` identically — while
the reference DC table puts them 59 mV apart. No amount of parameter tuning
reaches the operating point; the model is structurally unable to express it.

Adding base current is therefore not a refinement, it is a precondition. Two
terms were added, and the effect on worst-node DC error is:

| Model | worst-node error |
|---|---|
| β = ∞ (pre-revision kernel) | 137 mV |
| + base current (BF and Ise/NE) | 27 mV |
| + high injection (IKF via qb) | **2.70 mV** |

The Ise recombination term dominates β at TR-1's 58 µA operating point; the IKF
high-injection term is what drops TR-2's effective β from ~850 to ~700 at its
3.3 mA. Closing the last ~2.7 mV would need the full GP card (q1, plus the
RE/RB/RC parasitics); that is the melange-generated solver's job
(`--features melange-preamp`), not this one.

### Incidence maps

Three sparse maps replace the pre-revision pair. `NV` extracts the controlling
voltages; `NIC` and `NIB` inject collector and base current respectively:

```
NV  = [ (base1, +1), (emit1, −1) ]      // vbe1
      [ (coll1, +1), (emit2, −1) ]      // vbe2   (TR-2's base IS TR-1's collector)

NIC = [ (emit1, +1), (coll1, −1) ]      // Ic leaves the collector, enters the emitter
      [ (emit2, +1), (coll2, −1) ]

NIB = [ (emit1, +1), (base1, −1) ]      // Ib leaves the base, enters the emitter
      [ (emit2, +1), (coll1, −1) ]      // TR-2's base node is coll1
```

## 7. DC Source Vector w

```
w[coll1] = Vcc / Rc1
w[coll2] = Vcc / Rc2
```

All other entries are zero — including `w[base1]`, per Section 4.

## 8. MNA System

```
(G + s·C) · v = w + N_i · i_NL + input
```

### 8.1 Input network companion: a two-port reduction

The drawn input network is a two-port, not a single series branch:

```
source ──R-1 22K── X ──Cin 0.022µF── base1
                   │
              R-2 1MEG
                   │
            +150V line (AC ground)
```

Seen from `base1` this reduces **exactly** — not approximately — to a Thévenin
source behind a series impedance:

```
R_IN_EFF = R-1 ∥ R-2          = 21.526 kΩ
K_IN_DIV = R-2 / (R-1 + R-2)  = 0.978474
v_eff    = input · K_IN_DIV
```

so the existing bilinear companion is reused unchanged with `R_IN_EFF` in place
of R-1 and the input pre-scaled by `K_IN_DIV`:

```
alpha  = 2 · R_IN_EFF · Cin · fs
g_cin  = 2 · Cin · fs / (1 + alpha)
c_cin  = (1 − alpha) / (1 + alpha)
```

Per sample, the companion contributes a conductance `g_cin` at `base1` and a
current source `g_cin·v_eff + j_cin`, with `j_cin` updated from the branch
voltage. This is what keeps R-2 out of the core matrices: the polarizing feed
lives on the pickup side of the coupling cap, exactly as drawn, without costing
two extra nodes.

The DC solve excludes `g_cin` entirely, since Cin blocks DC.

## 9. Trapezoidal Discretization and the Integrator Decision

The system is discretized trapezoidally:

```
A     = 2C/T + G
A_neg = 2C/T − G
S     = A⁻¹
```

**The solver stays trapezoidal, and this is a deliberate decision rather than an
inherited default.** The melange deck for this same topology is
trapezoidal-*unstable* — spectral radius 1.1394, dominant sign −1, i.e. a
Nyquist-marginal z = −1 mode — and is author-pinned to backward Euler in
`spice/melange/wurli-preamp.cir`. That pin governs the codegen path. The deck
header is explicit that the shipping hand solver treats stability separately,
and that BE's HF damping understates the ~16.7 kHz corner.

Two pieces of evidence support keeping trapezoidal here:

1. **No z = −1 mode is observable.** `test_numerics_no_nyquist_mode` excites the
   mode directly — impulse plus R_ldr modulation at the tremolo rate, which is
   what makes the deck's companion matrices time-varying and is the actual
   source of the deck's instability — then measures the Nyquist-bin energy of a
   2 s tail. It sits below 1e-9.
2. **BE would pull HF the wrong way.** The measured HF corner already sits
   inside the acceptance band with trapezoidal; BE's damping would move it down,
   away from the 16.79 kHz target.

The integrator is therefore pinned by a test, not by a comment.

## 10. Explicit R_ldr with Per-Sample Sherman-Morrison

R_ldr is **not** stamped into G. Instead its current is handled as an explicit
source term, corrected via Sherman-Morrison on a fixed `S_base`.

The reason is a coupling between R_ldr and Ce1's history. If R_ldr lives in G,
changing it changes `A = 2C/T + G`, which desynchronises the forward matrix from
the history stored in Ce1's companion. Ce1's companion conductance is enormous
(2·4.7µF/T ≈ 829 S at 88.2 kHz) and dominates the system, so even a small matrix
change triggers a charge redistribution that swamps the audio signal.

With R_ldr explicit, `S_base` and `A_neg_base` are constant, the Ce1 companion is
always self-consistent, and R_ldr only enters through a scalar correction:

```
sm_k  = g_ldr / (1 + S_base[fb][fb] · g_ldr)
v_pred = v_pred_base − sm_k · v_pred_base[fb] · S_base[:,fb]
```

The same correction applies to both kernels:

```
Kc_eff[i][j] = Kc[i][j] − sm_k · nv_sfb[i] · sfb_nic[j]
Kb_eff[i][j] = Kb[i][j] − sm_k · nv_sfb[i] · sfb_nib[j]
```

`test_l2_sm_gives_correct_s_eff` and `test_l2_k_eff_matches_brute_force` check
both against brute-force matrix inversion across a range of R_ldr.

## 11. The Pump Situation

Under the pre-revision topology the output was DC-coupled through R-10 into the
LDR leg, so modulating R_ldr pumped the operating point — a multi-volt swing at
the tremolo rate with zero audio input, whose harmonics spanned 28–200+ Hz and
overlapped bass fundamentals. No HPF can separate that without cutting bass,
which is why shadow subtraction exists.

**C-6 eliminates the pump at source.** `node_c6`, `out` and `fb` sit at 0 V DC,
so no DC flows into R-10 or the LDR, and modulating R_ldr no longer moves TR-1's
bias. Two guards pin this:

- `test_dc_output_node_is_zero` — the structural half: all three nodes must be at
  0 V DC behind C-6.
- `test_acceptance_pump_guard_c6` — the dynamic half: cycling the LDR
  19K↔1M at 5.63 Hz with zero input must move emit1/coll1 by less than 1 mV pp.
  It probes those nodes **directly**, so the shadow subtraction cannot mask it.

The SPICE mirror is `tb_pump_emit`, which is now a C-6 regression guard: any
nonzero pump there means C-6 went missing again.

### Shadow-pump status: kept, retirement pending

`DkPreamp` still maintains two solver states — `main` (audio) and `shadow` (zero
input, same R_ldr) — and subtracts. With C-6 present the pump it cancels is ~0 by
construction, so the machinery is now close to redundant and costs roughly 50% of
the solver's CPU.

It is kept deliberately. Removing it is an audible behaviour change and a
separate decision from the topology fix; it is flagged in the code as the top
follow-up for that file and belongs to the Phase-3 voicing pass.

## 12. DC Initialization

`full_dc_solve` inverts `G + R_ldr` (capacitors open), forms both DC kernels, and
runs Newton on the 2×2 system in `[vbe1, vbe2]` with a step clamp of 2·Vt. The
converged currents are then injected to recover the node voltages.

The `reset()` contract is load-bearing and easy to get wrong: **rebuild from
`default()` plus `set_sample_rate`, never from a stored operating point.** For
the oscillator this is the difference between starting and not starting; for the
preamp it keeps main and shadow bit-identical at t=0.

## 13. Validation

All figures below are measured by tests in `dk_preamp_legacy.rs` unless noted.

### DC operating point — `test_dc_operating_point`

Against the independent cross-check deck's table for the revised netlist,
BF-1434 card, Vcc 14.5 V, tolerance ±5 mV:

| Node   | target | error    |
|--------|--------|----------|
| base1  | 2.651  | +0.59 mV |
| emit1  | 2.093  | −2.25 mV |
| coll1  | 4.270  | −0.20 mV |
| emit2a | 3.602  | −1.93 mV |
| emit2b | 2.710  | −1.71 mV |
| coll2  | 8.561  | +2.70 mV |

Sample-rate independent (identical at 88.2 and 176.4 kHz). The test also asserts
the R-3 drop is in 30–90 mV, which fails immediately if base current ever
vanishes from the kernel again.

SPICE mirror: `tb_preamp_dc`.

### AC gain — `test_acceptance_ac_gain_1khz`

1 kHz, R_ldr = 12 K, measured at `out` into RLOAD: **15.56 dB** against the
re-baselined bench figure of 15.54 ±0.3 dB. The continuous-time linearised model
in the same file gives 15.65 dB.

SPICE mirror: `tb_preamp_ac`.

### Bandwidth — `test_acceptance_hf_bandwidth`

Target 16.8 kHz ±15%. Linearised model 17.96 kHz; trapezoidal solver at 88.2 kHz
~15.2 kHz. The gap is bilinear frequency warping, not topology: the bilinear map
sends an analog 17.96 kHz to `(fs/π)·atan(π·f/fs)` = 16.0 kHz at fs = 88.2 kHz.
Both sit inside the band.

### Stage split and closed-loop structure

| Quantity | Drawn | Pre-revision |
|---|---|---|
| A1 (base1→coll1) at 1 kHz | 8.0 (18.0 dB) | 420 (52.5 dB) |
| A2 (coll1→coll2) at 1 kHz | 137 (42.8 dB) | 2.17 (6.7 dB) |
| A_open at 1 kHz | 1095 (60.8 dB) | 912 (59.2 dB) |
| loop gain at the 14 dB point | 196 (45.8 dB) | ~180 |

Closed-loop gain is within 0.94 dB of the ideal feedback limit `1/β_fb`, i.e.
the R-10/Ce1 divider genuinely sets the gain and the transistors are along for
the ride. That is good news for modelling: the closed-loop gain is insensitive
to exactly the parameters that cannot be pinned (β2, Early, gm1).

The closed-form check that matches the full nodal solve:

```
k       = R_sh / (R_sh + R10)
R_th    = R10 ∥ R_sh
R_E,eff = Re1 ∥ R_th
β_fb    = k · Re1 / (Re1 + R_th)

G = A / (1 + gm1·R_E,eff + A·β_fb)
```

giving 13.96 dB against the nodal solver's 14.09 dB at R_sh = 19.4 kΩ.

### A caveat that has not been discharged

Stage 1's headroom asymmetry (1.96 V toward saturation vs 10.52 V toward cutoff,
5.38:1) survives in the DC numbers but is **unreachable**: closed-loop, TR-1's
collector swings 0.0073× TR-2's, so when TR-2 reaches its 4.99 V saturation
limit TR-1 has used 36 mV of its 1.96 V budget — a 54× margin. The conclusion
holds open-loop too (A2 = 137 either way). TR-2 clips first and is near-symmetric
(1.21:1).

This follows structurally from the confirmed A1/A2 inversion but has **not been
directly probed**. It is the one conclusion here resting on inference rather than
a measurement, and it is exactly the claim most likely to be cited as licence to
remove clipping behaviour from the DSP. Do not act on it without a measurement.

## 14. Per-Sample Algorithm

```
1.  rhs  = A_neg_base · v
2.  rhs[fb] −= g_ldr_prev · v[fb]                    // explicit backward term
3.  v_eff = input · K_IN_DIV                          // two-port Thévenin
    rhs[base1] += g_cin·v_eff + j_cin + cin_rhs_prev
4.  for each device j: rhs[NIC[j]] += i_c[j] ; rhs[NIB[j]] += i_b[j]
5.  rhs += 2w
6.  v_pred_base = S_base · rhs
7.  sm_k = g_ldr/(1 + S_fb_fb·g_ldr) ; apply SM correction to v_pred
8.  p = NV · v_pred
9.  Newton (≤6 iterations) on [vbe1, vbe2] with Kc_eff/Kb_eff
10. recover i_c, i_b at the converged Vbe
11. v = v_pred + S·(NIC·i_c + NIB·i_b) − sm_k·(…)·S_base[:,fb]
12. update the Cin companion and device state
13. return v[out]
```

Step 12's ordering matters: `g_ldr_prev` is advanced only after **both** the main
and shadow steps have consumed it.

## 15. Computational Cost

Measured, not estimated — `preamp_perf_probe`, release build, 4 s of audio:

| Solver | % of realtime |
|---|---|
| legacy 9-node (shipping) | 5.2% |
| melange-generated (`--features melange-preamp`) | 71.0% |

The legacy figure was 4.4% pre-revision; the rise is the added base-current and
high-injection terms (one extra `exp` per device per NR iteration, plus the
second kernel). The melange figure was 52.7%; its N grew 12→13 and M grew 3→5,
because the Gummel-Poon card is not pure Ebers-Moll so neither BJT reduces to
1-D any more.

## 16. Pre-Revision Material

Kept because the reasoning still teaches, not because the numbers are live.
**Every figure in this section describes a topology that is not the drawn
circuit.** Do not cite them.

### 16.1 Why the Bessel HPF was never deployed

Before shadow subtraction, a 4th-order Bessel HPF at 40 Hz (Q = 0.5219 and
0.8055) was designed to remove the tremolo pump. It was never deployed: any HPF
steep enough to reach the pump's low harmonics also cut bass fundamentals, and
the pump's harmonics ran up past 200 Hz. The lesson generalises — a
frequency-domain fix for an artifact that overlaps the signal band is usually the
wrong shape of solution, and a state-domain cancellation (shadow) or a
circuit-level fix (C-6, as it turned out) is the right one.

### 16.2 Designed-but-not-deployed tanh saturation limiting

A tanh soft-limit on the BJT collector swing was specified and never shipped,
on the grounds that the circuit topology and the NR solver constrain the
operating point without help. That remains true in the drawn topology, and the
absence of `satLimit`/`cutoffLimit` constants in the shipping solver is
deliberate — see the caveat in Section 13.

### 16.3 Pre-revision node set and gain structure

The pre-revision solver was 8-node: no `node_c6`, R-9 running `coll2 → out` and
R-10 `out → fb`, Ce2 bridging `emit2 ↔ emit2b`, R-2 stamped 2 MEG from `base1`
to Vcc, R-3 from `base1` to ground, and Vcc = 15 V. Its stage split was
A1 ≈ 420 / A2 ≈ 2.17, and its bandwidth story was "~15.5 kHz, nearly independent
of R_ldr" from a dominant C-3 Miller pole at ~23 Hz.

That gain structure is what made the pre-revision "Stage 1 asymmetric clipping is
the preamp's H2 source" account plausible. It does not survive the corrected
topology, and the measurement record already agreed with the corrected view
before the topology was re-read: the bark audit had long attributed >98% of H2 at
normal dynamics to the pickup's 1/(1−y) nonlinearity, with the preamp
transparent at millivolt levels.
