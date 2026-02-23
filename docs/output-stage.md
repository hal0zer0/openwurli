# Wurlitzer 200A Output Stage: Power Amplifier, Tremolo, and Speaker/Cabinet

> **See also:** [Preamp Circuit](preamp-circuit.md) (tremolo feedback integration, Section 7), [Signal Chain Architecture](signal-chain-architecture.md) (overall signal flow)

---

## Table of Contents

1. [Signal Flow Overview](#1-signal-flow-overview)
2. [Tremolo (Vibrato) Circuit](#2-tremolo-vibrato-circuit)
3. [Volume Control](#3-volume-control)
4. [Power Amplifier](#4-power-amplifier)
5. [Speaker and Cabinet](#5-speaker-and-cabinet)
6. [Auxiliary and Headphone Outputs](#6-auxiliary-and-headphone-outputs)
7. [Modeling Recommendations](#7-modeling-recommendations)
8. [Sources](#8-sources)

---

## 1. Signal Flow Overview

### Signal Path

```
Reed Pickup
  -> Preamp (TR-1, TR-2 on reed bar PCB)
     [LDR tremolo is in the preamp FEEDBACK LOOP, not post-preamp]
  -> Volume Pot (3K audio taper)
  -> C-8 coupling cap
  -> Power Amplifier (TR-7 through TR-13, on main amp board)
     -> Differential input (TR-7/TR-8)
     -> VAS/pre-driver (TR-11)
     -> Bias control (TR-9, Vbe multiplier)
     -> Complementary drivers (TR-10 NPN, TR-12 PNP)
     -> Quasi-complementary output (TR-13 NPN / TIP35C, TR-11 PNP / TIP36C)
  -> Speaker (two 16-ohm 4"x8" oval drivers in parallel = 8 ohm load)
  -> Headphone jack (switching, parallel with speaker, 8-ohm load resistor)
```

### Tremolo Position and Feedback Topology

The Wurlitzer 200A service manual explicitly states:

> "The reed bar signal is modulated by inserting the vibrato voltage into the feedback loop of the high impedance preamp. A divider is formed by the feedback resistor R-10, and the light dependent resistor of LG-1. The L.D.R., in conjunction with the light emitting diode in the same package, creates a variable leg in the feedback divider and makes possible amplitude modulation of the reed bar voltage."

R-10 (56K) feeds back from the preamp output to a feedback junction (fb_junct). Ce1 (4.7 MFD coupling cap) AC-couples fb_junct to TR-1's **emitter**. This is **series-series (emitter) NEGATIVE feedback**. Re1 (33K) provides the separate DC path from emitter to ground. The LDR (LG-1) shunts fb_junct to ground via cable Pin 1 → 50K VIBRATO pot → 18K → LG-1 LED. When the LDR resistance changes, it diverts feedback current away from the emitter, modulating the preamp's closed-loop gain.

**Implications for modeling:**
- Tremolo modulates preamp GAIN via emitter feedback, which means the distortion character changes with the tremolo cycle
- At the high-gain phase of the tremolo (LDR resistance low / LED on, feedback junction shunted to ground, feedback can't reach emitter), the preamp runs at higher gain and distorts more
- At the low-gain phase (LDR resistance high / LED off, full feedback reaches emitter via Ce1), the preamp has lower gain and operates more linearly
- This is more complex than simple amplitude modulation and produces subtle timbral variation during the tremolo cycle

---

## 2. Tremolo (Vibrato) Circuit

### 2.1 Oscillator

The oscillator is a **twin-T (parallel-T) oscillator**. The twin-T network forms a notch filter in the negative feedback path of TR-3. At the notch frequency, feedback is minimized and loop gain peaks, satisfying the Barkhausen criterion.

**Topology:**
- TR-3 and TR-4 share a **common collector node** (Node G)
- R17 (4.7K) from Vcc to Node G: sole collector load for both transistors
- R15 (680K) from base3 to **ground** (pull-down bias, NOT pull-up to Vcc)
- R16 (10K) from emit3/.68V junction to ground (emitter current path)
- TR-3 emitter connects **directly** to TR-4 base (shared .68V junction)
- TR-4 emitter grounded; collector shares Node G with TR-3

**Twin-T network (non-standard ratios):**
- Highpass T: C17 (.12uF) → node_hp → C16 (.12uF), with R12 (27K) shunt to GND
- Lowpass T: R14 (680K) → node_lp → R13 (680K), with C18 (.12uF) shunt to GND
- R_shunt/R_series = 27K/680K = 0.040 (standard = 0.5)
- C_shunt/C_series = 0.12/0.12 = 1.0 (standard = 2.0)
- This produces a shallow notch (~-23.5 dB) rather than a deep null

**Oscillation frequency:** ~5.6 Hz (SPICE). Service manual: ~6 Hz. Measured instruments: 5.3-7 Hz.

**DC operating points:**

| Node | Schematic | SPICE | Match |
|------|-----------|-------|-------|
| TR-3 base | 1.25V | 1.249V | Excellent |
| TR-3 emitter / TR-4 base | 0.68V | 0.668V | Excellent |
| Shared collector (Node G) | 5.9V | 4.95V | See note |

Note: Collector is ~1V low because the subcircuit models R17 (4.7K) direct to Vcc. In the real circuit, the LG-1 LED in series adds ~1.5V forward drop, reducing effective Vcc and raising the quiescent collector point.

**Output swing:** 11.8 Vpp (target ~11.5 Vpp). Near rail-to-rail.

**Waveform:** The real twin-T oscillator produces a mildly distorted sinusoid (estimated THD 3-10%). The OpenWurli implementation uses a pure sine LFO (`phase.sin()`) -- the mild oscillator distortion is not modeled, as it has negligible audible effect on the tremolo character.

**LED drive path:** Node G → R17 (4.7K) → LG-1 pin 1 (LED cathode) → LED → pin 2 (LED anode) → return to Vcc via cable. The LG-1 LED symbol points downward on the schematic (anode=pin 2 at top, cathode=pin 1 at bottom).

**SPICE netlist:** `spice/subcircuits/tremolo_osc.cir` (validated in `spice/testbench/tb_tremolo_osc.cir`)

### 2.2 LDR/Optocoupler (LG-1)

- Component designation: LG-1, Wurlitzer part #142312 (LED/LDR opto-isolator). Modern replacement: VTL5C3.
- Package: LED + CdS LDR in lightproof enclosure ("lightproof black box")
- Original part: manufacturer-specific, now commonly replaced with VTL5C3
- Replacement with VTL5C3 confirmed to work well by repair community
- NSL-32 also used as replacement, though VTL5C3 considered more consistent

**CdS LDR Characteristics (from vactrol datasheets):**

| Parameter | VTL5C3 | VTL5C4 | NSL-32SR2 |
|-----------|--------|--------|-----------|
| Rise time (on) | 2.5 ms | 6 ms | 5 ms |
| Fall time (off) | 18-35 ms | 180-1500 ms | 50 ms |
| Typical R_on | ~50 ohm | ~50 ohm | ~40 ohm |
| R_off (dark) | 1.3M-10M | 1.3M-10M | Several megohms |

CdS devices exhibit strongly asymmetric time constants (fast on, slow off). This produces the characteristic "choppy" tremolo quality of the 200A.

**CdS nonlinearity:** Resistance follows a power law. Datasheet values for gamma are typically 0.7-0.9, but the OpenWurli implementation uses gamma = 1.1, calibrated to match OBM tremolo depth measurements. The code uses a log-space interpolation model: `log_r = log_max + (log_min - log_max) * drive^gamma` (see `tremolo.rs`), rather than the simpler `R = R_dark * illumination^(-gamma)` formula.

### 2.3 Feedback Divider Operation

R-10 (56K) feeds from the preamp output to a feedback junction (fb_junct). Ce1 (4.7 MFD) AC-couples fb_junct to TR-1's emitter -- series-series negative feedback. The LDR (LG-1) shunts fb_junct to ground via cable Pin 1 → 50K VIBRATO pot → 18K → LG-1. The LDR path diverts feedback current away from the emitter.

- When LDR resistance is LOW (LED on/bright): fb_junct is shunted to ground → feedback cannot reach emitter → emitter AC-grounded via Ce1 → **HIGHER** preamp gain
- When LDR resistance is HIGH (LED off/dim): full feedback reaches emitter via Ce1 → strong emitter degeneration → **LOWER** preamp gain
- R-17 trimpot adjusts modulation depth
- Front panel vibrato pot: 50K (in the cable path between fb_junct and LG-1)

This is consistent with the EP-Forum "6 dB gain boost" measurement — tremolo boosts average gain above the no-tremolo baseline because the LDR periodically weakens emitter feedback.

**Gain modulation depth:**
- Without vibrato (LDR dark, Rldr_path ≈ 1M): gain = **6.0 dB (2.0x)**
- With vibrato at maximum depth, bright phase (Rldr_path ≈ 19K): gain = **12.1 dB (4.0x)**
- **Modulation range: 6.1 dB** — matches EP-Forum "6 dB gain boost" measurement exactly
- Bandwidth decreases with gain: ~11.8 kHz (no trem) → ~9.7 kHz (trem bright). GBW is NOT constant (scales with gain) -- captured by the DK MNA solver
- Excessive depth causes rail clipping in the power amp (distortion at high vibrato settings is a known issue)
- Typical depth in practice: 3-6 dB of gain modulation

### 2.4 Tremolo Character: 200 vs 200A

| Feature | Model 200 | Model 200A |
|---------|-----------|------------|
| Mechanism | Bias-shifting (reactance modulation) | LDR optocoupler in feedback loop |
| Location | Preamp transistor bias injection | Preamp feedback network |
| Character | Smoother, more gradual | Choppier, more intense |
| Timbral modulation | YES (bias changes distortion) | YES (gain changes distortion operating point) |
| Phase modulation | Subtle component | None (pure gain/AM) |
| Depth control | Fixed or limited | Trimpot + front panel pot |
| Adjustability | Limited | More range via trimpot |
| Heritage | Unique to 200 | Return to 140B technique (updated) |

**Key insight:** Both the 200 and 200A tremolo circuits modulate the timbral content, not just volume. The 200A does this through gain modulation (changing the preamp's operating point on its transfer curve), while the 200 does it through bias-point modulation (shifting the transistor's DC operating point). The common simplification that the 200A is "pure AM" is not quite correct -- it is gain-modulated AM, which subtly changes harmonic content through the tremolo cycle.

---

## 3. Volume Control

| Parameter | Value |
|-----------|-------|
| Potentiometer value | 3K ohm |
| Taper | Audio (logarithmic) |
| Position in signal chain | After preamp output, before power amp input |
| Preamp output level | 2-7 mV AC (Brad Avenson measurement) |

The 3K audio pot is unusually low impedance for a volume control. This has implications:
- Very low output impedance to the power amp input
- Minimal noise pickup on the wiring between pot and amp board
- Compatible with the low-impedance preamp output

**The volume pot is between the preamp (on the reed bar) and the power amp (on the main amp board).** The wiring runs from the reed bar preamp PCB through the volume pot to the amp board input via C-8 coupling capacitor.

**DECISION: Model as real attenuator, not output gain.** The volume pot must sit between preamp and power amp in the plugin signal chain, not at the output. At low volume settings, the signal level at the power amp input drops into the crossover distortion region, changing the character of the distortion (more odd harmonics from the dead zone). This interaction is audible and should be preserved. Implementation: audio-taper gain curve applied between preamp output and power amp input.

**Volume taper implementation:** `gain = volume^2` (quadratic approximation of audio taper). The parameter UI uses a skew factor of 2.0 for display. Default volume is 0.63, giving effective gain of approximately 0.40 (-8 dB).

---

## 4. Power Amplifier

### 4.1 Topology Overview

The 200A power amplifier is a **quasi-complementary Class AB push-pull** design. The service manual states:

> "The audio output amplifier is of a quasi complementary design. The driver transistors provide the necessary phase inversion for the output transistors. The collector current of the driver transistor becomes the base current of the output transistor. The output transistors which are operated as emitter followers, provide additional current gain."

**Rated power:** 20 watts (service manual specification). Wikipedia's "30 watt" claim for the model 200 may refer to peak power or an earlier revision; the 200A service manual consistently specifies 20W.

### 4.2 Circuit Stages

#### Input Stage: Differential Amplifier (TR-7, TR-8)

- TR-7 and TR-8 form a long-tailed pair (differential amplifier)
- Both: 2N5087 (PNP), or 2N3702 in earlier production (Wurlitzer part 142128-1)
- Must be matched for proper operation
- Signal input coupled to TR-7 base via C-8 (coupling capacitor)
- TR-8 receives negative feedback from output via R-31
- Common emitters provide differential operation

> "The signal input is coupled to TR-7 (one-half of the differential amplifier stage) via C-8. The other half of this stage, TR-8, monitors the final output level via R-31."

The negative feedback through R-31 serves three purposes (from service manual):
1. Increases frequency response (extends bandwidth)
2. Lowers distortion (linearizes the amplifier)
3. Minimizes DC offset voltage at the output

#### Pre-Driver / VAS Stage (TR-11)

- TR-11 receives the differential signal from TR-7's collector
- Acts as voltage amplifier stage (VAS)
- Provides the voltage swing needed to drive the output stage

#### Bias Control: Vbe Multiplier (TR-9)

- TR-9: MPSA06 (NPN), or MPSA14 in later production (serial #102905+)
- Functions as a constant-current source / variable voltage reference
- Generates approximately 1.3V across its terminals (two diode drops)
- This voltage biases the driver/output transistors into Class AB operation
- R-34 and R-35 set the bias point

From service manual:
> "The bias control circuit, TR-9, is a constant current source; its base emitter diode junction is used as a reference voltage. If too much current passes through resistor R-35 and exceeds the threshold of the base emitter junction of TR-9 (.7V), the transistor will turn on more, reducing the excessive current through R-35, establishing the stable bias current."

**Bias current target:** 10 mA quiescent (from schematic specification). Measured as 5 mV across each 0.47-ohm emitter resistor (V = I * R = 0.01 * 0.47 = 0.0047V, approximately 5 mV).

#### Driver Stage (TR-10, TR-12)

- TR-10: MPSA06 (NPN driver) -- drives NPN output transistor
- TR-12: MPSA56 (PNP driver) -- drives PNP output transistor
- The driver transistors provide phase inversion for the quasi-complementary output

#### Output Stage (TR-11/TIP36C, TR-13/TIP35C)

| Transistor | Type | Function | Package | Ratings |
|-----------|------|----------|---------|---------|
| TR-11 | TIP36C (PNP) | PNP output | TO-247 | 100V, 25A, 125W |
| TR-13 | TIP35C (NPN) | NPN output | TO-247 | 100V, 25A, 125W |

**NOTE on transistor designation:** TR-11 serves double duty in different sources. In some schematic descriptions, TR-11 refers to the pre-driver/VAS stage, and TIP36C is the PNP output. The numbering may vary between schematic revisions. The key fact is: TIP36C (PNP) and TIP35C (NPN) form the complementary output pair.

**Emitter degeneration resistors:**
- R-37: 0.47 ohm (NPN side)
- R-38: 0.47 ohm (PNP side)
- Purpose: Current sensing for bias stability; prevent thermal runaway
- Measurement point for bias current: voltage across these resistors should be approximately 5 mV each at idle

### 4.3 Supply Voltages

| Rail | Service Manual Spec | Measured (typical) |
|------|--------------------|--------------------|
| V+ | +22V (nominal) | +24 to +24.5V |
| V- | -22V (nominal) | -24 to -24.5V |
| Preamp supply | +15V (regulated) | +15V |

**NOTE:** The actual rail voltages are typically 10% higher than the nominal specification (24.5V vs 22V). This is normal for unregulated supplies at light load.

### 4.4 Bootstrap Capacitor (C-12)

> "Capacitor C-12 performs two functions: 1) it acts as a bypass to decouple any power supply ripple from the driver stages, and 2) it is connected as a 'bootstrap' capacitor to provide the drive necessary to pull TR-10 and TR-11 into saturation. The stored voltage of the capacitor (with reference to the output) provides a higher voltage than the normal collector-supply voltage to drive TR-10 and TR-11."

The bootstrap capacitor is standard practice in quasi-complementary designs. It allows the upper driver transistor to swing the output close to the positive rail by effectively providing a floating supply above the output voltage.

### 4.5 Complete Power Amp Component Summary

#### Transistors

| Ref | Type | Function |
|-----|------|----------|
| TR-7 | 2N5087 (PNP) | Differential input (signal) |
| TR-8 | 2N5087 (PNP) | Differential input (feedback) |
| TR-9 | MPSA06 or MPSA14 | Vbe multiplier (bias) |
| TR-10 | MPSA06 (NPN) | NPN driver |
| TR-12 | MPSA56 (PNP) | PNP driver |
| TR-11* | TIP36C (PNP) | PNP output, 125W |
| TR-13 | TIP35C (NPN) | NPN output, 125W |

*TR-11 designation may vary by schematic revision; see note in section 4.2.

TR-7 = TR-8 = Wurlitzer part #142128

#### Key Resistors

| Ref | Value | Function |
|-----|-------|----------|
| R-30 | 220 Ω | Feedback ground-side resistor (with R-31 forms voltage divider) |
| R-31 | 15K | Output-to-input negative feedback |
| R-32 | 1.8K | Differential pair collector load (TR-7) |
| R-33 | 1.8K | Differential pair collector load (TR-8) |
| R-34 | 160 ohm | Bias network (confirmed by GroupDIY measurement of 150-160 ohm) |
| R-35 | 220 ohm | Bias network |
| R-36 | 270 ohm | Base-emitter TR-11 |
| R-37 | 0.47 ohm | NPN output emitter degeneration |
| R-38 | 0.47 ohm | PNP output emitter degeneration |
| R-58 | Optional 1K (across R-34) | Bias reduction modification |

#### Key Capacitors

| Ref | Value | Function |
|-----|-------|----------|
| C-8 | 4.7 MFD | Input coupling to TR-7 |
| C-11 | 100 PF | Pre-driver feedback cap |
| C-12 | 100 MFD | Bootstrap / ripple bypass |

### 4.6 Power Output and Clipping Analysis

**Rated output:** 20 watts into 8 ohms (service manual)

**Theoretical maximum:**
With +/-22V rails (nominal), accounting for transistor saturation voltage drops of approximately 2-3V per side:
- Effective peak swing: approximately +/-19V to +/-20V
- P_max = V_peak^2 / (2 * R_load) = (19)^2 / (2 * 8) = 361/16 = ~22.5W RMS
- With measured +/-24.5V rails: P_max = (21.5)^2 / 16 = ~29W
- This is consistent with 20W rated / 30W peak specifications

**Clipping behavior:**
- At moderate levels (mf single notes): power amp is clean, preamp dominates tonal character
- At ff polyphonic (multiple notes, high velocity): output can approach rail clipping
- The preamp output (pre volume pot) measured at 1.8-4V peak-to-peak
- Through the 3K volume pot at moderate settings, signal to power amp is in millivolt range
- At full volume with ff polyphonic playing: power amp may clip against rails
- Clipping is symmetric (equal positive and negative excursion to rails)
- Produces primarily odd harmonics when clipping occurs

**Crossover distortion (common aging issue):**
- The Class AB bias (10 mA) is set by the Vbe multiplier (TR-9)
- With component aging, bias drifts toward zero, increasing the dead zone
- Crossover distortion produces odd harmonics, especially audible at low signal levels
- This is a well-documented repair issue in the Wurlitzer community
- Repair involves adjusting R-34/R-35 (Vbe multiplier network) to restore 5 mV across R-37/R-38

### 4.7 Does the Power Amplifier Contribute to Tone?

**Answer: Generally NO for well-maintained instruments at normal levels, but YES in several edge cases.**

| Condition | Power Amp Contribution | Character |
|-----------|----------------------|-----------|
| mf single notes | Negligible | Clean amplification |
| ff single notes | Minimal | Slight compression near rails |
| ff polyphonic (chords) | Moderate | Rail clipping adds compression, slight odd harmonics |
| Aged bias (crossover) | Significant | Odd-harmonic "grittiness" at all levels |
| Full volume + ff chords | Significant | Hard clipping, dense saturation |

**For modeling purposes:** The power amplifier is modeled as a closed-loop negative feedback amplifier. The R-31/R-30 feedback network (loop gain ≈ 275) linearizes the output at normal signal levels. Distortion becomes significant only near the ±22V supply rails. The power amp is NOT a major tonal contributor — the Wurlitzer's characteristic bark comes primarily from the pickup's 1/(1-y) nonlinearity, with the preamp's asymmetric soft-clipping adding further coloring at high dynamics.

**Gain staging (v0.1.5):** The voice output_scale uses target_db=-35 dBFS so the power amp sees realistic signal levels: a single ff note uses ~5-10% of the ±22V headroom, matching Brad Avenson's measurements of 2-7 mV at the volume pot. A post-speaker gain of +10 dB (applied AFTER the speaker model) maps physical SPL to DAW-friendly digital levels. This separates two concerns: the analog circuit model operates at realistic voltages, while the digital output is set for typical DAW workflows (~-15 dBFS for single ff notes, ~-10 dBFS for 6-note ff chords).

---

## 5. Speaker and Cabinet

### 5.1 Speaker Specifications

| Parameter | Value |
|-----------|-------|
| Driver count | 2 (stereo placement, mono signal) |
| Driver size | 4" x 8" oval (schematic shows 4x6, but all vendors and repair sources confirm 4x8 in production units) |
| Individual impedance | 16 ohm each |
| Wiring | Parallel |
| Combined impedance | 8 ohm (16 || 16) |
| Magnet type (200) | Alnico |
| Magnet type (200A) | Ceramic (most units) |
| Mounting (200) | Welded to amplifier rail |
| Mounting (200A) | Screwed to ABS plastic lid |

**200A speaker evolution:**
1. Very early 200A production: alnico speakers (brief transition period)
2. Brief period: square ceramic magnet speakers
3. Most common (majority of production): round ceramic magnet speakers

**Tonal difference between magnet types:**
- Alnico: smoother treble response, natural compression at volume, warmer character
- Ceramic: brighter, more articulate, more headroom before compression

### 5.2 Cabinet/Enclosure

- The 200A's speakers are mounted to the ABS plastic lid (the flip-up top)
- The lid serves as the speaker baffle
- Speakers face the player (forward-facing) when lid is in playing position
- The lid is NOT a sealed enclosure -- it is essentially an open-backed baffle
- The plastic material resonates and colors the sound (thin ABS plastic)

**Acoustic characteristics:**

The 200A "cabinet" is more accurately described as an **open baffle** formed by the plastic lid. This means:
- No bass reinforcement from cabinet resonance (unlike sealed or ported designs)
- Bass rolloff follows the baffle step response: approximately 6 dB/octave below the baffle step frequency
- For a 4x8" driver in a ~24" wide baffle, the baffle step frequency is approximately 100-150 Hz
- Low-frequency rolloff is primarily set by the speaker's own resonant frequency and the open baffle cancellation
- High-frequency rolloff is set by cone breakup and the ceramic magnet driver characteristics

### 5.3 Frequency Response Analysis

No direct measurements of the 200A speaker+cabinet system are publicly available. The following is derived from physical analysis:

#### Low Frequency Rolloff

Multiple factors contribute to the bass rolloff:
1. **Speaker free-air resonance (Fs):** For a small 4x8" oval driver, Fs is typically 100-150 Hz. Below Fs, the cone's mechanical compliance dominates and output falls at ~12 dB/oct.
2. **Open baffle dipole cancellation:** Below the baffle step frequency (~100-150 Hz for the ~24" ABS lid), front and rear waves partially cancel. This adds an additional ~6 dB/oct rolloff.
3. **Combined effect:** Approximately **18 dB/octave** rolloff below ~80 Hz (speaker resonance 12 dB/oct + open baffle dipole 6 dB/oct)

**Original design (not implemented) proposed three cascaded HPF sections:**
- **HPF1** at 150 Hz, Q=0.75: Models cone resonance and mechanical rolloff (Fs). The slightly underdamped Q produces the mild resonant bump near the rolloff frequency. 150 Hz is typical for a small 4x8" oval ceramic-magnet driver.
- **HPF2** at 100 Hz, Q=0.707: Models the open-baffle front/rear wave cancellation (dipole effect). Butterworth Q for a smooth transition with no resonant peak.
- **HPF3** at 70 Hz, Q=0.5: Models the radiation impedance rolloff. Below ka=1 (~1090 Hz for a ~5cm effective piston radius), acoustic radiation resistance falls as f². The overdamped Q captures the gradual onset of this regime.

The three cascaded HPFs provide ~30 dB/oct combined rolloff below 70 Hz, matching the physics of a small open-baffle speaker. Note: the preamp's tremolo pump (5.63 Hz harmonics spanning 28-200+ Hz) is eliminated at source via shadow preamp subtraction (a second DK solver instance runs with zero input, producing pure pump; subtracting it from the main output cancels all pump at every frequency). The speaker HPFs are purely physics-motivated — they model the real speakers' inability to reproduce deep bass, not pump suppression.

#### High Frequency Rolloff

1. **Cone breakup:** Small ceramic-magnet paper-cone drivers typically break up above 5-8 kHz
2. **Voice coil inductance:** Creates a natural LPF, typically around 8-12 kHz for this size driver
3. **General specifications for similar 4x8" oval drivers:** Frequency response typically quoted as 120 Hz - 10 kHz

**Previous model used LPF at 8 kHz, Butterworth (Q=0.707).** This is reasonable. A 4x8" ceramic driver would have significant rolloff above 8-10 kHz. The Butterworth (maximally flat) response is appropriate for a natural cone driver rolloff.

#### Speaker Resonance Effects

The HPF1 near 150 Hz naturally creates a resonant bump that can boost harmonics in the 120-250 Hz range. For bass notes (A1 = 55 Hz fundamental), the H2 at 110 Hz sits just below this resonance. This partially explains why real 200A recordings show stronger H2 in the bass register than the preamp alone would produce.

### 5.4 Current Speaker Model

The current implementation (`speaker.rs`) uses a generalized Hammerstein-like architecture: static polynomial waveshaper -> tanh excursion limiter -> thermal voice coil compression -> linear filters (HPF + LPF).

**Linear filters:**

```
HPF: 2nd-order highpass at 95 Hz, Q = 0.75   (combined cone resonance + open-baffle rolloff)
LPF: 2nd-order lowpass at 5500 Hz, Q = 0.707 (Butterworth, cone breakup + voice coil inductance; lowered from 7500 Hz per OBM A/B comparison)
```

> **Note:** An earlier design (documented below in §5.4.1) specified three cascaded HPFs at 150/100/70 Hz to separately model cone resonance, dipole cancellation, and radiation impedance. The implementation simplified this to a single HPF at 95 Hz, which provides adequate bass rolloff (~12 dB/oct, ~3-4 dB down at C2 fundamental of 65 Hz) without the over-aggressive filtering of the three-HPF cascade.

**Nonlinear features:**
- **Normalized Hammerstein polynomial waveshaper:** `y = (x + a2*x^2 + a3*x^3) / (1 + a2 + a3)` where a2 = 0.2 (BL force factor asymmetry, generates even harmonics) and a3 = 0.6 (Kms suspension hardening, generates odd harmonics). Coefficients scale with the Speaker Character parameter. The normalization by `(1 + a2 + a3)` ensures `y(1) = 1`, preserving peak positive level. The even-order term (a2) introduces asymmetry, so `y(-1) != -1`.
- **Cone excursion limiting (Xmax):** `tanh()` soft saturation after the polynomial, modeling the physical excursion limits of the spider and surround. At normal levels (|x| < 0.5): < 8% compression. At ff chords (|x| > 1.0): graceful saturation.
- **Thermal voice coil compression:** Slow envelope follower (tau = 5.0 s) reduces gain under sustained loud signal, modeling the increase in voice coil DC resistance as the coil heats up.

**Speaker Character parameter:** Blends from bypass (0.0: flat, linear passthrough) to authentic (1.0: full nonlinearity + HPF + LPF). Filter cutoffs interpolate logarithmically between bypass positions (HPF: 20 Hz, LPF: 20 kHz) and authentic positions.

#### 5.4.1 Original Three-HPF Design (Not Implemented)

The following design was proposed to separately model each physical mechanism but was simplified to the single-HPF approach above:

```
HPF1: 2nd-order highpass at 150 Hz, Q = 0.75  (cone resonance Fs)
HPF2: 2nd-order highpass at 100 Hz, Q = 0.707 (open-baffle dipole cancellation)
HPF3: 2nd-order highpass at 70 Hz,  Q = 0.5   (radiation impedance rolloff)
```

This cascade produced ~30 dB/oct rolloff below 70 Hz, which proved too aggressive for the 200A's bass character. The combined effect removed too much fundamental energy from bass notes (C2-C3), making the low end thin.

---

## 6. Auxiliary and Headphone Outputs

### 6.1 Auxiliary Output

- The 200A has a dedicated auxiliary amplifier circuit (TR-15 and TR-16)
- Two direct-coupled transistors with feedback
- Taps the signal BEFORE the power amplifier (from the preamp output)
- Provides line-level output suitable for external amplifiers or recording
- Has its own gain control potentiometer
- Late production (serial #102905+) used MPSA14 transistor for TR-16

> "On models that require a signal to drive an auxiliary amplifier, a two transistor direct-coupled stage with feedback consisting of TR-15 and TR-16 is provided."

**Key implication:** The aux output does NOT include the power amplifier's characteristics. It represents the preamp output (with tremolo modulation) at line level. Many studio recordings of the 200A use this output, meaning the "classic Wurlitzer sound" on records often excludes the power amp and speaker coloration entirely.

### 6.2 Headphone Output

- Switching mono jack -- physically disconnects speakers when headphones are inserted
- Signal tapped from the power amp output (parallel with speaker connection)
- Contains an 8-ohm load resistor that substitutes for the speaker impedance when speakers are disconnected
- Delivers speaker-level signal (fully amplified)
- Low impedance output
- May contain more noise/distortion than aux output since it includes the full power amp chain

---

## 7. Modeling Recommendations

### 7.1 Tremolo Model

**Status: IMPLEMENTED.** Tremolo operates inside the preamp feedback loop. The `Tremolo` module (`tremolo.rs`) computes a per-sample LDR path resistance, which is passed to the DkPreamp via `set_ldr_resistance()`. The DkPreamp's 8-node MNA circuit solver then modulates the feedback loop gain accordingly, producing the correct timbral variation (gain + distortion character change) through the tremolo cycle.

**Implementation details:**

```
// Oscillator (tremolo.rs)
rate = 5.63 Hz (pure sine LFO; real twin-T oscillator's mild THD not modeled)
waveform: phase.sin(), half-wave rectified for LED drive

// LDR time constants (VTL5C3-like, tuned to match perceived tremolo
// character of real 200A instruments. CdS time constants vary significantly
// between individual devices; datasheet: 2.5 ms / 18-35 ms)
attack_tau = 3.0 ms  (fast on)
release_tau = 50 ms   (slow off)

// CdS LDR resistance model (log-space interpolation)
log_r = log(R_max) + (log(R_min) - log(R_max)) * drive^gamma
R_min = 50 ohm, R_max = 1M ohm, gamma = 1.1
(gamma calibrated to OBM tremolo depth; datasheet range 0.7-0.9)

// Total LDR path resistance → DkPreamp::set_ldr_resistance()
R_ldr_path = R_series + R_ldr
R_series = 18K + 50K * (1 - depth)
```

**Depth control:** The vibrato depth pot and trimpot (R-17) control how much of the oscillator signal reaches the LED. At full depth, the preamp gain can approximately double, which at high signal levels causes clipping.

### 7.2 Power Amplifier Model

**Status: IMPLEMENTED** in `power_amp.rs`.

The power amp is modeled as a **closed-loop negative feedback amplifier** using
a per-sample Newton-Raphson solver. This captures the linearizing effect of the
R-31/R-30 feedback network, which reduces distortion by the loop gain factor.

**Feedback equation (solved per sample):**

    y = f(A_ol × (input − β × y))

where f() = output stage crossover + tanh rail saturation.

**Forward path nonlinearities (inside the loop):**

1. **Crossover distortion:** Gaussian dead zone with quiescent bias floor.
   `gain(v) = q + (1−q) × (1 − exp(−v²/vt²))`
   - At v=0: gain = q (quiescent transconductance, not zero)
   - At |v| >> vt: gain → 1.0

2. **Rail saturation:** `rail × tanh(v_cross / rail)` — tanh models gradual
   transistor saturation into ±22V rails.

**Parameters:**

| Constant | Value | Derivation |
|----------|-------|------------|
| `OPEN_LOOP_GAIN` | 19,000 | Diff pair (68×) × VAS (300×) × output (0.95×) |
| `FEEDBACK_BETA` | 0.01445 | R30/(R30+R31) = 220/(220+15K) |
| `HEADROOM` | 22.0 V | ±24V supply − 2V Vce_sat |
| `CROSSOVER_VT` | 0.013 V | Lightly aged bias (5−7 mA) |
| `QUIESCENT_GAIN` | 0.1 | Output stage gain at zero signal |
| `NR_MAX_ITER` | 8 | Convergence: 2−4 iterations typical |
| `NR_TOL` | 1e-6 V | Convergence threshold |

**Derived quantities:**

- Closed-loop gain: A_ol/(1 + A_ol×β) = 19000/275.6 = **69×** (37 dB)
- Loop gain T = A_ol×β = **275** at DC → THD reduced by 49 dB
- At crossover (zero signal): T_zero = A_ol×β×q = **27.5** → 29 dB linearization

The power amp output is normalized to [-1.0, +1.0] by dividing by HEADROOM (22V). The effective closed-loop gain from input to output is thus 69/22 = 3.14x.

The feedback loop keeps the output tracking the input linearly until the
output stage saturates at the ±22V rails. At moderate levels (<90% of rail),
deviation from linear is <0.1%. This eliminates the polyphonic intermodulation
artifacts that plagued the earlier open-loop model, where tanh compression on
the summed signal created audible buzz on polyphonic material.

### 7.3 Speaker Model

**Status: IMPLEMENTED** in `speaker.rs`. See Section 5.4 for full details.

Variable speaker emulation with bypass-to-authentic range. The plugin exposes a "Speaker Character" knob that blends from bypass (flat, linear passthrough) to authentic (full Hammerstein nonlinearity + HPF + LPF).

At "authentic" position (character = 1.0):
- HPF: 95 Hz, Q=0.75 (combined cone resonance + open-baffle rolloff)
- LPF: 5500 Hz, Q=0.707 (Butterworth, lowered from 7500 Hz)
- Hammerstein polynomial: (x + 0.2x² + 0.6x³) / 1.8, normalized
- tanh Xmax soft stop
- Thermal voice coil compression (tau = 5.0s)

At "bypass" position (character = 0.0): flat linear passthrough (HPF 20 Hz, LPF 20 kHz, no nonlinearity). Intermediate positions interpolate logarithmically.

Possible refinements:
- Add mild midrange presence peak (1-3 kHz) from speaker's natural response
- Measured impulse response from a real 200A would improve accuracy, but none is publicly available

### 7.4 Signal Chain Order

**Signal chain:**

```
Per-voice processing (oscillator, pickup)
  -> Sum to mono
  -> [Tremolo modulates emitter feedback via LDR shunt at fb_junct]
  -> Preamp (with tremolo-modulated gain via R-10/Ce1 emitter feedback)
  -> Volume control (3K pot)
  -> Power amplifier (closed-loop negative feedback, tanh soft-clip at ±22V)
  -> Speaker model (HPF + LPF)
  -> Output
```

---

## 8. Sources

### Primary Sources (Service Manual)

- Wurlitzer 200/200A Service Manual (PDF available from multiple hosts):
  - https://static1.squarespace.com/static/581b462f5016e14ae76bd275/t/5ebb550ed544905f3db1d03a/1589335321798/wurlitzer-200-200a-service-manual.pdf
  - https://archive.org/details/wurlitzer-200-and-200-a-service-manual
  - https://www.manualslib.com/manual/1002608/Wurlitzer-200.html
- Wurlitzer 200A Series Schematic: https://www.bustedgear.com/images/schematics/Wurlitzer_200A_series_schematics.pdf
- Wurlitzer 200 Series Schematic: https://www.bustedgear.com/images/schematics/Wurlitzer_200_series_schematics.pdf

### Transistor Specifications

- Wurlitzer 200A Transistor Specs: https://www.bustedgear.com/res_Wurlitzer_200A_transistors.html
- TIP35C Datasheet: https://www.st.com/resource/en/datasheet/tip35c.pdf
- TIP36C Datasheet: https://www.onsemi.com/pdf/datasheet/tip35a-d.pdf

### Repair and Circuit Analysis

- GroupDIY: Troubleshooting Wurlitzer 200A bias and crossover distortion: https://groupdiy.com/threads/troubleshooting-wurlitzer-200a-amp-board-for-bias-and-crossover-notch-distortion.62917/
- GroupDIY: Wurlitzer 200A preamp discussion: https://groupdiy.com/threads/wurlitzer-200a-preamp.44606/
- GroupDIY: Wurlitzer 200 general discussion: https://groupdiy.com/threads/one-more-wurlitzer-200-question.13555/
- illdigger: Wurlitzer 200A repair and low noise mod: https://illdigger.wordpress.com/2016/07/03/wurlitzer-200a-piano-repair-and-low-noise-mod/
- EP-Forum: Wurlitzer speaker impedance: https://ep-forum.com/smf/index.php?topic=8182.0
- EP-Forum: Wurlitzer 200A vibrato hum and distortion: https://ep-forum.com/smf/index.php?topic=10483.0
- EP-Forum: Wurlitzer 200 amp output: https://ep-forum.com/smf/index.php?topic=7813.0
- EP-Service.nl: Troubleshooting guide: https://ep-service.nl/upload/files/wurlitzer_200_series_troubleshooting.pdf

### Wurlitzer Comparison and Overview

- Tropical Fish: 200 vs 200A differences: https://www.tropicalfishvintage.com/blog/2019/5/27/what-is-the-difference-between-a-wurlitzer-200-and-a-wurlitzer-200a
- Tropical Fish: 200 Series overview: https://www.tropicalfishvintage.com/200series-wurlitzers
- Tropical Fish: Headphone vs aux output: https://www.tropicalfishvintage.com/blog/2020/5/25/what-is-the-difference-between-a-wurlitzers-headphone-output-and-aux-output
- Tropical Fish: Component replacement guide: https://www.tropicalfishvintage.com/blog/2019/7/3/what-components-should-i-replace-in-my-vintage-amp-and-why
- Chicago Electric Piano: 200 vs 200A: https://chicagoelectricpiano.com/wurlitzer/wurlitzer-200-vs-200a/

### Tremolo and LDR References

- Strymon: Amplifier Tremolo Technology White Paper: https://www.strymon.net/amplifier-tremolo-technology-white-paper/
- Aiken Amps: Designing Phase Shift Oscillators for Tremolo: https://www.aikenamps.com/index.php/designing-phase-shift-oscillators-for-tremolo-circuits
- VTL5C3/VTL5C4 Vactrol datasheets: https://www.qsl.net/wa1ion/vactrol/vactrol.pdf
- NSL-32 datasheet: https://www.digikey.com/en/products/detail/advanced-photonix/NSL-32/5039800

### Parts and Replacement

- Vintage Vibe: Wurlitzer 200A LDR (W140): https://www.vintagevibe.com/products/wurlitzer-200a-ldr
- Vintage Vibe: Volume pot: https://www.vintagevibe.com/products/wurlitzer-volume-pot
- Vintage Vibe: Speakers: https://www.vintagevibe.com/products/vintage-vibe-wurlitzer-speakers-200-series
- RetroLinear: 200A amplifier: https://retrolinear.com/wurlitzer-ep-200a-amplifier.html
- Custom Vintage Keyboards: 200A speakers: https://www.cvkeyboards.com/products/wurlitzer-200a-electric-piano-speaker

### Amplifier Design References

- Quasi-Complementary Push-Pull Amplifier theory: https://www.eeeguide.com/quasi-complementary-push-pull-amplifier/
- Class AB Amplifier Biasing: https://www.electronics-tutorials.ws/amplifier/class-ab-amplifier.html

