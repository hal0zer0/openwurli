# Wurlitzer 200/200A Electrostatic Pickup System — Complete Technical Reference

## Document Purpose and Audience

This document provides the complete physics, circuit analysis, and modeling decisions for the Wurlitzer 200A electrostatic (capacitive) pickup system.

> **See also:** [Reed and Hammer Physics](reed-and-hammer-physics.md) (the vibrating element), [Preamp Circuit](preamp-circuit.md) (what the pickup feeds into), [Signal Chain Architecture](signal-chain-architecture.md) (overall signal flow)

---

## 1. Physical Construction

### 1.1 Overview

The Wurlitzer 200/200A uses an **electrostatic (capacitive) pickup** to convert reed vibration into an electrical signal. This is fundamentally different from the Rhodes piano's electromagnetic pickup: the Wurlitzer senses **displacement** (gap change) via a varying capacitor, while the Rhodes senses **velocity** (rate of flux change) via electromagnetic induction.

### 1.2 Pickup Plate (Comb Electrode)

The pickup electrode is a single continuous metal plate with **comb-like teeth** (slots) cut into it, forming a U-channel groove for each reed. All 64 reeds share this single common pickup plate — the signals from all reeds are inherently summed at the electrical node of the pickup plate before reaching the preamp.

The U-channel geometry means each reed is surrounded by the pickup electrode on **three faces** (bottom and two sides), not just one face as in a simple parallel-plate capacitor. This increases the effective capacitance per reed and makes the capacitance depend on both vertical displacement and lateral centering of the reed.

### 1.3 Reed-to-Plate Gap Dimensions

Measured physical pickup slot widths from EP-Forum thread (140B/200 series, similar geometry):

| Reed Range | Reed Width | Slot Width | Side Clearance (each) |
|-----------|-----------|-----------|----------------------|
| Reeds 1-14 (bass) | 0.151" (3.84 mm) | 0.172" (4.37 mm) | 0.0115" (0.29 mm) |
| Reeds 15-20 | 0.127" (3.23 mm) | 0.145" (3.68 mm) | 0.009" (0.23 mm) |
| Reeds 21-42 (mid) | 0.121" (3.07 mm) | 0.139" (3.53 mm) | 0.0075" (0.19 mm) |
| Reeds 43-50 | 0.110" (2.79 mm) | 0.114" (2.90 mm) | ~0.002" (0.05 mm) |
| Reeds 51-64 (treble) | 0.097" (2.46 mm) | 0.114" (2.90 mm) | 0.0085" (0.22 mm) |

**Patent design specification (US 2,919,616, Andersen, 1960):** The pickup slot is specified as **"0.005 inch greater than the width of the associated reed, thereby providing a normal clearance gap of about 0.0025 inch on either side of the reed."** The patent also references spacing tools of ".002, .0025, .003 inch etc." for adjusting pickup sensitivity. The 0.0025"/side (0.064mm) design specification is tighter than the community-measured values above (0.002"-0.012"), likely reflecting manufacturing tolerances and differences between patent ideals and production reality.

**Critical note:** These are **slot widths** (lateral clearance), not the vertical gap between the reed face and the bottom of the slot. The vertical gap (d₀) is **not documented in any patent, service manual, or academic source** — confirmed by a 99-agent adversarial deep-research pass (2026-07). Service manual procedures describe voicing only qualitatively (reed flat-face coplanar with the pickup face; the over-loud remedy bends the pickup ends up 1/32″–1/16″), with no absolute gap dimension. **Correction:** an earlier version of this doc cited "Pfeifle's 1.5 mm (EP300)" as the only published d₀ — that is a **misattribution**. The Pfeifle DAFx-17 (2017) paper models the **EP200** (not EP300, not 200A), contains no "1.5 mm" and no rest-gap value at all, and reports its instrument's polarizing voltage as 170 V manual / 130 V measured. There is **no published d₀ for any Wurlitzer** that survives verification. The full provenance and the reason d₀ is *structurally* underdetermined (not merely unfound) live in **[Reed–Pickup Displacement](reed-pickup-displacement.md)**; `DISPLACEMENT_SCALE` absorbs the unknown as an OBM-calibrated constant.

### 1.4 Slot Width Ratios

- Bass (reed 1) to treble (reed 64) slot width ratio: 0.172 / 0.114 = **1.51:1**
- Bass to treble side clearance ratio: 0.0115 / 0.0085 = **1.35:1**

These ratios constrain any register-dependent gap scaling model. The current OpenWurli model uses `2^((60-key)/60)` which gives a ratio of ~1.74:1 across the keyboard — moderately steeper than the measured 1.51:1 slot width ratio.

### 1.5 Pickup Plate Dimensions (per reed)

The effective electrode area per reed depends on the U-channel geometry (estimated from slot widths and typical 200-series photos):

- **Bottom face:** approximately slot_width x active_length. Active length (portion of reed over the pickup) varies by register but is roughly 3-8 mm.
- **Side faces (two):** approximately gap_depth x active_length per side. The depth of the U-channel is approximately 2-4 mm based on photos.
- **Total effective area per reed (bass):** roughly 3-5 mm x 6-8 mm = 18-40 mm^2 for the bottom face, plus two side faces of perhaps 2-3 mm x 6-8 mm = 12-24 mm^2 each.

These are rough estimates. Precise dimensions require direct measurement of a disassembled reed bar.

### 1.6 Grounding and Shielding

The Wurlitzer 200A includes two shielding elements to reduce hum pickup:

- **Hum shield:** A separate conductive shield placed close to the pickup plate.
- **Reed bar shield:** Added in later production runs and as an aftermarket upgrade. Reduces electromagnetic interference from power transformer and mains wiring.

"Most hum from Wurlitzer electric pianos derives from the reed bar" because the high-impedance electrostatic pickup acts as an antenna for electromagnetic interference.

### 1.7 Pickup Longitudinal Position (Along Reed Axis)

Multiple Miessner and Andersen patents specify the pickup electrode position along the reed axis:

- **US 2,942,512** (Miessner, 1960): "the nodal point of the second partial of the reed vibration (typically removed from the secured end of the reed by approximately 78%, and from the free end by approximately 22%, of the active length of the reed)"
- **US 2,966,821** (Miessner, 1961): "approximately 78/100 of the reed length away from the fixed extremity"
- **US 2,919,616** (Andersen, 1960): "Pick-up centers align at approximately 0.22L from the reed's free end"

This position (0.78L from the clamp = 0.22L from the tip) corresponds to the **node of the 2nd partial** for a bare cantilever beam. Placing the pickup at this node strongly suppresses the inharmonic 2nd partial (at 6.27x the fundamental), which would otherwise produce dissonant overtones.

**Discrepancy with current model:** The OpenWurli spatial coupling code (`tables.rs:spatial_coupling_coefficients()`) integrates mode shapes over the last 6mm at the reed tip (`PLATE_ACTIVE_LENGTH_MM = 6.0`), placing the pickup at xi=[0.92, 1.0] — the **antinode** of mode 2, not its node. At the tip, mode 2 is at maximum amplitude and couples strongly.

**Resolution (open question):** The 200/200A production reed bar has a comb construction where the reed tip enters a slot. The physical geometry places the pickup electrode at or very near the free end. The 0.22L patent specification may describe an ideal that was not achieved in the mass-production design, or it may describe the effective center of the pickup face (which could be inset from the tip). Without direct measurement of a disassembled 200A reed bar, this discrepancy remains unresolved. Since the OBM-calibrated `BASE_MODE_AMPLITUDES` already embed the correct per-mode energy (including whatever spatial coupling the real pickup provides), this primarily affects the physical interpretation, not the sound.

**Pickup face geometry (US 2,966,821):** The pickup face can be rectangular (horizontal:vertical limited to 3:1), square, or circular. Rectangular faces produce stronger signals; square faces "flatten peak tops." Maximum angular deviation between pickup face and reed plane: approximately 11-18 degrees in production.

---

## 2. Electrical Circuit — Polarizing Voltage and Bias Network

### 2.1 Polarizing Voltage Supply

The reed bar requires a DC polarizing voltage to function. This voltage establishes the electric field between the reed and pickup plate.

| Parameter | Value |
|-----------|-------|
| Nominal polarizing voltage | **147 V DC** |
| Voltage source | Half-wave rectifier from dedicated transformer winding |
| Filter capacitors | Three 0.33 uF capacitors in RC filter chain |
| Feed resistor to reed bar | **1 MEG** (component 56 in HV supply filter chain) |

**Voltage variation:** Different sources cite slightly different voltages:
- Service manual: 147 V
- Tropical Fish: "approximately 150V DC"
- Some repair sources: 170 V, 180 V (likely different models or measurement conditions)

The 200A service manual specifies 147V from the half-wave rectifier. Higher values (170-180V) may reflect the model 200 (non-A) or measurements before load.

### 2.2 Complete Bias Circuit (DC Path)

```
AC Mains → Power Transformer (dedicated winding)
         → Half-wave rectifier diode
         → RC filter (3 × 0.33 µF capacitors with series resistors)
         → R-2 (1 MΩ) → R-1 (22K) → Reed bar pickup plate (all reeds in parallel)
         → Through per-reed capacitance to grounded reeds
```

> **Naming note (corrected 2026-09):** the polarizing feed resistor IS
> **R-2 (1 MEG)** — instrumented re-reads of the schematic place it from the
> +150V line to the R-1/C-1 junction, on the PICKUP side of the coupling
> cap. Earlier revisions of this document invented a separate "R_feed"
> and additionally placed a phantom "R-2 = 2M" at TR-1's base; those were
> two names for a misreading of one part (see preamp-circuit.md §11).
> There is NO bias resistor from TR-1's base to any supply — the base is
> biased by R-3's DC-feedback return from TR-2's emitter divider.

The reeds themselves are grounded through the reed bar mounting, which is electrically connected to chassis ground. The polarizing voltage appears across the air gap between the pickup plate and each reed.

### 2.3 Signal Path (AC Path)

```
Reed vibration → Varying capacitance per-reed
               → Current flow through all reed-plate capacitors (summed)
               → Pickup plate node → R-1 (22K)
               → R-1/C-1 junction
                    - R-2 (1M) provides the DC path to the 147V rail
                      (AC ground at audio via the supply filter caps)
               → C-1 (.022 uF) coupling cap (blocks 147V DC; passes audio)
               → TR-1 base node
                    - R-3 (470K) to the R-7/R-8 junction: DC-feedback bias
                      (AC ground at audio via C-7)
                    - C-2 (220 pF) shunt: bridged to the plate capacitance
                      at audio because C-1 is a short (see §3.7)
                    - D1: reverse-polarity protection diode
               → TR-1 base (preamp stage 1)
```

**Critical topology note:** C-1 separates the DC domains (the plate side
sits near 147V; the base sits at ~2.45V from the R-3 servo) but is a
short at audio — which puts **C-2's 220 pF effectively in parallel with
the plate capacitance**, nearly doubling the HPF capacitance. This is
the mechanism the pre-revision model missed.

### 2.4 Input Impedance Network

**Plate-side network:**

| Component | Value | Function |
|-----------|-------|----------|
| R-1 | 22 kΩ | Series, plate → R-1/C-1 junction |
| R-2 | 1 MΩ | Polarizing feed: +150V line → R-1/C-1 junction (AC ground at audio) |
| C-1 | .022 µF | Coupling to TR-1 base; blocks 147V DC; a SHORT at audio |

**At the TR-1 base node:**

| Component | Value | Function |
|-----------|-------|----------|
| R-3 | 470 kΩ | DC-feedback bias, base → R-7/R-8 junction (470K to AC ground at audio) |
| C-2 | 220 pF | At the base; bridged to the plate capacitance through C-1 at audio. Return assumed ground (broken-rail drawing defect) |
| D1 | Small signal diode, 25 PIV, 10 mA (part #142136-5) | Reverse-polarity transient protection |

The base's AC input impedance is R-3 rolled off by C-2 plus a small Miller
term (≈459K peak near 200 Hz, 339K at 1 kHz, tremolo-independent) — the
transistor's own contribution is bootstrapped to ~34 MΩ by the series
feedback. The old "R-2‖R-3 = 380K bias impedance" and the "C20 1903 Hz
LPF" figures were artifacts of the misread topology (preamp-circuit.md §11).

**Why BJT, not FET?** Two reasons hold under the corrected topology:
1. **Microphonics:** a few-hundred-kΩ base impedance (vs a FET's tens of
   MΩ) reduces sensitivity to reed-bar mechanical vibration coupling in
   electrically.
2. **Overvoltage protection:** during tuning, reeds can short to the
   pickup plate, producing ~150V transients. The BJT base-emitter
   junction and D1 clamp these naturally; a FET gate would be damaged.

---

## 3. Electrostatics — Signal Voltage Derivation

### 3.1 Fundamental Capacitance Relationship

For a parallel-plate capacitor:

```
C = epsilon_0 * A / d
```

where:
- `epsilon_0` = 8.854 x 10^-12 F/m (permittivity of free space)
- `A` = effective plate area (m^2)
- `d` = gap distance (m)

For the Wurlitzer's U-channel geometry, the effective capacitance is larger than a simple parallel plate due to three faces, but the 1/d dependence on gap distance still dominates for the bottom face (where the reed's vibration axis is perpendicular to the plate).

### 3.2 Constant-Charge Regime (High Frequency)

When the signal frequency is much higher than the RC cutoff frequency (f >> f_c), charge on the capacitor cannot change fast enough to track the capacitance variations. The charge remains approximately constant at:

```
Q_0 = C_0 * V_bias
```

where `C_0` is the static (rest) capacitance and `V_bias` is the polarizing voltage.

The instantaneous voltage across the capacitor becomes:

```
V(t) = Q_0 / C(t) = V_bias * C_0 / C(t) = V_bias * d(t) / d_0
```

Since `d(t) = d_0 + x(t)` where `x(t)` is the reed displacement:

```
V(t) = V_bias * (d_0 + x(t)) / d_0 = V_bias * (1 + x(t)/d_0)
```

The AC signal component (subtracting the DC bias) is:

```
V_ac(t) = V_bias * x(t) / d_0
```

This is the **open-circuit electrical sensitivity formula**: `Se = V_bias / d_0`.

**For small displacements (x << d_0), this is LINEAR in displacement.** The signal voltage is directly proportional to reed displacement with gain `V_bias / d_0`.

### 3.3 Constant-Voltage Regime (Low Frequency)

When f << f_c, the bias circuit can supply or absorb charge fast enough to maintain constant voltage across the capacitor. In this case:

```
V(t) = V_bias = constant
Q(t) = C(t) * V_bias
```

The signal current (not voltage) is:

```
i(t) = dQ/dt = V_bias * dC/dt
```

Since `C = epsilon_0 * A / d(t)` and `d(t) = d_0 + x(t)`:

```
dC/dt = -epsilon_0 * A / d(t)^2 * dx/dt
i(t) = -V_bias * epsilon_0 * A / d(t)^2 * dx/dt
```

In this regime, the output is proportional to **velocity** (dx/dt), not displacement. The signal is much weaker at low frequencies and naturally rolls off the bass.

### 3.4 Transition Frequency (f_c)

The transition between constant-voltage (f << f_c) and constant-charge (f >> f_c) regimes occurs at:

```
f_c = 1 / (2 * pi * R_total * C_total)
```

where:
- `R_total` = effective resistance the pickup capacitance relaxes through
- `C_total` = total system capacitance at the plate (240 pF nominal, LOW confidence — §3.6)

Corrected network (2026-09, cross-verified — see preamp-circuit.md §2.3):
the plate relaxes through **R-1 (22K) into the junction where R-2 (1M, to
the AC-grounded polarizing rail) and C-1 hang; C-1 is a short at audio,
so C-2 (220 pF) at the base sits effectively in parallel with the plate
capacitance, and R-3 (470K, to AC ground) loads the base side.** The
network is second-order with one in-band pole. See §3.7.

### 3.5 Per-Reed Capacitance Estimate

For a single bass reed (the largest), estimated from geometry:
- Bottom face: ~4 mm x 7 mm = 28 mm^2 = 2.8 x 10^-5 m^2
- Gap (side clearance as proxy): ~0.29 mm = 2.9 x 10^-4 m
- Bottom face capacitance: epsilon_0 * A / d = 8.854e-12 * 2.8e-5 / 2.9e-4 = **0.85 pF**

With U-channel (three faces, ~2.5x correction for sides + fringe fields):
- Per-reed capacitance (bass): **~2-4 pF**

For a treble reed (smaller area, narrower gap):
- Bottom face: ~2.5 mm x 4 mm = 10 mm^2
- Gap: ~0.22 mm
- Bottom face capacitance: ~0.4 pF
- With U-channel correction: **~1-3 pF**

### 3.6 Total System Capacitance

A single GroupDIY forum post reports **~240 pF** ("reed pickup capacitance was measured at 240pF"). **Confidence: LOW.** The 2026-07 deep-research pass adversarially **refuted** this figure as unverifiable single-source forum hearsay — no primary source (patent, service manual, or academic paper) gives any pickup-capacitance value for the 200A. We retain 240 pF as the model's `C_total` because (a) it is the only number available and (b) it is geometrically plausible (see below), but it is an **assumption, not a measurement**, and the pickup RC corner (f_c ≈ 880 Hz post-revision) inherits that low confidence. An LCR-meter reading of a real reed bar would settle it. See [Reed–Pickup Displacement §5](reed-pickup-displacement.md).

240 pF cannot be per-reed (64 reeds × 240 pF = 15.4 nF, impossibly large); if real it is the **total system capacitance** at the preamp input node, comprising:
- 64 reed-to-plate capacitors in parallel: 64 x 2-4 pF = ~130-250 pF
- Wiring capacitance (reed bar to preamp board): ~10-50 pF
- Stray/parasitic capacitance: ~10-30 pF

The geometric estimate (130-250 pF for 64 reeds) is consistent with the ~240 pF figure — which is why we retain it despite the sourcing being weak.

### 3.7 RC Time Constant and Cutoff Frequency (REVISED 2026-09)

Pole extraction of the full corrected network gives **one in-band pole**
(the others sit at ~4.9 Hz and ~53 kHz). The shipping model fits it as a
one-pole HPF at:

```
f_c = 880.5 Hz   (fits the cross-verified network response to 0.23 dB
                  over 55 Hz-10 kHz; independent SPICE measures the
                  -3 dB point at ~897 Hz)
tau = 1/(2*pi*f_c) ≈ 181 µs
```

Scaling with the (LOW-confidence) plate capacitance:
`f = f_ref·(C_ref + C-2)/(C_total + C-2)` — verified ±4.3% over
120-500 pF (C_total 150 pF → 1021 Hz; 400 pF → 652 Hz). The C-2-return
fork (ground, assumed, vs TR-1 emitter) would move the corner to
≈1.4 kHz; one hardware continuity check settles it.

**Summary:** f_c ≈ **880 Hz** — the superseded 2312 Hz figure was built
on the misread 287K network and over-filtered the bass by ~8-11 dB below
400 Hz (part of the 2026-07 downstream bass voicing was compensating for
it). The physical reed drive is a current ∝ jω; state the drive when
quoting any response from this network (preamp-circuit.md §9, pitfall 7).

**This means:**

| Frequency | Regime (f_c ≈ 880 Hz) | Signal Type |
|-----------|--------|-------------|
| < ~230 Hz | Strongly constant-voltage | Proportional to velocity, heavily attenuated |
| ~230-2300 Hz | Transition zone | Mixed displacement/velocity response |
| > ~2300 Hz | Strongly constant-charge | Proportional to displacement (linear) |

**Key implication for bass notes:**
- A1 (55 Hz): well below f_c ≈ 880 Hz — ~55/880 = 6.3% of the constant-charge voltage (−24 dB).
- C4 (262 Hz): still below f_c — ~262/880 = 30% (−10.5 dB). (Pre-revision, the wrong 2312 Hz corner put these at −32/−19 dB — the origin of the over-thin bass.)
- C5 (523 Hz): Below f_c. ~22% (-13 dB).
- C6 (1047 Hz): Below f_c. ~41% (-8 dB).
- C7 (2093 Hz): Near f_c. ~67% (-3 dB).

This natural high-pass filtering is a **fundamental characteristic** of the Wurlitzer's sound — bass notes are inherently attenuated, which is partially compensated by the speaker's bass boost near its resonance.

### 3.8 Register-Dependent f_c Variation

The capacitance per reed varies with register (different plate areas and gaps), so the total system capacitance changes slightly depending on which notes are depressed. However, since all 64 reeds contribute to the total capacitance simultaneously (and most reeds are at rest with their static capacitance), the system f_c is relatively stable.

More importantly, the per-reed contribution to the total current is register-dependent:
- Bass reeds: larger area, wider gap → C_reed is moderate (~3-4 pF)
- Treble reeds: smaller area, narrower gap → C_reed is slightly smaller (~1-3 pF)

But the signal voltage at the summing node depends on the ratio of the vibrating reed's capacitance change to the total system capacitance (including all other static reeds):

```
delta_V = delta_C_reed / C_total * V_bias
```

This means each reed's signal is attenuated by the factor `C_reed / C_total`. For a single reed with C_reed ~ 3 pF and C_total ~ 240 pF, this parasitic loading factor is:

```
C_reed / C_total = 3/240 = 0.0125 = 1.25%
```

The remaining 237 pF of static capacitance from the other 63 reeds (and wiring) acts as a **parasitic capacitance** that attenuates the signal and reduces nonlinear distortion (see Section 4).

---

## 4. Nonlinearity Analysis

### 4.1 Pickup Nonlinearity — Taylor Expansion

In the constant-charge regime, the exact signal voltage is:

```
V(t) = V_bias * d(t) / d_0 = V_bias * (d_0 + x(t)) / d_0
V_ac(t) = V_bias * x(t) / d_0
```

This is **exactly linear** in displacement x(t). There is NO nonlinearity in the constant-charge regime when expressed in terms of gap distance.

However, if we express the signal in terms of the **inverse** relationship (capacitance), nonlinearity appears. From arXiv 2407.17250 (Honzik & Novak), the output voltage of a condenser microphone including parasitic capacitance is:

```
u(t) = K_0 * [y(t) - y(t)^2 + y(t)^3 - ...]
```

where:
- `y(t) = x(t) / d_0` (normalized displacement, positive = reed moving toward plate)
- `K_0 = V_bias * C_0 / (C_p + C_0)` (effective sensitivity including parasitic cap)
- `C_p` = parasitic capacitance (other reeds + wiring)

**Wait -- this contradicts the "exactly linear" claim above.** The resolution: the Taylor expansion comes from `V = Q/(C_0 - delta_C)` where `delta_C = C_0 * x/d_0`, which gives `V = V_bias / (1 - x/d_0)`. This is NOT the same as `V = V_bias * (1 + x/d_0)`.

Let me derive this carefully:

### 4.2 Correct Derivation of Pickup Voltage

**Sign convention:** Let x > 0 mean the reed moves TOWARD the plate (gap decreases).

```
d(t) = d_0 - x(t)        (gap decreases when reed moves toward plate)
C(t) = C_0 * d_0 / d(t) = C_0 * d_0 / (d_0 - x(t)) = C_0 / (1 - x/d_0)
```

In the constant-charge regime:

```
V(t) = Q_0 / C(t) = V_bias * C_0 / C(t) = V_bias * (1 - x(t)/d_0)
V_ac(t) = -V_bias * x(t) / d_0
```

This IS linear (just inverted sign). The signal is proportional to displacement.

**But wait** — the arXiv paper uses a DIFFERENT convention. Let me reconcile:

If x > 0 means reed moves AWAY from plate (gap increases):

```
d(t) = d_0 + x(t)
C(t) = C_0 * d_0 / (d_0 + x(t)) = C_0 / (1 + x/d_0)
V(t) = Q_0 / C(t) = V_bias * (1 + x/d_0)
V_ac(t) = V_bias * x(t) / d_0
```

Also linear. So where does the nonlinearity come from?

### 4.3 Source of Pickup Nonlinearity

The nonlinearity arises when we consider that **the capacitor connected in parallel with other capacitances** forms a voltage divider. The signal at the preamp input is NOT `V_bias * x/d_0` but rather:

```
V_signal = V_bias * (C_0 / (C_0 + C_p)) * [y - y^2 + y^3 - ...]
```

where `y = x/d_0` and `C_p` is the parasitic (stray + other reeds) capacitance.

The nonlinear terms come from the fact that when the vibrating capacitance changes, the voltage division ratio also changes:

```
V_out = V_bias * C_vibrating / (C_vibrating + C_parasitic)
     = V_bias * C_0/(1-y) / (C_0/(1-y) + C_p)
     = V_bias * C_0 / (C_0 + C_p*(1-y))
```

Expanding as a Taylor series in y (for y << 1):

```
V_out ≈ V_bias * C_0/(C_0+C_p) * [1 + C_p/(C_0+C_p)*y + (C_p/(C_0+C_p))^2 * y^2 + ...]
```

The fundamental term gives the linear sensitivity. The y^2 term generates second harmonic (H2). The y^3 term generates third harmonic (H3).

### 4.4 Second Harmonic from Pickup

For harmonic excitation `y(t) = y_m * sin(wt)`:

```
H2/H1 = y_m * C_p / (2 * (C_0 + C_p))
```

With the Wurlitzer's values:
- `y_m` at mf: reed displacement / gap ~ 0.05-0.15 (estimated)
- `C_0` (per-reed): ~3 pF (estimated)
- `C_p` (system - this reed): ~237 pF (estimated)

```
H2/H1 = 0.10 * 237 / (2 * 240) = 0.049 = -26 dB
```

**Note:** This -26 dB estimate uses the small-signal arXiv formula with parasitic capacitance dilution. SPICE simulation of the full 1/(1-y) model at y=0.10 yields H2/H1 ~ -21 dB (THD ~ 8.7%), which is significantly higher because the full nonlinearity includes terms beyond the first-order Taylor expansion. At millivolt input levels, the preamp itself produces THD < 0.01%, making the pickup the dominant H2 source at normal dynamics. The preamp's asymmetric headroom (5.3:1) contributes additional H2 only at extreme ff where it enters saturation.

### 4.5 When Does the Linear Approximation Break Down?

The linear approximation `V_ac = V_bias * x/d_0` is valid when:
1. `x/d_0 << 1` (small displacement relative to gap)
2. The pickup operates in constant-charge regime (f >> f_c)
3. Parasitic capacitance is accounted for in the sensitivity factor

For the Wurlitzer:
- At **pp**: x/d_0 ~ 0.02-0.05 -- linear to within 0.1%.
- At **mf**: x/d_0 ~ 0.05-0.15 -- linear to within 1-2%.
- At **ff**: x/d_0 ~ 0.15-0.40 -- nonlinear terms become significant (H2 ~ -14 to -20 dB from pickup alone).
- At **extreme ff**: reed approaches plate (x -> d_0) -- severe nonlinearity, physical clamp. Service manual warns against reed-plate contact.

The **minGap clamp** (reed cannot physically contact the plate) is the most severe nonlinearity at extreme dynamics. When the reed gets very close to the plate, the 1/(d_0 - x) behavior produces extreme voltage spikes that are then clamped by the preamp's input protection (D1 diode).

---

## 5. Frequency Response Analysis

### 5.1 Pickup RC High-Pass Filter

The pickup behaves as a first-order high-pass filter:

```
H(f) = j*f/f_c / (1 + j*f/f_c)
|H(f)| = f / sqrt(f^2 + f_c^2)
```

where `f_c ≈ 880 Hz` (see Section 3.7).

This means the pickup's transfer function from displacement to voltage is:

```
V_signal(f) = V_bias/d_0 * |H(f)| * X(f)
```

where X(f) is the reed displacement spectrum.

### 5.2 Frequency Response by Register

Using f_c = 880 Hz (values below predate the revision — rescale by the corner ratio when precision matters):

| Note | MIDI | Freq (Hz) | |H(f)| | Attenuation (dB) | Regime |
|------|------|-----------|-------|------------------|--------|
| A1 | 33 | 55 | 0.024 | -32.5 | Constant-voltage |
| C2 | 36 | 65 | 0.028 | -31.0 | Constant-voltage |
| C3 | 48 | 131 | 0.057 | -24.9 | Constant-voltage |
| C4 | 60 | 262 | 0.113 | -19.0 | Transition |
| C5 | 72 | 523 | 0.221 | -13.1 | Transition |
| C6 | 84 | 1047 | 0.413 | -7.7 | Transition |
| C7 | 96 | 2093 | 0.671 | -3.5 | Near constant-charge |

**Critical insight:** The pickup's natural HPF is the **primary mechanism for register balancing** in the Wurlitzer. Bass notes are attenuated 25-34 dB more than treble notes. This is not a design flaw — it is how the instrument achieves tonal balance despite bass reeds deflecting much more than treble reeds.

### 5.3 C20 HPF at TR-1 Base

> **Model note (corrected 2026-09):** the base-node 220 pF is **C-2, a real 200A part** (the "C20 = 206A-only" claim was a designator conflation; C20 is a different aux-section part). C-2 does not make a separate 1903 Hz filter — through C-1 (a short at audio) it parallels the plate capacitance and is folded into the single ≈880 Hz corner of Section 3.7, which is what the code implements.

> **SUPERSEDED (2026-09).** Earlier revisions of this section derived a
> separate "C20 HPF at 1903 Hz" from the misread bias network (2M‖470K =
> 380K, which was circular arithmetic, not a measurement) and debated
> whether it cascaded with the pickup RC. The corrected picture: the
> base-node 220 pF is **C-2** (a real 200A part), C-1 is a short at
> audio, so C-2 sits in parallel with the plate capacitance and there is
> **one combined ≈880 Hz corner**, not two cascaded HPFs. Full analysis:
> §3.7 and preamp-circuit.md §2.3/§11.

The 240 pF measured at GroupDIY is the reed bar capacitance alone (reed-to-plate + wiring + strays). C20 (220 pF) is a separate discrete component at TR-1 base. They are at different nodes (pickup plate vs. TR-1 base), and 240 - 220 = only 20 pF for 64 reeds + wiring would be implausibly small. The two HPFs are independent.

### 5.5 Upper Frequency Response

The pickup has no inherent upper frequency limit in the audio band. The capacitive reactance decreases with frequency, making the pickup more efficient at higher frequencies. The upper frequency response is limited by:

1. **Preamp closed-loop bandwidth** (~3.7 kHz, from Miller-effect dominant pole and R-10 feedback)
2. **Speaker rolloff** (~8 kHz)
3. **Reed vibration modes** (higher modes decay faster, limiting HF content)

---

## 6. Comparison: Wurlitzer (Electrostatic) vs. Rhodes (Electromagnetic)

| Property | Wurlitzer (Electrostatic) | Rhodes (Electromagnetic) |
|----------|--------------------------|-------------------------|
| Sensing quantity | Displacement (gap) | Velocity (flux change) |
| Bias requirement | 147V DC polarizing voltage | Permanent magnet |
| Signal source | dQ/dt from varying capacitance | dPhi/dt from varying reluctance |
| Natural frequency response | High-pass (constant-charge regime) | Band-pass (resonant pickup coil) |
| Harmonic generation | Minimal (linear in constant-charge) | Significant (1/(d+x)^2 from magnetic field) |
| Signal level | Very low (millivolts) | Low-moderate (tens of millivolts) |
| Impedance | Very high (~380 kOhm resistive at TR-1 base) | High (inductive, ~5-10 kOhm at resonance) |
| Noise susceptibility | EMI sensitive (high-Z capacitive) | EMI sensitive (inductive) |
| Distortion character | Even harmonics from pickup 1/(1-y) + preamp at ff | Even + odd from magnetic nonlinearity |

**Key sonic consequence:** The Wurlitzer's "bark" comes primarily from the **pickup's 1/(1-y) nonlinearity** at normal dynamics, with the preamp contributing additional H2 at extreme ff. The Rhodes' growl comes from **both** the pickup (magnetic nonlinearity) and the preamp. Both instruments derive their character from the combined pickup + preamp chain, but the dominant nonlinearity source differs.

---

## 7. Miessner Patent Analysis

### 7.1 US Patent 3,038,363 (Filed 1950, Issued 1962)

Benjamin Franklin Miessner's patent describes the fundamental design used in Wurlitzer electronic pianos.

Key innovations documented in the patent:

1. **Asymmetric capacity modulation:** "vibrations of the reed produce asymmetrical modulations of the capacity between the reed and pick-up" -- the reed-plate geometry is intentionally designed so that the capacitance change is not symmetric with reed displacement.

2. **Multiple pickup positions:** Each reed can have "from one to three separate electrodes" at different positions (center, tip, edge), each producing a different tonal character.

3. **Adjustability:** "tone quality, tone volume and tone damping are obtained by axial and lateral adjustments of a vibratory reed relative to a suitable pick-up."

4. **Grounded shield:** Adding a grounded shield "increase[s] the abruptness of the capacity changes between the reed and pick-up" -- it focuses the electric field.

5. **Signal characteristics:** The pickup produces "strongly-peaked electrical vibrations" containing "both odd and even numbered components."

### 7.2 Implication of Asymmetric Modulation

Miessner explicitly designed for **asymmetric** capacitance change. This means the U-channel geometry is NOT a simple parallel plate — as the reed moves toward one wall and away from the other, the capacitance change is inherently asymmetric. This produces even harmonics from the pickup geometry itself, contradicting the "pickup is linear" claim.

However, in the production Wurlitzer 200/200A, the reed vibrates **vertically** (toward/away from the bottom of the U-channel), not laterally. The side walls contribute a symmetric capacitance that partially cancels any asymmetry. The degree of pickup-generated harmonics depends on the exact geometry and reed alignment.

The patent describes the general principle; the specific 200A implementation may have less asymmetry than Miessner's original designs.

---

## 8. Modeling Decisions and Recommendations

### 8.1 Current Model (OpenWurli) — post-restructure (2026-09)

`pickup.rs` implements the corrected physics: **the 1/(1−y) nonlinearity
lives in the SOURCE (the moving reed's charge injection), and the network
is nearly LTI** (one ~3 pF reed against the fixed ~240 pF node — the node
capacitance swings only 240→274 pF even at y = 0.92). The node equation
`C_node(t)·dv/dt + v/R_net = −V_pol·dC_reed/dt` is discretized exactly
(trapezoidal); the differentiator-inside-a-lag structure IS the jω-driven
high-pass the circuit reference specifies.

```rust
// pickup.rs core (post-restructure)
let y = pickup_soft_saturate(sample * scale);   // physical gap fraction
let s = y / (1.0 - y);        // SOURCE: reed charge — THE nonlinearity
let m = 1.0 + C_REED_RATIO * s;   // NETWORK: ~LTI (ratio ≈ 1/80)
// exact trapezoidal step of C_node·dv/dt + v/R_net = −V_pol·dC_reed/dt
w = (w * (m_avg - beta) - (s - s_prev)) / (m_avg + beta);
out = w * PICKUP_SENSITIVITY;
```

**Key parameters:**

| Parameter | Value | Derivation |
|-----------|-------|------------|
| `C_TOTAL` | 240 pF | nominal, LOW confidence (§3.6); corner scales with it |
| `C2_BASE` | 220 pF | C-2, bridged to the plate at audio; `C2_RETURNS_TO_GROUND = true` (the one open fork — flipping it moves the corner to ≈1.4 kHz) |
| `PICKUP_FC` | 880.5 Hz | one-pole fit to the cross-verified network response (0.23 dB max error 55 Hz–10 kHz) |
| `C_REED_RATIO` | ≈ 1/80 | one reed's capacitance vs the node — the physical modulation depth |
| `DISPLACEMENT_SCALE` | per-note, `tables.rs` | now honestly a **gap-fraction calibration** (it previously absorbed the 80× depth error) |
| soft saturation | knee → asymptotic `PICKUP_MAX_Y` | replaced the hard clamp (derivative discontinuity was audible HF distortion) |

**What the restructure fixed** (full story: preamp-circuit.md §11 and the
pickup.rs module docs): the previous model modulated the ENTIRE node as
`1/(1−y)` — 80× the physical depth — which made its HF asymptote linear
(`SENSITIVITY·y`, no harmonics above the corner, per-order harmonic
falloff) and its 12.5× time-constant swing an intermodulation generator
(inharmonic hash, since measured −13 dB improved). Its asymmetry was also
inverted relative to its own documentation.

### 8.2 Assessment

**What it gets right:** physically-placed 1/(1−y) (H2-dominant, correct
asymmetry direction, frequency-independent above the corner); one-pole
HPF at 880 Hz matching the verified network to 0.23 dB; register-dependent
gap-fraction scaling; shared-plate mono summation; hash-free harmonic
generation (harmonic-to-hash ≈ 20 dB at C2 ff).

**Open refinements:** the `ds_correction` MLP head's out-of-distribution
velocity behavior (opt-in path); Miessner/U-channel asymmetry beyond
1/(1−y) (deferred); real measured `C_TOTAL` and the C-2 return fork
(hardware pending).

### 8.3 Implementation Status

Implemented: the source-honest restructure above, the 880 Hz corner with
the C_TOTAL scaling law, the C-2 fork as a one-constant switch, soft
saturation. The pre-revision "future refinement: full time-varying RC"
section is obsolete — the shipped model IS the full network model now,
with the time variation placed where physics puts it.

### 8.4 Signal Level Estimation

The AC signal voltage at the preamp input can be estimated:

```
V_ac_peak = V_bias * x_peak / d0 * C_reed / (C_reed + C_parasitic) * |H(f)|
```

For C4 at mf (values below predate the corner revision; rescale by 880/2312 where precision matters):
- V_bias = 147V
- x_peak / d0 ~ 0.10 (estimated)
- C_reed / C_total = 3/240 = 0.0125
- |H_pickup(262 Hz)| = 262/sqrt(262^2 + 880^2) = 0.285  (corrected corner)

```
V_ac_peak = 147 * 0.10 * 0.0125 * 0.113 = 0.021 V = 21 mV
```

> **Note:** C20 (220 pF) is a 206A component, NOT present on the 200A. Without C20, the estimated preamp input at C4 mf is ~21 mV. Brad Avenson measured **2-7 mV AC at the volume pot output** (AFTER ~6 dB of preamp gain in the stock 200A — SPICE-measured closed-loop gain is 2.0x/6.0 dB at 1 kHz; Avenson's "15 dB" figure was from his replacement preamp design, not the stock 200A). The discrepancy between 21 mV preamp input and 2-7 mV post-gain output suggests the pickup signal estimate above is an upper bound, possibly due to smaller actual reed displacement or higher system capacitance than assumed.

---

## 9. Summary of Key Facts

### Verified Facts
| Item | Value |
|------|-------|
| Pickup type | Electrostatic (capacitive) |
| Polarizing voltage | 147V DC (half-wave rectified) |
| R-2 (147V feed to the plate-side network) | 1 MΩ (glyph-verified; the doc's former "R_feed" and phantom "2M R-2" were one part, misread) |
| R-2 (polarizing feed) | 1 MΩ (glyph-verified; the old "2M to +15V at the base" reading and its circular 380K "confirmation" are superseded — preamp-circuit.md §11) |
| R-3 (TR-1 base bias to GND) | 470 kΩ |
| base AC impedance | ≈459K peak @200 Hz, 339K @1 kHz (R-3 ∥ C-2 rolloff; tremolo-independent) |
| .022 uF coupling cap | AC couples pickup plate to TR-1 base |
| C20 (shunt cap at TR-1 base) | 220 pF (GroupDIY's 270 pF likely tolerance variation) |
| Total system capacitance | ~240 pF (measured at pickup plate) |
| Pickup construction | Single comb plate, U-channel slots |
| Reed slot widths | 0.114" (treble) to 0.172" (bass) |
| Signal summing | All reeds sum at pickup plate (mono) |
| 200A preamp transistors | 2N5089 (originally 2N2924) |

### Inferred Values
| Item | Value | Derivation |
|------|-------|-----------|
| Per-reed capacitance | ~2-4 pF | Geometric calculation |
| Pickup RC f_c | ≈ 880 Hz | one-pole fit to the corrected network (§3.7); scales with C_TOTAL |
| C-2 (220 pF) | folded into the 880 Hz corner | no separate filter — C-1 is a short at audio (§3.7) |
| Preamp input signal (C4 mf) | ~1-5 mV peak | Electrostatic calculation |
| Pickup H2 contribution (mf) | ~-26 dB | arXiv formula |

### Estimated Values
| Item | Value | Basis |
|------|-------|-------|
| Reed displacement / gap (mf) | ~5-15% | Typical for electrostatic instruments |
| Reed displacement / gap (ff) | ~15-40% | Typical for electrostatic instruments |
| Pickup plate active length | ~3-8 mm | Photos, proportional reasoning |
| U-channel depth | ~2-4 mm | Photos |

### Open Questions
| Item | Status |
|------|--------|
| Vertical gap (reed face to slot bottom) | **DEFERRED** to OBM recording comparison phase. Not documented; affects absolute H2/H1 ratio. Will calibrate against OldBassMan recordings. |
| Miessner's asymmetric modulation in 200A | **DEFERRED** to OBM recording comparison phase. Unknown if preserved in production design. Will calibrate against OldBassMan recordings. |
| Exact signal level at pickup output | Sub-mV to low mV estimated; no direct measurement found |

**Pickup nonlinearity DECISION: IMPLEMENTED.** The full 1/(1-y) nonlinearity is implemented in `pickup.rs` as a bilinear-discretized time-varying RC circuit, with soft saturation toward PICKUP_MAX_Y, f_c = PICKUP_FC ≈ 880 Hz, and register-dependent displacement scaling from beam compliance (`tables.rs:pickup_displacement_scale()`). The two deferred items above affect calibration constants only, not the model topology.

---

## 10. References

### Primary Sources

1. **Wurlitzer 200/200A Service Manual** — polarizing voltage (147V), component values, adjustment procedures
   - Available: [Internet Archive](https://archive.org/details/wurlitzer-200-and-200-a-service-manual)
   - Available: [Vintage Vibe](https://www.vintagevibe.com/pages/service-manuals)

2. **Wurlitzer 200A Schematic** — circuit topology, component references
   - [Busted Gear](https://www.bustedgear.com/images/schematics/Wurlitzer_200A_series_schematics.pdf)

3. **GroupDIY "Wurlitzer 200A preamp" thread** — 240 pF measurement, R1/R3 values, circuit analysis
   - [GroupDIY Thread 44606](https://groupdiy.com/threads/wurlitzer-200a-preamp.44606/)

4. **GroupDIY "One more Wurlitzer 200 question" thread** — Avenson preamp, 15 dB gain, 499k feed resistor (note: 499k is Avenson's replacement design value; original 200A uses 1M)
   - [GroupDIY Thread 13555](https://groupdiy.com/threads/one-more-wurlitzer-200-question.13555/)

5. **EP-Forum "Wurlitzer 200 Reed Dimensions" thread** — slot widths, reed widths, side clearances
   - [EP-Forum Topic 8418](https://ep-forum.com/smf/index.php?topic=8418.0)

### Academic / Technical Sources

6. **Pfeifle, F. (2017)** — "Real-Time Physical Model of a Wurlitzer and Rhodes Electric Piano." DAFx-17.
   - [DAFx Paper Archive](https://www.dafx.de/paper-archive/2017/papers/DAFx17_paper_79.pdf)

7. **Honzik, P. & Novak, A. (2024)** — "Reduction of Nonlinear Distortion in Condenser Microphones Using a Simple Post-Processing Technique." arXiv:2407.17250.
   - [arXiv](https://arxiv.org/abs/2407.17250)
   - Used for: pickup nonlinearity Taylor expansion, H2 formula

8. **US Patent 3,038,363** — Miessner, B.F. "Electronic Piano." Filed 1950, issued 1962.
   - [Google Patents](https://patents.google.com/patent/US3038363)
   - U-channel geometry, asymmetric modulation, pickup electrode design

9. **US Patent 2,919,616** — Andersen, C.W. "Clamping and Control Apparatus for Reed Generators." Issued 1960.
   - [Google Patents](https://patents.google.com/patent/US2919616)
   - **Gold standard for manufacturing tolerances**: slot clearance 0.0025"/side, reed material Rockwell C-50, spacing tools, pickup nodal position 0.22L from free end, hammer strike at 0.25-0.35L

10. **US Patent 2,942,512** — Miessner, B.F. "Electronic Piano." Issued 1960.
    - [Google Patents](https://patents.google.com/patent/US2942512)
    - Pickup at 2nd partial node (0.78L), capstan rod adjustability, reed base grouping

11. **US Patent 2,966,821** — Miessner, B.F. "Electronic Piano." Issued 1961.
    - [Google Patents](https://patents.google.com/patent/US2966821)
    - Pickup face geometry, angular deviation specs, frequency ratios

12. **US Patent 2,932,231** — Miessner, B.F. "Tone Generating Apparatus." Issued 1960.
    - [Google Patents](https://patents.google.com/patent/US2932231)
    - Hammer contact: neoprene foam 1/8-1/4" thick, contact length 10-30% of reed, duration 3/4-1 cycle

13. **US Patent 3,215,765** — Miessner, B.F. "Fixed Free-Reed Electronic Piano with Improved Interpartial-Ratio Integralizing Arrangements." Issued 1965.
    - [Google Patents](https://patents.google.com/patent/US3215765)
    - Interpartial ratios (6.001-6.029), tuner-damper system (NOT used in 200A production), reed taper specs

9. **Physics LibreTexts** — "Changing the Distance Between the Plates of a Capacitor"
   - [LibreTexts 5.15](https://phys.libretexts.org/Bookshelves/Electricity_and_Magnetism/Electricity_and_Magnetism_(Tatum)/05:_Capacitors/5.15:__Changing_the_Distance_Between_the_Plates_of_a_Capacitor)

### Community / Informational Sources

10. **Tropical Fish Vintage** — "How Does a Wurlitzer Electronic Piano Work?"
    - [Tropical Fish](https://www.tropicalfishvintage.com/blog/2019/5/27/how-does-a-wurlitzer-electronic-piano-work)

11. **Vintage Vibe** — Parts diagrams, reed dimensions, service documentation
    - [Vintage Vibe](https://www.vintagevibe.com/blogs/news/wurlitzer-200-parts-diagrams)

12. **Wikipedia** — "Electrostatic pickup"
    - [Wikipedia](https://en.wikipedia.org/wiki/Electrostatic_pickup)

13. **Instructables** — "Electric Wurlitzharmonica" (DIY electrostatic pickup project)
    - [Instructables](https://www.instructables.com/Electric-Wurlitzharmonica/)

---

## Appendix A: Derivation of Constant-Charge Signal Voltage

### Setup

A parallel-plate capacitor with:
- Static gap: d_0
- Static capacitance: C_0 = epsilon_0 * A / d_0
- Polarizing voltage: V_bias
- Static charge: Q_0 = C_0 * V_bias
- Bias resistor: R (through which charge can flow to/from the voltage source)
- Reed displacement: x(t), positive = away from plate (gap increases)

### Instantaneous Values

```
d(t) = d_0 + x(t)
C(t) = epsilon_0 * A / d(t) = C_0 * d_0 / (d_0 + x(t))
```

### RC Circuit ODE

The charge on the capacitor evolves according to:

```
dQ/dt = (V_bias - Q/C(t)) / R
```

The driving term `(V_bias - Q/C)` is the voltage across the resistor. When `Q/C = V_bias`, no current flows (equilibrium).

### Constant-Charge Approximation (f >> f_c)

When the signal frequency is much higher than `f_c = 1/(2*pi*R*C_0)`, the charge cannot change appreciably during one cycle. We approximate `Q ≈ Q_0 = C_0 * V_bias`.

```
V(t) = Q_0 / C(t) = V_bias * C_0 / C(t) = V_bias * (d_0 + x(t)) / d_0
V_ac(t) = V_bias * x(t) / d_0
```

**Linear in x(t).** Sensitivity: `S = V_bias / d_0`.

### Constant-Voltage Approximation (f << f_c)

When f << f_c, charge adjusts freely to maintain `V = V_bias`:

```
Q(t) = C(t) * V_bias
i(t) = dQ/dt = V_bias * dC/dt
```

Signal current, not voltage. Must be converted to voltage by the load impedance.

### Exact Solution (Numerical)

For intermediate frequencies, the ODE must be solved numerically. The bilinear (trapezoidal) discretization is:

```
Let c_n = C(t_n) / C_0 = d_0 / (d_0 + x(t_n))     (normalized capacitance)
Let q_n = Q(t_n) / (C_0 * V_bias)                    (normalized charge)

The continuous ODE in normalized form:
dq/dt = (1 - q/c) / tau

Bilinear integration:
alpha_n = beta * (1 - y_n)        // algebraically identical to dt/(2*tau*c_n); avoids division
beta_n = dt / (2 * tau)

q_{n+1} = (q_n * (1 - alpha_{n+1}) + 2 * beta) / (1 + alpha_{n+1})
```

**Note on the existing model's bug:** The original wurlitzer-physics.md notes that using `2*alpha` instead of `2*beta` in the driving term forces `q_equilibrium = 1` (constant charge) at all frequencies instead of `q_equilibrium = c` (constant voltage at DC). This is only correct if the system is always in constant-charge regime (f >> f_c). Given f_c ≈ 880 Hz (see Section 3.7), bass fundamentals (55-260 Hz) are still below the corner, and the bug matters.

### Parasitic Capacitance Correction

When other static capacitances `C_p` are in parallel with the vibrating reed capacitance `C_reed`:

```
V_out = V_bias * C_reed(t) / (C_reed(t) + C_p)
```

For small x:

```
V_out ≈ V_bias * C_0 / (C_0 + C_p) * (1 + C_p/(C_0+C_p) * x/d_0 + ...)
V_ac ≈ V_bias * C_0 * C_p / (C_0 + C_p)^2 * x / d_0
```

Wait, let me redo this more carefully:

```
C_reed(t) = C_0 / (1 + y),  where y = x/d_0

V_out = V_bias * (C_0/(1+y)) / (C_0/(1+y) + C_p)
     = V_bias * C_0 / (C_0 + C_p * (1+y))
     = V_bias * C_0 / (C_0 + C_p) * 1 / (1 + C_p*y/(C_0+C_p))
     ≈ V_bias * C_0 / (C_0+C_p) * [1 - C_p*y/(C_0+C_p) + (C_p*y/(C_0+C_p))^2 - ...]
```

DC component: `V_dc = V_bias * C_0 / (C_0 + C_p)`

AC fundamental (first-order in y):
```
V_ac ≈ -V_bias * C_0 * C_p / (C_0 + C_p)^2 * y
```

The **negative sign** means the output voltage **decreases** when the gap increases (reed moves away). The sensitivity is reduced by the factor `C_p / (C_0 + C_p)` compared to the isolated capacitor case.

For the Wurlitzer: `C_p / (C_0 + C_p) = 237/240 = 0.988`, so the sensitivity reduction is only 1.2%. The parasitic capacitance has negligible effect on linear sensitivity but does reduce nonlinear distortion.

---

## Appendix B: Equivalent Circuit

```
                  R-2 (1M)
+147V DC ────────────┤
                     │
                     │  PICKUP PLATE NODE
                     │
              ┌──────┴──────┐
              │             │
    C_reed_1  C_reed_2 ... C_reed_64
    (2-4pF)   (2-4pF)      (2-4pF)
              │             │
              │  (all reeds grounded)
              └──────┬──────┘
                   GND (reed bar chassis ground)
                     │
                .022 uF coupling cap (blocks 147V DC)
                     │
                     │  TR-1 BASE NODE
                     │
              ┌──────┼──────┐
              │      │      │
         R-2 (2M)  C20    D1 (protection)
          to +15V  (220    │
              │     pF)   GND
              │      │
              │     GND
              │
         R-3 (470K)
              │
             GND
              │
         TR-1 base ──── TR-1 (Stage 1 preamp)
```

The signal path is:
1. Reed vibration changes `C_reed_n` for the struck reed(s)
2. AC current flows from the pickup plate through the .022 uF coupling cap
3. At TR-1 base node: R-2 (2M) to +15V and R-3 (470K) to GND set the DC bias
4. C-2 (220 pF) at the base is bridged to the plate capacitance through C-1 (a short at audio) — folded into the single ≈880 Hz corner
5. D1 clamps transients from reed-plate shorts (during tuning, reed can short to plate)
6. Signal reaches TR-1 base-emitter junction

**Two distinct circuit nodes:**
- **Pickup plate** (~147V DC): R-2 (1M) via R-1 provides the DC charging path. The pickup RC corner (≈880 Hz) comes from the corrected network of §3.7 (C_TOTAL + C-2 against the R-1/R-2/R-3 network).
- **TR-1 base** (~2.45V DC): biased by R-3's DC-feedback return from TR-2's emitter divider (no divider from the supply exists). C-1 isolates the 147V plate DC from the base.

---

## Appendix C: Answers to Key Questions

### Q1: Is the pickup truly in constant-charge regime at all audio frequencies?

**No.** With f_c ≈ 880 Hz, bass fundamentals (55-260 Hz) sit below the corner (attenuated, though 8-11 dB less than the superseded 2312 Hz model claimed); mids are in the transition; treble above ~1-2 kHz approaches constant-charge behavior. C-2 does not add an independent filter — it is folded into the single corner (§3.7).

### Q2: What is the actual per-reed capacitance?

**~2-4 pF** (estimated from geometry). The 240 pF figure measured at GroupDIY is the total system capacitance (all 64 reeds in parallel + wiring + strays). 64 reeds x 3 pF average = 192 pF, plus ~50 pF wiring/strays = ~240 pF.

### Q3: What is the bias voltage and how is it generated?

**147V DC** from a half-wave rectifier on a dedicated transformer winding, filtered by three 0.33 uF capacitors in an RC chain. Fed to the reed bar pickup plate through R-2 (1 MEG, glyph-verified) and R-1 (22K). Avenson's "499K" refers to their replacement preamp design, not the original 200A value.

### Q4: How does the gap vary by register?

Slot widths vary from 0.172" (bass) to 0.114" (treble), a ratio of **1.51:1**. Side clearances vary from 0.0115" to 0.0085", a ratio of **1.35:1**. The vertical gap is undocumented but likely follows a similar trend.

### Q5: Is the signal truly proportional to displacement?

**Approximately, for small displacements.** V_ac = V_bias * x/d_0 is the first-order approximation. For small displacements (x/d_0 < 0.05), this is linear to better than 1%. At mf (x/d_0 ~ 0.10), the full 1/(1-y) nonlinearity produces H2/H1 ~ -21 dB (SPICE), making the pickup the dominant H2 source at normal dynamics.

### Q6: What is the C20 shunt capacitor value and HPF frequency?

C-2 = 220 pF at TR-1's base (GroupDIY's 270 pF likely tolerance variation). It does not form a separate HPF: through C-1 (a short at audio) it parallels the plate capacitance inside the single ≈880 Hz corner (§3.7). The old "1903 Hz with 380K" figure was built on the misread bias network.

### Q7: Does the pickup introduce harmonic distortion?

**Yes, significantly.** SPICE simulation of the full 1/(1-y) model at mf (y_m ~ 0.10) yields H2/H1 ~ -21 dB (THD ~ 8.7%). The earlier -26 dB estimate used a first-order arXiv formula that underestimates the full nonlinearity. At millivolt input levels, the preamp produces THD < 0.01%, making the pickup the dominant H2 source at normal dynamics. At extreme ff (y_m > 0.3), both the pickup and the preamp's asymmetric headroom contribute significantly.
