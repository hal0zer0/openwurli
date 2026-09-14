# Wurlitzer 200A Reed-Bar Preamp — Complete Circuit Reference

Comprehensive technical reference for implementing a digital model of the Wurlitzer 200A reed-bar preamplifier: verified topology and component values, DC bias analysis, AC signal analysis, harmonic generation, tremolo integration, and modeling recommendations.

> **See also:** [DK Preamp Derivation](dk-preamp-derivation.md) (MNA math — ⚠ pre-revision, rewrite pending with the 9-node solver), [DK Preamp Testing](../reference/dk-preamp-testing.md) (test pyramid), [Output Stage](output-stage.md) (tremolo LDR integration)

---

## ⚠ TOPOLOGY REVISION 2026-09-13 — this document reflects the CORRECTED circuit

Pixel-level instrumented re-reads of schematic #203720-S-3 (both circulating scan surfaces) overturned four topology readings that earlier revisions of this document were built on. Every corrected reading below has since been **cross-verified by at least two independent methods** (instrumented drawing read, independent SPICE implementation, independent hand derivation, and a real-instrument DC measurement set). The superseded readings are preserved in §11 so they are never re-introduced.

**Still genuinely open** (flagged inline where they matter):
1. **C-2's return node** — its bottom rail is drawn broken on both surviving scan surfaces (drawing defect). Ground return is the settled working assumption; an emitter return would move the pickup-side corner from ≈900 Hz to ≈1.4 kHz. Needs hardware/board evidence.
2. **The transistor gain grade** — carried as a documented band (§3), a voicing decision, never a fitted point.
3. **The Stage-1 clipping story** (§6.2) — its collapse is *inferred* from the verified stage-gain inversion, not yet directly measured; the DSP's asymmetry constants stay until a direct probe.

---

## Table of Contents

1. [Circuit Overview](#1-circuit-overview)
2. [Complete Schematic with Component Values](#2-complete-schematic-with-component-values)
3. [Transistor Specifications](#3-transistor-specifications)
4. [DC Bias Analysis](#4-dc-bias-analysis)
5. [AC Signal Analysis](#5-ac-signal-analysis)
6. [Harmonic Generation](#6-harmonic-generation)
7. [Tremolo Integration — LDR in Feedback Loop](#7-tremolo-integration--ldr-in-feedback-loop)
8. [Modeling Recommendations](#8-modeling-recommendations)
9. [Implementation Pitfalls](#9-implementation-pitfalls)
10. [Sources](#10-sources)
11. [Superseded Readings (do not re-introduce)](#11-superseded-readings-do-not-re-introduce)

---

## 1. Circuit Overview

### 1.1 Topology

The Wurlitzer 200A reed-bar preamp is a **two-stage direct-coupled NPN common-emitter amplifier** mounted on a small PCB attached to the reed bar. The two transistors (TR-1 and TR-2) amplify the millivolt-level signals from the electrostatic pickup to a level suitable for the volume chain and power amplifier.

### 1.2 Defining Features

1. **Direct coupling**: TR-1 collector connects directly to TR-2 base — no coupling capacitor between stages. TR-1's DC operating point sets TR-2's bias.

2. **Bias by DC feedback, not a divider**: R-3 (470K) returns from TR-1's base to the R-7/R-8 junction in TR-2's emitter leg. It is TR-1's **only** DC bias path — a negative DC servo from stage 2's operating point back to stage 1's base (§4.3). There is **no resistor from TR-1's base to the supply**; +14.5V feeds only R-4 (150K, TR-1 collector load) and R-6 (1.8K, TR-2 collector load).

3. **Stage-gain split**: C-7 (22 µF) AC-grounds TR-2's emitter across **both** emitter resistors, so stage 2 is a high-gain CE stage and stage 1 — its collector loaded by TR-2's low r_π — is a low-gain transconductance stage (A1 ≈ 8, A2 ≈ 120–140 at 1 kHz; §5.3). Closed-loop gain is set almost entirely by the R-10/Ce1 global feedback divider.

4. **AC-coupled output**: C-6 (4.7 µF) couples TR-2's collector to the R-9/R-10 node, which sits at **0 V DC**. R-10's feedback is taken at the collector (R-9 is outside the loop, pure output series loss), and **no DC reaches the LDR leg** — the "tremolo bias pump" of earlier models does not exist in the real circuit.

5. **Tremolo integration**: R-10 (56K) feeds back from the C-6-coupled output node to TR-1's emitter via Ce1 (4.7 µF). The LDR (LG-1) shunts the feedback junction to ground through the 50K VIBRATO pot wired as a 3-terminal divider (§7.2), modulating how much feedback reaches the emitter and thus the closed-loop gain.

6. **Supply**: **+14.5 V DC regulated.** The service manual text specifies "+14.5 volts regulated" (p.64); the schematic marks "+15V". 14.5 V closes a collector/emitter current imbalance implied by the drawing's own DC marks (§4.5) and was independently bracketed (13.98–14.40 V) by a blind fit to the anchor sets.

### 1.3 Position in Signal Chain

```
Reed vibration
  -> Electrostatic pickup (all 64 reeds summed at single pickup plate)
  -> +150V polarizing feed: R-2 (1 MEG) onto the plate-side network
  -> R-1 (22K series) -> C-1 (.022 uF coupling cap) -> TR-1 base
     [D-1 protection diode and C-2 (220 pF) at the base node]
  -> TR-1 collector = TR-2 base (direct coupling)
  -> TR-2 collector -> C-6 (4.7 uF) -> R-9/R-10 node (0 V DC)
       R-10 (56K) feedback from this node to TR-1 emitter via Ce1;
       LG-1 (LDR) + 50K VIBRATO divider shunt the feedback junction (tremolo)
  -> R-9 (6.8K series)
  -> R-11 "REED BAR VOLUME" 25K trimmer -> Main volume pot (10K audio)
  -> C-8 coupling cap to power amplifier
```

---

## 2. Complete Schematic with Component Values

### 2.1 Schematic Diagram

```
 +150V ──R-2 (1M)──┐                     +14.5V Regulated
                    │                      │            │
 pickup ──R-1(22K)──┤ mid_in          R-4(150K)      R-6(1.8K)
 plate              │                      │            │
                 C-1(.022u)          C-3(100p)     C-4(100p)
                    │                  ┌──||──┐      ┌──||──┐
                    ├────────── base1 ─┤      │      │      │
                    │                  │ TR-1 │      │ TR-2 │
        C-2(220p)───┤            B ────┤C     │ B ───┤C     ├── coll2
             │      │                  │  E   │ (=coll1)    │      │
            GND(†)  │                  │  │   │      │  E   │   C-6(4.7u)
                    │             R-5(33K) │         │  │   │      │
        D-1─────────┘                  │ Ce1(4.7u)   │ R-7(270)    ├── node_c6 (0V DC)
         │                            GND  │         │  ├── emit2b │      │
        GND(†)                             │         │ R-8(820)    │   R-9(6.8K)── out
                                       fb_junct      │  │   C-7(22u)      │
                                           │        GND GND  │        R-10(56K)
              R-3 (470K): base1 ── emit2b ─┘              (emitter      │
              (DC feedback bias — crosses               → ground)    fb_junct
               R-5 and Ce1 as HOPS, no junction)                        │
                                                            50K VIBRATO divider
                                                            (18K top→wiper; LDR
                                                             directly on wiper)
                                                                        │
                                                                       GND
 (†) C-2's and D-1's bottom rail is drawn BROKEN on both scan surfaces —
     ground return is the settled assumption, not a reading.
```

### 2.2 Component Values Table

| Ref | Value | Function |
|-----|-------|----------|
| R-1 | 22K | Series input from pickup plate to the C-1 junction |
| R-2 | **1 MEG** | Polarizing feed: **+150V line → R-1/C-1 junction** (pickup side of the input cap). AC-grounded at audio. See Note 1. |
| R-3 | 470K | **TR-1 base → R-7/R-8 junction**: DC feedback bias from TR-2's emitter divider — TR-1's only DC bias path |
| R-4 (Rc1) | 150K | TR-1 collector load resistor |
| R-5 (Re1) | 33K | TR-1 emitter DC path to ground (lower end: drawn broken; ground by DC analysis, high confidence) |
| Ce1 (C-5) | 4.7 µF | Feedback coupling cap: AC-couples TR-1 emitter to the R-10/LDR feedback junction (NOT a bypass — see §7) |
| R-6 (Rc2) | 1.8K | TR-2 collector load resistor |
| R-7 (Re2a) | 270 Ω | TR-2 emitter resistor (upper) |
| R-8 (Re2b) | 820 Ω | TR-2 emitter resistor (lower; junction with R-7 = R-3's return node) |
| C-7 (Ce2) | 22 µF | **TR-2 emitter → ground** — spans R-7 + R-8 in series; AC-grounds the emitter |
| C-6 | 4.7 µF | **Series coupling, TR-2 collector (+) → R-9/R-10 node** (node sits at 0 V DC) |
| R-9 | 6.8K | Series output resistor (outside the feedback loop) |
| R-10 | 56K | Feedback resistor from the C-6-coupled node to fb_junct |
| C-3 | 100 pF | TR-1 collector-base feedback capacitor |
| C-4 | 100 pF | TR-2 collector-base feedback capacitor — **the dominant bandwidth-setting element** (§5.5) |
| C-2 | 220 pF | At TR-1's base (real 200A part — see Note 2). Return assumed ground (broken-rail drawing defect) |
| D-1 | 25 PIV, 10 mA (part #142136-5) | Reverse-polarity transient protection at the base node |
| C-1 | .022 µF | Input coupling cap (blocks the polarizing DC from the base) |
| LG-1 | "Linear Gate", part #142312 — LED + CdS LDR | Tremolo gain modulation in feedback network (LED side: see output-stage.md §2) |

**Note 1 (R-2 — the 1M/2M resolution):** The schematic label really is **1 MEG** (glyph-verified against same-sheet width controls). Earlier revisions overrode it to 2 MEG because, with R-2 misplaced at the base as a divider against R-3-to-ground, 1M gave an absurd bias and 2M was the least-bad fit. The "GroupDIY 380K input impedance" that appeared to confirm 2M∥470K was **circular** — it *is* that arithmetic, not a measurement. In the drawn topology R-2 never touches the base; both the printed base mark and the hardware measurement are reproduced exactly (§4.3), and the label stands.

**Note 2 (C-2 vs C20):** These are **different 220 pF parts**. C-2 is at TR-1's base and IS on the 200A. C20 is an aux-section part on the 206A-family boards and is NOT on the 200A. An earlier "remove the phantom C20" pass deleted the base-node 220 pF by designator conflation; it is restored. Map by node, not designator.

### 2.3 Polarizing / Pickup-Side Network

```
AC Mains -> dedicated winding -> half-wave rectifier
         -> RC filter chain (3 x 0.33 uF with series resistors)  [+150V line]
         -> R-2 (1 MEG) -> R-1/C-1 junction -> R-1 (22K) -> pickup plate
```

The polarizing line's filter caps place it at **AC ground**, so at audio the pickup-side network is: plate capacitance (C_total ≈ 240 pF nominal, LOW confidence — see pickup-system.md §3.7) working against R-1 in series with C-1 into the base node, with R-2 (1M) shunting the R-1/C-1 junction and C-2 (220 pF) at the base. Because **C-1 is a short at audio, C-2's 220 pF sits effectively in parallel with the plate capacitance** — the mechanism the old model missed entirely.

**Measured/derived corner (current-source drive, the physical electrostatic case):**

| Case | −3 dB corner |
|---|---|
| **C-2 → ground (settled assumption), C_total = 240 pF** | **≈ 900 Hz** (SPICE 897; derivation 843–938) |
| C-2 → TR-1 emitter (open alternative) | ≈ 1.4 kHz |
| Sensitivity: C_total 150 pF / 400 pF | 1021 Hz / 652 Hz |

Two warnings for the DSP:
- **The response is NOT one-pole.** The asymptote above the corner measures −17.3 dB/dec (second-order-with-zero network). Fit the model to the measured response table, not to a corner frequency.
- **The drive spectrum matters.** The physical reed drive is a current ∝ jω (I = V_pol·dC/dt): a high-pass rising ≈ +15.6 dB/dec across 55 Hz–1.76 kHz. A flat-current transimpedance measurement of the same network is a low-pass; the two differ by exactly 20 dB/dec. The DSP needs the jω-driven version.
- vs the superseded model (one-pole at 2312 Hz): the corrected network passes **≈ +11 dB more bass below 440 Hz**. Parts of the 2026-07 downstream bass voicing were likely compensating for this.

| Component | Value | Notes |
|-----------|-------|-------|
| Polarizing voltage | 147 V DC nominal | |
| Feed resistor (R-2) | 1 MEG | On the plate-side network. (Avenson's "499K" is his replacement design, not the original.) |
| Filter capacitors | 3 × 0.33 µF | |
| Rectifier | Half-wave | |

---

## 3. Transistor Specifications

### 3.1 The house-number grade system

The schematic legend assigns Wurlitzer house numbers: **TR-1/2/5/6/15 = 142083-3; TR-3/4 = 142083-2** (TR-7/8 print 142128 PNP at the symbols; the legend's "142083-1" line is a transcription slip — parts list and symbol-adjacent numbers agree against it).

The GE 1971 Semiconductor Data Handbook (pp. 498–500) documents that **2N2923/24/25 are the color-coded beta grades of the 2N2926 population** — one die family sold as gain grades with identical limits:

| Grade | color | small-signal hfe (10V, 2mA, 1kHz) | DC β typical (4.5V, 2mA) |
|---|---|---|---|
| 2N2923 | Orange | 90–180 | 115 |
| 2N2924 | Yellow | 150–300 | 155 |
| 2N2925 | Green | 235–470 | 215 |

The service manual's only 2N cross-reference (non-A models) stocks 2N2926/2N2924 under one interchangeable number — Wurlitzer treated adjacent grades as one item. Reading 142083-2/-3 as Yellow/Green grades of this family is **rule-of-thumb-strong** (mechanism documented; the suffix meaning itself is not printed anywhere). The manual prints **no** 2N type for any A-series transistor.

**Consistency:** the hardware-anchored TR-1 DC hFE (350 ± 50, §4.4) sits inside the Green band and above Yellow's ceiling; the simulation-fitted TR-2 band (≈ 240–400) sits inside Green; the oscillator's behavior matches the Yellow typical (output-stage.md §2.1).

### 3.2 Replacement Transistor: 2N5089

Later production and typical service replacements use the 2N5089 (NPN, high-gain, low-noise; hFE 450–1800 at 1 mA; Cob ≈ 2.5 pF). Many surviving instruments carry a mix.

### 3.3 Modeling policy

**Carry the transistor card as a BAND, never a fitted point.** The DC anchors cannot pin β2 (§4.4), the plausible original-part band is Green-grade 235–470, and the shipped 2N5089 card (BF 1434) is a defensible "serviced instrument" character. The choice audibly shifts sub-clip H2 by ~3 dB across the 240–400 band (§6.3) and clipping character — it is a **voicing decision** for the maintainer's ear, bounded by documentation.

---

## 4. DC Bias Analysis

### 4.1 Anchor sets — and which one to trust

| | TR-1 B | TR-1 E | TR-1 C | TR-2 E | TR-2 B | TR-2 C |
|---|---|---|---|---|---|---|
| Schematic marks | 2.45 | — | 4.1 | 3.4 | 4.1 | 8.8 |
| GroupDIY hardware (real instrument, DMM) | 2.447 | 1.923 | 3.980 | 3.356 | 3.988 | 8.450 |

**The hardware set is the better anchor.** An Ebers-Moll cross-check on ΔV_BE between the two junctions (predicted 0.103 V for the current ratio) matches the hardware to 5 mV and misses the printed marks by 2× — the marks are values rounded to 0.05 V with "0.70 V" stock-silicon V_BE annotations. (The hardware set even reads one physical node twice — V_C1 3.980 vs V_B2 3.988 — giving its own ±8 mV meter floor.) Use the marks for topology and nominal values; anchor bias arithmetic to hardware.

### 4.2 Direct Coupling Verification

TR-1 collector = TR-2 base (identical marks, identical hardware readings within meter floor): **no coupling capacitor between stages.**

### 4.3 The bias chain (corrected topology)

R-3 is the only DC path to TR-1's base — C-1 blocks the polarizing line, C-2/C-3 block, D-1 is reverse-biased:

```
V_J  = V_E2 · R8/(R7+R8)              (the R-7/R-8 junction)
I_B1 = (V_J − V_B1)/R3                 (current INTO the base through 470K)
I_E1 = V_E1/R5 ;  I_C1 = I_E1 − I_B1
I_R4 = (Vcc − V_C1)/R4 ;  I_B2 = I_R4 − I_C1     ← R-4 feeds TWO loads
V_E2 = V_C1 − V_BE2 ;  I_E2 = V_E2/(R7+R8) ;  I_C2 = I_E2 − I_B2
```

**Structurally decisive:** under the drawn topology, if R-3 returned to ground (the superseded reading) TR-1's base would have **no DC bias source at all**. The corrected reading isn't merely a better fit — the old one is not a working circuit. (Which of R-7/R-8 grounds is also derivable: the other orientation forces current *out* of the base and β1 ≈ 21 — non-physical.)

**The R-3 loop is a negative DC servo** (V_B1↑ → V_C1↓ → V_E2↓ → V_J↓ → V_B1↓): loop gain ≈ 2.1, desensitivity ≈ 3.1. It holds TR-1's operating point against β spread and thermal drift, and makes the two stages drift *together* — the physical basis of the bias-coupling "sag" behavior.

Worked at the hardware anchors (Vcc = 14.5 V): V_J = 2.525 V → ΔV(R-3) = 78 mV → I_B1 = 0.165 µA; the printed 2.45 V and measured 2.447 V both emerge. The topology that could never close (best miss 0.35 V) closes exactly.

### 4.4 Extracted parameters (hardware anchors, 14.5 V)

| Quantity | Value | Notes |
|---|---|---|
| I_C1 | **58.1 µA** | = I_E1 − I_B1. (NOT I_R4 = 70 µA: R-4 also feeds TR-2's base) |
| I_B2 | **12.0 µA** | 15–17% of I_R4 — not negligible |
| I_C2 | **3.07 mA** | |
| β1 (TR-1 DC) | **350 ± 50** | ±50 = the ±8 mV meter floor propagated. Green-grade-consistent |
| β2 (TR-2 DC) | **not extractable** | difference of two ~70 µA numbers; ±25% per 5% input error. **Never let a deck tune β2 to absorb a residual** |
| gm1 | 2.24 mA/V | from I_C1 (the old 2.8 used I_R4) |
| gm2 | 118 mA/V | |
| r_π1 / r_π2 | 157K / **2.97K** | r_π2 is what collapses stage 1's gain (§5.3) |
| V_CE1 / V_CE2 | 2.06 / 5.09 V | |

Independent SPICE cross-check (same card, BF 1434, 14.5 V): every node within 1 mV between two implementations; the drawn deck at TR-2 BF ≈ 350–400 lands within 15–60 mV of the hardware set. Validated in `spice/testbench/tb_preamp_dc.cir`.

### 4.5 The supply rail

At 15 V, the drawing's own marks imply TR-2 collector current 10% above emitter current, and the hardware set 18% — with **no other DC path at the collector node** (instrumented enumeration: R-6, C-4, C-6, collector only). At **14.5 V** (the manual text's figure) the marks close to +1.9% and the model to ~1.0. A blind best-fit rail from the anchors bracketed 13.98–14.40 V. Use 14.5 V; a meter on a real reed-bar supply pin outranks further paper analysis. (The hardware set's residual ~+9.6% is consistent with that instrument's rail being lower still, or R-6 high — bounded, not fitted.)

### 4.6 Summary

| Parameter | Stage 1 (TR-1) | Stage 2 (TR-2) |
|-----------|----------------|----------------|
| V_B / V_E / V_C (hardware) | 2.447 / 1.923 / 3.980 | 3.988 / 3.356 / 8.450 |
| Rc | 150K | 1.8K |
| Re | 33K (Ce1 couples emitter to fb_junct) | 270 + 820 Ω, **C-7 spans both** |
| I_C | ~58 µA | ~3.07 mA |
| gm | ~2.24 mA/V | ~118 mA/V |
| re (1/gm) | 447 Ω | 8.5 Ω |

---

## 5. AC Signal Analysis

### 5.1 Input Signal Levels

The pickup delivers millivolt-level signals. ⚠ The register-dependence estimates previously tabulated here assumed the superseded 2312 Hz pickup corner; with the corrected ≈900 Hz non-one-pole network, bass fundamentals arrive ≈ 11 dB hotter below 440 Hz. Re-tabulate after the pickup model refit. Avenson's output-side measurement (2–7 mV at the volume pot) still anchors the overall scale.

### 5.2 Input network and base impedance

- C-1's corner against the base load is subsonic: a short at all audio frequencies.
- The base node's AC impedance is **R-3 (470K to AC ground — C-7 grounds its far end at audio) rolled off by C-2 (220 pF), plus a small C-3 Miller term**. The transistor's own term is enormous — series feedback multiplies r_π1 by the loop factor to ≈ 34 MΩ — and contributes nothing.
- |Z_in(base)| ≈ 459K peak near 200 Hz, 380K at ~755 Hz, 339K at 1 kHz, falling 6 dB/oct above. At the jack: ≈ 289K at 1 kHz.
- **Z_in is independent of the tremolo state** (feedback keeps the transistor term out of reach at every shunt value): a useful negative — no tremolo-dependent input loading to model.
- The old "380K measured input impedance" was the Thevenin arithmetic of the misread divider, not a measurement (§2.2 Note 1). The corrected circuit *happens* to pass through 380K at 755 Hz — the anchor is not violated, but it never confirmed anything.

### 5.3 The stage-gain split (INVERTED vs the superseded analysis)

C-7 AC-grounds TR-2's emitter → TR-2's input resistance is r_π2 ≈ 3K → it loads TR-1's 150K collector down to ≈ 3.9K:

| | corrected | superseded |
|---|---|---|
| A1 (v_c1 / v_be1, 1 kHz) | **≈ 8** (card-dependent 4–14 across the BF band — r_π2 ∝ β2) | 420 |
| A2 (node_c6 / v_c1, 1 kHz) | **≈ 120–140** (SPICE 121–131 across cards; derivation 137) | 2.17 |
| A_open (1 kHz) | ≈ 1100 | 912 |

Independently measured (SPICE phasor probes) and derived; the decisive structural check of the C-7 reading. Note the ground-referenced ratio v(coll1)/v(base1) reads ≈ 0.04–0.05 under closed loop (the emitter tracks the base); A1 is defined base-*emitter* differential to collector.

**Stage-2 low-frequency shaping:** because re2 ≈ 8.5 Ω, the 22 µF C-7 is *not* a full bypass until ~1 kHz: the local stage gain has a **zero at ≈ 6.6 Hz and a pole at ≈ 971 Hz** (|A2|: 4.6 at 20 Hz → 136 at 971 Hz → 178 at 10 kHz). The global loop flattens most of it, but it sets how loop gain — and therefore distortion suppression — varies with frequency. Measured shelf: |v(node_c6)/v(coll1)| rises ≈ +19 dB/dec between corners at ~5.5 Hz and ~715 Hz (3 dB-corner reading; interacts with the loop).

### 5.4 Closed-loop gain vs the feedback shunt

Ce1 is a short at audio, so fb_junct and TR-1's emitter are one AC node. With k = R_sh/(R_sh+R10), R_th = R10∥R_sh:

```
G = A / (1 + gm1·(Re1∥R_th) + A·k·Re1/(Re1+R_th))
```

Global feedback dominates local degeneration ≈ 9:1 at the idle point; loop gain ≈ 196 (46 dB); the closed loop sits within ~1 dB of the ideal 1/β limit — **the R-10/Ce1 divider is genuinely in charge, and the closed-loop gain is insensitive to the parameters that can't be pinned** (β2, gm1, Early).

**Gain vs shunt R (at the R-9/R-10 node, 14.5 V; independent SPICE, BF-1434 card / derivation):**

| R_shunt | Gain (dB) | Notes |
|---|---|---|
| 8K | 18.5 | full-depth bright region |
| **13.24K** | **15.7–15.9** | the 50K/18K divider's depth-0 floor — compare Avenson's ~15 dB |
| **18–19.4K** | **14.0** | (bisection 18.22K measured; 19.4±2.5K derived) |
| 48K | 10.5 | full-depth dark region |
| 1M | 7.6–7.9 | not reachable through the divider (§7.2) |

The whole curve sits ≈ +1.3…+1.9 dB above the superseded table (which was generated on the wrong topology). ⚠ The **tremolo partition** — which shunt values the divider actually presents, and therefore the idle gain and AM depth — changed with the R-18 relocation (output-stage.md §2.3) and is **parametric pending the light-law re-fit**. Depth-swing over 8K↔48K is ≈ 7.9 dB in every card variant, so the *shape* of the shipped depth calibration survives.

Where "gain" is measured matters: R-9 into the 10K volume chain costs ≈ −4.5 dB after node_c6 — always state the node.

### 5.5 Bandwidth

**Preamp-only −3 dB (re 1 kHz) ≈ 16.7–17.1 kHz** at the idle shunt (triple-confirmed: melange-SPICE 16.70K, ngspice 16.79K, matched-drive derivation 17.11K; shunt-dependence ≈ +1 kHz from 13K→1M). Full-chain values are lower once the pickup-side network loads in.

**C-4 is the dominant bandwidth-setting element** — under A2 ≈ 137 its Miller multiplication at the coll1 node rules the response (knockout removes the −3 dB crossing entirely; the pole scales as 1/C-4). C-3's Miller multiplication, by contrast, collapsed with A1 (420 → 8): the old "23 Hz dominant pole from C-3" story is dead, and with it the constant-GBW-per-stage framing.

⚠ Definition trap that produced two wrong numbers historically: an ideal voltage drive at the base **short-circuits C-2 and C-3's loading** and reads ≈ 30 kHz; −3 dB-re-peak with a shelving response misreads entirely. Measure driven through the real R-1/C-1/R-2 network, −3 dB re 1 kHz, first crossing.

vs shipping: the pre-revision model measures 9.4 kHz on the same testbench — **the corrected circuit is ≈ 1.5× wider than what ships.** A mild LF rise (to ≈ +1 dB near 30 Hz) from the stage-2 zero/pole + loop-gain shaping is real physics, not a defect. Re-baselined in `tb_preamp_ac.cir` (15.54 dB @ 1 kHz, R_ldr 12K loaded; HF −3 dB 16.79 kHz).

### 5.6 What survives from the nested-loop analysis

The two-loop picture (local collector-base caps inside a global R-10 emitter loop) survives; the *roles* changed: the inner loop's bandwidth-setting element is C-4 at stage 2 (not C-3 at stage 1), and preamp-only bandwidth remains nearly independent of R_ldr while GBW scales with gain. The old section's SPICE tables were generated on the superseded topology; use the figures in §5.4–5.5.

---

## 6. Harmonic Generation

### 6.1 The Exponential Transfer Function (unchanged physics)

A single BJT CE stage has the Ebers-Moll exponential transfer `Ic = Is·exp(Vbe/nVt)`; its Taylor expansion makes H2 dominate over H3 at small signals (H2/H3 ≈ 3Vt/2Vpeak — e.g. +17.8 dB for 5 mV peak). Single-ended stages produce even harmonics; differential stages (tanh) cancel them. This section's physics is topology-independent and stands.

### 6.2 ⚠ The Stage-1 asymmetric-clipping story — INFERRED DEAD, pending direct probe

Stage 1's DC headroom asymmetry (≈ 2 V toward saturation vs ≈ 10.5 V toward cutoff, ~5.3:1) **still exists — but can never be reached.** With the corrected stage split, TR-1's collector swings ~0.007× the output node: when TR-2 hits its (near-symmetric, 1.21:1) limit, TR-1 has used ~36 mV of its ~2 V budget — a 54× margin. **TR-2 clips first.**

Status and consequences:
- This collapse follows structurally from the verified A1/A2 inversion but has **no direct measurement yet**. The DSP's `satLimit/cutoffLimit` Stage-1 asymmetry stays in place until a probe of which stage clips first in the corrected solver/deck.
- It is **consistent with what measurement always said**: the pickup's 1/(1−y) owns >98% of H2 at normal dynamics and the preamp is transparent at mV levels. The revision removes a claimed *mechanism*, not a measured behavior.
- At playing levels the preamp does not clip at all (a ff base signal sits ~40 dB below TR-2's threshold). The preamp's surviving nonlinearity is TR-2's exponential V_BE, suppressed a further ~46 dB by the loop.

#### ⚠ KNOWN ISSUE — the shipping solver has no saturation region, so it cannot be used for this probe

`dk_preamp_legacy.rs`'s BJT kernel is `fn bjt(vbe: f64)` — collector current is a function
of **Vbe alone**, with no Vbc term, by deliberate design (the Early effect is omitted to keep
the kernel one-dimensional per device). A forward-active-only device **has no saturation
region**. Nothing in the model stops the collector once the global feedback loop can no longer
hold the node, so the solver does not clip — it **diverges**:

| shunt | input | peak gain | implied peak output |
|---|---|---|---|
| 13.24 kΩ | 0.83 V | 5.65× (15.04 dB) | 4.7 V — linear |
| 13.24 kΩ | 1.0 V | 5.61× (14.97 dB) | 5.6 V — knee, matches the 0.85–1 V figure in §6.3 |
| 13.24 kΩ | 1.2 V | **15.3× (23.7 dB)** | **18 V, above the 14.5 V rail** |
| 1 MΩ | 2.4 V | 2.24× | 5.4 V — linear |
| 1 MΩ | 4.0 V | **107.6×** | **430 V on a 14.5 V rail** |

At 1.5 V in (13.24 kΩ) the DFT fundamental *collapses* to 0.52 V while the peak rises and THD
stays at 1.3% — fundamental collapse with large peaks and little harmonic content is
divergence, not soft clipping. Onset is at ~5.5 V **output** at both shunts, i.e. exactly
TR-2's saturation point, which is the tell.

Consequences:
- The "which stage clips first" probe called for above **cannot be run on this solver.** Use
  the SPICE deck until the kernel gains a Vbc term. The §6.3 clipping-character figures are
  deck measurements and stand; the solver cannot reproduce them.
- The in-code comment claiming "the circuit topology and NR solver naturally constrain the
  operating point" is true only below the knee, and should not be read as a clipping model.
- **Not audible today**: the shipping chain applies `output_scale` ahead of the preamp, so it
  sees millivolts — ~40 dB below TR-2's threshold, and roughly two orders of magnitude below
  the divergence onset. This is a measurement-capability limit, not a playing defect.
- Any bench that drives the preamp directly from the pickup bus (the bark audit does) must
  check its drive against the onset. The bark audit is safe only because it hardcodes a 1 MΩ
  shunt, where onset is ~2.4 V against its 1.33 V worst case; at the instrument's real
  13.24 kΩ idle shunt the onset is ~1.05 V and several ff rows would be inside the divergent
  region.

#### ⚠ KNOWN ISSUE — the pre-revision model carried a +0.69 dB offset against its own deck

Recorded so a future correction is not misread as a regression. The **pre-revision** Rust
preamp measured **+0.69 dB above its own SPICE target at both shunts** (6.69 vs 6.0 dB at
1 MΩ; 12.79 vs 12.1 dB at 19 kΩ — the same offset at both, so it is systematic, not a
curve-shape difference). Its origin was never established.

This matters when comparing builds across the revision. The bench-vs-bench gain deltas
(+0.35 dB at 1 MΩ, +0.47 dB at 19 kΩ) are *smaller* than the deck-node offsets (+1.3…+1.9 dB)
for three reasons, of which this is one:

| term | value |
|---|---|
| deck offset at 19 kΩ (12.1 → 14.0 dB, each at its own topology's R-9/R-10 node) | +1.90 dB |
| new model stamps `RLOAD = 100 kΩ` at `out`; the pre-revision model had **no load** there — and the old topology's R-9/R-10 node *was* `out`, after R-9, while the new deck quotes `node_c6`, before it | −0.58 dB |
| input-network change (R-2 2M-at-base → 1M-on-pickup-side, C-2 restored) — the smallest term, often wrongly blamed for the whole gap | −0.18 dB |
| **this issue**: old model ran 0.69 dB hot vs its own deck, inflating the old baseline | −0.69 dB |
| **predicted bench-vs-bench delta** | **+0.45 dB** (measured +0.47) |

If the +0.69 dB is ever tracked down and fixed, the bench-vs-bench delta moves to ≈ +1.16 dB.
That is the correction landing, not the revision regressing.

### 6.3 Measured harmonic character of the corrected circuit (SPICE, 14.5 V, idle shunt)

- **Sub-clip H2 is 9–12 dB higher than the pre-revision model** (BF 400 / BF 240 respectively, vs the 1434 card's level; robust in direction across the whole plausible card band, monotone in BF). Directly relevant to the long-standing ~3 dB full-chain H2 deficit vs reference recordings — expect recalibration, not a free win.
- **Clipping parity flips**: the corrected circuit at Green-band cards clips with a **soft, even-order knee** (H2-dominant to ≈ 0.85–1 V input) where the pre-revision model clips odd-order from ≈ 0.5 V. Even-order-soft is the right character for a single-ended 200A.
- Below clipping all variants are H2-dominant with H3 at −85…−115 dBc.

### 6.4 Cascaded enrichment (unchanged)

Stage 2 processes stage 1's output; cascaded nonlinearity produces harmonics-of-harmonics (H4, combination tones) at extreme drive. With the inverted stage split, the *ordering* changes (stage 2 is now the high-gain, first-to-clip stage) but the enrichment mechanism stands.

---

## 7. Tremolo Integration — LDR in Feedback Loop

### 7.1 The Critical Finding (service manual, unchanged)

> "The reed bar signal is modulated by inserting the vibrato voltage into the feedback loop of the high impedance preamp. A divider is formed by the feedback resistor R-10, and the light dependent resistor of LG-1…"

Tremolo modulates the preamp's **gain**, not the output volume.

### 7.2 Feedback Topology

```
TR-2 collector ── C-6 (4.7µF) ── node_c6 (0V DC) ── R-9 (6.8K) ── out
                                     │
                                  R-10 (56K)
                                     │
                                  fb_junct ── Ce1 (4.7µF) ── TR-1 emitter
                                     │                          │
                          50K VIBRATO pot (3-terminal         R-5 (33K)
                          divider: top=fb_junct, bottom=GND,     │
                          wiper→LDR branch; 18K top→wiper;      GND
                          LDR DIRECTLY on the wiper branch)
```

- **R-18 (680 Ω) is NOT in the LDR leg** — it is in the LED drive path on the oscillator side (output-stage.md §2.3). The LDR (LG-1 pins 4→3) connects cable-direct from the wiper branch to ground.
- Shunt impedance seen by fb_junct: `Z = (R_up ∥ 18K) + (R_low ∥ R_ldr)`, `R_up = 50K·(1−depth)`, `R_low = 50K·depth`. Depth-0 floor = 50K∥18K ≈ **13.24K → ≈ 15.9 dB** (a good match to Avenson's ~15 dB).
- Because C-6 blocks DC, **no DC crosses R-10 into the LDR leg: the tremolo bias pump of earlier models does not exist** (measured 0.000 mV bias excursion under 19K↔1M LDR cycling — `tb_pump_emit.cir`, now a C-6 regression guard). The shadow-pump compensation in the pre-revision DSP loses its physical basis.
- ⚠ **Quantitative partition figures are pending**: the pre-revision "no-vib ≈ 13K / 8K↔48K / ~7 dB AM" set was computed with a spurious 680 Ω in the LDR leg AND a fixed-LED-current assumption; both fell. Re-derive with the light-law re-fit (decision test: re-fit the cell range on the drawn LED path; ship changes only if the depth ladder or drive shape audibly differs). Depth-swing shape (≈ 7.9 dB over 8K↔48K) is robust across all card variants.

### 7.3 Gain modulation

LDR low (bright) → fb_junct shunted → feedback can't reach the emitter → higher gain, less loop linearization. LDR high (dark) → feedback reaches the emitter → lower gain, more linear. Use the §5.4 gain-vs-shunt table. Distortion character therefore rides the tremolo cycle — timbral modulation, not volume modulation.

### 7.4 Tremolo Oscillator

See output-stage.md §2.1 for the revised oscillator + LED path (twin-T, ≈5.5–5.9 Hz rate — R-17-dependent; drawn LED chain R-18 → LED → R-17 trimmer; the print's 11.5 Vpp "TYPICAL WAVE FORM" reproduced untuned at the documented Yellow-grade card with R-17 near 0).

---

## 8. Modeling Recommendations

### 8.1 Architecture (updated for the revision)

The `PreampModel` trait architecture stands. The shipping implementation is the hand-written MNA solver (`dk_preamp_legacy.rs`), rewritten for the drawn topology as a **9-node** system (base1, emit1, coll1, emit2, emit2b, coll2, node_c6, out, fb) with the R-1/R-2/C-1 input network folded into an exact companion two-port and C-2 at the base. The melange-generated solver (opt-in feature) regenerates from `spice/melange/wurli-preamp.cir` (11-node deck, `.integrator be` author-pinned — the deck is trapezoidal-unstable via a Nyquist-marginal z=−1 mode that BE correctly damps).

### 8.2 Perceptually Important Elements (revised priority)

| Element | Impact | Priority |
|-------------|-------------------|----------|
| Pickup-side network: ≈900 Hz non-one-pole corner, jω drive (fit to table) | ≈ +11 dB bass below 440 Hz vs the old model; primary tonal consequence of the revision | CRITICAL |
| Closed-loop gain via R-10/Ce1 divider (§5.4 curve) | Idle gain + tremolo modulation | CRITICAL |
| Exponential V_BE (TR-2 dominant post-revision) | H2 ≫ H3 ratio; sub-clip H2 level (card-band-dependent, +9..12 dB vs old) | HIGH |
| C-4 Miller loop (bandwidth ≈ 16.7 kHz; ~1.5× wider than the old model) | HF openness | HIGH |
| Stage-2 zero/pole (6.6 Hz / 971 Hz) inside the loop | Frequency-dependent loop gain / distortion suppression; mild LF rise | MEDIUM-HIGH |
| Direct-coupling bias servo (R-3 loop) | "Sag"/"bloom"; stages drift together | MEDIUM-HIGH |
| Tremolo gain modulation | **Level**, not timbre — see note | MEDIUM |
| Even-order soft clipping knee at TR-2 (card-dependent onset) | ff character | MEDIUM (pending §6.2 probe) |
| Early effect / β(Ic) / thermal | masked | LOW |

**⚠ Note on the tremolo row — what rides the cycle is level, not audible timbre.**

§7.3's mechanism (more shunt → less loop linearization → more distortion) is **real and
measurable**, but it is ~90 dB down. Measured on the solver at C4 / 5 mV across the
divider's reachable swing:

| R_shunt | gain | preamp H2/H1 |
|---|---|---|
| 8 kΩ (full-depth bright) | 17.92 dB | −93.6 dBc |
| 13.24 kΩ (depth-0 floor) | 15.01 dB | −96.7 dBc |
| 48 kΩ (full-depth dark) | 9.95 dB | −102.1 dBc |

So the preamp's own H2/H1 does swing **8.5 dB across the reachable range** — the mechanism
is not fictitious. But it swings between −94 and −102 dBc, i.e. 60+ dB below the pickup's
1/(1−y) contribution, which owns >98% of H2 at normal dynamics (§6.2). The audible
consequence of the tremolo is therefore **level**; the timbral component is real physics that
the MNA gets for free and that no behavioural model needs to reproduce deliberately. The row
stays at MEDIUM for the level modulation, not for timbre.

⚠ **Do not cite a "≤0.14 dB H2 change across the swing" figure** — that number came from the
pre-fix `preamp-bench` harmonic estimator, whose unwindowed rectangular DFT had a leakage
floor near −43 dBc. At −94…−102 dBc the real H2 was 50+ dB *under* that floor, so every shunt
returned the same leakage value and the change looked like nothing. The estimator was fixed
(Hann + integer-cycle windowing, floor now ≈ −120 dBc); any harmonic figure measured with that
tool before the fix needs re-taking.

### 8.3 Simplifications that changed with the revision

- **Shadow-pump subtraction: obsolete.** The real circuit has no pump (C-6). Remove after the 9-node solver lands and the pump guard passes.
- **Stage-1 asymmetric soft-clip constants: retain but flagged** (§6.2) until directly probed.
- **"Stage 2 fully linear": no longer safe** — stage 2 is now the gain stage and the first to clip; its exponential is the surviving preamp nonlinearity.
- Ce1-as-short, Ce2-27 Hz-corner shortcuts: the 27 Hz figure was the *wrong-span* corner; the real C-7 shaping is the 6.6/971 Hz pair, inside the loop — the MNA captures it natively; behavioral models must not drop it.

---

## 9. Implementation Pitfalls

1. **Never validate a netlist by internal consistency alone.** Rust-vs-SPICE agreement proves code matches netlist, never that the netlist matches the instrument. The 2026-09 revision existed because every check was internal for months while the one discriminating hardware measurement sat unused. Anchor to hardware.
2. **Map components by node, not designator** (C-2/C20). **Read hops vs junctions instrumentally, not by eye** (R-3's return, R-18's placement).
3. **Don't fit a parameter to absorb a residual** (the 2M override; β2). State bands; leave residuals visible.
4. **Miller feedback polarity**: more feedback at HF, less at LF.
5. **Tremolo is gain modulation, not volume modulation** — and its DC story is *nothing*: any bias pump in a model of the drawn circuit is a bug (regression guard: `tb_pump_emit.cir`).
6. **R-10 must feed the emitter via Ce1** (series-series NF). Feeding the base node creates positive feedback and oscillation.
7. **State the measurement node and drive** for every gain/bandwidth figure (node_c6 vs out; voltage-at-base vs through-the-input-network; jω vs flat pickup drive). Three historical wrong numbers came from definition mismatches, not circuit errors.

---

## 10. Sources

### Primary Sources

1. **Wurlitzer 200/200A Service Manual** — circuit descriptions, "+14.5 volts regulated" (p.64), parts lists
   - [Internet Archive](https://archive.org/details/wurlitzer-200-and-200-a-service-manual)
2. **Wurlitzer 200A Schematic #203720-S-3** — the titled archive scan is authoritative; the Tropical Fish compendium's redraw is a different print (see docs/SCHEMATIC_SOURCE.md)
   - [BustedGear 200A Schematic PDF](https://www.bustedgear.com/images/schematics/Wurlitzer_200A_series_schematics.pdf)
3. **GE Semiconductor Data Handbook, 1971** — 2N2923/24/25/26 grade system (pp. 498–500)
4. **GroupDIY thread** — real-instrument DC measurements (the load-bearing hardware anchor)
5. **Brad Avenson measurements** — ~15 dB preamp gain; 2–7 mV at the volume pot

### Transistor / Opto Datasheets

6. 2N5089 (ON Semiconductor); 2N292x family (GE 1971)
7. TI TIL209A bulletin DL-S 12024 (June 1973) — period red GaAsP LED I-V and intensity curves (tremolo LED card)
8. PerkinElmer Photoconductive Cells and Analog Optoisolators catalog (2001) — CdS spectral matching, VTL5C3 anchors

---

## 11. Superseded Readings (do not re-introduce)

Preserved so the errors stay findable. Each was overturned 2026-09-13 by pixel-level instrumented re-reads of both scan surfaces and confirmed wrong by independent SPICE + derivation + hardware:

| Superseded reading | Why it seemed right | What's actually drawn |
|---|---|---|
| R-2 = 2 MEG from Vcc to TR-1 base | 1M "made no sense" in the misread divider; the circular 380K figure "confirmed" 2M∥470K | R-2 = 1 MEG, +150V line → R-1/C-1 junction (pickup side). No base-to-Vcc resistor exists |
| R-3 = 470K to ground | The crossings over R-5 and Ce1 read as junctions at low resolution | R-3 returns to the R-7/R-8 junction (hops, not junctions) — DC feedback bias |
| C-7 across R-7 only → Av2 = −2.2 | Plausible partial-bypass reading | C-7 spans emitter→ground (both resistors); stage 2 is the high-gain stage |
| Output DC-coupled through R-9 | C-6's junction dot is weak on the archive scan | C-6 4.7 µF series cap; the R-9/R-10 node sits at 0 V DC; **no tremolo bias pump exists** |
| "C20 220 pF is 206A-only, remove it" | Real fact about C20, wrong designator mapping | The base-node 220 pF is C-2, a real 200A part |
| Vcc = 15 V | The drawing's rail mark | Manual text: +14.5 V regulated; closes the KCL imbalance in the drawing's own marks |
| Stage split A1=420 / A2=2.2; 23 Hz C-3 dominant pole; Stage-1 5.3:1 clipping as the preamp H2 mechanism | Correct arithmetic on the wrong topology | A1≈8 / A2≈130; C-4 sets bandwidth; TR-2 clips first (§6.2 — collapse inferred, probe pending) |
| Tremolo: R-18 in the LDR leg; fixed ~0.84 mA LED | Hops read as junctions; R-17 arrowhead lost on the redraw | R-18 in the LED path; R-17 is the VIBRATO ADJUST trimmer; LED current rides the oscillator swing |

The associated DSP-era artifacts — the shadow-pump subtraction, the 2312 Hz pickup TAU, the R_LDR_MIN floor — were compensations for these readings and are retired with them (each via its own validated change).
