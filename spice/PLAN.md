# Plan: Complete ngspice Netlist for Wurlitzer 200A

## Context

We've resolved ALL component values from the 200A schematic (confirmed at 900-2400 DPI). Before writing any DSP code, we're building a complete SPICE simulation as ground truth — every subcircuit validated against known DC operating points, frequency response, and harmonic behavior. This netlist IS the reference implementation.

ngspice 42 confirmed available. Python `.venv` needs numpy/scipy/matplotlib for post-processing.

## File Structure

```
spice/
  models/
    transistors.lib           # All .MODEL statements (2N5089, 2N5087, MPSA06, MPSA56, TIP35C, TIP36C, 2N2924)
    diodes.lib                # 1N4148 (D-1), 1N4004 (D6 HV rectifier)
    ldr_behavioral.lib        # VTL5C3 behavioral model (ngspice B-source)
  subcircuits/
    preamp.cir                # TR-1/TR-2 preamp with feedback network
    tremolo_osc.cir           # TR-3/TR-4 phase-shift oscillator + LED drive
    hv_supply.cir             # 147V polarizing supply (D6 + 3-stage RC filter)
    power_amp.cir             # Full power amp (TR-7 through TR-13)
    pickup_rc.cir             # Pickup plate equivalent circuit
    volume_control.cir        # 3K audio taper pot
    aux_amp.cir               # TR-15/TR-16 auxiliary output (simplified/estimated)
  testbench/
    tb_preamp_dc.cir          # .op — validate all DC operating points
    tb_preamp_ac.cir          # .ac — Bode plot, closed-loop gain/BW
    tb_preamp_tran.cir        # .tran — single tone, harmonic content
    tb_preamp_transfer.cir    # .tran — slow ramp for static transfer curve
    tb_preamp_harmonic.cir    # .tran + .step — THD vs input level sweep
    tb_tremolo_osc.cir        # .tran — standalone oscillator, verify ~6 Hz
    tb_tremolo_preamp.cir     # .tran — tremolo modulating preamp gain
    tb_hv_supply.cir          # .tran — ripple analysis, DC level
    tb_poweramp_dc.cir        # .op — bias current, DC offset
    tb_poweramp_ac.cir        # .ac — frequency response with feedback
    tb_poweramp_tran.cir      # .tran — clipping behavior at various levels
    tb_full_chain.cir         # .tran — complete pickup->speaker path
  output/                     # Simulation results (.raw, .log, .csv)
  scripts/
    run_all.sh                # Batch-run all testbenches
    run_one.sh                # Run single testbench
    extract_dc_op.py          # Parse .op, compare to schematic values
    extract_freq_resp.py      # Parse .ac, plot Bode diagrams
    extract_harmonics.py      # FFT transient output, compute THD/H2/H3
    extract_transfer.py       # Plot input-output transfer curve
    plot_compare.py           # Compare SPICE vs DSP output (for later)
```

## Implementation Phases

### Phase 1: Transistor Models + Preamp

**Files:** `transistors.lib`, `diodes.lib`, `preamp.cir`, `tb_preamp_dc.cir`, `tb_preamp_ac.cir`, `tb_preamp_tran.cir`, `tb_preamp_transfer.cir`, `tb_preamp_harmonic.cir`

Transistor models sourced from LTspice standard.bjt / ON Semi datasheets:
- `Q2N5089` NPN (Bf~1434, Gummel-Poon) — TR-1, TR-2
- `Q2N2924` NPN (Bf~200) — TR-3, TR-4 (tremolo), historical comparison
- `Q2N5087` PNP (Bf~254) — TR-7, TR-8
- `QMPSA06` NPN — TR-9, TR-10
- `QMPSA56` PNP — TR-12
- `QTIP35C` NPN — TR-13 (output)
- `QTIP36C` PNP — TR-11 (output)

Preamp subcircuit node map:
```
Input coupling (.022uF) -> n_base1 [R-2 to Vcc, R-3 to GND, C20 to GND, D-1]
  -> TR-1 (CE): Rc1=150K, Re1=33K||Ce1=4.7uF, C-3=100pF (Ccb)
  -> n_coll1 = n_base2 (DIRECT COUPLED)
  -> TR-2 (CE): Rc2=1.8K, Re2a=270||Ce2=22uF + Re2b=820, C-4=100pF (Ccb)
  -> R-9 (6.8K) -> n_out
  -> R-10 (56K) -> n_fb -> LG-1 (LDR) -> GND
  Feedback: n_fb connects back to n_base1 region
```

**Critical detail from schematic:** The feedback path is R-10 from output to n_fb, then LG-1 from n_fb to GND. The n_fb node connects to the input side — specifically the junction with R-1/R-2/R-3/C20 (i.e., n_base1). This forms a shunt-feedback topology where R-10+LG-1 act as a voltage divider in the negative feedback loop.

**DC validation targets (+/-10%):**

| Node | Expected |
|------|----------|
| TR-1 Base | 2.45V |
| TR-1 Emitter | 1.95V |
| TR-1 Collector | 4.1V |
| TR-2 Emitter | 3.4V |
| TR-2 Collector | 8.8V |
| Ic(TR-1) | 66-73 uA |
| Ic(TR-2) | 3.3-3.5 mA |

**AC validation targets:**
- Closed-loop gain: ~5.6x (15 dB) at 1 kHz
- Bandwidth (-3dB): ~3.7 kHz
- C20 HPF corner: ~1903 Hz

### Phase 2: Tremolo Oscillator + LDR

**Files:** `tremolo_osc.cir`, `ldr_behavioral.lib`, `tb_tremolo_osc.cir`, `tb_tremolo_preamp.cir`

Phase-shift oscillator caps: **UNKNOWN — must derive.** Formula gives C ~ 0.39 uF for f=6 Hz with R=27K. Start there, adjust to hit 5.5-6 Hz measured range.

LDR model: ngspice behavioral `B` source implementing R = R_dark x (I_led/I_ref)^(-gamma) with asymmetric smoothing (tau_attack=2.5ms, tau_release=30ms). For initial validation, use `.step` sweep of fixed resistance values (50 Ohm to 10M Ohm).

**Validation:** Sustained oscillation at 5.5-6 Hz. Preamp gain modulation 3-12 dB.

### Phase 3: HV Supply + Pickup RC

**Files:** `hv_supply.cir`, `pickup_rc.cir`, `tb_hv_supply.cir`

HV supply chain (fully traced from schematic):
```
AC -> D6 -> 56K -> 0.33uF -> 8.2M(shunt) -> 1M -> 0.33uF -> 22M(shunt) -> 0.33uF -> 1M(R_feed) -> Point 15
```

Pickup modeled as voltage source through R_feed(1M) || R_input(402K) = 287K with C_pickup=240pF. For testbenches, inject signal voltage directly; for accuracy, can use behavioral variable capacitor.

**Validation:** HV DC output ~147V. Ripple in millivolt range. Pickup RC cutoff ~2312 Hz.

### Phase 4: Power Amplifier

**Files:** `power_amp.cir`, `tb_poweramp_dc.cir`, `tb_poweramp_ac.cir`, `tb_poweramp_tran.cir`

Known components: ALL resistors (R-31 through R-38), caps (C-8, C-11, C-12), all transistors.

**Gap:** Exact node-by-node VAS/driver wiring not fully documented — service manual describes stages functionally. Use standard quasi-complementary Class AB topology (PNP diff pair -> NPN VAS -> Vbe multiplier -> complementary drivers -> push-pull output) as template, substituting all known Wurlitzer component values. Missing values (tail current resistor, VAS load) estimated from DC rail voltage and operating point constraints.

**Validation:**
- Quiescent current: 10 mA (5 mV across each 0.47 Ohm)
- DC output offset: < 50 mV
- Clean output: ~20W into 8 Ohm before clipping
- Clipping: symmetric at +/-19-21V

### Phase 5: Volume Control + Aux Amp + Full Chain

**Files:** `volume_control.cir`, `aux_amp.cir`, `tb_full_chain.cir`

Volume pot: 3K audio taper (modeled as resistive divider with log taper).

Aux amp: TR-15/TR-16 direct-coupled with feedback. Component values incomplete — mark as ESTIMATED.

Full chain testbench connects all subcircuits: pickup -> preamp (with tremolo) -> volume -> power amp -> 8 Ohm load. Captures golden reference waveforms.

### Phase 6: Post-Processing Scripts + Reference Data Export

**Files:** All `scripts/*.py`, `run_all.sh`

Install numpy, scipy, matplotlib in `.venv`. Write extraction scripts that parse ngspice `.raw` files and produce CSV reference data + plots.

**Key reference outputs:**
- DC operating point table (all nodes)
- Bode plot CSV (freq, gain_dB, phase_deg) for preamp and power amp
- THD vs input level CSV (Vin, H1, H2, H3, THD%)
- Static transfer curve CSV (Vin, Vout) — asymmetric soft-clip shape
- Transient waveform CSVs at C4(262Hz), A3(220Hz), C6(1047Hz) at multiple levels
- Tremolo envelope waveform

## Known Unknowns

| Component | Status | Strategy |
|-----------|--------|----------|
| Tremolo oscillator caps | Unknown | Derive from formula: ~0.39 uF. Validate against 5.5-6 Hz target |
| Power amp tail resistor | Unknown | Estimate ~10-15K from DC rail and diff pair bias |
| Power amp VAS load | Unknown | Estimate from topology; validate via DC operating point |
| Aux amp component values | Mostly unknown | Simplified model, mark ESTIMATED |
| R-10/LG-1 feedback wiring detail | Confirmed from schematic | R-10 from output, LG-1 to GND, junction feeds back to base1 node |

## Verification

After each phase:
1. Run `ngspice -b` on all testbenches for that phase
2. Compare DC operating points to schematic annotations
3. Compare AC response to calculated values from docs
4. Visual inspection of transient waveforms for expected behavior
5. No phase proceeds until prior phase validates

Final validation: Full chain output compared qualitatively to OldBassMan 200A recordings (harmonic content, frequency balance — not waveform-matched, since we're injecting synthetic signals not real reed vibrations).
