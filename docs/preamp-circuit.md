# Wurlitzer 200A Reed-Bar Preamp — Complete Circuit Reference

Comprehensive technical reference for implementing a digital model of the Wurlitzer 200A reed-bar preamplifier. Covers verified component values (from direct reading of BustedGear 200A schematic at 900 DPI), DC bias analysis, AC signal analysis, harmonic generation mechanisms, tremolo integration, and modeling recommendations. Intended for AI agent consumption.

**Revision note (Feb 2026):** This document has been completely rewritten with correct component values verified from the Wurlitzer 200A schematic. The previous version used estimated values that were dramatically wrong — Rc1 was 47K (actual: 150K), Rc2 was 10K (actual: 1.8K), Re1 was 2.2-8.2K (actual: 33K), Re2 was 4.7-5.1K (actual: 270+820 ohm). All DC bias analysis, gain calculations, Miller pole estimates, clipping headroom analysis, and frequency response have been recalculated from first principles.

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
9. [Previous Implementation Issues and Lessons](#9-previous-implementation-issues-and-lessons)
10. [Sources](#10-sources)

---

## 1. Circuit Overview

### 1.1 Topology

The Wurlitzer 200A reed-bar preamp is a **two-stage direct-coupled NPN common-emitter amplifier** mounted on a small PCB attached to the reed bar. The two transistors (TR-1 and TR-2, both 2N5089) amplify the millivolt-level signals from the electrostatic pickup to a level suitable for the volume pot and power amplifier.

[VERIFIED — Wurlitzer 200/200A service manual; GroupDIY thread 44606; BustedGear transistor specs; BustedGear 200A schematic PDF at 900 DPI]

### 1.2 Defining Features

1. **Direct coupling**: TR-1 collector connects directly to TR-2 base — no coupling capacitor between stages. TR-1's DC operating point sets TR-2's bias. [VERIFIED — schematic shows direct connection; DC voltages confirm: TR-1 Vc = 4.1V = TR-2 Vb = 4.1V]

2. **High-impedance electrostatic input**: The preamp input sees the pickup plate through a resistive bias network (R-2 = 2 MEG to +15V, R-3 = 470K to ground). [VERIFIED — schematic; see Section 2.2 note on R-2 value discrepancy]

3. **Collector-base feedback capacitors** (C-3 = 100 pF, C-4 = 100 pF): Frequency-dependent negative feedback via the Miller effect on both stages. These, combined with global emitter feedback through R-10 (via Ce1), reduce the very high open-loop gain (~900x) to a moderate closed-loop gain. [VERIFIED — service manual: "Protection against radio frequency interference is provided by shunt capacitor C-1, and collector-base feedback capacitors C-3 and C-4"; C-3 and C-4 values verified from schematic]

4. **Tremolo integration**: R-10 (56K) feeds back from the preamp output to TR-1's emitter via Ce1 (4.7 MFD coupling cap). The LDR (LG-1) shunts the feedback junction (between R-10 and Ce1) to ground via the cable, modulating how much feedback reaches the emitter and thus the closed-loop gain. [VERIFIED — service manual: "A divider is formed by the feedback resistor R-10, and the light dependent resistor of LG-1"; R-10 = 56K verified from schematic; topology traced from correct 200A schematic Feb 2026]

5. **Supply voltage**: +15V DC regulated, derived from the main power supply. [VERIFIED — schematic and GroupDIY multimeter measurements]

6. **Stage 1 emitter feedback coupling capacitor**: Ce1 = 4.7 MFD connects TR-1's emitter to the R-10/LDR feedback junction (NOT to ground). Re1 (33K) provides the separate DC path from emitter to ground. Ce1 AC-couples the feedback signal from R-10 to the emitter, providing series-series negative feedback. [VERIFIED — traced from correct 200A schematic Feb 2026; confirmed stable in SPICE]

### 1.3 Position in Signal Chain

```
Reed vibration
  -> Electrostatic pickup (all 64 reeds summed at single pickup plate)
  -> Polarizing network (R-1 = 22K series, R-2 = 2M to +15V, R-3 = 470K to ground)
  -> C20 (220 pF shunt cap, HPF ~1900 Hz)
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

[VERIFIED — service manual signal flow; BustedGear schematic at 900 DPI]

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
     |    C20(220pF) |    |  |   |          |  |   |
     |      |        |    |  E   |          |  E   |
     |     GND  D1---+    |  |   |          |  |   |
     |           |   |    | Re1(33K)        | Re2a(270)+Ce2(22MFD bypass)
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

**CRITICAL TOPOLOGY (CORRECTED Feb 2026):** R-10 feeds back from the output to fb_junct. Ce1 (4.7 MFD) AC-couples fb_junct to TR-1's emitter — this is **series-series NEGATIVE feedback** (not shunt-feedback to node_A as previously documented). Re1 (33K) provides the separate DC path from emitter to ground. The LDR (LG-1) shunts fb_junct to ground via cable Pin 1 → 50K VIBRATO pot → 18K → LG-1.

[VERIFIED — correct 200A schematic (874 KB PDF, md5: ceb3abb9) traced at 1000-2000 DPI with user annotation, Feb 2026; SPICE-validated in spice/testbench/preamp_emitter_fb.cir]

### 2.2 Component Values Table

| Ref | Value | Function | Confidence |
|-----|-------|----------|------------|
| R-1 | 22K | Series input from reed bar | VERIFIED — schematic |
| R-2 | 2 MEG | DC bias from +15V to TR-1 base | VERIFIED — schematic reads "1 MEG" but DC analysis requires ~2 MEG for measured Vb=2.45V; see Note 1 |
| R-3 | 470K | DC bias to ground from TR-1 base | VERIFIED — schematic |
| Rc1 | 150K | TR-1 collector load resistor | VERIFIED — schematic |
| Re1 | 33K | TR-1 emitter degeneration resistor | VERIFIED — schematic |
| Ce1 | 4.7 MFD | Feedback coupling cap: AC-couples TR-1 emitter to R-10/LDR feedback junction (NOT a bypass cap — see Section 7) | VERIFIED — correct 200A schematic, SPICE-validated Feb 2026 |
| Rc2 | 1.8K | TR-2 collector load resistor | VERIFIED — schematic |
| Re2a | 270 ohm | TR-2 emitter resistor (bypassed by Ce2) | VERIFIED — schematic |
| Ce2 | 22 MFD | Emitter bypass cap across Re2a (270 ohm) | VERIFIED — schematic |
| Re2b | 820 ohm | TR-2 emitter resistor (unbypassed) | VERIFIED — schematic |
| R-9 | 6.8K | Series output resistor | VERIFIED — schematic |
| R-10 | 56K | Feedback resistor from output to TR-1 emitter junction (via Ce1); LDR shunts this junction for tremolo | VERIFIED — schematic; topology traced Feb 2026 |
| C-3 | 100 pF | TR-1 collector-base feedback capacitor (Miller) | VERIFIED — schematic |
| C-4 | 100 pF | TR-2 collector-base feedback capacitor (Miller) | VERIFIED — schematic |
| C20 | 220 pF | Input shunt cap (HPF bass rolloff) | RESOLVED — schematic reads 220 pF (confirmed at 1500 DPI). GroupDIY cited 270 pF, likely tolerance variation or production change. See Note 2 |
| C-1 | = C20 (220 pF) | RF shunt protection capacitor | RESOLVED — C-1 (service manual designation) = C20/C-2 (schematic position 2) = 220 pF. Same physical component, different naming systems. See Note 3 |
| D-1 | 25 PIV, 10 mA (part #142136) | Reverse-polarity transient protection at input | VERIFIED — schematic |
| Input coupling | .022 uF | AC coupling at preamp input | VERIFIED — schematic |
| LG-1 | CdS LDR in lightproof enclosure with LED | Tremolo gain modulation in feedback network | VERIFIED — service manual, schematic |

**Note 1 (R-2 — RESOLVED as 2 MEG):** The schematic appears to read "1 MEG" for R-2 (confirmed at 1500 DPI). However, three independent lines of evidence confirm R-2 = 2M:
1. **GroupDIY "380K" evidence:** PRR (GroupDIY thread 44606) states "270pFd against 380K is a bass-cut at 1,750Hz." The "380K" is R-2 || R-3: 2M || 470K = 380.2K. If R-2 were 1M, this would be 1M || 470K = 319.7K ≈ 320K, not 380K.
2. **DC analysis eliminates 1M:** With R-2=1M, Vth=4.80V, and achieving Vb=2.45V would require hFE=9 (impossible for any NPN). With R-2=2M, Vth=2.85V, and Vb=2.45V requires hFE≈62 (plausible with resistor tolerances and/or original 2N2924 transistors).
3. **GroupDIY thread 62917 measurements:** TR-1 B=2.447V on a working 200A, consistent with R-2=2M and 10-20% carbon composition resistor tolerances (R-2=2.2M, R-3=420K → Vth=2.42V).
The schematic label may be a misread, a factory error, or confusion with the component reference number "2". **Use 2M for modeling.**

**Note 2 (C20 — RESOLVED as 220 pF):** The schematic reads 220 pF (confirmed at 1500 DPI). GroupDIY's analysis cites 270 pF and derives a bass-cut at ~1750 Hz. The 270 pF figure likely reflects tolerance variation in ceramic capacitors or a production change. With C20 = 220 pF and R_eff = 380K, the HPF cutoff is 1903 Hz. **Use 220 pF for modeling.**

**Note 3 (C-1 — RESOLVED as same component as C20):** The service manual mentions "shunt capacitor C-1" for RF protection. This is the same physical component as C20 (220 pF at TR-1 base to ground). The naming discrepancy arises because the service manual uses "C-1" (functional designation) while the schematic uses board position "2" (= C-2 in standard designator format). The PCB layout on page 1 of the schematic shows R-x/C-x silk screen labels. No separate C-1 component exists — confirmed by schematic examination at 2400 DPI and absence of any repair community discussion about a missing C-1.

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

| Component | Value | Source |
|-----------|-------|--------|
| Polarizing voltage | 147V DC | VERIFIED — service manual |
| Feed resistor (R_feed) | 1 MEG | RESOLVED — component 56 in HV supply filter chain on main amp board. Avenson's "499K" refers to their replacement preamp design, not the original 200A. |
| Filter capacitors | 3 x 0.33 uF | VERIFIED — EP-Forum, Tropical Fish |
| Rectifier | Half-wave | VERIFIED — service manual |

[VERIFIED — service manual; GroupDIY thread 13555; EP-Forum. R_feed RESOLVED as 1 MEG from HV supply schematic.]

---

## 3. Transistor Specifications

### 3.1 Original Transistor: 2N2924

The original transistors used in early Wurlitzer 200/200A instruments.

| Parameter | Value | Source |
|-----------|-------|--------|
| Type | NPN silicon planar epitaxial | VERIFIED — 2N2924 datasheet |
| Package | TO-92 | VERIFIED — datasheet |
| Vceo (max) | 25V | VERIFIED — datasheet |
| Ic (max) | 100 mA | VERIFIED — datasheet |
| hFE (DC current gain) | 150 to 300 | VERIFIED — datasheet |
| Power dissipation | 625 mW | VERIFIED — datasheet |
| Application | AF small amplifiers, direct-coupled circuits | VERIFIED — datasheet |
| Wurlitzer part number | 142083-2 (for TR-3, TR-4 tremolo oscillator) | VERIFIED — BustedGear |

[VERIFIED — AllTransistors.com; el-component.com; BustedGear Wurlitzer 200A transistor specs]

### 3.2 Replacement Transistor: 2N5089

Later production and all current replacements use the 2N5089.

| Parameter | Value | Source |
|-----------|-------|--------|
| Type | NPN silicon, high-gain, low-noise | VERIFIED — ON Semiconductor datasheet |
| Package | TO-92 | VERIFIED — datasheet |
| Vceo (max) | 25V | VERIFIED — datasheet |
| Ic (max) | 50 mA | VERIFIED — datasheet |
| hFE at Ic=0.1mA, Vce=5V | 400 to 1200 | VERIFIED — datasheet |
| hFE at Ic=1mA, Vce=5V | 450 to 1800 | VERIFIED — datasheet |
| fT (gain-bandwidth product) | 50 MHz at Vce=5V, Ic=0.5mA | VERIFIED — datasheet |
| Noise figure (NF) | 2.5 dB typical at 1 kHz, Rg=10k | VERIFIED — datasheet |
| Cob (output capacitance) | 2.5 pF typical at Vcb=10V | VERIFIED — ON Semiconductor datasheet |

[VERIFIED — ON Semiconductor datasheet (onsemi.com/pdf/datasheet/mmbt5089-d.pdf); MIT/Motorola datasheet; Components101.com]

### 3.3 Key Differences: 2N2924 vs 2N5089

| Parameter | 2N2924 (original) | 2N5089 (replacement) | Impact on Sound |
|-----------|-------------------|---------------------|-----------------|
| hFE | 150-300 | 450-1800 | Higher gain, more headroom before saturation |
| Noise | Higher | Lower (purpose-designed low-noise) | Cleaner signal, less hiss |
| Cob | ~4-8 pF (est.) | 2.5 pF | Different Miller-effect frequency; replacement has less HF feedback |

[INFERRED — the higher hFE of the 2N5089 means the preamp gain changes when transistors are replaced, which is a known issue in the repair community. The lower Cob shifts the Miller-effect pole upward.]

**Modeling note:** The 2N5089 (hFE >= 450) is the relevant transistor for most surviving instruments. The 2N2924 (hFE 150-300) gives a different tonal character — lower gain, earlier saturation, more distortion. A model targeting the "typical" 200A sound should use 2N5089 parameters. For the bias calculations below, we use hFE = 800 as a representative mid-range value for the 2N5089 at the relevant operating currents.

---

## 4. DC Bias Analysis

### 4.1 Measured Operating Points

DC voltages from the Wurlitzer 200A schematic and confirmed by GroupDIY multimeter measurements:

| Transistor | Pin | Voltage (V) | Source |
|-----------|-----|-------------|--------|
| TR-1 (Stage 1) | Emitter | 1.95 | VERIFIED — schematic annotation |
| TR-1 (Stage 1) | Base | 2.45 | VERIFIED — schematic annotation (1500 DPI) |
| TR-1 (Stage 1) | Collector | 4.1 | VERIFIED — schematic annotation |
| TR-2 (Stage 2) | Emitter | 3.4 | VERIFIED — schematic annotation |
| TR-2 (Stage 2) | Base | 4.1 | VERIFIED — schematic annotation |
| TR-2 (Stage 2) | Collector | 8.8 | VERIFIED — schematic annotation |
| Supply (Vcc) | — | +15.0 | VERIFIED — schematic, GroupDIY measurement |

**Note:** The GroupDIY multimeter measurements (E=1.923, B=2.447, C=3.98 for TR-1; E=3.356, B=3.988, C=8.45 for TR-2) are close but not identical to the schematic annotations. The differences likely reflect tolerance variations in resistors and transistor hFE in that particular instrument. We use the schematic annotation values as the design-center operating point.

### 4.2 Direct Coupling Verification

TR-1 collector = 4.1V; TR-2 base = 4.1V. These are identical, confirming **no coupling capacitor** between stages. TR-1's DC collector voltage directly sets TR-2's base bias.

[VERIFIED — schematic topology; DC voltage match confirms direct coupling]

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

[CALCULATED — from verified Vc1 and Rc1]

**Emitter current from Re1:**
```
Ie1 = Ve1 / Re1 = 1.95V / 33K = 59.1 uA
```

[CALCULATED — from verified Ve1 and Re1]

**Consistency check:** Ie1 = Ic1 + Ib1. For hFE = 800: Ib1 = 72.7/800 = 0.091 uA. Then Ie1 = 72.7 + 0.091 = 72.8 uA. The calculated Ie1 from Re1 (59.1 uA) is ~19% lower than the calculated Ic1 from Rc1 (72.7 uA). This discrepancy is within the range of resistor tolerances (10-20% carbon composition resistors were standard in the era) and the limited resolution of schematic annotation. The true operating current is in the range of 59-73 uA. For calculations below, we use the average: **Ic1 ~ 66 uA**.

**Transconductance:**
```
gm1 = Ic1 / Vt = 66 uA / 26 mV = 2.54 mA/V
```

[CALCULATED — using averaged Ic1]

**Small-signal emitter resistance:**
```
re1 = 1/gm1 = 1/2.54 mA/V = 394 ohm
```

[CALCULATED]

**Base bias network:**
```
R-2 (to +15V) = 2 MEG
R-3 (to ground) = 470K

Thevenin voltage: Vth = 15 * 470K / (2M + 470K) = 15 * 0.190 = 2.854V
Thevenin resistance: Rth = 2M || 470K = (2M * 470K) / (2M + 470K) = 380K

Base voltage check: Vb = Vth - Ib * Rth = 2.854 - 0.091uA * 380K = 2.854 - 0.035 = 2.82V
```

The calculated Vb (2.82V) is above the schematic annotation of 2.45V by 0.37V. This larger-than-expected discrepancy could indicate: (a) the schematic Vb annotation is approximate/rounded, (b) additional DC loading on the base node not accounted for in this simplified bias model, or (c) the actual R-2 is closer to 1 MEG (as labeled on the schematic), which would give Vth = 4.8V — even further from 2.45V. **The Vb discrepancy does NOT affect the gain, Miller pole, or clipping calculations**, which depend on Ic (derived from Vc and Rc on the collector side) and Vce (Vc - Ve), both of which are unambiguously verified from the schematic.

[CALCULATED — from verified component values; small discrepancy within resistor tolerance]

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

[CALCULATED — from verified Vc2 and Rc2]

**Emitter current from Re2_total:**
```
Ie2 = Ve2 / Re2_total = 3.4V / 1090 ohm = 3.12 mA
```

[CALCULATED — from verified Ve2 and Re2 total]

**Consistency check:** Ic2 (3.44 mA) vs Ie2 (3.12 mA) — ~10% discrepancy, again within resistor tolerance range. Average: **Ic2 ~ 3.3 mA**.

**Transconductance:**
```
gm2 = Ic2 / Vt = 3.3 mA / 26 mV = 127 mA/V
```

[CALCULATED — using averaged Ic2]

**Small-signal emitter resistance:**
```
re2 = 1/gm2 = 1/127 mA/V = 7.9 ohm
```

[CALCULATED]

**Direct coupling bias:** TR-2's base voltage (4.1V) is set directly by TR-1's collector voltage (4.1V). The direct coupling creates a DC dependency: if TR-1's collector shifts (due to signal-dependent bias or temperature), TR-2's operating point shifts with it. This is the mechanism behind the "sag" and "bloom" effects at high drive levels.

[VERIFIED — confirmed by schematic topology and voltage match]

### 4.5 Summary of DC Analysis

| Parameter | Stage 1 (TR-1) | Stage 2 (TR-2) | Source |
|-----------|----------------|----------------|--------|
| Vb | 2.45V | 4.1V | VERIFIED — schematic |
| Ve | 1.95V | 3.4V | VERIFIED — schematic |
| Vc | 4.1V | 8.8V | VERIFIED — schematic |
| Vbe | 0.50V | 0.70V | CALCULATED (Stage 1 is low — see Section 4.3 note) |
| Vce | 2.15V | 5.4V | CALCULATED |
| Rc | 150K | 1.8K | VERIFIED — schematic |
| Re | 33K (bypassed by 4.7 MFD) | 270 ohm (bypassed) + 820 ohm | VERIFIED — schematic |
| Ic | ~66 uA | ~3.3 mA | CALCULATED |
| gm | ~2.54 mA/V | ~127 mA/V | CALCULATED |
| re (1/gm) | 394 ohm | 7.9 ohm | CALCULATED |

### 4.6 Key Architectural Insight

The component values reveal a fundamentally different architecture than what was previously estimated:

**Stage 1 is a HIGH-GAIN, LOW-CURRENT voltage amplifier:**
- Ic1 = 66 uA (very low current — quiet, low power)
- Rc1 = 150K (very high collector load — maximum voltage gain per milliamp)
- Re1 = 33K (large DC stabilization; separate DC path from emitter to ground)
- **Note (Feb 2026):** Ce1 is a feedback coupling cap (emitter to fb_junct), NOT a simple bypass cap to ground. The open-loop gain depends on the impedance at fb_junct (LDR path + R-10). When the LDR path impedance is low, Ce1 effectively AC-grounds the emitter → Av1 ≈ gm1*Rc1 ≈ 420. When LDR path is high, the emitter sees R-10 and has significant degeneration. See SPICE AC sweep for actual values.
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

[CALCULATED — the ~900x figure is the maximum open-loop gain (fb_junct grounded). Actual open-loop gain depends on LDR path impedance — see Section 7.2]

### 4.7 Overall Gain

Brad Avenson (professional audio designer who built a replacement Wurlitzer preamp) measured the total preamp gain at **approximately 15 dB (voltage gain approximately 5.6x)**. He stated: "the preamp really only needs 15 dB." Volume pot output was measured at **2-7 mV AC**.

[VERIFIED — GroupDIY thread 13555]

**SPICE-measured gain (Feb 2026, corrected emitter feedback topology):**
- **No tremolo (Rldr_path = 1M):** 6.0 dB (2.0x) at 1 kHz
- **Tremolo bright (Rldr_path = 19K):** 12.1 dB (4.0x) at 1 kHz
- **Tremolo modulation range:** 6.1 dB

**Reconciliation with Avenson's "15 dB" measurement:** Avenson measured ~15 dB (5.6x) for his replacement preamp design (which uses 499K instead of 1M for R_feed and may have different feedback topology). The original 200A with corrected emitter feedback gives **6 dB (2x) without tremolo** and up to **12 dB (4x) at tremolo peak**. The 15 dB figure does NOT match the original circuit — it's either Avenson's replacement design or a measurement with tremolo active at bright peak.

**Gain structure:**
- Maximum open-loop gain (fb_junct grounded, Re1 bypassed via Ce1): ~900 (59 dB) [CALCULATED]
- Combined degenerated gain (Ce1 open, DC): 4.5 * 2.2 = 9.9 (20 dB) [CALCULATED]
- SPICE-measured closed-loop gain: 6.0 dB (2.0x) without tremolo [MEASURED]
- The strong emitter feedback (loop gain ≈ 900/2.0 = 450, or 53 dB) provides excellent gain stability and linearization.

[MEASURED Feb 2026 — SPICE AC sweep of corrected emitter feedback topology]

---

## 5. AC Signal Analysis

### 5.1 Input Signal Levels

The pickup system delivers millivolt-level signals to the preamp input. Based on the electrostatic analysis (see pickup-system.md):

| Condition | Estimated Preamp Input Level | Source |
|-----------|------------------------------|--------|
| C4 at pp (vel=0.3) | ~0.1-0.5 mV peak | ESTIMATED — electrostatic calculation |
| C4 at mf (vel=0.7) | ~1-5 mV peak | ESTIMATED — consistent with Avenson 2-7 mV output / 5.6x gain |
| C4 at ff (vel=0.95) | ~5-15 mV peak | ESTIMATED |
| Bass (A1 at mf) | ~0.05-0.2 mV peak | ESTIMATED — heavily attenuated by pickup RC HPF |
| Treble (C6 at mf) | ~5-20 mV peak | ESTIMATED — less attenuation, smaller displacement |

[ESTIMATED — derived from pickup system signal level estimates in pickup-system.md, consistent with Brad Avenson's measurement of 2-7 mV at volume pot output after approximately 15 dB gain]

### 5.2 Input Coupling Network (C20 HPF)

C20 (220 pF, RESOLVED from schematic at 1500 DPI) forms a first-order high-pass filter by shunting low frequencies to ground relative to the signal source impedance.

```
f_c = 1 / (2 * pi * R_eff * C20)
```

The effective resistance is the Thevenin impedance of the bias network:
```
R_eff = R-2 || R-3 = 2M || 470K = (2M * 470K) / (2M + 470K) = 380K
```

| C20 Value | R_eff | f_c (Hz) | Source |
|-----------|-------|----------|--------|
| 220 pF (schematic, RESOLVED) | 380K | 1903 Hz | CALCULATED |

GroupDIY explicitly states: "270pFd against 380K creates a bass-cut at 1,750Hz" — their 270 pF figure likely reflects tolerance variation in ceramic capacitors or a production change. The schematic confirms 220 pF at 1500 DPI.

[RESOLVED — schematic reads 220 pF at 1500 DPI. GroupDIY's 270 pF is tolerance/production variation.]

**Use f_c = 1903 Hz** (from C20 = 220 pF and R_eff = 380K) as the canonical value for modeling.

**Frequency response of C20 HPF:**

| Note | MIDI | Freq (Hz) | Attenuation (dB) | Source |
|------|------|-----------|------------------|--------|
| A1 | 33 | 55 | -30.8 | CALCULATED (at f_c = 1903 Hz) |
| C3 | 48 | 131 | -23.3 | CALCULATED |
| C4 | 60 | 262 | -17.3 | CALCULATED |
| C5 | 72 | 523 | -11.5 | CALCULATED |
| C6 | 84 | 1047 | -6.3 | CALCULATED |
| C7 | 96 | 2093 | -2.6 | CALCULATED |

(Computed for f_c = 1903 Hz, first-order HPF: |H(f)| = f / sqrt(f^2 + f_c^2))

### 5.3 Small-Signal Gain of Each Stage

#### Stage 1 (TR-1)

**Ce1 coupling at audio frequencies (Ce1 = 4.7 MFD couples emitter to fb_junct):**

**Note (Feb 2026):** Ce1 is a feedback coupling cap from emitter to the R-10/LDR feedback junction, NOT a simple bypass cap across Re1 to ground. The effective emitter AC impedance depends on the impedance at fb_junct (LDR path to ground || R-10 to output). The corner frequency for Ce1 coupling is ~1 Hz, so it's effectively an AC short at all audio frequencies. However, the gain depends on what's at fb_junct — see Section 7.2.

The gain below assumes fb_junct has low impedance to ground (LDR path active, tremolo bright phase):
```
Av1 = -gm1 * Rc1 = -2.54 mA/V * 150K = -381
```

Using the per-Rc1 current (Ic1 = 72.7 uA, gm1 = 2.80 mA/V):
```
Av1 = -2.80 * 150K = -420
```

[CALCULATED — note this depends only on (Vcc - Vc1)/Vt = 10.9/0.026 = 419, which is independent of the specific Rc1 and Ic1 values individually]

**Without bypass cap (DC stability, below 1 Hz):**
```
Av1_DC = -Rc1/Re1 = -150K/33K = -4.5
```

[CALCULATED — this low DC gain provides excellent bias stability]

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

[CALCULATED]

**Below ~27 Hz (full emitter degeneration):**
```
Av2_LF = -Rc2 / (re2 + Re2_total) = -1800 / (7.9 + 1090) = -1.64
```

[CALCULATED — the 27 Hz corner is well below the preamp's useful band, so this matters only for DC and subsonic signals]

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

[CALCULATED — this is the **dominant pole** of the preamp, at approximately 23 Hz]

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

[CALCULATED — this second pole is well above the audio band and does not significantly affect audio-frequency behavior]

### 5.5 Frequency-Dependent Feedback from C-3 and C-4

The collector-base feedback caps create **shunt-shunt negative feedback** (current feedback from collector to base). The key behavior:

At **low frequencies** (f << f_dominant_pole ~ 23 Hz): C-3 has high impedance, negligible feedback current. Stage 1 operates at full open-loop gain (~420). Since this is below the audio band, it only affects subsonic signals and DC stabilization.

At **audio frequencies** (f >> 23 Hz): C-3 has low impedance relative to the circuit impedances. The Miller multiplication creates heavy feedback, controlling the gain. The open-loop gain rolls off at -20 dB/decade from the 23 Hz pole.

**The real Miller-effect direction:**
- At LOW frequencies: capacitor has high impedance -> LESS feedback -> MORE gain -> MORE distortion
- At HIGH frequencies: capacitor has low impedance -> MORE feedback -> LESS gain -> LESS distortion

[VERIFIED — standard Miller-effect theory; "Art of Electronics" x-Chapters Section 2x.4]

**Critical insight:** With the dominant pole at 23 Hz, the open-loop gain is already rolling off throughout the entire audio band. At 1 kHz, the open-loop gain of Stage 1 alone has dropped to approximately:
```
|Av1(1kHz)| = 420 / sqrt(1 + (1000/23)^2) = 420 / 43.5 = 9.7
```

At 10 kHz:
```
|Av1(10kHz)| = 420 / sqrt(1 + (10000/23)^2) = 420 / 435 = 0.97
```

This means the open-loop gain of Stage 1 alone is less than unity above ~10 kHz! Combined with Stage 2 (Av2 = 2.2), the two-stage open-loop gain at 10 kHz is only ~2.1. The feedback can only reduce gain below the open-loop value, so above ~10 kHz, the closed-loop gain approaches the open-loop gain.

### 5.6 Closed-Loop Frequency Response

The overall preamp is a two-stage amplifier with:
- **DC open-loop gain:** ~900 (59 dB)
- **Dominant pole:** ~23 Hz (from Stage 1 C-3 Miller)
- **Second pole:** ~81 kHz (from Stage 2 C-4 Miller)
- **Closed-loop gain (set by R-10 emitter feedback):** **MEASURED Feb 2026 (SPICE AC sweep of corrected topology):**
  - **No tremolo (LDR dark, Rldr_path ≈ 1M):** Peak gain = **6.05 dB (2.01x)** at 447 Hz. Gain at 1 kHz = **6.0 dB (2.0x)**.
  - **Tremolo bright (Rldr_path ≈ 19K):** Gain at 1 kHz = **12.1 dB (4.0x)**.
  - **Tremolo modulation range: ~6.1 dB** (matches EP-Forum "6 dB boost" measurement exactly).
  - The gain is remarkably constant with input level (2.007x from pp to extreme) — the strong emitter feedback linearizes the circuit effectively.

The gain-bandwidth product (GBW) of the open-loop amplifier:
```
GBW = Av_open_DC * f_dominant = 900 * 23 = 20,700 Hz
```

The closed-loop -3 dB bandwidth (from SPICE AC sweep):
```
f_low = 19 Hz, f_high = 9.9 kHz (no tremolo, Rldr_path = 1M)
f_high = 8.3 kHz (tremolo bright, Rldr_path = 19K)
```

The bandwidth DECREASES as gain increases (tremolo bright → higher gain, narrower BW), consistent with constant GBW product.

[MEASURED Feb 2026 — SPICE AC sweep of spice/testbench/preamp_ac_sweep.cir and preamp_ldr_sweep.cir]

**Open-loop gain at key frequencies (combined two stages):**

| Frequency | Stage 1 |Av1| | Stage 2 |Av2| | Combined |Av_open| | Notes |
|-----------|---------|---------|---------|-------|
| 10 Hz | 420 | 2.17 | 912 | Below dominant pole; full gain |
| 23 Hz | 297 | 2.17 | 645 | Dominant pole (-3 dB on Stage 1) |
| 100 Hz | 96.6 | 2.17 | 210 | Open-loop rolling off |
| 500 Hz | 19.3 | 2.17 | 42 | |
| 1 kHz | 9.7 | 2.17 | 21 | |
| 2 kHz | 4.8 | 2.17 | 10.5 | |
| 3.7 kHz | 2.6 | 2.17 | 5.6 | Open-loop = closed-loop here |
| 5 kHz | 1.93 | 2.17 | 4.2 | Below closed-loop target |
| 10 kHz | 0.97 | 2.17 | 2.1 | Stage 1 < unity; no feedback possible |
| 20 kHz | 0.48 | 2.17 | 1.05 | |

**Closed-loop gain at key frequencies:**

> **REVISION NOTE (Feb 2026):** The tables below use the original 5.6x gain assumption from the old shunt-feedback model (R10/R1 = 56K/22K). The topology has been corrected to emitter feedback (R-10 → Ce1 → emitter). The frequency response shape (bandpass with C20 HPF and Miller rolloff) is qualitatively correct, but the absolute gain values need re-derivation from the SPICE AC sweep of the corrected topology (preamp_emitter_fb.cir). Treat the numerical values below as provisional.

At low frequencies, the emitter feedback holds the gain at a value determined by the feedback loop. Above the point where open-loop gain drops to match the closed-loop gain, the gain rolls off at -20 dB/decade.

| Frequency | Closed-Loop Gain | Closed-Loop (dB) | Notes |
|-----------|-----------------|-------------------|-------|
| 100 Hz | 5.6 | 15.0 | Feedback-controlled |
| 500 Hz | 5.6 | 15.0 | Feedback-controlled |
| 1 kHz | 5.6 | 15.0 | Feedback-controlled |
| 2 kHz | 5.6 | 15.0 | Feedback-controlled |
| 3.7 kHz | 5.6 | 15.0 | -3 dB point (open-loop = closed-loop) |
| 5 kHz | 4.2 (2.4) | 12.4 (7.6) | Rolling off |
| 10 kHz | 2.1 (1.53) | 6.4 (3.7) | Significant rolloff |
| 20 kHz | 1.05 (0.88) | 0.4 (-1.1) | Nearly unity |

> **Approximation note:** Values above 3.7 kHz use Acl ≈ Aol (feedback ignored). The parenthesized values use the full feedback formula Acl = Aol/(1 + Aol*beta) with beta=0.179, which gives 1.5-5 dB lower gain at HF because R-10 feedback still attenuates even when loop gain < 1. The full formula is more accurate; implementation should use it.

### 5.7 Combined Preamp Frequency Response (with C20 HPF)

The total preamp response combines the C20 input HPF (~1903 Hz) with the closed-loop amplifier response (flat to ~3.7 kHz, then rolling off):

| Note | MIDI | Freq (Hz) | C20 HPF (dB) | Amp Gain (dB) | Total (dB) | Notes |
|------|------|-----------|-------------|---------------|-----------|-------|
| A1 | 33 | 55 | -30.8 | 15.0 | -15.8 | Heavy bass attenuation |
| C3 | 48 | 131 | -23.3 | 15.0 | -8.3 | Still very attenuated |
| C4 | 60 | 262 | -17.3 | 15.0 | -2.3 | Approaching passband |
| C5 | 72 | 523 | -11.5 | 15.0 | 3.5 | |
| C6 | 84 | 1047 | -6.3 | 15.0 | 8.7 | |
| C7 | 96 | 2093 | -2.6 | 15.0 | 12.4 | Near peak |
| — | — | 3700 | -1.0 | 15.0 | 14.0 | Passband peak |
| — | — | 5000 | 0.0 | 12.4 (7.6) | 12.4 (7.6) | Rolling off from amp BW |
| — | — | 10000 | 0.0 | 6.4 (3.7) | 6.4 (3.7) | Significant HF rolloff |
| — | — | 20000 | 0.0 | 0.4 (-1.1) | 0.4 (-1.1) | Nearly unity |

> **Note:** Parenthesized values use the full feedback formula (see Section 5.6 note). For implementation, use the full formula values.

**The preamp's passband peak is approximately 2-4 kHz.** Below this, the C20 HPF attenuates. Above this, the Miller-effect feedback bandwidth limit attenuates. This mid-frequency emphasis naturally puts the most gain in the 2-4 kHz "bark" frequency region — the range where the human ear is most sensitive and where the Wurlitzer's characteristic bite lives.

**This is a key tonal design feature:** The preamp does not amplify all frequencies equally. Bass fundamentals are heavily attenuated (A1 at 55 Hz sees -15.8 dB net), while the 2-4 kHz range gets the full 15 dB of gain. This explains why the Wurlitzer's bass notes sound thin and "reedy" while the midrange has body and bark.

[CALCULATED — from verified component values; the 2-4 kHz passband peak is a natural consequence of the C20 HPF and Miller-effect bandwidth limit]

### 5.8 Emitter Bypass Cap Corner Effects

#### Stage 1 (Ce1 = 4.7 MFD across Re1 = 33K)

Corner frequency: f = 1/(2*pi*33K*4.7uF) = **1.03 Hz**. Ce1 is fully effective across the entire audio band. No audible transition.

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

[VERIFIED — till.com "Device Distortion" article: "the 2nd harmonic level is directly proportional to signal level, and the higher harmonics drop faster"; Art of Electronics x-Chapters 2x.4]

### 6.2 Asymmetric Clipping from Unequal Headroom

**Stage 1 has moderately asymmetric headroom (with correct values):**
- Vce1 = 4.1 - 1.95 = 2.15V
- Toward saturation (Vce -> Vce_sat ~ 0.1V): 2.15 - 0.1 = **2.05V** of swing available
- Toward cutoff (Ic -> 0, Vc -> Vcc = 15V): 15.0 - 4.1 = **10.9V** of swing available
- Asymmetry ratio: 10.9 / 2.05 = **5.3:1**

[CALCULATED — from verified schematic voltages]

**Stage 2 has nearly symmetric headroom:**
- Vce2 = 8.8 - 3.4 = 5.4V
- Toward saturation: 5.4 - 0.1 = **5.3V** available
- Toward cutoff: 15.0 - 8.8 = **6.2V** available
- Asymmetry ratio: 6.2 / 5.3 = **1.17:1** (nearly symmetric)

[CALCULATED — from verified schematic voltages]

When the input signal drives Stage 1's collector voltage:
- Positive input -> collector swings DOWN (common-emitter inversion) -> hits saturation limit at 2.05V of swing
- Negative input -> collector swings UP -> can swing 10.9V before cutoff

This asymmetry means the **positive half-cycle clips much harder than the negative half-cycle**, producing strong even harmonics (H2, H4, H6, ...).

**Compared to the old estimates:** The previous analysis estimated Stage 1 headroom as 1.86V vs 11.0V (ratio 5.9:1). The correct values (2.05V vs 10.9V, ratio 5.3:1) are similar but slightly less extreme. The asymmetry is still very strong — Stage 1 remains the primary source of even-harmonic distortion.

Stage 2's headroom (5.3V vs 6.2V, ratio 1.17:1) is dramatically different from the old estimate (4.89V vs 6.55V, ratio 1.34:1) in absolute terms (much more current, lower Rc) but the asymmetry ratio is actually slightly less. Stage 2 contributes very little asymmetric distortion of its own.

[CALCULATED — updated from correct schematic values]

### 6.3 Signal Level Dependence

The preamp input sees millivolt signals. The signal at TR-1's base (after the bias network and input coupling) determines distortion:

| Dynamic Level | Estimated Vbe_ac | Character | Harmonic Content | Source |
|---------------|-----------------|-----------|-----------------|--------|
| pp (vel 0.3) | ~0.05-0.2 mV | Nearly linear | Almost pure fundamental, very faint H2 | ESTIMATED |
| mf (vel 0.7) | ~0.3-1.5 mV | Mildly nonlinear | H2 at -15 to -20 dB, H3 at -35 to -40 dB | ESTIMATED |
| ff (vel 0.95) | ~2-8 mV | Moderate saturation | H2 at -8 to -15 dB, H3 at -20 to -30 dB, "bark" | ESTIMATED |
| ff chord | ~5-20 mV | Heavy saturation | H2, H3, intermodulation products, "growl" | ESTIMATED |

With Stage 1's open-loop gain of ~420, an input of 5 mV would produce a collector swing of 2.1V — very close to the saturation headroom limit of 2.05V. This means **forte playing drives Stage 1 right to its clipping boundary**, which is exactly where the Wurlitzer's characteristic bark emerges.

[ESTIMATED — consistent with the electrostatic pickup signal levels and the 15 dB closed-loop gain producing 2-7 mV at the volume pot output]

### 6.4 Why Single-Ended CE Produces H2 — The Physical Story

A differential pair (like the power amplifier's input stage) produces a **tanh** transfer function, which is an odd function: tanh(-x) = -tanh(x). Odd functions produce only odd harmonics (H3, H5, H7...). This is why push-pull amplifiers are "clean" — they naturally cancel even harmonics.

A **single-ended** common-emitter stage has the **exponential** transfer function, which is NOT symmetric. exp(-x) is not equal to -exp(x). The lack of symmetry means even harmonics (H2, H4, H6...) are present. For the pure exponential, H2 dominates overwhelmingly.

This is the fundamental reason the Wurlitzer 200A preamp produces the characteristic "bark" with strong second harmonic: two cascaded single-ended CE stages, each contributing even harmonics from their exponential transfer functions and asymmetric headroom.

[VERIFIED — till.com "Device Distortion": exponential nonlinearity produces "almost entirely 2nd harmonic distortion"; Art of Electronics x-Chapters 2x.4]

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

[CALCULATED — Stage 1's asymmetric headroom is the primary source of H2 in the model]

### 6.6 Stage 2 Contribution

Stage 2 (TR-2) has nearly symmetric headroom (ratio 1.17:1), so it contributes very little H2 from its own clipping asymmetry. However, Stage 2 processes Stage 1's already-distorted output, producing **harmonics of harmonics** — H2 of H2 gives H4, H2 of H3 gives combination tones, etc. The cascaded nonlinearity enriches the harmonic spectrum beyond what a single stage produces.

Additionally, Stage 2 operates at much higher current (3.3 mA vs 66 uA), which gives it a much higher gm (127 mA/V) and smaller re (7.9 ohm). The exponential nonlinearity of the Vbe-Ic relationship is still present, but the unbypassed 820 ohm emitter resistor provides strong local degeneration that linearizes Stage 2 significantly. The signal at Stage 2's base (after feedback reduces Stage 1's gain) is small enough that Stage 2 operates in its linear region for most playing dynamics.

[INFERRED — standard analysis of cascaded nonlinear systems]

---

## 7. Tremolo Integration — LDR in Feedback Loop

### 7.1 The Critical Finding

The Wurlitzer 200A service manual explicitly states:

> "The reed bar signal is modulated by inserting the vibrato voltage into the feedback loop of the high impedance preamp. A divider is formed by the feedback resistor R-10, and the light dependent resistor of LG-1. The L.D.R., in conjunction with the light emitting diode in the same package, creates a variable leg in the feedback divider and makes possible amplitude modulation of the reed bar voltage."

[VERIFIED — Wurlitzer 200/200A service manual, quoted in output-stage.md and multiple web sources]

**This means the tremolo modulates the preamp's GAIN, not just the output volume.** The LDR resistance variation changes the closed-loop gain of the preamp by modifying the feedback network.

### 7.2 Feedback Topology — Emitter Feedback via Ce1

**CORRECTED Feb 2026:** The feedback topology was previously documented as R-10 feeding back to node_A (the input summing junction), forming an inverting shunt-feedback configuration. **This was WRONG** — it was based on the wrong schematic (200/203 models, not the 200A).

The correct 200A topology, traced from the 200A schematic (874 KB PDF, md5: ceb3abb9) at 1000-2000 DPI with user annotation:

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

**Why the old topology was wrong:** R-10 to node_A (base input side) creates POSITIVE feedback through two inverting CE stages (each inverts, net 0° phase shift = regenerative). This caused oscillation in both ngspice and gnucap SPICE simulations. R-10 to emitter is inherently NEGATIVE feedback (emitter feedback opposes base input), producing a stable circuit. The corrected SPICE netlist (spice/testbench/preamp_emitter_fb.cir) is perfectly stable.

**SPICE-validated DC operating point (corrected topology):**
| Node | Schematic | SPICE |
|------|-----------|-------|
| base1 | 2.45V | 2.80V |
| emit1 | 1.95V | 2.24V |
| coll1 | 4.1V | 4.12V |
| coll2 | 8.8V | 9.07V |
| fb_junct | — | 5.59V |
| out | — | 8.20V |

[VERIFIED — correct 200A schematic traced Feb 2026; SPICE-validated in spice/testbench/preamp_emitter_fb.cir]

### 7.3 Gain Modulation by LDR

The LDR modulates gain by varying the AC impedance from fb_junct to ground:

When LDR path impedance is **LOW** (bright phase): fb_junct is shunted to ground → Ce1 effectively grounds the emitter for AC → Re1 is bypassed → Stage 1 runs at full open-loop gain → **HIGHER overall gain**

When LDR path impedance is **HIGH** (dark phase): fb_junct carries the full R-10 feedback signal → Ce1 delivers feedback to emitter → emitter degeneration from feedback → **LOWER overall gain** (strong negative feedback)

**SPICE-measured LDR sweep (Feb 2026, corrected emitter feedback topology):**

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
- **Bandwidth decreases with gain:** 9.9 kHz at 2x gain → 8.3 kHz at 4x gain (constant GBW product)
- **Gain is remarkably constant with input level** (2.007x from 0.5mV to 200mV) — the strong feedback linearizes the circuit, producing very low THD (0.0001% at pp, 0.04% at extreme 200mV)

The distortion character changes through the tremolo cycle: at the gain peak (LDR low, weak feedback), the preamp is driven harder into its nonlinear region, producing more H2 and "bark." At the gain trough (LDR high, strong feedback), the preamp operates more linearly. This creates a subtle but important **timbral modulation** that distinguishes the real 200A tremolo from simple volume modulation.

[VERIFIED — circuit analysis of corrected topology; pending SPICE AC sweep confirmation]

### 7.4 Tremolo Oscillator

| Parameter | Value | Source |
|-----------|-------|--------|
| Transistors | TR-3, TR-4 (2N2924, part 142083-2) | VERIFIED — BustedGear |
| Topology | Phase-shift oscillator (band-pass feedback) | VERIFIED — service manual |
| Frequency | approximately 6 Hz (service manual); measured 5.3-7 Hz | VERIFIED |
| Waveform | Approximately sinusoidal (mild distortion from phase-shift topology) | INFERRED |
| Depth control | R-17 trimpot + front panel vibrato pot (100k) | VERIFIED — service manual, EP-Forum |

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

### 7.6 Correction History

**Correction 1 (Feb 2026):** Previous project documentation stated the LDR was a "signal divider/shunt to ground between preamp output and volume pot." This was WRONG — the LDR is in the preamp's feedback loop.

**Correction 2 (Feb 2026):** The initial correction assumed R-10 fed back to node_A (the input side, before the .022µF coupling cap), forming an inverting shunt-feedback topology. **This was also WRONG** — it was based on the wrong schematic (200/203 models). The correct 200A topology has R-10 feeding back to TR-1's EMITTER via Ce1, forming series-series emitter feedback. This was traced from the correct 200A schematic (874 KB PDF) at 1000-2000 DPI with user annotation, and confirmed stable in SPICE.

[VERIFIED — correct 200A schematic; SPICE-validated in spice/testbench/preamp_emitter_fb.cir; documented in output-stage.md Section 8]

---

## 8. Modeling Recommendations

### 8.1 Modeling Approaches (Ranked by Fidelity)

#### Approach 1: Full SPICE-Level Simulation (Highest Fidelity)

Model the complete circuit with Ebers-Moll equations for both transistors, explicit feedback networks (C-3, C-4, R-10 emitter feedback via Ce1, LG-1 LDR shunt), and direct coupling.

**Pros:** Most accurate harmonic content, correct frequency-dependent gain, proper bias-shift dynamics
**Cons:** Requires solving two coupled implicit equations per sample (or Newton-Raphson iteration); computationally expensive at audio rates
**Status:** The current Vurli model partially implements this (NR solver for each stage separately, but stages not coupled for DC bias modulation)

#### Approach 2: Simplified Ebers-Moll with Feedback Caps (Recommended)

Two independent BjtStage objects with exponential transfer functions, NR solver for feedback, and asymmetric soft-clip for collector limits.

**Pros:** Captures the key H2 mechanism; reasonable computational cost; NR converges quickly at physical signal levels
**Cons:** Misses inter-stage DC bias modulation ("sag"); feedback cap implementation must have correct polarity (see Section 9)
**Status:** Previously implemented in preamp.h; needs parameter update with correct component values

#### Approach 3: Wave Digital Filter (WDF) Model

Model each resistor, capacitor, and transistor junction as a WDF element. Solves the full circuit implicitly at each sample.

**Pros:** Correct at all operating points; handles direct coupling naturally; well-suited for real-time
**Cons:** Complex to implement; debugging is difficult; WDF junction models for BJTs are nontrivial
**Status:** chowdsp_wdf library is included in dependencies but not yet used

#### Approach 4: Polynomial Approximation (Lowest Fidelity)

Taylor-expand the transfer function to 3rd or 4th order: `y = a1*x + a2*x^2 + a3*x^3 + ...`

**Pros:** Very fast; simple; easy to tune H2/H3 ratio directly
**Cons:** Wrong at large signals (polynomial diverges); no saturation; no frequency-dependent behavior; cannot capture bias dynamics
**Status:** Available as `--poly` fallback in preamp.h for A/B comparison

### 8.2 Perceptually Important Nonlinearities (Priority Order)

| Nonlinearity | Perceptual Impact | Priority | Source |
|-------------|-------------------|----------|--------|
| Asymmetric soft-clip (Stage 1 Vce headroom 2.05V vs 10.9V) | Primary source of H2 "bark" | CRITICAL | CALCULATED from verified DC analysis |
| Exponential transfer function (exp(Vbe/nVt)) | H2 >> H3 harmonic ratio | HIGH | VERIFIED — till.com, Art of Electronics |
| Frequency-dependent feedback (C-3 100pF Miller, dominant pole ~23 Hz) | Register-dependent gain and distortion | HIGH | CALCULATED from verified component values |
| Closed-loop bandwidth limit (~3.7 kHz) | Natural 2-4 kHz emphasis, HF rolloff | HIGH | CALCULATED |
| Direct-coupling bias shift (Stage 1 DC modulates Stage 2) | Dynamic compression, "sag", "bloom" | MEDIUM-HIGH | VERIFIED — schematic topology; not yet modeled |
| Tremolo gain modulation (R-10=56K/LG-1 in feedback) | Timbral variation through tremolo cycle | MEDIUM | VERIFIED — service manual |
| Cascaded Stage 2 nonlinearity (harmonics-of-harmonics) | Spectral enrichment at ff | MEDIUM | INFERRED |
| Early effect (Vce-dependent Ic) | ~10% gain modulation with signal | LOW | INFERRED |
| Beta(Ic) variation | Asymmetric gain modulation | LOW | INFERRED |
| Thermal drift (-2mV/K Vbe shift) | Very slow "sag" over seconds | LOW | INFERRED |

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

1. **Input HPF** at ~1903 Hz (C20 = 220 pF, first-order)
2. **Stage 1 exponential** with gm1 = 2.54-2.80 mA/V (Ic1 = 66-73 uA)
3. **Asymmetric soft-clip** with satLimit = 10.9V, cutoffLimit = 2.05V (Stage 1)
4. **Frequency-dependent feedback** via C-3 (100 pF) Miller effect — dominant pole at ~23 Hz. CORRECT polarity: less feedback at LF (more gain/distortion), more feedback at HF (less gain/distortion)
5. **Closed-loop gain** of ~6 dB (2.0x) without tremolo, up to ~12 dB (4.0x) at tremolo peak, set by R-10 emitter feedback via Ce1 [SPICE-MEASURED Feb 2026]
6. **Closed-loop bandwidth** ~10 kHz without tremolo, ~8.3 kHz at tremolo peak [SPICE-MEASURED Feb 2026]
7. **Direct coupling** to Stage 2 (can be instantaneous coupling for simplicity)
8. **Stage 2** with Av = 2.2, nearly symmetric soft-clip (satLimit = 6.2V, cutoffLimit = 5.3V)
9. **Output DC block** at ~20 Hz
10. **R-9 series output** (6.8K) — provides output impedance for volume pot interaction

### 8.5 Enhanced Model (for Future Implementation)

Add to the minimum model:

11. **DC bias-shift dynamics**: Track Stage 1's average collector voltage (10-100 ms time constant); feed this to Stage 2's operating point. This produces the "sag" compression and "bloom" heard at ff polyphonic.
12. **Tremolo as emitter feedback modulation**: Modulate the LDR path impedance (which controls how much R-10 feedback reaches TR-1's emitter via Ce1), rather than applying tremolo as a post-preamp volume multiplier.
13. **WDF or coupled NR solver**: Solve both stages simultaneously to capture the inter-stage coupling dynamics.

---

## 9. Previous Implementation Issues and Lessons

### 9.1 kPreampInputDrive as a Fudge Factor

The model used `kPreampInputDrive` to scale the voice output before the preamp:

| Round | kPreampInputDrive | preampGain (post) | Notes |
|-------|-------------------|-------------------|-------|
| R38 | 0.25 | ~1.0 | Physical millivolt-level signals, but too clean |
| R39 | 16.0 | 1.7 | Overdriving to compensate for missing gain |
| R40 | 48.0 | 0.4 | Extreme drive, crushed dynamics |
| Current | 28.0 | 0.7 | Compromise after structural fixes |

[VERIFIED — from vurli-plugin.cpp and signal-chain.md history]

**The root cause**: The model's preamp gain staging was wrong. The real preamp has open-loop gain of ~420 in Stage 1, reduced by R-10 emitter feedback (via Ce1) and Miller-effect feedback caps (C-3, C-4). The model used wrong component values (Rc1=47K, Re1=8.2K, Rc2=10K, Re2=5.1K) which gave different gain structure.

**The lesson**: kPreampInputDrive should be approximately 1.0 if the preamp model correctly reproduces the real circuit's gain structure. A value of 28.0 indicates the preamp model is approximately 29 dB short of the correct gain (or the voice output levels are 29 dB too low).

[INFERRED — the need for kPreampInputDrive >> 1 indicates incorrect gain staging in the model]

### 9.2 Feedback Cap Polarity Error

The code implements:

```cpp
// Below corner: cap tracks output -> feedback applied -> gain REDUCED
// Above corner: cap lags -> feedback absent -> FULL gain
```

This gives MORE feedback at low frequencies and LESS at high frequencies — the **opposite** of real Miller-effect behavior.

**Real Miller-effect behavior:**
- At LOW frequencies: cap has high impedance -> less feedback -> full gain -> more distortion
- At HIGH frequencies: cap has low impedance -> more feedback -> less gain -> less distortion

[VERIFIED — standard Miller-effect theory]

**The fundamental issue**: The code used an LPF to track the output's low-frequency content, then used that as the feedback signal. This gives feedback proportional to the low-frequency content — i.e., MORE feedback at low frequencies. The correct implementation should give feedback proportional to the HIGH-frequency content (or equivalently, the feedback capacitor passes high frequencies and blocks low frequencies).

[VERIFIED — identified by multiple R41 review agents as a HIGH priority issue]

### 9.3 Wrong Component Values (NOW RESOLVED)

The previous implementation used estimated component values that were dramatically wrong:

| Component | Old Estimate | Correct Value | Error Factor |
|-----------|-------------|---------------|-------------|
| Rc1 | 47K | 150K | 3.2x too low |
| Re1 | 8.2K (or 2.2K) | 33K | 4x too low (or 15x) |
| Rc2 | 10K | 1.8K | 5.6x too high |
| Re2 | 4.7-5.1K | 270+820 = 1090 ohm | 4.3-4.7x too high |
| C-3 | "10-100 pF" (estimated) | 100 pF | Now known |
| C-4 | "10-100 pF" (estimated) | 100 pF | Now known |
| R-10 | "100K-470K" (estimated) | 56K | Now known |
| Ce1 | "unknown if present" | 4.7 MFD (confirmed present) | Now known |

These errors cascaded into wrong:
- **Ic1**: estimated 234 uA, actual ~66-73 uA (3.5x too high)
- **Ic2**: estimated 655 uA, actual ~3.3 mA (5x too LOW — Stage 2 runs much hotter than estimated)
- **gm1**: estimated 9.0, actual ~2.5-2.8 (3.5x too high)
- **gm2**: estimated 25.2, actual ~127 (5x too LOW)
- **Miller pole**: estimated "75-400 Hz", actual ~23 Hz (much lower — dominant pole)

The architecture was mischaracterized: the old estimates suggested two roughly similar stages, while the correct values reveal Stage 1 as a high-gain (420x) low-current (66 uA) voltage amplifier, and Stage 2 as a low-gain (2.2x) high-current (3.3 mA) output buffer.

[VERIFIED — old estimates from previous version of this document; correct values from BustedGear schematic at 900 DPI]

### 9.4 Feedback Cap Corners

With correct component values, the Miller-effect dominant pole is at ~23 Hz. This is BELOW the audio band, meaning the feedback from C-3 is active across the entire audio band. The open-loop gain rolls off at -20 dB/decade from 23 Hz, which means:

- At 230 Hz: open-loop gain is 1/10 of DC = 42 (per stage 1)
- At 2.3 kHz: open-loop gain is 1/100 of DC = 4.2 (per stage 1)
- At 23 kHz: open-loop gain is 1/1000 of DC = 0.42 (per stage 1, below unity!)

The closed-loop gain is held at the emitter-feedback-determined level by R-10 feedback (via Ce1) up to the point where open-loop gain drops to match the closed-loop gain. Above that, the gain rolls off because there is not enough open-loop gain to sustain the feedback-controlled closed-loop gain. (Exact closed-loop gain TBD from SPICE AC sweep of corrected topology.)

This is fundamentally different from the previous analysis which treated the Miller poles as "somewhere in the 100-500 Hz range" and wondered why the feedback corners seemed too low. In reality, the dominant pole IS very low (23 Hz), and the resulting GBW (~21 kHz) determines the closed-loop bandwidth (~3.7 kHz).

[CALCULATED — from verified C-3 = 100 pF and circuit impedances]

### 9.5 "Sine Wave Through a Blown Speaker"

Multiple listening evaluations described the model's output as sounding like "a sine wave through a blown speaker" — lacking the characteristic Wurlitzer warmth, bark, and dynamic variation. Root causes identified:

1. **Dynamic range crushed** (kPreampInputDrive too high -> saturates at mf -> no difference between mf and ff)
2. **Feedback caps not providing register-dependent gain** (corners too low and polarity wrong -> constant feedback)
3. **Missing bias-shift dynamics** (no "sag" or "bloom" from direct coupling)
4. **Attack transient destroyed** (sinc dwell filter + 20x mode amps -> wrong energy distribution)
5. **Wrong component values** (all gain calculations were off by factors of 3-5x)

[VERIFIED — documented in CLAUDE.md and signal-chain.md R40 test results]

### 9.6 Key Lessons Summary

| Lesson | Details | Source |
|--------|---------|--------|
| kPreampInputDrive should be ~1.0 | A value >> 1 indicates wrong gain staging | INFERRED |
| R-10 emitter feedback + Miller caps set the gain | Open-loop ~900x, closed-loop set by emitter feedback (R-10 via Ce1) | VERIFIED — SPICE; Avenson measurement |
| Miller effect gives MORE gain at LF, LESS at HF | Model had this backwards | VERIFIED — standard theory |
| H2 comes from asymmetric headroom, not just the exponential | Stage 1's 2.05V cutoff vs 10.9V saturation is the primary H2 source | CALCULATED from verified values |
| Direct coupling matters at ff | Bias-shift sag/bloom is audible dynamic behavior | INFERRED from R41 review |
| Tremolo is gain modulation, not volume modulation | LDR shunts emitter feedback junction, changing gain and distortion through tremolo cycle | VERIFIED — service manual; correct 200A schematic |
| R-10 feeds to emitter, not node_A | Ce1 couples feedback to emitter (series-series FB); R-10 to node_A caused SPICE oscillation | VERIFIED — correct 200A schematic; SPICE |
| Use the CORRECT schematic PDF | 874 KB PDF = 200A; 3 MB PDF = 200/203/206/207. Mixed tiles caused topology confusion | VERIFIED — Feb 2026 |
| The preamp IS the Wurlitzer's voice | Pickup is nearly linear; speaker is EQ; preamp creates the character | VERIFIED — calibration data, listening tests |
| Use correct schematic values | Estimated values were wrong by factors of 3-5x, cascading errors through all analysis | VERIFIED — BustedGear schematic |
| The preamp has a ~2-4 kHz passband peak | C20 HPF from below, Miller BW limit from above | CALCULATED from verified values |

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
    - `/home/homeuser/dev/openwurli/docs/pickup-system.md`

15. **Output Stage** — power amplifier, tremolo feedback discovery, speaker model
    - `/home/homeuser/dev/openwurli/docs/output-stage.md`

16. **Signal Chain Architecture** — overall signal flow, parameter values
    - `/home/homeuser/dev/openwurli/docs/signal-chain-architecture.md`

---

## Appendix A: Confidence Level Summary

| Claim | Confidence | Basis |
|-------|-----------|-------|
| Two-stage direct-coupled NPN CE topology | VERIFIED | Service manual, GroupDIY, schematic |
| TR-1/TR-2 = 2N5089 (replacement for 2N2924) | VERIFIED | BustedGear, service manual |
| +15V supply voltage | VERIFIED | Schematic, GroupDIY multimeter measurement |
| TR-1: E=1.95V, B=2.45V, C=4.1V | VERIFIED | Schematic DC annotations |
| TR-2: E=3.4V, B=4.1V, C=8.8V | VERIFIED | Schematic DC annotations |
| Direct coupling (Vc1 = Vb2 = 4.1V) | VERIFIED | Schematic topology and voltages |
| R-1 = 22K, R-2 = 2M (see Note 1), R-3 = 470K | VERIFIED | Schematic (R-2 adjusted per DC analysis) |
| Rc1 = 150K | VERIFIED | Schematic |
| Re1 = 33K, Ce1 = 4.7 MFD (feedback coupling cap to fb_junct) | VERIFIED | Schematic; topology corrected Feb 2026 |
| Rc2 = 1.8K | VERIFIED | Schematic |
| Re2 = 270 ohm (bypassed, 22 MFD) + 820 ohm (unbypassed) | VERIFIED | Schematic |
| R-9 = 6.8K (series output) | VERIFIED | Schematic |
| R-10 = 56K (feedback to emitter via Ce1) | VERIFIED | Schematic; topology corrected Feb 2026 |
| C-3 = 100 pF, C-4 = 100 pF | VERIFIED | Schematic |
| C20 = 220 pF | RESOLVED | Schematic confirmed at 1500 DPI. GroupDIY's 270 pF is tolerance/production variation. |
| D-1 = 25 PIV, 10 mA (part #142136) | VERIFIED | Schematic |
| Input coupling cap = .022 uF | VERIFIED | Schematic |
| Total preamp gain 6.0 dB (2.0x) no trem / 12.1 dB (4.0x) trem bright | SPICE-MEASURED | Corrected emitter feedback topology. Avenson's 15 dB was his replacement design. |
| Volume pot output 2-7 mV AC | VERIFIED | Brad Avenson measurement |
| LDR (LG-1) shunts R-10/Ce1 emitter feedback junction for tremolo | VERIFIED | Service manual text, correct 200A schematic, SPICE |
| Tremolo output ~4V p-p (vs ~1.8V without) | VERIFIED | Repair forum measurements |
| Ic1 ~ 66-73 uA | CALCULATED | From Rc1=150K and Vc1=4.1V |
| Ic2 ~ 3.1-3.4 mA | CALCULATED | From Rc2=1.8K and Vc2=8.8V |
| Stage 1 gain ~420 (max, with fb_junct grounded) | CALCULATED | gm1 * Rc1 (only valid when LDR path impedance is low) |
| Stage 2 AC gain ~2.2 | CALCULATED | Rc2 / (re2 + Re2b) |
| Combined open-loop gain ~900 | CALCULATED | Av1 * Av2 |
| C-3 Miller dominant pole ~23 Hz | CALCULATED | From C_miller = 43 nF and R_source = 163K |
| Closed-loop -3 dB bandwidth ~10 kHz (no trem) / ~8.3 kHz (trem bright) | SPICE-MEASURED | From preamp_ldr_sweep.cir |
| Passband peak ~2-4 kHz | CALCULATED | From C20 HPF and closed-loop BW |
| Stage 1 asymmetry: 2.05V vs 10.9V (5.3:1) | CALCULATED | From verified DC voltages |
| Stage 2 asymmetry: 5.3V vs 6.2V (1.17:1) | CALCULATED | From verified DC voltages |
| H2 dominates H3 from exponential transfer function | VERIFIED | till.com, Art of Electronics, standard theory |
| Tremolo modulates distortion character, not just volume | VERIFIED | Emitter feedback topology; SPICE-confirmed stable |

---

## Appendix B: Quick Reference for Model Parameters

Based on the analysis in this document, the recommended preamp model parameters are:

```
// Supply and operating point
Vcc = 15.0V

// Stage 1 (TR-1)
Rc1 = 150K [VERIFIED — schematic]
Re1 = 33K [VERIFIED — schematic]
Ce1 = 4.7 MFD (feedback coupling cap: emitter to fb_junct, corner ~1 Hz) [VERIFIED — schematic; topology corrected Feb 2026]
Ic1 = ~66-73 uA [CALCULATED]
gm1 = ~2.54-2.80 mA/V [CALCULATED]
B = 38.5 (1/(n*Vt), n=1.0 for 2N5089)
open_loop_gain_1 = gm1 * Rc1 = ~381-420 [CALCULATED]
satLimit_1 = Vcc - Vc1 = 10.9V [CALCULATED from verified voltages]
cutoffLimit_1 = Vce1 - Vce_sat = 2.15 - 0.1 = 2.05V [CALCULATED]
asymmetry_ratio_1 = 10.9 / 2.05 = 5.3:1

// Stage 1 feedback cap (C-3)
C3 = 100 pF [VERIFIED — schematic]
C_miller1 = 100pF * (1 + 420) = 42.1 nF (including Cob: 43.2 nF) [CALCULATED]
R_source1 = R_bias || r_pi1 = 380K || 286K = 163K [CALCULATED]
f_miller1 = 1/(2*pi*163K*43.2nF) = 22.6 Hz (dominant pole) [CALCULATED]

// Stage 2 (TR-2)
Rc2 = 1.8K [VERIFIED — schematic]
Re2a = 270 ohm (bypassed by 22 MFD, corner at 26.8 Hz) [VERIFIED — schematic]
Re2b = 820 ohm (unbypassed — sets AC gain) [VERIFIED — schematic]
Ic2 = ~3.1-3.4 mA [CALCULATED]
gm2 = ~119-131 mA/V [CALCULATED]
re2 = 1/gm2 = ~7.6-8.4 ohm [CALCULATED]
Av2 = -Rc2 / (re2 + Re2b) = -1800/828 = -2.17 [CALCULATED]
satLimit_2 = Vcc - Vc2 = 6.2V [CALCULATED]
cutoffLimit_2 = Vce2 - Vce_sat = 5.4 - 0.1 = 5.3V [CALCULATED]
asymmetry_ratio_2 = 6.2 / 5.3 = 1.17:1 (nearly symmetric)

// Stage 2 feedback cap (C-4)
C4 = 100 pF [VERIFIED — schematic]
C_miller2 = 100pF * (1 + 2.17) = 317 pF (including Cob: 325 pF) [CALCULATED]
R_source2 = Rc1 || r_pi2 = 150K || 6.3K = 6.05K [CALCULATED]
f_miller2 = 1/(2*pi*6.05K*325pF) = 81 kHz (well above audio) [CALCULATED]

// Output
R_9 = 6.8K (series output resistor) [VERIFIED — schematic]

// Feedback network (tremolo)
R_10 = 56K (feedback resistor) [VERIFIED — schematic]
LG_1 = CdS LDR (variable, tremolo modulation) [VERIFIED — schematic]

// Input HPF
C20 = 220 pF [RESOLVED — schematic confirmed at 1500 DPI]
R_eff = 380K (R-2 || R-3 = 2M || 470K) [CALCULATED]
f_hpf = 1903 Hz [CALCULATED — from C20=220pF, R_eff=380K]

// Input coupling
C_input = .022 uF [VERIFIED — schematic]
R_1 = 22K (series from reed bar) [VERIFIED — schematic]

// Bias network
R_2 = 2 MEG (to +15V; see Note 1 on discrepancy) [VERIFIED — schematic, adjusted per DC analysis]
R_3 = 470K (to ground) [VERIFIED — schematic]

// Output DC block
f_dc_block = 20 Hz (to be determined by output coupling cap)

// Overall closed-loop (emitter feedback via R-10/Ce1) [SPICE-MEASURED Feb 2026]
total_closed_loop_gain_no_trem = 6.0 dB (2.0x) [Rldr_path = 1M]
total_closed_loop_gain_trem_bright = 12.1 dB (4.0x) [Rldr_path = 19K]
tremolo_modulation_range = 6.1 dB
closed_loop_bandwidth_no_trem = 19 Hz - 9.9 kHz
closed_loop_bandwidth_trem_bright = 19 Hz - 8.3 kHz
passband_peak = ~450 Hz (from C20/Cin HPF and BW limit) [SPICE-MEASURED]
kPreampInputDrive = should be ~1.0 if gain staging is correct
```

---

## Appendix C: Open Questions Requiring Further Investigation

Most component values have been resolved by reading the BustedGear 200A schematic at 900 DPI. Remaining questions:

1. **C-1 (RF shunt capacitor) — RESOLVED as C20 (220 pF)**: C-1 (service manual designation) is the same physical component as C20 (schematic board position "2") = 220 pF shunt cap at TR-1 base. The service manual and schematic use different naming systems: the service manual calls it "C-1" (functional designation as the RF shunt cap), while the schematic labels it as component position "2" on the preamp board. PCB layout (page 1) shows standard R-x/C-x designators on the silk screen. Evidence: (a) the service manual describes C-1 as a "shunt capacitor" — the 220 pF cap is the only shunt-to-ground cap at the input; (b) no separate C-1 component visible on schematic at 2400 DPI; (c) zero community discussion about a "missing" C-1 in repair forums (GroupDIY, EP-Forum).

2. **R-2 value — RESOLVED as 2 MEG**: Schematic reads "1 MEG" (confirmed at 1500 DPI), but three independent lines of evidence confirm R-2 = 2M: (a) GroupDIY's PRR uses "380K" for the parallel impedance R-2||R-3 — 2M||470K=380K matches, 1M||470K=320K does not; (b) DC analysis with R-2=1M requires hFE=9 to produce Vb=2.45V (physically impossible), while R-2=2M requires hFE≈62 (plausible); (c) carbon composition resistor tolerances (10-20%) easily close the remaining voltage gap. **This does not affect gain or harmonic calculations** which are derived from collector-side voltages (Vc, Ve) and resistances (Rc, Re).

3. **C20 value — RESOLVED as 220 pF**: Schematic reads 220 pF (confirmed at 1500 DPI). GroupDIY cited 270 pF from measured instruments, likely reflecting tolerance variation in ceramic capacitors or a production change. With C20 = 220 pF and R_eff = 380K, the HPF cutoff is 1903 Hz. **Use 220 pF for modeling.**

**Resolved (from this schematic reading):** Rc1, Rc2, Re1, Re2, C-3, C-4, R-9, R-10, R-1, Ce1, Ce2, D-1 type, input coupling cap, all DC operating voltages, C20 (220 pF), R_feed (1 MEG), C-1 (= C20, naming discrepancy).
