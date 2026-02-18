# SPICE ↔ Rust Translation Reference

**Purpose:** Field manual for debugging mismatches between ngspice circuit simulations and Rust DSP implementations. Structured, explicit, cross-referenced.

---

## 1. Component Mapping Table

Each row maps a circuit block from schematic → SPICE netlist → Rust implementation.

| Circuit Block | Schematic Ref | SPICE File(s) | Rust File(s) | Key Equations | Notes |
|---|---|---|---|---|---|
| **Electrostatic pickup** | Pickup plate, reed | `spice/subcircuits/pickup.cir`, `spice/testbench/tb_pickup.cir` | `pickup.rs` | `C(y) = C₀/(1-y)`, `v = dC/dt · V_pol` | 1/(1-y) is primary bark source |
| **Preamp Stage 1 (TR-1)** | TR-1 (2N5089), Rc1=150K, Re1=33K | `spice/subcircuits/preamp.cir`, `spice/testbench/preamp_emitter_fb.cir` | `dk_preamp.rs` (DK), `bjt_stage.rs` + `preamp.rs` (EM) | Ebers-Moll: `Ic = Is(e^(Vbe/Vt) - 1)` | Headroom: 2.05V sat / 10.9V cutoff (5.3:1) |
| **Preamp Stage 2 (TR-2)** | TR-2 (2N5089), Rc2=1.8K, Re2a=270Ω, Re2b=820Ω | Same as above | Same as above | Direct-coupled from TR-1 collector | Headroom: 5.3V sat / 6.2V cutoff (1.17:1) |
| **Miller caps (C-3, C-4)** | C-3=100pF (TR-1), C-4=100pF (TR-2) | `spice/testbench/tb_variable_gbw.cir` | `dk_preamp.rs` (inner loop) | Trapezoidal companion: `i = g_c·v + J_prev` | Forms inner BW-controlling feedback loop |
| **Input network (Cin, R-1)** | Cin=0.022µF, R-1=22K | `spice/testbench/tb_dk_ac_extract.cir` | `dk_preamp.rs` (bilinear companion, §8.1) | `g_cin = 2C/T`, history source `cin_rhs_prev` | See Cin-R1 bug in §4 below |
| **Feedback network** | R-10=56K, Ce1=4.7µF, Re1=33K | `spice/testbench/preamp_emitter_fb.cir` | `dk_preamp.rs` (outer loop) | Series-series negative feedback | Ce1 is DC-blocking short at audio freq |
| **LDR / Tremolo** | LG-1 (CdS LDR), cable-routed | `spice/subcircuits/tremolo_osc.cir`, `spice/testbench/tb_tremolo_osc.cir` | `tremolo.rs` | Rldr shunts fb_junction to GND | Modulates closed-loop gain, not volume |
| **Power amplifier** | TR-7/TR-8 (142128), Class AB | `spice/subcircuits/power_amp.cir`, `spice/testbench/tb_power_amp.cir` | `power_amp.rs` | Crossover distortion at ~3mV | Transparent at normal signal levels |
| **Speaker** | 4×8" oval, 16Ω ceramic | (not modeled in SPICE) | `speaker.rs` | Hammerstein polynomial: a2=0.2, a3=0.6 | Variable HPF/LPF + nonlinearity + tanh Xmax |
| **Full chain** | End-to-end | `spice/testbench/tb_full_chain.cir` | `voice.rs` → plugin `lib.rs` | — | Integration testbench |

### SPICE Subcircuits
- `spice/subcircuits/preamp.cir` — Two-stage preamp with feedback
- `spice/subcircuits/tremolo_osc.cir` — Twin-T oscillator
- `spice/subcircuits/pickup.cir` — Electrostatic pickup behavioral model
- `spice/subcircuits/power_amp.cir` — Class AB output stage

### Key SPICE Testbenches
- `tb_dk_validation.cir` — DK method validation (gain, BW, harmonics)
- `tb_dk_ac_extract.cir` — AC transfer function extraction
- `tb_variable_gbw.cir` — GBW scaling with Rldr
- `tb_harmonic_audit.cir` — Per-stage harmonic attribution
- `tb_real_thd.cir` — THD measurement
- `tb_full_chain.cir` — Full signal chain
- `tb_pickup.cir` — Pickup model isolation
- `tb_power_amp.cir` — Power amp isolation
- `tb_tremolo_osc.cir` — Oscillator frequency/waveform

---

## 2. DK Preamp Node Map

The DK method uses an 8-node MNA system. This is the critical mapping between SPICE nodes and
the Rust `dk_preamp.rs` matrix indices.

| MNA Index | Circuit Node | SPICE Name | Description |
|---|---|---|---|
| 0 | v_in_internal | After Cin-R1 | Internal node past input coupling cap |
| 1 | v_b1 | TR-1 base | Base of first transistor |
| 2 | v_c1 = v_b2 | TR-1 collector / TR-2 base | Direct-coupled interstage |
| 3 | v_e1 | TR-1 emitter | Feedback injection point |
| 4 | v_c2 | TR-2 collector | Output node |
| 5 | v_e2 | TR-2 emitter | Second stage emitter |
| 6 | v_fb | Feedback junction | Where R-10, Ce1, and Rldr meet |
| 7 | v_out | After D-1/R-9 | Output after clamp diode |

**Critical cross-check:** When comparing SPICE `.print` statements against Rust node indices,
verify that node numbering matches. SPICE uses named nodes; Rust uses integer indices.
Off-by-one errors here cascade through the entire MNA system.

---

## 3. Discretization Quick Reference

| Method | Formula | Rust Pattern | When Used |
|---|---|---|---|
| **Trapezoidal** | `x[n+1] = x[n] + (T/2)(f[n] + f[n+1])` | `g_c = 2C/T; J = g_c*v_prev + i_prev` | C-3, C-4 Miller caps in DK preamp |
| **Bilinear (pre-discretized)** | `H(z) = H(s)\|_{s=(2/T)(z-1)/(z+1)}` | Companion conductance + history source | Cin-R1 in DK preamp (§8.1) |
| **ZDF (zero-delay feedback)** | Implicit trapezoidal for filters | `OnePoleHpf`, `OnePoleLpf` in `filters.rs` | Pickup HPF, speaker filters |
| **Forward Euler** | `x[n+1] = x[n] + T·f[n]` | **DO NOT USE** — unstable at high freq | — |

### Trapezoidal Companion Model (capacitor)
```
SPICE:  i_C = C · dv/dt
Rust:   i_C[n] = g_c · v[n] + J[n-1]
        where g_c = 2C/T
              J[n-1] = g_c · v[n-1] + i_C[n-1]
```
The companion conductance `g_c` goes into the G matrix diagonal.
The history source `J[n-1]` goes into the w (RHS) vector.

---

## 4. Bug Archaeology — Past Translation Errors

### Bug 1: Cin-R1 Companion Conductance (Feb 2026)
**Symptom:** High-frequency response wrong.
**Root cause:** `A_neg` matrix was built from `G_dc` (excluding `g_cin` companion conductance),
but `A` matrix included it. Broke trapezoidal symmetry — the forward and backward matrices
must use the SAME G.
**Fix:** Both `A` and `A_neg` use the same G (including `g_cin`). Added `cin_rhs_prev` field
to store previous step's `g_cin*Vin + J_cin` for the trapezoidal average.
**Lesson:** When mixing pre-discretized companions with trapezoidal MNA, the companion conductance
IS part of G. The history source completes the average — it's not double-counting.

### Bug 2: C20 Ghost Component (Feb 2026)
**Symptom:** Spurious HPF at 1903 Hz in signal chain.
**Root cause:** C20 (220pF) exists on the 206A board schematic, NOT the 200A. Was incorrectly
included as an HPF in the signal chain.
**Fix:** Removed C20 from 200A signal path entirely.
**Lesson:** Always verify which schematic revision a component appears on. The verified 200A
schematic (`docs/verified_wurlitzer_200A_series_schematic.pdf`) is the ONLY reference.

### Bug 3: Constant-GBW Assumption (Feb 2026)
**Symptom:** Trem-bright bandwidth was 5.2 kHz (should be ~10 kHz).
**Root cause:** Decoupled (EbersMoll) model assumed constant GBW like a simple op-amp. But the
200A preamp has TWO nested feedback loops — inner (C-3/C-4 Miller) controls BW, outer
(R-10/Ce1/Rldr) controls gain. GBW scales with gain, not constant.
**Fix:** DK method models both loops as a coupled 8-node MNA system, capturing the interaction.
**Lesson:** Don't assume op-amp rules for discrete BJT circuits. Simulate first, simplify later.

---

## 5. Common Translation Error Patterns

This is the checklist. For each circuit element in SPICE, verify against Rust:

### 5.1 MNA Stamp Errors
- [ ] **Sign of off-diagonal entries:** In MNA, conductance between nodes i and j appears as
  `+g` on diagonal (i,i) and (j,j), and `-g` on off-diagonal (i,j) and (j,i). Missing a
  negative sign is the #1 most common MNA bug.
- [ ] **Ground references:** SPICE node 0 is ground. In the Rust 8-node system, ground is
  implicit (not a matrix row). Every component connected to ground only stamps the diagonal.
  A component between nodes i and ground stamps ONLY `G[i][i]`, not `G[i][0]`.
- [ ] **Transconductance direction:** `gm` in a BJT stamps `+gm` at (collector, base) and
  `-gm` at (emitter, base) — or the transpose, depending on convention. Verify the Ebers-Moll
  linearization matches the SPICE model's sign convention.

### 5.2 Discretization Errors
- [ ] **Companion conductance in G:** When using trapezoidal discretization, the companion
  conductance (2C/T for caps, T/(2L) for inductors) MUST be included in G. It's not separate.
- [ ] **History term updates:** Every companion model needs its history source updated AFTER
  the solve, BEFORE the next timestep. Missing this = Forward Euler (wrong, potentially unstable).
- [ ] **Bilinear vs trapezoidal equivalence:** For linear elements, bilinear transform and
  trapezoidal rule give identical results. But the CODE patterns look different. If mixing both
  in one system, verify they produce the same companion equations.

### 5.3 Value/Unit Errors
- [ ] **Hz vs rad/s:** `ω = 2πf`. SPICE `.ac` sweeps use Hz. Rust code may use either.
  A factor-of-2π error shifts every frequency-dependent result.
- [ ] **Conductance vs resistance:** MNA works in conductances (Siemens). SPICE netlists specify
  resistances (Ohms). `g = 1/R`. Missing this inversion is a quadratic error (g vs 1/g).
- [ ] **Capacitance units:** SPICE uses Farads. Schematic says "MFD" (microfarads) and "pF".
  `4.7 MFD = 4.7e-6 F`. `100pF = 100e-12 F`. Triple-check unit prefixes.
- [ ] **Temperature:** SPICE default is 27°C (300.15K). Vt = kT/q ≈ 25.85mV at 27°C.
  If Rust uses `Vt = 0.026` (26mV), that's a 0.6% error — small but it compounds.

### 5.4 Topology Errors
- [ ] **Missing components:** Every SPICE element must have a Rust counterpart. Grep the SPICE
  netlist for all R/C/Q/D elements and verify each appears in the Rust code.
- [ ] **Floating nodes:** A node connected in SPICE but missing a connection in Rust creates
  a singular matrix. The NR solver will diverge or produce garbage.
- [ ] **Coupling paths:** Direct-coupled stages (TR-1 collector → TR-2 base) must share the
  same node in the MNA system. If they're separate nodes with a wire between them, there's
  an extra equation but no extra physics — just wasted computation or, worse, a bug.

### 5.5 Nonlinear Solver Errors
- [ ] **Jacobian consistency:** The NR Jacobian must be the exact derivative of the residual
  function. An approximate Jacobian may converge but to the WRONG answer. Verify analytically.
- [ ] **Initial guess:** SPICE uses `.nodeset` or ramp-up for DC operating point. Rust code
  needs equivalent initialization. Starting from zero may converge to wrong operating point
  or not converge at all.
- [ ] **Convergence criterion:** SPICE uses both voltage and current tolerances (VNTOL, ABSTOL).
  Rust code should check both, not just one.

---

## 6. Verification Methodology

When Rusty Spice is invoked to debug a mismatch, follow this protocol:

### Step 1: Reproduce in SPICE
Run the relevant testbench and extract the exact SPICE result. Record node voltages, currents,
and any `.measure` outputs. Save to a text file for comparison.

### Step 2: Reproduce in Rust
Run `preamp-bench` or the relevant test with matching input conditions (same frequency, amplitude,
Rldr, etc.). Record the Rust output.

### Step 3: Narrow the Delta
If the outputs disagree:
1. **Bisect the signal chain.** Test each stage in isolation until you find the divergent block.
2. **Compare DC operating points.** If DC is wrong, AC will be wrong too. Fix DC first.
3. **Compare small-signal gain at one frequency.** This isolates linear errors from nonlinear ones.
4. **Sweep frequency.** The shape of the mismatch (low-freq, high-freq, resonance) points to the cause.

### Step 4: Trace to Root Cause
Use the error patterns in §5 as a diagnostic checklist. The most common causes, in order:
1. Sign error in MNA stamp
2. Missing/wrong companion conductance
3. Node numbering mismatch
4. Unit conversion error
5. Missing component or connection

### Step 5: Verify Fix
After fixing, re-run both SPICE and Rust. The delta should be within tolerance at ALL frequencies,
not just the one you were debugging. Check the DK preamp test pyramid (all 5 layers) if the
fix touches the preamp.

---

## 7. Key File Paths

### Rust DSP Source
| File | Purpose |
|---|---|
| `crates/openwurli-dsp/src/dk_preamp.rs` | DK method preamp (8-node MNA, trapezoidal, NR) |
| `crates/openwurli-dsp/src/bjt_stage.rs` | Single BJT CE stage (used by EbersMoll preamp) |
| `crates/openwurli-dsp/src/preamp.rs` | PreampModel trait + EbersMollPreamp |
| `crates/openwurli-dsp/src/pickup.rs` | Electrostatic pickup: 1/(1-y) + HPF |
| `crates/openwurli-dsp/src/power_amp.rs` | Class AB crossover distortion |
| `crates/openwurli-dsp/src/tremolo.rs` | LFO + CdS LDR model |
| `crates/openwurli-dsp/src/speaker.rs` | Hammerstein nonlinearity + HPF/LPF |
| `crates/openwurli-dsp/src/filters.rs` | ZDF one-pole filters, DC blocker, biquad |
| `crates/openwurli-dsp/src/voice.rs` | Voice assembly (reed→hammer→pickup→preamp→...) |

### Documentation
| File | Purpose |
|---|---|
| `docs/dk-preamp-derivation.md` | Full MNA math, trapezoidal discretization, Sherman-Morrison |
| `docs/dk-preamp-testing.md` | Five-layer test pyramid strategy |
| `docs/preamp-circuit.md` | Circuit analysis, DC bias, feedback topology |
| `docs/output-stage.md` | Power amp, tremolo, speaker, volume |
| `docs/pickup-system.md` | Electrostatic pickup physics |
| `docs/signal-chain-architecture.md` | Overall plugin architecture |

### SPICE (key testbenches only)
| File | Purpose |
|---|---|
| `spice/testbench/tb_dk_validation.cir` | DK method validation targets |
| `spice/testbench/tb_dk_ac_extract.cir` | AC transfer function for DK comparison |
| `spice/testbench/tb_variable_gbw.cir` | GBW scaling proof |
| `spice/testbench/tb_full_chain.cir` | End-to-end simulation |
| `spice/testbench/preamp_emitter_fb.cir` | Feedback topology reference |
| `spice/testbench/tb_harmonic_audit.cir` | Per-stage harmonic attribution |
