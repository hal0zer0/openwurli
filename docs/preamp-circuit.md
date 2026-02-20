# Wurlitzer 200A Reed-Bar Preamp — Complete Circuit Reference

Comprehensive technical reference for implementing a digital model of the Wurlitzer 200A reed-bar preamplifier. Covers verified component values (from direct reading of BustedGear 200A schematic at 900 DPI), DC bias analysis, AC signal analysis, harmonic generation mechanisms, tremolo integration, and modeling recommendations.

> **See also:** [DK Preamp Derivation](dk-preamp-derivation.md) (MNA math), [DK Preamp Testing](dk-preamp-testing.md) (test pyramid), [Output Stage](output-stage.md) (tremolo LDR integration)

---

## Table of Contents

1. [Circuit Overview](#1-circuit-overview)
2. [Complete Schematic with Component Values](#2-complete-schematic-with-component-values)
3. [Transistor Specifications](#3-transistor-specifications)
4. [DC Bias Analysis](#4-dc-bias-analysis)
5. [AC Signal Analysis](#5-ac-signal-analysis)
6. [Harmonic Generation — Why H2 > H3](#6-harmonic-generation--why-h2--h3)
7. [Tremolo Integration — LDR in Feedback Loop](#7-tremolo-integration--ldr-in-feedback-loop)
8. [Modeling Recommendations](#8-modeling-recommendations)
9. [Implementation Pitfalls](#9-implementation-pitfalls)
10. [Sources](#10-sources)

---

## 1. Circuit Overview

### 1.1 Topology

The Wurlitzer 200A reed-bar preamp is a **two-stage direct-coupled NPN common-emitter amplifier** mounted on a small PCB attached to the reed bar. The two transistors (TR-1 and TR-2, both 2N5089) amplify the millivolt-level signals from the electrostatic pickup to a level suitable for the volume pot and power amplifier.

### 1.2 Defining Features

1. **Direct coupling**: TR-1 collector connects directly to TR-2 base — no coupling capacitor between stages. TR-1's DC operating point sets TR-2's bias.

2. **High-impedance electrostatic input**: The preamp input sees the pickup plate through a resistive bias network (R-2 = 2 MEG to +15V, R-3 = 470K to ground).

3. **Collector-base feedback capacitors** (C-3 = 100 pF, C-4 = 100 pF): Frequency-dependent negative feedback via the Miller effect on both stages. These, combined with global emitter feedback through R-10 (via Ce1), reduce the very high open-loop gain (~900x) to a moderate closed-loop gain.

4. **Tremolo integration**: R-10 (56K) feeds back from the preamp output to TR-1's emitter via Ce1 (4.7 MFD coupling cap). The LDR (LG-1) shunts the feedback junction (between R-10 and Ce1) to ground via the cable, modulating how much feedback reaches the emitter and thus the closed-loop gain.

5. **Supply voltage**: +15V DC regulated, derived from the main power supply.

6. **Stage 1 emitter feedback coupling capacitor**: Ce1 = 4.7 MFD connects TR-1's emitter to the R-10/LDR feedback junction (NOT to ground). Re1 (33K) provides the separate DC path from emitter to ground. Ce1 AC-couples the feedback signal from R-10 to the emitter, providing series-series negative feedback.

### 1.3 Position in Signal Chain

```
Reed vibration
  -> Electrostatic pickup (all 64 reeds summed at single pickup plate)
  -> Polarizing network (R-1 = 22K series, R-2 = 2M to +15V, R-3 = 470K to ground)
  -> D-1 (25 PIV, 10 mA protection diode, Wurlitzer part #142136)
  -> .022 uF input coupling cap
  -> TR-1 base (Stage 1)
  -> TR-1 collector = TR-2 base (direct coupling)
  -> TR-2 collector
  -> R-9 (6.8K series output)
  -> R-10 (56K) feedback to TR-1 emitter (via Ce1); LG-1 (LDR) shunts feedback junction (tremolo)
  -> Volume pot (3K audio taper)
  -> C-8 coupling cap to power amplifier
```

---

## 2. Complete Schematic with Component Values

### 2.1 Schematic Diagram

```
                        +15V Regulated
                          |
                     Rc1 (150K)        Rc2 (1.8K)
                          |                 |
   147V DC                |                 |
     |                    |  C-3 (100pF)    |  C-4 (100pF)
    R-1 (22K)             |--||--+          |--||--+
     |                    |      |          |      |
     +---R-2 (2M)---+15V |      |          |      |
     |               |    |TR-1  |          |TR-2  |
     +---R-3 (470K)--+  C B     |        C B      |
     |               |    |--+   |          |--+   |
     |          D1---+    |  |   |          |  |   |
     |           |        |  E   |          |  E   |
     |          GND       |  |   |          |  |   |
     |                    | Re1(33K)        | Re2a(270)+Ce2(22MFD bypass)
     |          GND  |    |  |              |  |
     |               |    | Ce1(4.7MFD)     | Re2b(820) unbypassed
     |          .022uF    | (coupling cap)  |  |
     |          coupling  |  |              GND
     |          cap  |   GND |
     |               |      fb_junct---R-10(56K)---Output
     |               +------->   |
     |                    TR-1  Pin 1 (cable)
     |                    base   |
     |                          50K VIBRATO
     +--- TR-2 collector --->  R-9 (6.8K) ---> Output    |
                                                         18K
                                                          |
                                                     LG-1 (LDR)
                                                          |
                                                         GND
```

### 2.2 Component Values Table

| Ref | Value | Function |
|-----|-------|----------|
| R-1 | 22K | Series input from reed bar |
| R-2 | 2 MEG | DC bias from +15V to TR-1 base (see Note 1) |
| R-3 | 470K | DC bias to ground from TR-1 base |
| Rc1 | 150K | TR-1 collector load resistor |
| Re1 | 33K | TR-1 emitter degeneration resistor |
| Ce1 | 4.7 MFD | Feedback coupling cap: AC-couples TR-1 emitter to R-10/LDR feedback junction (NOT a bypass cap — see Section 7) |
| Rc2 | 1.8K | TR-2 collector load resistor |
| Re2a | 270 ohm | TR-2 emitter resistor (bypassed by Ce2) |
| Ce2 | 22 MFD | Emitter bypass cap across Re2a (270 ohm) |
| Re2b | 820 ohm | TR-2 emitter resistor (unbypassed) |
| R-9 | 6.8K | Series output resistor |
| R-10 | 56K | Feedback resistor from output to TR-1 emitter junction (via Ce1); LDR shunts this junction for tremolo |
| C-3 | 100 pF | TR-1 collector-base feedback capacitor (Miller) |
| C-4 | 100 pF | TR-2 collector-base feedback capacitor (Miller) |
| D-1 | 25 PIV, 10 mA (part #142136) | Reverse-polarity transient protection at input |
| Input coupling | .022 uF | AC coupling at preamp input |
| LG-1 | CdS LDR in lightproof enclosure with LED | Tremolo gain modulation in feedback network |

**Note 1 (R-2):** Schematic reads "1 MEG" but DC analysis and GroupDIY's 380K impedance (= 2M || 470K) confirm 2M. Use 2M.

**Note 2 (C20/C-1):** The 220 pF capacitor labeled C20 or C-1 in some Wurlitzer schematics appears only on the 206A board. The verified 200A schematic (#203720-S-3, serial 102905+) does NOT include this component. Bass rolloff in the 200A comes from the pickup's system RC HPF (f_c = 2312 Hz, see pickup-system.md Section 3.7), not from a preamp input capacitor.

### 2.3 Polarizing Voltage Circuit

```
AC Mains -> Power Transformer (dedicated winding)
         -> Half-wave rectifier diode
         -> RC filter chain (3 x 0.33 uF caps with series resistors)
         -> 1 MEG feed resistor (component 56 in HV supply filter chain)
         -> Reed bar pickup plate (all 64 reeds in parallel)
```

The feed resistor topology from +150V supply to the pickup plate is:
```
+150V → R_feed (1M) → pickup plate → R-1 (22K) → .022 coupling cap → preamp input node
```

With R_feed = 1M, the total effective resistance seen by the pickup for the RC HPF is:
```
R_total = R_feed || (R-1 + R-2||R-3) = 1M || (22K + 380K) = 1M || 402K = 287K
Pickup RC f_c = 1/(2*pi * 287K * 240pF) = 2312 Hz
```

| Component | Value | Notes |
|-----------|-------|-------|
| Polarizing voltage | 147V DC | |
| Feed resistor (R_feed) | 1 MEG | Component 56 in HV supply filter chain. Avenson's "499K" refers to his replacement design, not the original 200A. |
| Filter capacitors | 3 x 0.33 uF | |
| Rectifier | Half-wave | |

---

## 3. Transistor Specifications

### 3.1 Original Transistor: 2N2924

The original transistors used in early Wurlitzer 200/200A instruments.

| Parameter | Value |
|-----------|-------|
| Type | NPN silicon planar epitaxial |
| Package | TO-92 |
| Vceo (max) | 25V |
| Ic (max) | 100 mA |
| hFE (DC current gain) | 150 to 300 |
| Power dissipation | 625 mW |
| Application | AF small amplifiers, direct-coupled circuits |
| Wurlitzer part number | 142083-2 (for TR-3, TR-4 tremolo oscillator) |

### 3.2 Replacement Transistor: 2N5089

Later production and all current replacements use the 2N5089.

| Parameter | Value |
|-----------|-------|
| Type | NPN silicon, high-gain, low-noise |
| Package | TO-92 |
| Vceo (max) | 25V |
| Ic (max) | 50 mA |
| hFE at Ic=0.1mA, Vce=5V | 400 to 1200 |
| hFE at Ic=1mA, Vce=5V | 450 to 1800 |
| fT (gain-bandwidth product) | 50 MHz at Vce=5V, Ic=0.5mA |
| Noise figure (NF) | 2.5 dB typical at 1 kHz, Rg=10k |
| Cob (output capacitance) | 2.5 pF typical at Vcb=10V |

### 3.3 Key Differences: 2N2924 vs 2N5089

| Parameter | 2N2924 (original) | 2N5089 (replacement) | Impact on Sound |
|-----------|-------------------|---------------------|-----------------|
| hFE | 150-300 | 450-1800 | Higher gain, more headroom before saturation |
| Noise | Higher | Lower (purpose-designed low-noise) | Cleaner signal, less hiss |
| Cob | ~4-8 pF (est.) | 2.5 pF | Different Miller-effect frequency; replacement has less HF feedback |

**Modeling note:** The 2N5089 (hFE >= 450) is the relevant transistor for most surviving instruments. The 2N2924 (hFE 150-300) gives a different tonal character — lower gain, earlier saturation, more distortion. A model targeting the "typical" 200A sound should use 2N5089 parameters. For the bias calculations below, we use hFE = 800 as a representative mid-range value for the 2N5089 at the relevant operating currents.

---

## 4. DC Bias Analysis

### 4.1 Measured Operating Points

DC voltages from the Wurlitzer 200A schematic and confirmed by GroupDIY multimeter measurements:

| Transistor | Pin | Voltage (V) |
|-----------|-----|-------------|
| TR-1 (Stage 1) | Emitter | 1.95 |
| TR-1 (Stage 1) | Base | 2.45 |
| TR-1 (Stage 1) | Collector | 4.1 |
| TR-2 (Stage 2) | Emitter | 3.4 |
| TR-2 (Stage 2) | Base | 4.1 |
| TR-2 (Stage 2) | Collector | 8.8 |
| Supply (Vcc) | — | +15.0 |

**Note:** The GroupDIY multimeter measurements (E=1.923, B=2.447, C=3.98 for TR-1; E=3.356, B=3.988, C=8.45 for TR-2) are close but not identical to the schematic annotations. The differences likely reflect tolerance variations in resistors and transistor hFE in that particular instrument. We use the schematic annotation values as the design-center operating point.

### 4.2 Direct Coupling Verification

TR-1 collector = 4.1V; TR-2 base = 4.1V. These are identical, confirming **no coupling capacitor** between stages. TR-1's DC collector voltage directly sets TR-2's base bias.

### 4.3 Stage 1 (TR-1) Operating Point Derivation

**Known/verified values:**
- Ve1 = 1.95V, Vb1 = 2.45V, Vc1 = 4.1V
- Rc1 = 150K, Re1 = 33K, Ce1 = 4.7 MFD (feedback coupling cap to fb_junct — see Section 7.2)
- Vbe1 = 2.45 - 1.95 = 0.50V (lower than typical ~0.65V; see Note 1 — schematic annotations may be approximate)
- Vce1 = 4.1 - 1.95 = 2.15V
- Vcc = 15.0V

**Collector current from Rc1:**
```
Ic1 = (Vcc - Vc1) / Rc1 = (15.0 - 4.1) / 150K = 10.9V / 150K = 72.7 uA
```

**Emitter current from Re1:**
```
Ie1 = Ve1 / Re1 = 1.95V / 33K = 59.1 uA
```

**Consistency check:** Ie1 = Ic1 + Ib1. For hFE = 800: Ib1 = 72.7/800 = 0.091 uA. Then Ie1 = 72.7 + 0.091 = 72.8 uA. The calculated Ie1 from Re1 (59.1 uA) is ~19% lower than the calculated Ic1 from Rc1 (72.7 uA). This discrepancy is within the range of resistor tolerances (10-20% carbon composition resistors were standard in the era) and the limited resolution of schematic annotation. The true operating current is in the range of 59-73 uA. For calculations below, we use the average: **Ic1 ~ 66 uA**.

**Transconductance:**
```
gm1 = Ic1 / Vt = 66 uA / 26 mV = 2.54 mA/V
```

**Small-signal emitter resistance:**
```
re1 = 1/gm1 = 1/2.54 mA/V = 394 ohm
```

**Base bias network:**
```
R-2 (to +15V) = 2 MEG
R-3 (to ground) = 470K

Thevenin voltage: Vth = 15 * 470K / (2M + 470K) = 15 * 0.190 = 2.854V
Thevenin resistance: Rth = 2M || 470K = (2M * 470K) / (2M + 470K) = 380K

Base voltage check: Vb = Vth - Ib * Rth = 2.854 - 0.091uA * 380K = 2.854 - 0.035 = 2.82V
```

The calculated Vb (2.82V) is above the schematic annotation of 2.45V by 0.37V. This larger-than-expected discrepancy could indicate: (a) the schematic Vb annotation is approximate/rounded, (b) additional DC loading on the base node not accounted for in this simplified bias model, or (c) the actual R-2 is closer to 1 MEG (as labeled on the schematic), which would give Vth = 4.8V — even further from 2.45V. **The Vb discrepancy does NOT affect the gain, Miller pole, or clipping calculations**, which depend on Ic (derived from Vc and Rc on the collector side) and Vce (Vc - Ve), both of which are unambiguously verified from the schematic.

### 4.4 Stage 2 (TR-2) Operating Point Derivation

**Known/verified values:**
- Ve2 = 3.4V, Vb2 = 4.1V, Vc2 = 8.8V
- Rc2 = 1.8K, Re2 = 270 ohm (bypassed) + 820 ohm (unbypassed) = 1090 ohm total
- Vbe2 = 4.1 - 3.4 = 0.70V
- Vce2 = 8.8 - 3.4 = 5.4V
- Vcc = 15.0V

**Collector current from Rc2:**
```
Ic2 = (Vcc - Vc2) / Rc2 = (15.0 - 8.8) / 1.8K = 6.2V / 1.8K = 3.44 mA
```

**Emitter current from Re2_total:**
```
Ie2 = Ve2 / Re2_total = 3.4V / 1090 ohm = 3.12 mA
```

**Consistency check:** Ic2 (3.44 mA) vs Ie2 (3.12 mA) — ~10% discrepancy, again within resistor tolerance range. Average: **Ic2 ~ 3.3 mA**.

**Transconductance:**
```
gm2 = Ic2 / Vt = 3.3 mA / 26 mV = 127 mA/V
```

**Small-signal emitter resistance:**
```
re2 = 1/gm2 = 1/127 mA/V = 7.9 ohm
```

**Direct coupling bias:** TR-2's base voltage (4.1V) is set directly by TR-1's collector voltage (4.1V). The direct coupling creates a DC dependency: if TR-1's collector shifts (due to signal-dependent bias or temperature), TR-2's operating point shifts with it. This is the mechanism behind the "sag" and "bloom" effects at high drive levels.

### 4.5 Summary of DC Analysis

| Parameter | Stage 1 (TR-1) | Stage 2 (TR-2) |
|-----------|----------------|----------------|
| Vb | 2.45V | 4.1V |
| Ve | 1.95V | 3.4V |
| Vc | 4.1V | 8.8V |
| Vbe | 0.50V (low — see Section 4.3 note) | 0.70V |
| Vce | 2.15V | 5.4V |
| Rc | 150K | 1.8K |
| Re | 33K (Ce1 = 4.7 MFD couples emitter to fb_junct) | 270 ohm (bypassed) + 820 ohm |
| Ic | ~66 uA | ~3.3 mA |
| gm | ~2.54 mA/V | ~127 mA/V |
| re (1/gm) | 394 ohm | 7.9 ohm |

### 4.6 Key Architectural Insight

The component values reveal a fundamentally different architecture than what was previously estimated:

**Stage 1 is a HIGH-GAIN, LOW-CURRENT voltage amplifier:**
- Ic1 = 66 uA (very low current — quiet, low power)
- Rc1 = 150K (very high collector load — maximum voltage gain per milliamp)
- Re1 = 33K (large DC stabilization; separate DC path from emitter to ground)
- Ce1 is a feedback coupling cap (emitter to fb_junct), NOT a simple bypass cap to ground. The open-loop gain depends on the impedance at fb_junct (LDR path + R-10). When the LDR path impedance is low, Ce1 effectively AC-grounds the emitter, giving Av1 = gm1*Rc1 = 420. When LDR path is high, the emitter sees R-10 and has significant degeneration.
- Without Ce1 (DC): Av1 = -Rc1/Re1 = -150K/33K = **-4.5**

**Stage 2 is a LOW-GAIN, HIGH-CURRENT buffer/output stage:**
- Ic2 = 3.3 mA (50x higher current than Stage 1)
- Rc2 = 1.8K (low collector load — current drive capability)
- Re2_unbypassed = 820 ohm (sets AC gain)
- Av2 = -Rc2 / (re2 + Re2_unbypassed) = -1800 / (7.9 + 820) = **-2.2**

**Combined open-loop gain (Ce1 bypassed):**
```
Av_open = Av1 * Av2 = 381 * 2.2 = 838 (approximately 912 using the per-resistor Ic values: 420 * 2.17 = 912)
```

Using the individual per-resistor Ic values (Ic1 from Rc1 = 72.7 uA, Ic2 from Rc2 = 3.44 mA):
```
Av1 = (72.7 uA / 26 mV) * 150K = 2.80 * 150K = 420
Av2 = 1800 / (7.9 + 820) = 2.17
Av_open = 420 * 2.17 = 912
```

We use **Av_open ~ 900** as a round figure for the combined open-loop gain.

### 4.7 Overall Gain

Brad Avenson (professional audio designer who built a replacement Wurlitzer preamp) measured the total preamp gain at **approximately 15 dB (voltage gain approximately 5.6x)**. He stated: "the preamp really only needs 15 dB." Volume pot output was measured at **2-7 mV AC**.

**SPICE-measured gain (corrected emitter feedback topology):**
- **No tremolo (Rldr_path = 1M):** 6.0 dB (2.0x) at 1 kHz
- **Tremolo bright (Rldr_path = 19K):** 12.1 dB (4.0x) at 1 kHz
- **Tremolo modulation range:** 6.1 dB

**Reconciliation with Avenson's "15 dB" measurement:** Avenson measured ~15 dB (5.6x) for his replacement preamp design (which uses 499K instead of 1M for R_feed and may have different feedback topology). The original 200A with corrected emitter feedback gives **6 dB (2x) without tremolo** and up to **12 dB (4x) at tremolo peak**. The 15 dB figure does NOT match the original circuit — it's either Avenson's replacement design or a measurement with tremolo active at bright peak.

**Gain structure:**
- Maximum open-loop gain (fb_junct grounded, Re1 bypassed via Ce1): ~900 (59 dB)
- Combined degenerated gain (Ce1 open, DC): 4.5 * 2.2 = 9.9 (20 dB)
- SPICE-measured closed-loop gain: 6.0 dB (2.0x) without tremolo
- The strong emitter feedback (loop gain ≈ 900/2.0 = 450, or 53 dB) provides excellent gain stability and linearization.

---

## 5. AC Signal Analysis

### 5.1 Input Signal Levels

The pickup system delivers millivolt-level signals to the preamp input. Based on the electrostatic analysis (see pickup-system.md):

| Condition | Estimated Preamp Input Level | Notes |
|-----------|------------------------------|-------|
| C4 at pp (vel=0.3) | ~0.1-0.5 mV peak | Electrostatic calculation |
| C4 at mf (vel=0.7) | ~1-5 mV peak | Avenson measured 2-7 mV at output on his replacement design (15 dB gain); original 200A gain is 6.0 dB (2.0x) |
| C4 at ff (vel=0.95) | ~5-15 mV peak | |
| Bass (A1 at mf) | ~0.05-0.2 mV peak | Heavily attenuated by pickup RC HPF |
| Treble (C6 at mf) | ~5-20 mV peak | Less attenuation, smaller displacement |

### 5.2 Input Coupling Network

The input coupling network consists of:
- **.022 µF coupling cap** — blocks the 147V DC polarizing voltage from reaching the preamp
- **R-2 (2M) and R-3 (470K)** — bias divider providing DC path to TR-1 base
- **D-1 protection diode** — clamps transient overvoltages (e.g., from reed-to-plate shorts during tuning)

The .022 µF coupling cap has a corner frequency of ~19 Hz (with the 380K bias network), so it's effectively a short circuit at all audio frequencies.

**IMPORTANT — C20 is NOT on the 200A:** The 220 pF capacitor labeled C20 or C-1 in some Wurlitzer schematics appears only on the 206A board. The verified 200A schematic (#203720-S-3, serial 102905+) does NOT include this component at the preamp input. Bass rolloff in the 200A comes from the **pickup's system RC HPF** (f_c = 2312 Hz from R_total = 287K and C_total = 240 pF; see pickup-system.md Section 3.7), not from a preamp input capacitor.

### 5.3 Small-Signal Gain of Each Stage

#### Stage 1 (TR-1)

**Ce1 coupling at audio frequencies (Ce1 = 4.7 MFD couples emitter to fb_junct):**

Ce1 is a feedback coupling cap from emitter to the R-10/LDR feedback junction, NOT a simple bypass cap across Re1 to ground. The effective emitter AC impedance depends on the impedance at fb_junct (LDR path to ground || R-10 to output). The corner frequency for Ce1 coupling is ~1 Hz, so it's effectively an AC short at all audio frequencies. However, the gain depends on what's at fb_junct — see Section 7.2.

The gain below assumes fb_junct has low impedance to ground (LDR path active, tremolo bright phase):
```
Av1 = -gm1 * Rc1 = -2.54 mA/V * 150K = -381
```

Using the per-Rc1 current (Ic1 = 72.7 uA, gm1 = 2.80 mA/V):
```
Av1 = -2.80 * 150K = -420
```

**Without bypass cap (DC stability, below 1 Hz):**
```
Av1_DC = -Rc1/Re1 = -150K/33K = -4.5
```

#### Stage 2 (TR-2)

**At audio frequencies (Ce2 = 22 MFD bypasses Re2a = 270 ohm):**

The 22 MFD bypass cap across 270 ohm has a corner frequency of:
```
f_bypass2 = 1 / (2 * pi * 270 * 22e-6) = 26.8 Hz
```

Above ~27 Hz, Re2a is bypassed. The AC gain is set by the unbypassed Re2b = 820 ohm:
```
Av2 = -Rc2 / (re2 + Re2b) = -1800 / (7.9 + 820) = -2.17
```

**Below ~27 Hz (full emitter degeneration):**
```
Av2_LF = -Rc2 / (re2 + Re2_total) = -1800 / (7.9 + 1090) = -1.64
```

**Important: Stage 2 has NO emitter bypass for the 820 ohm resistor.** The 820 ohm sets the AC gain permanently at ~2.2x. This is a low-gain, current-drive buffer stage.

### 5.4 Miller-Effect Analysis

The collector-base capacitors C-3 and C-4 create the **Miller effect**: the effective input capacitance at each transistor's base is amplified by the voltage gain:

```
C_miller = C_cb * (1 + |Av|)
```

For the 2N5089, Cob ~ 2.5 pF (at Vcb = 10V). The external C-3 and C-4 (both 100 pF) dominate:

```
C_cb_total = Cob + C_external
Stage 1: C_cb1 = 2.5 + 100 = 102.5 pF
Stage 2: C_cb2 = 2.5 + 100 = 102.5 pF
```

#### Stage 1 Miller Effect

```
C_miller1 = C_cb1 * (1 + |Av1|) = 102.5 pF * (1 + 420) = 102.5 * 421 = 43,153 pF = 43.2 nF
```

This enormous effective capacitance creates a dominant pole. The source impedance seen at TR-1's base determines the pole frequency:

```
r_pi1 = beta / gm1 = 800 / 2.80 mA/V = 286K
R_bias = R-2 || R-3 = 2M || 470K = 380K
R_source1 = R_bias || r_pi1 = 380K || 286K = (380K * 286K) / (380K + 286K) = 163K

f_miller1 = 1 / (2 * pi * R_source1 * C_miller1)
          = 1 / (2 * pi * 163K * 43.2 nF)
          = 1 / (2 * pi * 7.04e-3)
          = 22.6 Hz
```

#### Stage 2 Miller Effect

```
C_miller2 = C_cb2 * (1 + |Av2|) = 102.5 pF * (1 + 2.17) = 102.5 * 3.17 = 325 pF
```

The source impedance at TR-2's base is the output impedance of Stage 1:
```
r_pi2 = beta / gm2 = 800 / 127 mA/V = 6.3K
R_source2 = Rc1 || r_pi2 = 150K || 6.3K = (150K * 6.3K) / (150K + 6.3K) = 6.05K

f_miller2 = 1 / (2 * pi * R_source2 * C_miller2)
          = 1 / (2 * pi * 6.05K * 325 pF)
          = 1 / (2 * pi * 1.966e-6)
          = 81 kHz
```

### 5.5 Frequency-Dependent Feedback from C-3 and C-4

The collector-base feedback caps create **shunt-shunt negative feedback** (current feedback from collector to base). The key behavior:

At **low frequencies** (f << f_dominant_pole ~ 23 Hz): C-3 has high impedance, negligible feedback current. Stage 1 operates at full open-loop gain (~420). Since this is below the audio band, it only affects subsonic signals and DC stabilization.

At **audio frequencies** (f >> 23 Hz): C-3 has low impedance relative to the circuit impedances. The Miller multiplication creates heavy feedback, controlling the gain. The open-loop gain rolls off at -20 dB/decade from the 23 Hz pole.

**The real Miller-effect direction:**
- At LOW frequencies: capacitor has high impedance -> LESS feedback -> MORE gain -> MORE distortion
- At HIGH frequencies: capacitor has low impedance -> MORE feedback -> LESS gain -> LESS distortion

**Critical insight:** With the dominant pole at 23 Hz, the open-loop gain is already rolling off throughout the entire audio band. At 1 kHz, the open-loop gain of Stage 1 alone has dropped to approximately:
```
|Av1(1kHz)| = 420 / sqrt(1 + (1000/23)^2) = 420 / 43.5 = 9.7
```

At 10 kHz:
```
|Av1(10kHz)| = 420 / sqrt(1 + (10000/23)^2) = 420 / 435 = 0.97
```

This means the open-loop gain of Stage 1 alone is less than unity above ~10 kHz! Combined with Stage 2 (Av2 = 2.2), the two-stage open-loop gain at 10 kHz is only ~2.1. The feedback can only reduce gain below the open-loop value, so above ~10 kHz, the closed-loop gain approaches the open-loop gain.

### 5.5.1 Nested Feedback Loops — Critical Finding (Feb 2026 SPICE Analysis)

**The preamp has TWO nested feedback loops, not one.** This has major implications for bandwidth modeling.

| Loop | Components | Type | Effect |
|------|-----------|------|--------|
| **Inner** | C-3 (100pF) coll1→base1, C-4 (100pF) coll2→coll1 | Local Miller feedback | Dominates bandwidth; sets BW at ~15.5 kHz |
| **Outer** | R-10 (56K) via Ce1 to emitter, R_ldr shunt | Global emitter feedback | Controls gain (2x–4x); barely affects BW |

**SPICE experiments confirmed:**

1. **Preamp-only bandwidth (base1→out) is nearly constant regardless of R_ldr:**

| R_ldr | Gain (dB) | Gain (x) | BW (Hz) | GBW (Hz) |
|-------|-----------|----------|---------|----------|
| 1M | 6.70 | 2.16 | 15,686 | 33,905 |
| 19K | 12.83 | 4.38 | 15,211 | 66,647 |

   BW changes by only 3% (15,686 → 15,211 Hz) despite a 2x gain change. GBW is **NOT constant** — it scales proportionally with gain. This is the signature of a nested-loop topology where the inner loop dominates bandwidth.

2. **Ce1 is NOT the mechanism for variable BW.** Replacing Ce1 with a wire (huge capacitance) produces identical results. Ce1's impedance is negligible at audio frequencies (34Ω at 1 kHz). Its only job is DC blocking.

3. **The feedback fraction beta is frequency-independent in the audio band.** Delta-beta between R_ldr=1M and R_ldr=19K is flat at 6.17 dB from 100 Hz to 5 kHz. The R_ldr/R-10 resistive divider is the sole gain-control mechanism.

4. **The inner loop (C-3/C-4) determines bandwidth.** With R-10 disconnected (outer loop open), the combined two-stage gain is only ~7x (16.9 dB) — the inner Miller feedback has already reduced the gain from 912x to 7x. The bandwidth of this inner-loop-limited amplifier is ~15.5 kHz.

**Why the earlier analysis reported 9.9 kHz / 8.3 kHz bandwidth:**

Those measurements were **full-chain** (V(in_sig)→V(out)), which includes the input coupling network (R-1=22K, C_in=0.022µF, R-2/R-3 bias). The input network's HF attenuation increases with gain because:
- Higher gain → more Miller multiplication of C-3 → lower impedance at base1
- Lower base1 impedance → more loading on the input coupling network at HF
- This adds ~0.8 dB extra attenuation at 10 kHz when R_ldr drops from 1M to 19K

| Measurement | R_ldr=1M BW | R_ldr=19K BW | BW Ratio |
|------------|-------------|--------------|----------|
| Preamp-only (base1→out) | 15,686 Hz | 15,211 Hz | 0.97 |
| Full-chain (in_sig→out) | 11,760 Hz | 9,674 Hz | 0.82 |

**Implications for DSP modeling:**

A model that treats Stage 1 as having constant GBW (e.g., gain=420, pole=23 Hz, GBW≈9.7 kHz) will incorrectly halve the bandwidth when gain doubles. The real preamp maintains ~15.5 kHz bandwidth because the inner C-3/C-4 Miller loop dominates. Correctly modeling this requires either:
- A coupled solver that captures the inner Miller feedback within each stage
- A Wave Digital Filter model that handles the reactive feedback elements natively
- Or, as a pragmatic approximation, parameterizing the Miller pole as a function of outer-loop gain

**SPICE testbench:** `spice/testbench/tb_variable_gbw.cir`

### 5.6 Closed-Loop Frequency Response

The overall preamp is a two-stage amplifier with:
- **DC open-loop gain:** ~900 (59 dB)
- **Dominant pole:** ~23 Hz (from Stage 1 C-3 Miller)
- **Second pole:** ~81 kHz (from Stage 2 C-4 Miller)
- **Closed-loop gain (set by R-10 emitter feedback, SPICE AC sweep):**
  - **No tremolo (LDR dark, Rldr_path ≈ 1M):** Peak gain = **6.05 dB (2.01x)** at 447 Hz. Gain at 1 kHz = **6.0 dB (2.0x)**.
  - **Tremolo bright (Rldr_path ≈ 19K):** Gain at 1 kHz = **12.1 dB (4.0x)**.
  - **Tremolo modulation range: ~6.1 dB** (matches EP-Forum "6 dB boost" measurement exactly).
  - The gain is remarkably constant with input level (2.007x from pp to extreme) — the strong emitter feedback linearizes the circuit effectively.

**GBW is NOT constant** (see Section 5.5.1 for full analysis):
```
Preamp-only GBW (base1→out):
  R_ldr = 1M:  GBW = 33,905 Hz  (gain 2.16x, BW 15,686 Hz)
  R_ldr = 19K: GBW = 66,647 Hz  (gain 4.38x, BW 15,211 Hz)
```

The preamp-only bandwidth is nearly constant at ~15.5 kHz because the inner
C-3/C-4 Miller feedback loop dominates bandwidth, independent of the outer
R-10 emitter feedback loop that controls gain. GBW scales with gain (2:1 ratio).

The **full-chain** bandwidth (including input coupling network loading) is narrower:
```
Full-chain -3 dB bandwidth (in_sig→out, from SPICE AC sweep):
  f_low = 19 Hz, f_high = 11,760 Hz (no tremolo, Rldr_path = 1M)
  f_high = 9,674 Hz (tremolo bright, Rldr_path = 19K)
```

The full-chain bandwidth decreases with gain because higher gain increases
Miller multiplication of C-3, loading the input coupling network more at HF.
Earlier analysis reported 9.9 kHz and 8.3 kHz, likely measured with
R_ldr=120K (the original netlist default) and R_ldr=19K respectively.

**Open-loop gain at key frequencies (combined two stages):**

| Frequency | Stage 1 |Av1| | Stage 2 |Av2| | Combined |Av_open| | Notes |
|-----------|---------|---------|---------|-------|
| 10 Hz | 420 | 2.17 | 912 | Below dominant pole; full gain |
| 23 Hz | 297 | 2.17 | 645 | Dominant pole (-3 dB on Stage 1) |
| 100 Hz | 96.6 | 2.17 | 210 | Open-loop rolling off |
| 500 Hz | 19.3 | 2.17 | 42 | |
| 1 kHz | 9.7 | 2.17 | 21 | |
| 2 kHz | 4.8 | 2.17 | 10.5 | |
| 3.7 kHz | 2.6 | 2.17 | 5.6 | Open-loop still well above closed-loop (2.0x) |
| 5 kHz | 1.93 | 2.17 | 4.2 | Below closed-loop target |
| 10 kHz | 0.97 | 2.17 | 2.1 | Stage 1 < unity; no feedback possible |
| 20 kHz | 0.48 | 2.17 | 1.05 | |

**Closed-loop gain at key frequencies (SPICE-measured):**

The emitter feedback (R-10 via Ce1) holds the gain at ~2.0x (6.0 dB) without tremolo. The -3 dB bandwidth is ~9.9 kHz. Above this, gain rolls off as the open-loop gain drops below the closed-loop target.

| Frequency | Closed-Loop Gain | Closed-Loop (dB) | Notes |
|-----------|-----------------|-------------------|-------|
| 19 Hz | 2.0 | 6.0 | Low-frequency -3 dB point |
| 100 Hz | 2.0 | 6.0 | Feedback-controlled |
| 447 Hz | ~2.1 | ~6.4 | Peak gain (mild peaking) |
| 1 kHz | 2.0 | 6.0 | SPICE reference measurement point |
| 5 kHz | ~1.9 | ~5.6 | Still feedback-controlled |
| 9.9 kHz | ~1.4 | ~3.0 | -3 dB point |
| 20 kHz | ~0.7 | ~-3.1 | Significant rolloff |

With tremolo at bright peak (Rldr_path = 19K), gain increases to 4.0x (12.1 dB), and BW narrows to ~8.3 kHz (constant GBW product).

### 5.7 Preamp Frequency Response

The preamp's closed-loop response (no tremolo, Rldr_path = 1M):

| Frequency | Gain (dB) | Notes |
|-----------|-----------|-------|
| 19 Hz | ~3.0 | Low-frequency -3 dB point |
| 100 Hz | 6.0 | Passband |
| 447 Hz | ~6.4 | Mild gain peak from feedback loop resonance (SPICE) |
| 1 kHz | 6.0 | Reference measurement point |
| 5 kHz | ~5.6 | Approaching HF rolloff |
| 9.9 kHz | ~3.0 | High-frequency -3 dB point |
| 20 kHz | ~-3.1 | Significant HF rolloff |

**Tonal shaping:** The preamp itself has nearly flat gain from 19 Hz to 9.9 kHz (with a mild peak at ~447 Hz from feedback loop resonance). The Wurlitzer's characteristic mid-forward tonal balance comes from the **pickup's system RC HPF at 2312 Hz** (which heavily attenuates bass fundamentals before they reach the preamp) combined with the preamp's HF rolloff. This creates the signature sound: bass notes sound thin and "reedy" (harmonics dominate over fundamentals), while the midrange (500 Hz - 5 kHz) has body and bark.

With tremolo at bright peak (Rldr_path = 19K), gain increases to 12.1 dB (4.0x). The preamp-only BW barely changes (~15.2 kHz), but the full-chain BW narrows to ~9.7 kHz due to increased Miller loading on the input coupling network. See Section 5.5.1 for the nested-loop analysis.

### 5.8 Emitter Bypass Cap Corner Effects

#### Stage 1 (Ce1 = 4.7 MFD coupling emitter to fb_junct; Re1 = 33K DC path to GND)

Corner frequency of Ce1 relative to Re1: f = 1/(2*pi*33K*4.7uF) = **1.03 Hz**. Ce1's impedance is negligible across the entire audio band, so it effectively couples the emitter to fb_junct at all audible frequencies.

#### Stage 2 (Ce2 = 22 MFD across Re2a = 270 ohm)

Corner frequency: f = 1/(2*pi*270*22uF) = **26.8 Hz**. Below ~27 Hz, both 270 ohm and 820 ohm are in the emitter circuit (1090 ohm total, Av2 = 1.64). Above ~27 Hz, only 820 ohm matters (Av2 = 2.17). This is a subtle gain increase (~2.4 dB) in the transition from subsonic to audio. Not perceptually significant since other factors dominate in this range.

---

## 6. Harmonic Generation — Why H2 > H3

### 6.1 The Exponential Transfer Function

A single BJT common-emitter stage has the Ebers-Moll exponential transfer:

```
Ic = Is * exp(Vbe / (n*Vt))
```

The Taylor expansion of the exponential around the operating point gives:

```
ic(t) = gm * vbe + (gm^2 / (2*Ic)) * vbe^2 + (gm^3 / (6*Ic^2)) * vbe^3 + ...
```

The **second-order term (vbe^2)** generates second harmonic (H2). The **third-order term (vbe^3)** generates third harmonic (H3). For the exponential function, the ratio of H2 to H3 amplitude is:

```
H2/H3 = 3 * Vt / (2 * Vpeak)
```

For a signal with Vpeak = 5 mV (Vt = 26 mV):
```
H2/H3 = 3 * 0.026 / (2 * 0.005) = 7.8 = +17.8 dB
```

**The exponential nonlinearity produces predominantly second harmonic.** H3 is typically 20+ dB below H2 at moderate signal levels.

### 6.2 Asymmetric Clipping from Unequal Headroom

**Stage 1 has moderately asymmetric headroom (with correct values):**
- Vce1 = 4.1 - 1.95 = 2.15V
- Toward saturation (Vce -> Vce_sat ~ 0.1V): 2.15 - 0.1 = **2.05V** of swing available
- Toward cutoff (Ic -> 0, Vc -> Vcc = 15V): 15.0 - 4.1 = **10.9V** of swing available
- Asymmetry ratio: 10.9 / 2.05 = **5.3:1**

**Stage 2 has nearly symmetric headroom:**
- Vce2 = 8.8 - 3.4 = 5.4V
- Toward saturation: 5.4 - 0.1 = **5.3V** available
- Toward cutoff: 15.0 - 8.8 = **6.2V** available
- Asymmetry ratio: 6.2 / 5.3 = **1.17:1** (nearly symmetric)

When the input signal drives Stage 1's collector voltage:
- Positive input -> collector swings DOWN (common-emitter inversion) -> hits saturation limit at 2.05V of swing
- Negative input -> collector swings UP -> can swing 10.9V before cutoff

This asymmetry means the **positive half-cycle clips much harder than the negative half-cycle**, producing strong even harmonics (H2, H4, H6, ...).

**Compared to the old estimates:** The previous analysis estimated Stage 1 headroom as 1.86V vs 11.0V (ratio 5.9:1). The correct values (2.05V vs 10.9V, ratio 5.3:1) are similar but slightly less extreme. The asymmetry is still very strong — Stage 1 remains the primary source of even-harmonic distortion.

Stage 2's headroom (5.3V vs 6.2V, ratio 1.17:1) is dramatically different from the old estimate (4.89V vs 6.55V, ratio 1.34:1) in absolute terms (much more current, lower Rc) but the asymmetry ratio is actually slightly less. Stage 2 contributes very little asymmetric distortion of its own.

### 6.3 Signal Level Dependence

The preamp input sees millivolt signals. The signal at TR-1's base (after the bias network and input coupling) determines distortion:

| Dynamic Level | Estimated Vbe_ac | Character | Harmonic Content |
|---------------|-----------------|-----------|-----------------|
| pp (vel 0.3) | ~0.05-0.2 mV | Nearly linear | Almost pure fundamental, very faint H2 |
| mf (vel 0.7) | ~0.3-1.5 mV | Mildly nonlinear | H2 at -15 to -20 dB, H3 at -35 to -40 dB |
| ff (vel 0.95) | ~2-8 mV | Moderate saturation | H2 at -8 to -15 dB, H3 at -20 to -30 dB, "bark" |
| ff chord | ~5-20 mV | Heavy saturation | H2, H3, intermodulation products, "growl" |

With Stage 1's open-loop gain of ~420, an input of 5 mV would produce a collector swing of 2.1V — very close to the saturation headroom limit of 2.05V. At forte dynamics, the pickup's 1/(1-y) nonlinearity has already generated significant H2, and Stage 1 approaches its clipping boundary, adding further asymmetric distortion. Both sources contribute to the Wurlitzer's characteristic bark at high dynamics.

### 6.4 Why Single-Ended CE Produces H2 — The Physical Story

A differential pair (like the power amplifier's input stage) produces a **tanh** transfer function, which is an odd function: tanh(-x) = -tanh(x). Odd functions produce only odd harmonics (H3, H5, H7...). This is why push-pull amplifiers are "clean" — they naturally cancel even harmonics.

A **single-ended** common-emitter stage has the **exponential** transfer function, which is NOT symmetric. exp(-x) is not equal to -exp(x). The lack of symmetry means even harmonics (H2, H4, H6...) are present. For the pure exponential, H2 dominates overwhelmingly.

This asymmetric transfer function is why the preamp adds even harmonics when driven hard. At normal dynamics, the pickup's 1/(1-y) nonlinearity is the dominant H2 source; at ff and beyond, the preamp's two cascaded single-ended CE stages contribute additional even harmonics from their exponential transfer functions and asymmetric headroom.

### 6.5 The Role of Saturation vs Cutoff Limits

In the DSP model, the BJT's collector rail limits are modeled as exponential soft-clips:

```
if raw >= 0: output = satLimit * (1 - exp(-raw / satLimit))     // toward Vcc (cutoff)
if raw < 0:  output = -cutoffLimit * (1 - exp(raw / cutoffLimit))  // toward Vce_sat
```

**Stage 1 (with correct values):**
- satLimit = Vcc - Vc1 = 15.0 - 4.1 = 10.9V (cutoff direction)
- cutoffLimit = Vce1 - Vce_sat = 2.15 - 0.1 = 2.05V (saturation direction)
- Ratio: 5.3:1

```
H2_coefficient = 1/(2*cutoffLimit) - 1/(2*satLimit)
               = 1/4.10 - 1/21.8 = 0.244 - 0.046 = 0.198
```

**Stage 2 (with correct values):**
- satLimit = 15.0 - 8.8 = 6.2V
- cutoffLimit = 5.4 - 0.1 = 5.3V
- Ratio: 1.17:1

```
H2_coefficient = 1/(2*5.3) - 1/(2*6.2) = 0.0943 - 0.0806 = 0.014
```

Stage 1's H2 coefficient (0.198) is **14x larger** than Stage 2's (0.014). Stage 1 dominates the harmonic generation.

### 6.6 Stage 2 Contribution

Stage 2 (TR-2) has nearly symmetric headroom (ratio 1.17:1), so it contributes very little H2 from its own clipping asymmetry. However, Stage 2 processes Stage 1's already-distorted output, producing **harmonics of harmonics** — H2 of H2 gives H4, H2 of H3 gives combination tones, etc. The cascaded nonlinearity enriches the harmonic spectrum beyond what a single stage produces.

Additionally, Stage 2 operates at much higher current (3.3 mA vs 66 uA), which gives it a much higher gm (127 mA/V) and smaller re (7.9 ohm). The exponential nonlinearity of the Vbe-Ic relationship is still present, but the unbypassed 820 ohm emitter resistor provides strong local degeneration that linearizes Stage 2 significantly. The signal at Stage 2's base (after feedback reduces Stage 1's gain) is small enough that Stage 2 operates in its linear region for most playing dynamics.

---

## 7. Tremolo Integration — LDR in Feedback Loop

### 7.1 The Critical Finding

The Wurlitzer 200A service manual explicitly states:

> "The reed bar signal is modulated by inserting the vibrato voltage into the feedback loop of the high impedance preamp. A divider is formed by the feedback resistor R-10, and the light dependent resistor of LG-1. The L.D.R., in conjunction with the light emitting diode in the same package, creates a variable leg in the feedback divider and makes possible amplitude modulation of the reed bar voltage."

**This means the tremolo modulates the preamp's GAIN, not just the output volume.** The LDR resistance variation changes the closed-loop gain of the preamp by modifying the feedback network.

### 7.2 Feedback Topology — Emitter Feedback via Ce1

The 200A feedback topology:

```
TR-2 Collector
         |
       R-9 (6.8K) -----> Output to Volume Pot
         |
       R-10 (56K)
         |
       fb_junct ---------> Pin 1 (cable) --> 50K VIBRATO --> 18K --> LG-1 LED
         |
       Ce1 (4.7 MFD coupling cap)
         |
       TR-1 Emitter
         |
       Re1 (33K) -----> GND (DC bias path)
```

**R-10 feeds back from the output to TR-1's EMITTER** via Ce1 (4.7 MFD coupling cap). This is **series-series (emitter) NEGATIVE feedback** — inherently stable because feedback at the emitter opposes the input signal at the base.

Key topology details:
- **Ce1 is a feedback coupling cap**, NOT a bypass cap. It AC-couples the feedback junction (fb_junct) to TR-1's emitter.
- **Re1 (33K)** provides the separate DC path from emitter to ground. DC operating point is unaffected by the feedback network (Ce1 blocks DC).
- **fb_junct** connects via cable Pin 1 to the LDR tremolo shunt circuit: GRY JACKET → 50K VIBRATO pot → 18K → LG-1 pin 2 (LED).
- The LDR (LG-1) shunts the feedback junction to ground, diverting feedback current away from the emitter.

**SPICE-validated DC operating point:**
| Node | Schematic | SPICE |
|------|-----------|-------|
| base1 | 2.45V | 2.80V |
| emit1 | 1.95V | 2.24V |
| coll1 | 4.1V | 4.12V |
| coll2 | 8.8V | 9.07V |
| fb_junct | — | 5.59V |
| out | — | 8.20V |

### 7.3 Gain Modulation by LDR

The LDR modulates gain by varying the AC impedance from fb_junct to ground:

When LDR path impedance is **LOW** (bright phase): fb_junct is shunted to ground → Ce1 effectively grounds the emitter for AC → Re1 is bypassed → Stage 1 runs at full open-loop gain → **HIGHER overall gain**

When LDR path impedance is **HIGH** (dark phase): fb_junct carries the full R-10 feedback signal → Ce1 delivers feedback to emitter → emitter degeneration from feedback → **LOWER overall gain** (strong negative feedback)

**SPICE-measured LDR sweep:**

| Rldr_path | Gain @ 1kHz (dB) | Gain (x) | -3dB BW high (Hz) | Scenario |
|-----------|------------------|----------|-------------------|----------|
| 500 Ω | 34.2 | 51x | 1,749 | Unrealistic (minimum path > 18K) |
| 5 KΩ | 19.6 | 9.5x | 5,900 | |
| 10 KΩ | 15.3 | 5.8x | 7,327 | |
| **19 KΩ** | **12.1** | **4.0x** | **8,334** | **Tremolo bright peak** (18K + 50Ω LDR + ~1K wiring) |
| 50 KΩ | 8.8 | 2.8x | 9,246 | Tremolo half-depth |
| 120 KΩ | 7.2 | 2.3x | 9,639 | Moderate LDR |
| **1 MΩ** | **6.0** | **2.0x** | **9,913** | **No tremolo (LDR dark)** |
| 10 MΩ | 5.9 | 2.0x | 9,948 | LDR fully dark |

**Key findings:**
- **No tremolo (baseline):** Gain = 6.0 dB (2.0x) — strong emitter feedback from R-10 reaching emitter
- **Tremolo bright peak** (Rldr_path ≈ 19K): Gain = 12.1 dB (4.0x)
- **Modulation range: 6.1 dB** — matches EP-Forum "6 dB gain boost" measurement exactly
- **Full-chain bandwidth decreases with gain:** 9.9 kHz at 2x gain → 8.3 kHz at 4x gain (full-chain measurement including input coupling network loading; preamp-only BW is ~15.5 kHz, nearly constant — see Section 5.5.1)
- **Gain is remarkably constant with input level** (2.007x from 0.5mV to 200mV) — the strong feedback linearizes the circuit, producing very low THD (0.0004% at mf, 0.04% at extreme 200mV)

The distortion character changes through the tremolo cycle: at the gain peak (LDR low, weak feedback), the preamp's higher gain amplifies the pickup-generated harmonics more and pushes the preamp closer to its own saturation, producing more apparent H2 and "bark." At the gain trough (LDR high, strong feedback), the preamp operates more linearly. This creates a subtle but important **timbral modulation** that distinguishes the real 200A tremolo from simple volume modulation.

### 7.4 Tremolo Oscillator

| Parameter | Value |
|-----------|-------|
| Transistors | TR-3, TR-4 (2N2924, part 142083-2) |
| Topology | Twin-T (parallel-T) oscillator (notch filter feedback) |
| Frequency | 5.63 Hz (calculated from twin-T RC values); approximately 6 Hz (service manual); measured 5.3-7 Hz |
| Waveform | Approximately sinusoidal (mild distortion from twin-T topology, est. THD 3-10%) |
| Depth control | Front panel vibrato pot (50K) |

### 7.5 Implications for Modeling

The tremolo should be implemented as a **modulation of the emitter feedback amount**, not as a post-preamp volume multiplier. The LDR path impedance controls how much of the R-10 feedback signal reaches TR-1's emitter:

```
// LDR path: fb_junct -> Pin 1 -> 50K VIBRATO -> 18K -> LG-1 -> GND
// Total LDR path impedance = 50K*depth + 18K + R_ldr
// R_ldr varies with the tremolo oscillator's LED drive

// When LDR path impedance is low: emitter is AC-grounded through Ce1 -> higher gain
// When LDR path impedance is high: R-10 feedback reaches emitter -> lower gain
// The feedback modifies the effective emitter degeneration of Stage 1
```

At low preamp drive levels (pp), the distinction between gain modulation and volume modulation is negligible. At high drive levels (ff), the timbral variation through the tremolo cycle becomes audible and important for authenticity.

---

## 8. Modeling Recommendations

### 8.1 Modeling Approaches (Ranked by Fidelity)

**ARCHITECTURE DECISION: Trait-based A/B testing.** Implement a `PreampModel` trait (Rust) or abstract interface (C++) with `process_sample()`, `set_ldr_resistance()`, `reset()`. Build two implementations behind this interface:
1. Full SPICE-derived model (ground truth / reference)
2. Simplified Ebers-Moll (candidate for shipping)

The voice holds a swappable `PreampModel` implementation. A/B testing compares the two until the simplified version is perceptually indistinguishable from the reference. If CPU allows, ship both as a quality toggle.

#### Approach 1: DK Method — Coupled 8-Node MNA Solver — SHIPPING IMPLEMENTATION

Model the complete circuit as an 8-node Modified Nodal Analysis (MNA) system using the Discretization-Kernel (DK) method. Trapezoidal discretization with a precomputed system inverse reduces the per-sample nonlinear solve to a 2x2 Newton-Raphson iteration on Vbe1/Vbe2. Implemented as `DkPreamp` in `dk_preamp.rs`. See [DK Preamp Derivation](dk-preamp-derivation.md) for the full math.

**Pros:** Most accurate harmonic content, correct frequency-dependent gain (captures nested C-3/C-4 Miller loop), proper bias-shift dynamics, explicit R_ldr handling via Sherman-Morrison
**Cons:** Requires solving two coupled implicit equations per sample (Newton-Raphson iteration); ~900 FLOPs/sample (two dk_step calls: main + shadow)
**Role:** Shipping implementation. Correctly models the ~15.5 kHz bandwidth that is independent of R_ldr (see Section 5.5.1).

#### Approach 2: Simplified Ebers-Moll with Feedback Caps — LEGACY REFERENCE

Two independent BjtStage objects with exponential transfer functions, NR solver for feedback, and asymmetric soft-clip for collector limits. Implementation exists in `preamp.rs` as `EbersMollPreamp`.

**Pros:** Captures the key H2 mechanism; reasonable computational cost; NR converges quickly at physical signal levels
**Cons:** Misses inter-stage DC bias modulation ("sag"); constant-GBW model is structurally inadequate (gives ~5.2 kHz BW at trem-bright vs real ~15.5 kHz — see Section 5.5.1)
**Status:** Superseded by DkPreamp. Retained in `preamp.rs` as legacy reference for A/B comparison.

#### Approach 3: Wave Digital Filter (WDF) Model — NOT PURSUED

Model each resistor, capacitor, and transistor junction as a WDF element.

**Pros:** Correct at all operating points; handles direct coupling naturally; well-suited for real-time; would automatically capture the nested feedback loop behavior
**Cons:** No Rust WDF library exists; would require FFI to C++ chowdsp_wdf or a Rust port; WDF junction models for BJTs are nontrivial; R-type adaptor needed for the bridged feedback topology
**Status:** Not pursued. The DK method (Approach 1 variant) achieves the same coupled-system fidelity with a simpler implementation path.

#### Approach 4: Polynomial Approximation — NOT RECOMMENDED

Taylor-expand the transfer function to 3rd or 4th order: `y = a1*x + a2*x^2 + a3*x^3 + ...`

**Pros:** Very fast; simple; easy to tune H2/H3 ratio directly
**Cons:** Wrong at large signals (polynomial diverges); no saturation; no frequency-dependent behavior; cannot capture bias dynamics
**Status:** Not recommended. Too low fidelity for a project targeting physical accuracy.

### 8.2 Perceptually Important Nonlinearities (Priority Order)

| Nonlinearity | Perceptual Impact | Priority |
|-------------|-------------------|----------|
| Asymmetric soft-clip (Stage 1 Vce headroom 2.05V vs 10.9V) | Adds H2 at ff dynamics (pickup dominates at mf) | CRITICAL |
| Exponential transfer function (exp(Vbe/nVt)) | H2 >> H3 harmonic ratio | HIGH |
| Frequency-dependent feedback (C-3/C-4 100pF Miller caps, nested inner loop) | Register-dependent gain and distortion; inner loop dominates BW at ~15.5 kHz (see Section 5.5.1) | HIGH |
| Closed-loop bandwidth limit (preamp-only ~15.5 kHz; full-chain ~11.8 kHz no trem / ~9.7 kHz trem bright) | HF rolloff above ~10 kHz; full-chain BW varies with gain due to Miller loading on input network | HIGH |
| Direct-coupling bias shift (Stage 1 DC modulates Stage 2) | Dynamic compression, "sag", "bloom" | MEDIUM-HIGH |
| Tremolo gain modulation (R-10=56K/LG-1 in feedback) | Timbral variation through tremolo cycle | MEDIUM |
| Cascaded Stage 2 nonlinearity (harmonics-of-harmonics) | Spectral enrichment at ff | MEDIUM |
| Early effect (Vce-dependent Ic) | ~10% gain modulation with signal | LOW |
| Beta(Ic) variation | Asymmetric gain modulation | LOW |
| Thermal drift (-2mV/K Vbe shift) | Very slow "sag" over seconds | LOW |

### 8.3 What Can Be Simplified Without Audible Impact

| Simplification | Justification | Risk |
|---------------|---------------|------|
| Ignore Early effect | ~10% gain variation, masked by larger nonlinearities | LOW |
| Ignore beta(Ic) variation | hFE varies 400-1200 for 2N5089 but output is dominated by Rc load | LOW |
| Ignore thermal drift | Time constants of seconds; inaudible in normal playing | LOW |
| Approximate Stage 2 as fully linear | Av2=2.2 with 820 ohm degeneration; nearly symmetric headroom (1.17:1); H2 contribution negligible | LOW |
| Ignore varactor effect (signal-dependent Cob) | Cob varies ~2-4 pF; negligible vs external C-3/C-4 (100 pF) | LOW |
| Model Ce1 as AC short (ignore 1 Hz corner of coupling) | Ce1 is a feedback coupling cap; at audio frequencies it's essentially a short circuit. Corner at ~1 Hz is inaudible | LOW |
| Model Ce2 as fully bypassed (ignore 27 Hz corner) | Corner at 27 Hz is barely in audio band; effect is small (2.4 dB) | LOW |
| Approximate direct-coupling as instantaneous (no sag) | Loses "bloom" and dynamic compression; audible at ff polyphonic | MEDIUM |
| Ignore tremolo feedback interaction | Loses timbral modulation; audible with tremolo on at high drive | MEDIUM |

### 8.4 Recommended Minimum Model

For a perceptually accurate Wurlitzer 200A preamp model, the minimum implementation should include:

1. **Stage 1 exponential** with gm1 = 2.54-2.80 mA/V (Ic1 = 66-73 uA)
2. **Asymmetric soft-clip** with satLimit = 10.9V, cutoffLimit = 2.05V (Stage 1)
3. **Frequency-dependent feedback** via C-3 (100 pF) Miller effect — dominant pole at ~23 Hz. CORRECT polarity: less feedback at LF (more gain/distortion), more feedback at HF (less gain/distortion)
4. **Closed-loop gain** of ~6 dB (2.0x) without tremolo, up to ~12 dB (4.0x) at tremolo peak, set by R-10 emitter feedback via Ce1
5. **Closed-loop bandwidth** — preamp-only target ~15.5 kHz (nearly constant with R_ldr). Full-chain BW is ~11.8 kHz no-trem / ~9.7 kHz trem-bright due to input network Miller loading. See Section 5.5.1 for nested-loop analysis.
6. **Direct coupling** to Stage 2 (can be instantaneous coupling for simplicity)
7. **Stage 2** with Av = 2.2, nearly symmetric soft-clip (satLimit = 6.2V, cutoffLimit = 5.3V)
8. **Output coupling**: 4th-order Bessel HPF at 40 Hz (Q values 0.5219 and 0.8055) was designed but never implemented. The DK preamp uses shadow subtraction instead (two dk_step calls per sample — main with audio, shadow with zero input, same R_ldr — difference cancels tremolo pump at all frequencies with zero bass loss). See [DK Preamp Derivation](dk-preamp-derivation.md) Section 14.
9. **R-9 series output** (6.8K) — provides output impedance for volume pot interaction

### 8.5 Enhanced Model (for Future Implementation)

Add to the minimum model:

10. **DC bias-shift dynamics**: Track Stage 1's average collector voltage (10-100 ms time constant); feed this to Stage 2's operating point. This produces the "sag" compression and "bloom" heard at ff polyphonic.
11. **Tremolo as emitter feedback modulation**: IMPLEMENTED in the DK preamp. R_ldr directly modulates the MNA system via per-sample Sherman-Morrison update (see [DK Preamp Derivation](dk-preamp-derivation.md) Section 11). The LDR path impedance controls how much R-10 feedback reaches TR-1's emitter via Ce1, producing timbral modulation rather than simple volume modulation.
13. **WDF or coupled NR solver**: Solve both stages simultaneously to capture the inter-stage coupling dynamics.

---

## 9. Implementation Pitfalls

Key lessons from previous modeling attempts:

1. **(Historical — EbersMollPreamp only)** The old `kPreampInputDrive` constant (removed from codebase) should have been ~1.0. A value >> 1 indicated wrong gain staging. The DkPreamp implementation does not use an input drive constant; the MNA system handles gain staging implicitly.

2. **Miller feedback polarity: MORE feedback at HF, LESS at LF.** The capacitor passes high frequencies and blocks low frequencies. An LPF-based feedback implementation inverts this behavior.

3. **Preamp H2 comes from asymmetric headroom, not just the exponential.** Stage 1's 2.05V toward saturation vs 10.9V toward cutoff (5.3:1 ratio) is the preamp's primary H2 mechanism. However, the pickup's 1/(1-y) nonlinearity dominates H2 at normal dynamics; the preamp's asymmetric clipping adds to it at extreme ff.

4. **Direct coupling matters at ff.** Stage 1's DC collector voltage directly sets Stage 2's bias. Large-signal bias shifts produce audible "sag" and "bloom" compression.

5. **Tremolo is gain modulation, not volume modulation.** The LDR shunts the emitter feedback junction, modulating the closed-loop gain and distortion character through the tremolo cycle.

6. **R-10 feeds TR-1's emitter via Ce1 (series-series negative feedback).** R-10 to the base input node (node_A) creates positive feedback through two inverting CE stages and causes oscillation. The correct topology routes R-10 to the emitter through Ce1 (4.7 MFD coupling cap).

---

## 10. Sources

### Primary Sources

1. **Wurlitzer 200/200A Service Manual** — circuit descriptions, component references, signal flow
   - [Internet Archive](https://archive.org/details/wurlitzer-200-and-200-a-service-manual)
   - [Squarespace PDF](https://static1.squarespace.com/static/581b462f5016e14ae76bd275/t/5ebb550ed544905f3db1d03a/1589335321798/wurlitzer-200-200a-service-manual.pdf)

2. **Wurlitzer 200A Schematic** — component values, circuit topology
   - [BustedGear 200A Schematic PDF](https://www.bustedgear.com/images/schematics/Wurlitzer_200A_series_schematics.pdf)
   - [BustedGear 200 Schematic PDF](https://www.bustedgear.com/images/schematics/Wurlitzer_200_series_schematics.pdf)

3. **BustedGear Transistor Specifications** — TR-1/TR-2 replacement types
   - [Wurlitzer 200A Transistors](https://www.bustedgear.com/res_Wurlitzer_200A_transistors.html)

### Transistor Datasheets

4. **2N2924 Datasheet** — original transistor specifications
   - [AllTransistors.com](https://alltransistors.com/transistor.php?transistor=2615)
   - [el-component.com](https://www.el-component.com/bipolar-transistors/2n2924)

5. **2N5089 Datasheet** — replacement transistor specifications
   - [ON Semiconductor](https://www.onsemi.com/download/data-sheet/pdf/mmbt5089-d.pdf)
   - [MIT/Motorola Datasheet](https://www.mit.edu/~6.301/2N5089o.pdf)
   - [Components101](https://components101.com/transistors/2n5089-npn-amplifier-transistor-pinout-features-datasheet-working)

### Circuit Analysis and Repair

6. **GroupDIY — Wurlitzer 200A Preamp** — circuit analysis, 240 pF measurement, input network
   - [GroupDIY Thread 44606](https://groupdiy.com/threads/wurlitzer-200a-preamp.44606/)

7. **GroupDIY — One More Wurlitzer 200 Question** — Avenson preamp, 15 dB gain measurement. Note: the "499K feed resistor" cited here refers to Avenson's replacement preamp design, not the original 200A (which uses a 1 MEG feed resistor, component 56 in HV supply filter chain).
   - [GroupDIY Thread 13555](https://groupdiy.com/threads/one-more-wurlitzer-200-question.13555/)

8. **GroupDIY — Troubleshooting 200A Bias** — power amp voltages, crossover distortion
   - [GroupDIY Thread 62917](https://groupdiy.com/threads/troubleshooting-wurlitzer-200a-amp-board-for-bias-and-crossover-notch-distortion.62917/)

9. **illdigger — Wurlitzer 200A Repair and Low Noise Mod** — component upgrade recommendations
   - [illdigger blog](https://illdigger.wordpress.com/2016/07/03/wurlitzer-200a-piano-repair-and-low-noise-mod/)

10. **EP-Forum — Wurlitzer 200 Amp Schematic Questions**
    - [EP-Forum Topic 9240](https://ep-forum.com/smf/index.php?topic=9240.0)

### Harmonic Distortion Theory

11. **Art of Electronics x-Chapters 2x.4** — BJT amplifier distortion SPICE analysis
    - [AoE x-Chapters PDF](https://x.artofelectronics.net/wp-content/uploads/2019/11/2xp4_BJT_amplifier_distortion.pdf)

12. **till.com — Device Distortion** — exponential transfer function harmonic analysis
    - [Device Distortion](https://till.com/articles/devicedistortion/)

13. **Electronics Tutorials — Amplifier Distortion** — asymmetric clipping and even harmonics
    - [Electronics Tutorials](https://www.electronics-tutorials.ws/amplifier/amp_4.html)

### Related Documents in This Project

14. **Pickup System** — signal levels reaching the preamp, electrostatic analysis
    - `docs/pickup-system.md`

15. **Output Stage** — power amplifier, tremolo feedback discovery, speaker model
    - `docs/output-stage.md`

16. **Signal Chain Architecture** — overall signal flow, parameter values
    - `docs/signal-chain-architecture.md`

---

## Appendix B: Quick Reference for Model Parameters

Based on the analysis in this document, the recommended preamp model parameters are:

```
// Supply and operating point
Vcc = 15.0V

// Stage 1 (TR-1)
Rc1 = 150K
Re1 = 33K
Ce1 = 4.7 MFD (feedback coupling cap: emitter to fb_junct, corner ~1 Hz)
Ic1 = ~66-73 uA
gm1 = ~2.54-2.80 mA/V
B = 38.5 (1/(n*Vt), n=1.0 for 2N5089)
open_loop_gain_1 = gm1 * Rc1 = ~381-420
satLimit_1 = Vcc - Vc1 = 10.9V
cutoffLimit_1 = Vce1 - Vce_sat = 2.15 - 0.1 = 2.05V
asymmetry_ratio_1 = 10.9 / 2.05 = 5.3:1

// Stage 1 feedback cap (C-3)
C3 = 100 pF
C_miller1 = 100pF * (1 + 420) = 42.1 nF (including Cob: 43.2 nF)
R_source1 = R_bias || r_pi1 = 380K || 286K = 163K
f_miller1 = 1/(2*pi*163K*43.2nF) = 22.6 Hz (dominant pole)

// Stage 2 (TR-2)
Rc2 = 1.8K
Re2a = 270 ohm (bypassed by 22 MFD, corner at 26.8 Hz)
Re2b = 820 ohm (unbypassed — sets AC gain)
Ic2 = ~3.1-3.4 mA
gm2 = ~119-131 mA/V
re2 = 1/gm2 = ~7.6-8.4 ohm
Av2 = -Rc2 / (re2 + Re2b) = -1800/828 = -2.17
satLimit_2 = Vcc - Vc2 = 6.2V
cutoffLimit_2 = Vce2 - Vce_sat = 5.4 - 0.1 = 5.3V
asymmetry_ratio_2 = 6.2 / 5.3 = 1.17:1 (nearly symmetric)

// Stage 2 feedback cap (C-4)
C4 = 100 pF
C_miller2 = 100pF * (1 + 2.17) = 317 pF (including Cob: 325 pF)
R_source2 = Rc1 || r_pi2 = 150K || 6.3K = 6.05K
f_miller2 = 1/(2*pi*6.05K*325pF) = 81 kHz (well above audio)

// Output
R_9 = 6.8K (series output resistor)

// Feedback network (tremolo)
R_10 = 56K (feedback resistor)
LG_1 = CdS LDR (variable, tremolo modulation)

// Input coupling (.022 uF, corner ~19 Hz with 380K bias network)
C_input = .022 uF
R_1 = 22K (series from reed bar)

// Bias network
R_2 = 2 MEG (to +15V; see Note 1 on discrepancy)
R_3 = 470K (to ground)

// Output coupling: 4th-order Bessel HPF at 40 Hz was designed but never deployed.
// DK preamp uses shadow subtraction (main − shadow dk_step) instead —
// cancels tremolo pump at all frequencies with zero bass loss.
// Pump level after subtraction: < −120 dBFS.

// Overall closed-loop (emitter feedback via R-10/Ce1)
total_closed_loop_gain_no_trem = 6.0 dB (2.0x) [Rldr_path = 1M]
total_closed_loop_gain_trem_bright = 12.1 dB (4.0x) [Rldr_path = 19K]
tremolo_modulation_range = 6.1 dB

// Bandwidth — TWO NESTED FEEDBACK LOOPS (see Section 5.5.1):
// Inner loop (C-3/C-4 Miller) dominates BW at ~15.5 kHz, nearly constant.
// Outer loop (R-10/Ce1/R_ldr) controls gain only.
// GBW is NOT constant — scales with gain (nested-loop behavior).
preamp_only_bandwidth = ~15.5 kHz (nearly constant, both R_ldr values)
full_chain_bandwidth_no_trem = 19 Hz - 11.8 kHz (includes input network loading)
full_chain_bandwidth_trem_bright = 19 Hz - 9.7 kHz (input network loaded more by higher gain)
// Earlier "9.9 kHz / 8.3 kHz" figures were full-chain, possibly with R_ldr=120K default
passband_peak = ~450 Hz (from feedback loop resonance)
// kPreampInputDrive: removed (was EbersMollPreamp only; DkPreamp handles gain implicitly)

// Note: C20 (220 pF) appears only on 206A board, NOT on 200A.
// Bass rolloff comes from pickup RC (f_c = 2312 Hz), not from preamp.
```
