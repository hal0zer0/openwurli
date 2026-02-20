# Calibration Data, Evaluation Criteria, and Reference Targets

## Purpose and Scope

This document provides concrete, source-attributed calibration data and evaluation criteria for building a physically accurate Wurlitzer 200A electric piano synthesizer. Every value has an uncertainty range, every claim has a source, and every metric has an assessment of whether it actually correlates with perceptual quality.

> **See also:** [Reed and Hammer Physics](reed-and-hammer-physics.md) (modal synthesis parameters), [Signal Chain Architecture](signal-chain-architecture.md) (overall DSP spec), [DK Preamp Testing](dk-preamp-testing.md) (preamp validation targets)

CRITICAL CONTEXT: A previous iteration of this project achieved good FFT-derived metrics (H2>H3 compliance, decay rates within range, spectral centroid targets met) but sounded terrible to human listeners. This document addresses why that happened and how to prevent it.

---

## Table of Contents

1. [Existing Calibration Data and Reliability Assessment](#1-existing-calibration-data-and-reliability-assessment)
2. [What Makes a Wurlitzer Sound Like a Wurlitzer](#2-what-makes-a-wurlitzer-sound-like-a-wurlitzer)
3. [Spectral Targets by Register and Velocity](#3-spectral-targets-by-register-and-velocity)
4. [Envelope Targets: Attack, Sustain, Decay, Release](#4-envelope-targets-attack-sustain-decay-release)
5. [Perceptual Evaluation Criteria](#5-perceptual-evaluation-criteria)
6. [Metrics That Mislead vs Metrics That Matter](#6-metrics-that-mislead-vs-metrics-that-matter)
7. [Reference Recordings and What to Listen For](#7-reference-recordings-and-what-to-listen-for)
8. [Existing Plugin Benchmarks](#8-existing-plugin-benchmarks)
9. [Unit-to-Unit Variation and What We Are Modeling](#9-unit-to-unit-variation-and-what-we-are-modeling)
10. [Objective Metric Recommendations](#10-objective-metric-recommendations)
11. [Known Pitfalls: Passing Metrics While Sounding Terrible](#11-known-pitfalls-passing-metrics-while-sounding-terrible)
12. [Key Questions Answered](#12-key-questions-answered)
13. [Sources and References](#13-sources-and-references)

---

## 1. Existing Calibration Data and Reliability Assessment

### 1.1 Source 1: OldBassMan Wurlitzer 200A (PRIMARY)

- **Origin:** Freesound pack 5726, CC-BY 4.0
- **Format:** 48 kHz / 24-bit mono WAV
- **Recording chain:** Wurlitzer 200A -> Lundahl transformer -> tube preamp -> ADC
- **Content:** 13 isolated sustained notes, estimated mf dynamics (RMS -15 to -19 dBFS)
- **Reliability: HIGH for harmonic ratios, MODERATE for absolute levels, LOW for treble register**

#### Reliability Assessment

| Aspect | Rating | Rationale |
|--------|--------|-----------|
| H2/H1 ratios | HIGH (with correction) | Clean isolated notes allow reliable harmonic peak measurement. Recording chain adds +2-6 dB H2 (Lundahl iron core + triode asymmetry). Raw data adjusted by -3 dB for Wurlitzer-intrinsic H2. |
| H2>H3 ordering | VERY HIGH | Recording chain boosts H2 more than H3, so if H2>H3 in recording, it is certainly H2>H3 in the instrument. 13/13 compliance. |
| Decay rates | HIGH for MIDI 54-82 | Exponential fit `0.26 * exp(0.049 * MIDI)` has R-squared ~0.85. D3 (MIDI 50) shows anomalous negative value (unreliable). MIDI 86+ may have noise-floor contamination in decay tail. |
| Attack overshoot | MODERATE | All notes at similar (mf) dynamics. Range 2.0-6.9 dB. Cannot assess velocity dependence from this source alone. |
| Spectral centroids | HIGH for attack/sustain | Late centroids (>1s) unreliable above MIDI 78 due to noise floor artifacts pushing centroid to 3-7 kHz. |
| Treble register (MIDI 86+) | LOW | D6, F#6, Bb6, D7 show anomalous patterns: sub-octave spectral peaks, unusually high THD for high notes. Possible causes: sympathetic reed vibration, pickup crosstalk, aliased modes. DO NOT use these as sole treble targets. |
| Absolute dBFS levels | LOW | Depends on recording gain, volume pot position, and preamp condition. Only relative levels (H2/H1, register-to-register ratios) are usable. |

#### Pitch Label Corrections

Note labels "A" and "F" in filenames are exactly 1 semitone sharp (mislabeled). D notes are correctly tuned. See `calibration-data.md` for full correction table.

#### Harmonic Content Summary (sustain 150-800ms, recording-chain-corrected)

After subtracting estimated recording chain contribution (+3 dB H2, +1 dB H3):

| MIDI | Note | H2/H1 (dB) corrected | H3/H1 (dB) corrected | H2>H3 gap (dB) |
|------|------|----------------------|----------------------|-----------------|
| 50 | D3 | -4.5 | -7.1 | +2.6 |
| 54 | F#3 | -14.8 | -13.1 | -1.7 (borderline) |
| 58 | Bb3 | -11.6 | -24.4 | +12.8 |
| 62 | D4 | -13.9 | -29.4 | +15.5 |
| 66 | F#4 | -2.2 | -4.8 | +2.6 |
| 70 | Bb4 | -9.3 | -19.1 | +9.8 |
| 74 | D5 | -15.3 | -29.8 | +14.5 |
| 78 | F#5 | -22.0 | -27.6 | +5.6 |
| 82 | Bb5 | -29.2 | -41.4 | +12.2 |
| 86 | D6 | -33.4 | -43.3 | +9.9 |
| 90 | F#6 | -26.2 | -48.8 | +22.6 |
| 94 | Bb6 | -24.4 | -26.9 | +2.5 |
| 98 | D7 | -23.7 | -23.7 | +0.0 (borderline) |

**Key observation:** F#3 (MIDI 54) shows H2 and H3 nearly equal after correction. This may indicate the H2 advantage is smaller than raw data suggests at certain notes, or that the recording chain correction is imprecise. The correction should be treated as +/-2 dB uncertain.

#### H2/H1 Linear Model (recording-chain-corrected)

```
H2_dB = -0.48 * MIDI + 17.5    (adjusted from raw regression intercept 20.5)
Uncertainty: +/- 5 dB per note (instrument variation)
Uncertainty: +/- 2 dB systematic (recording chain correction uncertainty)
Valid range: MIDI 50-82 (HIGH confidence), MIDI 82-98 (LOW confidence)
```

This predicts:
- MIDI 36 (C2): H2/H1 = +0.2 dB (near fundamental level)
- MIDI 60 (C4): H2/H1 = -11.3 dB
- MIDI 84 (C6): H2/H1 = -22.8 dB
- MIDI 96 (C7): H2/H1 = -28.6 dB

### 1.2 Source 2: "Blue in Green" Solo Wurlitzer (SUPPLEMENTARY)

- **Reliability: LOW** -- unknown instrument model, limited clean notes, polyphonic contamination
- Only 4 usable notes. Bb3 measurements show anomalously low H2 (-33 to -35 dB), far below OBM's -8.6 dB at the same note
- Value: confirms that DIFFERENT instruments can show very different H2 levels at the same note
- Implication: the +/-5 dB per-note variation in the H2 model may be conservative; true unit-to-unit variation could be +/-10 dB

### 1.3 Source 3: Improv-Wurli200 (SUPPLEMENTARY)

- **Reliability: LOW-MODERATE** -- YouTube compression (lossy codec), Wurlitzer 200 (not 200A), polyphonic contamination
- Value: provides velocity-dependent H2 data (3 occurrences of G3 at pp/mp/mf)
- Key finding: **~0.92 dB increase in H2/H1 per 1 dB increase in signal level**
- D3 anomaly: H3>H2 at one note (only instance across all sources). Likely polyphonic overlap artifact.

### 1.4 Cross-Source Summary

| Metric | OBM (reliable) | BiG (low) | Improv (low-moderate) | Consensus |
|--------|----------------|-----------|----------------------|-----------|
| H2>H3 | 13/13 (100%) | 3/4 (75%) | 6/8 (75%) | >90% expected on clean isolated notes |
| H2/H1 at mid register mf | -9 to -2 dB (raw) | -7 to -35 dB | -4 to -36 dB | Wide variance; -14 to -1 dB corrected |
| Decay rate model | 0.26*exp(0.049*MIDI) | Lower (softer dynamics?) | Consistent | Model valid +/-30% |
| Overshoot at mf | 2.0-6.9 dB | 0-5 dB | 2-10 dB | 2-5 dB at mf, 5-10 dB at ff |

---

## 2. What Makes a Wurlitzer Sound Like a Wurlitzer

### 2.1 The Timbral Identity (Perceptual Core)

The Wurlitzer 200A has a distinctive sound that musicians describe with consistent vocabulary across decades of commentary. These descriptions define the perceptual targets:

**Velocity-dependent character (THE defining feature):**
- **pp-p:** Sweet, bell-like, vibraphone-like. Nearly sinusoidal with gentle upper partials. Warm and pure.
- **mf:** Classic Wurlitzer "sweet spot." Warm with moderate bark. Rich midrange. The sound most people associate with the instrument.
- **f-ff:** Aggressive bark, growl, "reedy" distortion. Pronounced midrange saturation. The reed swings close to the pickup plate, and the preamp saturates harder.

This velocity-to-timbre continuum is the SINGLE MOST IMPORTANT characteristic. Every Wurlitzer user, forum post, and review mentions it. A model that nails this transition and nothing else would be more useful than one that matches every spectral metric but has a flat velocity response.

Sources: Tropical Fish (tropicalfishvintage.com), Sweetwater recording guide, Native Instruments blog, Vintage Vibe case studies, KVR Audio forums, Gearspace forums.

**Compared to Rhodes:**
- Wurlitzer: sharper, more harmonically abundant, closer to sawtooth wave character, "reedy" and "jangly"
- Rhodes: rounder, more bell-like, closer to sine wave character, "glassy" and "smooth"
- Both: velocity-dependent timbre, but Wurlitzer's distortion onset is more abrupt ("bark breakpoint")

Source: Wikipedia (Wurlitzer electronic piano), Abbey Road Institute blog, multiple forum discussions.

**The "bark breakpoint":**
The transition from clean to distorted is not gradual -- it has a knee around velocity 0.6 (MIDI ~76). Below this point, the sound warms up gently. Above it, harmonic distortion increases rapidly. This is because:
1. The reed's displacement approaches the pickup gap at higher velocities, increasing the 1/(1-y) nonlinearity (primary H2 source)
2. At extreme dynamics, the preamp also enters saturation, adding further H2 from its asymmetric headroom

The abruptness of this knee is what musicians call "bark." A model with a smooth, linear velocity-to-distortion curve will sound "flat" or "lifeless" even if the steady-state spectrum matches perfectly.

Sources: Tropical Fish ("How Does a Wurlitzer Work"), Vintage Vibe reeds case study, EP-Forum discussions.

### 2.2 The Five Pillars of Wurlitzer Identity

Ranked by perceptual importance (based on musician feedback, forum consensus, and expert evaluation):

| Rank | Pillar | Description | Measurable? |
|------|--------|-------------|-------------|
| 1 | **Velocity-bark transition** | Smooth pp-to-ff timbre curve with bark knee at ~vel 0.6 | Partially (H2/H1 vs velocity, but perceptual knee sharpness is hard to capture) |
| 2 | **Percussive attack** | Bright, sharp initial transient (2-15 ms) with mode overshoot that decays into darker sustain | Yes (overshoot dB, spectral centroid attack vs sustain) |
| 3 | **Even-harmonic warmth** | H2 stronger than H3 at all dynamics, giving "warm distortion" not "harsh clipping" | Yes (H2/H1, H2-H3 gap) |
| 4 | **Register variation** | Bass: woody thump. Mid: balanced warmth. Treble: crystalline ping. Each register has distinct character. | Partially (THD by register, spectral shape) |
| 5 | **Temporal evolution** | Attack bright, sustain warm, late sustain near-sinusoidal. Timbral darkening over time. | Yes (spectral centroid over time) |

Secondary characteristics (important but not identity-defining):
- Tremolo: choppy LDR character (asymmetric dips), rate 5.3-7 Hz, variable depth
- Release: progressive damping (not a gate), with timbral darkening
- Mechanical sounds: key clack, damper thump, reed rattle (add realism but absence is not identity-breaking)
- Inharmonicity: subtle pitch beating from non-integer mode ratios (affects attack texture, not sustain)

### 2.3 What a Wurlitzer Must NOT Sound Like

These are equally important negative targets:

| Must NOT sound like | Why it happens | How to detect |
|---------------------|----------------|---------------|
| Pure sine wave / organ | Insufficient harmonic content, flat velocity response | THD < -30 dB at mf is too clean |
| DX7 / FM synthesis | Phase-locked harmonic ratios, metallic quality, no inharmonicity | Listen for "plasticky" or "metallic" quality |
| Generic "electric piano" | Missing the bark/growl, too smooth velocity curve | No bark breakpoint; ff sounds like louder mf |
| Clavinet | Too bright, too percussive, no warmth in sustain | Spectral centroid too high in sustain (>2 kHz at mid register) |
| Rhodes | Too bell-like, too round, insufficient reedy quality | Missing the H2 dominance and midrange "edge" |
| Distorted guitar | Too many odd harmonics, harsh clipping | H3>H2, or THD > 0 dB at mf mid register |

---

## 3. Spectral Targets by Register and Velocity

### 3.1 Harmonic Content at mf (the primary target)

These are corrected values from OBM data, cross-validated with Improv source where possible:

| Register | MIDI Range | H2/H1 (dB) | H3/H1 (dB) | H2-H3 gap (dB) | THD (dB) |
|----------|-----------|-------------|-------------|-----------------|----------|
| Bass | 33-48 | -5 to +5 | -15 to -3 | +1 to +10 | -8 to +3 |
| Mid | 49-72 | -14 to -1 | -30 to -4 | +1 to +18 | -12 to +3 |
| Treble | 73-84 | -22 to -16 | -28 to -20 | +2 to +12 | -22 to -14 |
| Top | 85-96 | -30 to -20 | -45 to -25 | +2 to +25 | -30 to -17 |

**CRITICAL NOTE:** The wide ranges above are NOT measurement noise -- they represent real note-to-note variation within a single instrument. A physically accurate model SHOULD produce different H2/H1 at different notes within the same register, driven by differences in reed geometry, pickup gap, and preamp operating point.

### 3.2 Velocity-Dependent Harmonic Content

Based on Improv source (G3, MIDI 55) and extrapolated from physics:

| Dynamic | Signal Level (rel. to mf) | H2/H1 shift from mf | Expected Character |
|---------|--------------------------|---------------------|--------------------|
| pp (vel 20-40) | -8 to -4 dB | -5 to -8 dB weaker | Near-sinusoidal, sweet |
| p (vel 40-60) | -4 to -1 dB | -3 to -5 dB weaker | Clean with hint of warmth |
| mf (vel 60-80) | 0 dB (reference) | 0 dB (reference) | Warm bark, sweet spot |
| f (vel 80-100) | +1 to +4 dB | +2 to +5 dB stronger | Pronounced bark |
| ff (vel 100-127) | +4 to +8 dB | +5 to +8 dB stronger | Aggressive growl/bark |

**Velocity-H2 coupling:** ~0.92 dB increase in H2/H1 per 1 dB increase in signal level. This is the pickup's 1/(1-y) nonlinearity at work — larger displacement means more H2, and the preamp adds to it at higher levels.

**Non-negotiable:** H2 must exceed H3 at ALL velocities. If ff produces H3>H2, the saturation model is wrong (likely using symmetric function like tanh instead of asymmetric exponential).

### 3.3 What the Harmonic Spectrum Actually Looks Like

A typical Wurlitzer 200A note at mid register (C4, MIDI 60) at mf should have:

```
Harmonic:  H1(fund)  H2      H3      H4      H5      H6
Level:     0 dB      -11 dB  -28 dB  -35 dB  -42 dB  -48 dB
           (ref)     (strong) (weak)  (very weak) ...decreasing...
```

The spectrum is NOT a smooth rolloff. The H2 "bump" above the otherwise decreasing harmonic series is the signature of the pickup's 1/(1-y) nonlinearity (with additional contribution from the preamp at high dynamics). The spectral envelope should approximate:
- H2/H1 ~ -11 dB (H2 from pickup nonlinearity + preamp asymmetry)
- H_n/H1 ~ -7*n dB for n>=3 (approximately exponential rolloff)

At bass register, H2 is much closer to H1 (can even exceed it). At treble, H2 is 20-30 dB below H1. This register-dependent H2 slope is one of the hardest things to model accurately.

### 3.4 Inharmonicity

Reed mode ratios (from Euler-Bernoulli cantilever beam with tip mass correction):

| Mode | Ideal (uniform beam) | Bass (mu~0.05) | Mid (mu~0.15) | Treble (mu~0) |
|------|---------------------|-----------------|----------------|----------------|
| 1 | 1.000 | 1.000 | 1.000 | 1.000 |
| 2 | 6.267 | 6.3 | 6.8 | 6.3 |
| 3 | 17.55 | 17.9 | 19.5 | 17.6 |
| 4 | 34.39 | 35.4 | 39.0 | 34.5 |

> **Note:** The fixed per-register ratios above are approximate reference values. The deployed code computes ratios dynamically from `tip_mass_ratio(midi)` via eigenvalue interpolation of the cantilever-with-tip-mass characteristic equation. This produces a smooth, per-note ratio that varies continuously across the keyboard rather than using discrete register boundaries.

Sources: Pfeifle DAFx 2017, Euler-Bernoulli beam theory, characteristic equation numerical solutions.

**Perceptual importance: LOW for sustained sound, MODERATE for attack.** The inharmonic modes are 65-86 dB below the fundamental in the sustained signal. The prominent harmonics in a Wurlitzer spectrum fall on exact integer multiples of f0 -- these come from the preamp nonlinearity applied to the fundamental-dominant displacement, not from individual reed modes. The inharmonic modes matter primarily in the first 5-30 ms (attack transient) where all modes are at comparable amplitudes and their non-integer relationships create the characteristic "complex" attack texture.

---

## 4. Envelope Targets: Attack, Sustain, Decay, Release

### 4.1 Attack Transient (0-30 ms)

The attack is the most perceptually important phase. It establishes the instrument's identity in the first few milliseconds of each note.

**Three components:**

1. **Hammer noise burst (0-5 ms):** Broadband energy from felt-on-steel impact. Center frequency = `4 * f0` clamped [200, 2000] Hz, Q = 0.7, amplitude = `0.015 * vel^2`, decay tau = 3 ms, duration = 15 ms. Bandwidth is note-tracking (not velocity-dependent). Creates the initial "click" or "thump." Spectral flatness ratio (first 2ms vs 5-10ms) >= 1.0 when present. More prominent in mid register.

2. **Mode overshoot (2-15 ms):** All reed modes are at their initial amplitudes in the first few ms. Since upper modes decay faster, the initial sum is louder and brighter than the sustained (fundamental-dominant) signal. This creates a natural "overshoot" of 2-5 dB at mf, 5-10 dB at ff.

3. **Spectral brightening:** The attack's spectral centroid is higher than the sustain's (see centroid targets below). The ear perceives this as "percussiveness."

**Target values:**

| Dynamic | Overshoot (dB) | Peak Time (ms) | Attack Centroid (Hz) | Source |
|---------|---------------|----------------|---------------------|--------|
| pp | 1-3 | 15-45 | Similar to sustain | Extrapolated |
| mf | 2-5 | 10-35 | 600-1200 | OBM measured |
| ff | 5-10 | 5-25 | 800-2000 | Improv measured |

**CRITICAL:** The overshoot must emerge from physical modal superposition, NOT from an artificial multiplicative envelope. Previous project iteration used `(1 + X * exp(-alpha*t))` which produced correct overshoot dB but destroyed the natural spectral evolution. The attack sounds RIGHT when each mode has its own amplitude and decay, and the sum naturally produces overshoot because all modes start strong but upper modes die quickly.

### 4.2 Decay Rate

**Exponential model:** `decay_dB_per_sec = 0.26 * exp(0.049 * MIDI)` with +/- 30% tolerance.

| Register | MIDI Range | Decay Rate (dB/s) | Sustain Time (~-40 dB) |
|----------|-----------|-------------------|----------------------|
| Low bass | 33-42 | 0.5-2.0 | 20-80 s |
| Bass | 43-54 | 1.5-5.0 | 8-27 s |
| Mid-low | 55-66 | 4-8 | 5-10 s |
| Mid | 67-74 | 6-13 | 3-7 s |
| Treble | 75-84 | 9-25 | 1.6-4.4 s |
| Top | 85-96 | 15-35 | 1.1-2.7 s |

**Doubling interval:** Decay rate roughly doubles every 14 semitones (~1.2 octaves).

**Per-mode decay:** Higher modes decay faster than the fundamental. The deployed code uses a power-law model:
```
decay_rate_n = base_rate * (f_n / f_1) ^ 2.0
```
where `MODE_DECAY_EXPONENT = 2.0` (Zener ∝ ω²) and `MIN_DECAY_RATE = 3.0 dB/s` (bass floor). The fundamental base rate uses OBM-calibrated `0.005 * f^1.22`. For C4 (f2/f1 = 6.3): mode 2 decays 6.3^2.0 = 39.7x faster than the fundamental. This is far more aggressive than the old fixed array `[1.00, 0.55, 0.30, 0.18, 0.10, 0.06, 0.035]` which implied only 1.8x faster decay for mode 2. The power-law model produces the characteristic "bright attack darkening to sine-like tail" timbral evolution.

### 4.3 Timbral Darkening (Centroid Drift)

The spectral centroid should decrease from attack to sustain as upper modes decay:

| Register | Centroid at 10 ms (Hz) | Centroid at 300 ms (Hz) | Drift (Hz) |
|----------|----------------------|------------------------|-----------|
| Bass (33-48) | 600-1000 | 500-800 | -50 to -200 |
| Mid (49-72) | 600-1200 | 600-1000 | -30 to -240 |
| Treble (73-84) | 800-1600 | 800-1400 | -30 to -250 |
| Top (85-96) | unmeasurable reliably | noise floor | N/A |

**CRITICAL WARNING:** Previous target of "-200 to -1000 Hz" centroid drift was WRONG (4x too aggressive). The -1000 Hz figure was a noise-floor artifact from measuring late centroids where the signal had decayed below the noise floor. Real attack-to-sustain (not attack-to-noise) drift is -30 to -240 Hz maximum.

**Measurement protocol:** Use 50ms Hanning windows at 10ms, 50ms, 100ms, 300ms, 500ms. Only use attack (10ms) to sustain (300ms) comparison. Late centroid (>1s) is unreliable for treble notes.

### 4.4 Release (Damper)

The Wurlitzer has felt dampers on all keys except the top 5 (MIDI >= 92). Release behavior:

| Phase | Time | What Happens | Perceptual Effect |
|-------|------|--------------|--------------------|
| 1 | 0-5 ms | Upper modes killed instantly | Rapid timbral darkening |
| 2 | 5-30 ms | Progressive fundamental damping | "Thump" as damper contacts reed |
| 3 | 30-80 ms (bass) | Residual fundamental ring | Sustain pedal feel if hold is brief |

**Register-dependent release times:**
- Bass: 40-80 ms total
- Mid: 20-40 ms total
- Treble: 8-15 ms total
- Top 5 keys: no damper, natural decay only

**Non-negotiable:** Release must NOT sound like an amplitude gate. It must show progressive timbral darkening (higher modes die first, fundamental lingers briefly). An amplitude gate that cuts all frequencies equally sounds artificial and is immediately audible.

---

## 5. Perceptual Evaluation Criteria

### 5.1 The Hierarchy of What the Ear Cares About

Based on musician feedback (KVR Audio forums, Gearspace, VI-Control, AudioThing Wurly reviews, Lounge Lizard EP-5 reviews, Keyscape discussions) and expert evaluations:

| Priority | What the Ear Cares About | What FFT Shows | Correlation |
|----------|--------------------------|----------------|-------------|
| 1 | Velocity-to-bark transition smoothness | H2/H1 vs velocity curve | MODERATE -- H2/H1 correlates with perceived bark, but the SHARPNESS of the transition (the knee) is not captured by steady-state spectrum |
| 2 | Attack "feel" -- percussive but not clicky | Overshoot dB, spectral flatness | LOW -- overshoot dB can be correct but still sound wrong if temporal shape is unnatural |
| 3 | Warmth vs harshness of distortion | H2/H3 ratio, THD | MODERATE -- H2>H3 is necessary for warmth, but the shape of the nonlinearity matters more than the ratio |
| 4 | Register variation (bass/mid/treble character) | Spectral envelope by register | MODERATE -- overall spectral shape correlates, but timbral "character" is multi-dimensional |
| 5 | Temporal evolution (bright attack -> warm sustain) | Centroid drift | HIGH -- centroid drift is one of the better metrics for this |
| 6 | Polyphonic interaction (chord compression) | Signal level at chord vs single note | LOW -- hard to measure, but chords should compress slightly and get slightly "barky" |
| 7 | Release naturalness | Damper envelope shape | LOW -- hard to quantify, best evaluated by listening |

### 5.2 Musician Priority Stack

From the design evaluation document (musician evaluator):

| Priority | Feature | Quote/Rationale |
|----------|---------|-----------------|
| 1 (critical) | Smooth velocity-to-bark transition | "If it nails this and nothing else, I would still open it regularly" |
| 2 | No velocity layer boundaries | Single biggest advantage over sample-based plugins |
| 3 | Latency < 10 ms total | Above 10 ms feels sluggish for fast playing |
| 4 | No audible artifacts | One click/pop during a take -> reach for different plugin |
| 5 | True half-damping | Something Keyscape cannot do at all |
| 6 | Correct tremolo (choppy LDR) | Most plugins get this wrong; immediately obvious |
| 7 | Stable polyphony (6+ notes + melody) | Graceful voice stealing, no audible glitches |
| 8 | Free + open source + Linux | Real value proposition |

### 5.3 What Technical People Miss

| Characteristic | What It Is | Why It Matters |
|----------------|-----------|----------------|
| "Bloom" (30-80 ms) | Timbral shift as attack transient dies and sustained harmonics establish | Players feel this as "responsiveness" -- too fast bloom = machine-like, too slow = sluggish |
| Chord preamp interaction | ff chord drives preamp harder than any single note | Bark from a 6-note ff chord != sum of individual bark. Must compress and "snarl" as a unit |
| Release varies with hold duration | 100 ms release has richer damper thud than 2 s release | Because modes are still present at 100 ms but not at 2 s |
| Tuning imperfection | Real Wurlitzers rarely in perfect 12-TET | +/- 3 cents frequency detuning (factory spec per US Patent 2,919,616), +/- 8% amplitude variation per mode, seeded deterministically by note number |
| Mechanical action noise | Key-bed thud, adjacent reed rattle, damper clunk | Low priority for core sound, but absence noticed by experienced players |

---

## 6. Metrics That Mislead vs Metrics That Matter

### 6.1 GOOD Metrics (correlate with perceptual quality)

| Metric | What It Measures | Why It Correlates | Caveat |
|--------|-----------------|-------------------|--------|
| H2>H3 compliance | Even harmonic dominance | Directly audible as warm vs harsh distortion | Must be checked at ALL velocities, not just mf |
| H2/H1 vs velocity slope | Bark-velocity coupling strength | ~0.92 dB/dB is the target; too flat = lifeless, too steep = harsh | Steady-state only; says nothing about the attack |
| Spectral centroid drift (atk->sus) | Timbral darkening | Directly audible as "percussive bell -> warm sustain" | Only valid for atk-to-300ms, not late sustain |
| Decay rate by register | Sustain envelope shape | Wrong decay is immediately audible (too long = organ, too short = pluck) | Must check MULTIPLE registers, not just C4 |
| Dynamic range (pp to ff dBFS) | Velocity response range | Should be 20-30 dB; < 10 dB means velocity compression | Must be measured POST-preamp including all gain stages |

### 6.2 BAD Metrics (can pass while sounding terrible)

| Metric | Why It Misleads | What Actually Went Wrong |
|--------|----------------|--------------------------|
| **H2/H1 at a single note/velocity** | Can be correct at mf C4 while being wrong everywhere else | Previous iteration nailed C4 mf H2/H1 = -16.2 dB (target -11.3+/-5) but had near-zero register variation |
| **THD at a single dynamic** | THD can be in range while the harmonic SHAPE is completely wrong | Symmetric nonlinearity (tanh) produces correct THD but wrong H2/H3 ratio |
| **Overshoot dB (measured as amplitude ratio)** | Can hit 2-5 dB by artificially inflating ALL modes uniformly | Previous iteration's `(1+X*exp(-at))` envelope hit target dB but sounded like a volume swell, not a percussive attack |
| **Spectral centroid at a single time point** | Centroid at 200ms can be correct while the temporal TRAJECTORY is wrong | A model with flat centroid at the right value sounds wrong vs one that starts high and drifts down to the same value |
| **FAD (Frechet Audio Distance)** | Distribution-level metric; insensitive to temporal structure within notes | FAD correlation with perceptual quality is only ~0.52 (moderate). Research shows VGGish-based FAD struggles with musical audio quality prediction (Kilgour et al. 2019, EUSIPCO 2024). |
| **MCD (Mel Cepstral Distortion)** | Weak negative correlation with naturalness (Spearman: -0.31) | Was designed for speech, not musical instrument timbral fidelity |
| **Pass/fail on non-negotiable list** | Can pass all 5 items while sounding dead | Non-negotiable list checks necessary conditions, not sufficient conditions |

### 6.3 The "Sounds Awful Despite Passing Metrics" Failure Mode

This happened in Round 40 of this project. Root cause analysis:

**What passed:**
- H2>H3: 10/10 notes PASS
- C4 mf H2/H1: -16.2 dB (within -11.3 +/- 5 target) PASS
- Decay rate increasing with register PASS
- Zero clipping PASS

**What was wrong (and why metrics did not catch it):**
1. **Attack destroyed** (0.8 dB overshoot, target 2-5 dB): The sinc dwell filter required 20x mode amplitude compensation, which raised SUSTAINED energy but not ATTACK energy. The overshoot metric showed low, but the deeper problem was that the attack had no percussive character at all -- it sounded like a slow fade-in.

2. **Dynamic range crushed** (1.3 dB mf-to-ff): The preamp input drive was so high that everything saturated at mf. Metrics checked H2/H1 at mf and it was fine, but ff sounded identical to mf because both were fully saturated.

3. **Register spread flat** (0.2 dB variation across 5 octaves): Preamp compression flattened everything. H2/H1 was checked per-note and each was within range, but the VARIATION across notes was gone.

4. **Temporal shape wrong**: The overshoot was artificial (multiplicative envelope on all modes) instead of natural (modal superposition). Even if the dB measurement matched, the temporal SHAPE of the attack was audibly wrong.

**Lesson:** Metrics must be evaluated COLLECTIVELY, not individually. A model that passes every individual metric but has zero dynamic range or zero register variation will sound terrible. The evaluation framework must include cross-register and cross-velocity comparisons, not just per-note checks.

---

## 7. Reference Recordings and What to Listen For

### 7.1 Isolated Note References

| Source | Type | Quality | Location | Use For |
|--------|------|---------|----------|---------|
| OldBassMan 200A (Freesound 5726) | 13 isolated notes | 48kHz/24bit, Lundahl+tube chain | [Freesound #5726](https://freesound.org/people/oldbassman/packs/91/) | Harmonic ratios, decay rates, spectral evolution. PRIMARY calibration source. |
| Unplugged 200A (Pianobook) | 3 dynamic layers x 2 RR | 48kHz/24bit stereo | Pianobook free download | Multi-velocity comparison, attack character |
| VReeds (Acoustic Samples) | Multi-velocity DI + speaker | Commercial | acousticsamples.net | DI recording reference (no recording chain coloration) |
| PM-200 (Puremagnetik) | Hi-res multi-velocity | Commercial | puremagnetik.com | Detailed velocity layering reference |
| Keyboard Waves 200A | Multi-velocity samples | Commercial | keyboardwaves.com | Through original preamp/components |

### 7.2 Musical References (What the Instrument Should Sound Like in Context)

These recordings demonstrate the Wurlitzer 200A's characteristic sound in musical context. Each highlights different aspects of the instrument's identity.

**Supertramp -- "Dreamer" (1974)**
- What to listen for: Bell-like clean tones in verses, aggressive bark in chorus when played harder. The velocity transition is clearly audible. Mid-register focused.
- Register: primarily mid (C4-C6)
- Dynamic range: pp in verses to f in chorus
- Notable: EQ boost around 1.2-1.5 kHz enhances the midrange character
- Why it matters: Demonstrates the velocity-to-bark transition that is the Wurlitzer's defining feature
- Source: Crime of the Century (1974)

**Supertramp -- "Goodbye Stranger" (1979)**
- What to listen for: Rhythmic "grinding" bass figure with moderate bark. Tremolo clearly audible. Bass-to-mid register.
- Why it matters: Shows the bass register character and rhythmic playing style
- Source: Breakfast in America (1979)

**Steely Dan -- "Do It Again" (1972)**
- What to listen for: Clean-to-barky mid register. Very dry recording (good spectral reference). Moderate dynamics.
- Why it matters: Minimal processing reveals the instrument's natural character
- Source: Can't Buy a Thrill (1972)

**Donny Hathaway -- "A Song for You" / "Little Ghetto Boy"**
- What to listen for: Expressive velocity control from pp gospel voicings to ff emotional peaks. Full dynamic range demonstrated.
- Why it matters: Hathaway exploited the full velocity-timbre range. The model must handle both extremes convincingly.

**Pink Floyd -- The Dark Side of the Moon / Wish You Were Here era (1973-1975)**
- What to listen for: Rick Wright's Wurlitzer with moderate tremolo. Mid-register warmth.
- Why it matters: Demonstrates the instrument in a studio context with effects

**Cannonball Adderley -- "Mercy, Mercy, Mercy" (1966, Joe Zawinul)**
- What to listen for: Clean, warm tone with subtle bark on accents. Jazz voicings demonstrating polyphonic character.
- Why it matters: Shows polyphonic interaction at moderate dynamics

**Ray Charles -- "What'd I Say" (1959)**
- What to listen for: Early Wurlitzer model (likely 120/140, not 200A). Rich chords with organic warmth.
- Why it matters: Establishes the fundamental timbral character of the electrostatic reed piano family
- Note: Different model than 200A; use for general character reference, not spectral calibration

### 7.3 Reference Recording Chain Corrections

When comparing model output to reference recordings:

| Chain Component | Effect on H2 | Effect on Attack | Effect on Spectrum |
|-----------------|-------------|------------------|-------------------|
| Lundahl transformer | +1-3 dB H2 (iron saturation) | Minimal | Roll-off above 15-20 kHz |
| Tube preamp | +1-3 dB H2 (triode asymmetry) | Minimal | Slight warmth |
| YouTube codec (AAC/Opus) | Unpredictable | -1-2 dB overshoot (transient smearing) | -3-5 dB above 10 kHz |
| Room microphone | Coloration | Smears transients | Room modes add peaks/nulls |
| DI recording | Neutral (reference) | Clean transients | Flat response |

A DI-recorded model output should have:
- H2 about 2-4 dB lower than Lundahl+tube recordings
- Sharper transients than any recording (no chain smearing)
- Cleaner decay curves than mic recordings

---

## 8. Existing Plugin Benchmarks

### 8.1 What the Best Plugins Get Right and Wrong

| Plugin | Approach | Strengths | Weaknesses | Relevance to Our Model |
|--------|----------|-----------|------------|----------------------|
| **Keyscape** (Spectrasonics, $399) | Deep multisampling | Gold standard for "sounds like a 200A." Multiple velocity layers, round robins. | 80 GB disk. Layer transitions perceptible under scrutiny. No half-damping. No parameter tweakability. | Sets the perceptual target. If our model sounds as good as Keyscape to non-expert listeners, it is a success. |
| **AudioThing Wurly** ($69) | Hybrid: physical modeling + samples | "Really does nail it" (20-year EP tech). Sharp transients in polyphonic. Linux support. | Newer (June 2024), less track record. | Validates hybrid approach. Their success with physical+sample hybrid is architecturally similar to our physical+ML approach. |
| **Arturia Wurli V3** ($149) | Pure physical modeling (TAE/Phi) | Low CPU, adjustable parameters, 11 stompboxes. | Velocity extremes less convincing. Bass overpowers treble. "Doesn't sound like a Wurli" (some users). | Proves pure physical modeling gets ~85% there. The remaining 15% is the target for ML correction. |
| **Lounge Lizard EP-5** (AAS, $199) | Physical modeling (dual engine) | Dedicated reed engine. 390+ presets. MPE. "Super smooth dynamics." 5/5 Sound on Sound review. | Wurli model weaker than Rhodes model. "Lacks mechanical sounds and rough edges." | Shows that physical modeling Wurlitzer is harder than Rhodes. The capacitive pickup is the hard part. |
| **Pianoteq** ($149+ base) | Physical modeling | <50 MB. Infinite tweakability. Zero latency. | Wurli model "doesn't sound right" out of box. Missing the characteristic "cough." | Demonstrates the limits of generic physical modeling without instrument-specific calibration. |
| **Sampleson Reed200** ($50-70) | Spectral modeling (600 sine waves) | 30 MB. "Sounds like a million bucks" (MusicRadar). | Less established. | Validates spectral/additive approach as computationally efficient path to good Wurlitzer tone. |
| **Cherry Audio Wurlybird** ($39) | Deep multisampling | "Incredibly authentic at base-sample level." | 140B only (not 200/200A). | Shows sampling + engineering at low price can compete. |
| **GSi MrTramp 2** (Free) | Physical modeling | Best free dedicated Wurli. 200A model. Hammer/damper noise. | Possibly 32-bit only. | Free/open competition benchmark. |

### 8.2 Confidence Ratings (from design evaluation)

| Comparison | Confidence | Notes |
|------------|-----------|-------|
| Produces high-quality plugin | 7/10 | Architecture is sound, execution is the challenge |
| Competes with Arturia Wurli V | 8/10 | Pure physical modeling comparison; our preamp model is more physically accurate |
| Competes with AudioThing Wurly | 4/10 | They have real samples for mechanical sounds; we have ML correction but no real samples |
| Competes with Keyscape | 2/10 | Keyscape has thousands of carefully edited multisampled recordings; we cannot match this with synthesis alone |

### 8.3 What No Plugin Gets Right

Based on forum discussions and expert assessments:

1. **The bark breakpoint knee:** Every physical modeler smooths this out. Sample-based plugins have it in the samples but with layer boundaries.
2. **Polyphonic preamp interaction at ff:** Chords should compress and "snarl" differently than the sum of individual notes. Most models apply effects per-note, not post-mix.
3. **Unit-to-unit variation:** No plugin models the variation between individual instruments.
4. **True half-damping:** Physical models can do this in principle, but none implement it convincingly for Wurlitzer specifically.

---

## 9. Unit-to-Unit Variation and What We Are Modeling

### 9.1 How Much Variation Exists?

Wurlitzer 200A instruments vary significantly from unit to unit:

**Reed-level variation:**
- Four basic reed manufacturing periods with changes every couple of years
- 200A reeds have thicker plate than 200 reeds -> "fuller tone, mellow overtone" vs "long dwell, sharp attack"
- Individual reed tuning by adding/removing solder -> different mass ratios per reed
- Reed aging: fatigue changes stiffness and damping properties over decades
- Vintage Vibe notes that reeds which "die out too soon" are replaced, implying significant reed-to-reed decay variation
- Sources: DocWurly, Vintage Vibe case study, EP-Forum

**Preamp variation:**
- Originally 2N2924 transistors (hFE 150-300), later replaced with 2N5089 (hFE >= 450)
- Component aging changes bias points, gain, and distortion character
- 200A preamp on separate PCB (lower noise) vs 200 preamp on same board as power amp (more hum/character)
- Sources: Busted Gear transistor specs, GroupDIY preamp thread

**Pickup variation:**
- Pickup gap varies with setup: factory spec vs technician adjustment vs accumulated drift
- Gap affects both sensitivity and nonlinearity threshold
- Sources: EP-Forum reed dimensions, Vintage Vibe service documentation

**Amplifier/speaker variation:**
- 200A went through multiple speaker transitions: Alnico -> square ceramic -> round ceramic
- Alnico speakers: smoother treble response, preferred by many players
- Ceramic speakers: brighter, more articulate
- Sources: Chicago Electric Piano, Tropical Fish, DocWurly

**Estimated total variation between well-maintained instruments:**
- H2/H1: +/- 5-8 dB at the same note and velocity
- Decay rate: +/- 30-50%
- Attack overshoot: +/- 3 dB
- Bark breakpoint velocity: +/- 0.1 (normalized velocity, i.e., bark onset can be vel 0.5-0.7)
- Tremolo rate: 5.3-7.0 Hz (fixed per unit, measured on multiple instruments)

### 9.2 What Are We Modeling?

**Target: A well-maintained, correctly set up Wurlitzer 200A with typical late-1970s components.**

This means:
- 2N5089 transistors (later replacement, higher hFE)
- +15V DC supply (confirmed from measurements)
- Ceramic speakers (200A standard)
- Moderate pickup gap (factory nominal)
- All reeds in good condition (no dead reeds, no excessively short decay)

We are NOT modeling:
- A specific individual instrument (we lack sufficient data)
- A worn/aged instrument with crossover distortion or bias drift
- The 200 (non-A) with its bias-shifting tremolo and different amplifier
- The 140B or earlier models

**Per-note variation within our model:** Should include +/- 0.8% frequency variation on modes 2+ and +/- 8% amplitude variation per mode, seeded deterministically by note number. This provides the "organic" quality of a real instrument without requiring per-note sample data.

---

## 10. Objective Metric Recommendations

### 10.1 Tier 1: Must-Pass (Necessary Conditions)

These are non-negotiable. Failing any one means the model is fundamentally wrong:

| Metric | Target | How to Measure | Rationale |
|--------|--------|----------------|-----------|
| H2>H3 on every note at mf | 100% compliance | Goertzel or FFT on 150-800ms window | Preamp asymmetry signature. If H3>H2, the nonlinearity is wrong. |
| H2>H3 on every note at ff | 100% compliance | Same, at vel=120 | Must hold at all dynamics, not just mf |
| Decay rate increases with pitch | R > 0.7 (correlation coefficient) | Measure at 8+ notes across range | Basic physics of shorter/stiffer treble reeds |
| Dynamic range >= 15 dB (pp to ff) | Measure dBFS at vel=30 vs vel=120 | At C4 (MIDI 60) | <10 dB means velocity compression is destroying expression |
| No clipping at mf single notes | 0 clipped samples | Check for |sample| > 0.99 | Basic gain staging |
| Attack overshoot >= 1 dB at mf | Measure peak-to-sustain ratio | 0-10ms peak vs 100-200ms RMS | Zero overshoot means no percussive character |

### 10.2 Tier 2: Should-Match (Quality Indicators)

These differentiate a good model from a mediocre one:

| Metric | Target | How to Measure | Weight |
|--------|--------|----------------|--------|
| H2/H1 matches linear model +/- 5 dB | -0.48*MIDI + 17.5 | At 6+ notes across range at mf | HIGH |
| Decay rate matches exponential model +/- 30% | 0.26*exp(0.049*MIDI) | At 6+ notes across range | HIGH |
| Register spread >= 5 dB | max(dBFS) - min(dBFS) across 8+ notes | At mf | HIGH |
| Centroid drift negative for bass/mid | atk_centroid > sus_centroid | Measure at 10ms and 300ms | MODERATE |
| Overshoot 2-5 dB at mf, 5-10 dB at ff | Peak-to-sustain ratio | At 3+ velocities | MODERATE |
| H2 velocity coupling >= 5 dB | H2(ff) - H2(pp) | At same note, vel=30 vs vel=120 | HIGH |

### 10.3 Tier 3: Nice-to-Have (Refinement Indicators)

| Metric | Target | Notes |
|--------|--------|-------|
| Multi-scale spectral loss < threshold | Compare to OBM recordings | Domain-dependent; must be calibrated empirically |
| Spectral centroid trajectory shape | Monotonically decreasing for bass/mid | Shape matters more than absolute value |
| Release timbral darkening | Centroid drops during release | Distinguishes progressive damper from amplitude gate |
| Polyphonic level compression | 6-note chord level < 6x single note level | Preamp saturation should compress chords |
| Per-note H2/H1 variation | +/- 2-3 dB across notes in same register | Organic quality from per-note reed variation |

### 10.4 Recommended Evaluation Protocol

**Automated test suite (run after every parameter change):**

```
1. Render single notes at 3 velocities (pp=30, mf=80, ff=120) across 8 registers
   Notes: C2(36), F2(41), C3(48), C4(60), F4(65), C5(72), C6(84), A6(93)

2. For each note, measure:
   - H2/H1, H3/H1, H2-H3 gap, THD
   - Decay rate (200ms to 1500ms)
   - Attack overshoot (0-10ms peak vs 100-200ms RMS)
   - Spectral centroid at 10ms, 100ms, 300ms

3. Cross-note comparisons:
   - H2>H3 compliance (target: 100%)
   - Register spread in dBFS (target: >= 5 dB)
   - Decay rate vs MIDI correlation (target: R > 0.7)
   - H2/H1 slope vs MIDI (target: approximately -0.48 dB/semitone)

4. Cross-velocity comparisons:
   - Dynamic range pp-to-ff (target: >= 15 dB)
   - H2 velocity coupling (target: >= 5 dB pp-to-ff)
   - Overshoot velocity scaling (target: ff overshoot >= 2x pp overshoot)

5. Polyphonic test:
   - 6-note ff chord: no clipping, perceptible compression
   - 2-note mf interval: clean, no intermodulation artifacts
```

**Listening evaluation (periodic, not automated):**

```
1. Play chromatic scale pp, mf, ff: Does bark onset feel natural? Is there a knee?
2. Play sustained mid-register chord (C-E-G): Does it breathe? Does timbre evolve?
3. Play staccato bass notes ff: Is there a "thump"? Does it decay naturally?
4. Play legato treble melody mp: Is it bell-like? Does it shimmer?
5. Compare to OBM recording: Does it sound like the same instrument family?
6. A/B with Keyscape (if available): What are the biggest perceptual differences?
```

---

## 11. Known Pitfalls: Passing Metrics While Sounding Terrible

### 11.1 Pitfall: Correct H2/H1 at Wrong Operating Point

**What happens:** You can achieve any H2/H1 ratio by adjusting gain and asymmetry independently. But if the preamp is operating at the wrong signal level (e.g., driven too hard so everything saturates identically), H2/H1 at mf may be correct while the velocity response is completely wrong.

**How to detect:** Check H2/H1 at pp AND ff. If the difference is < 3 dB, the preamp is either too linear (not saturating at ff) or too saturated (already clipping at pp).

### 11.2 Pitfall: Artificial Overshoot Envelope

**What happens:** Adding `(1 + X * exp(-alpha*t))` to the amplitude envelope produces correct overshoot dB measurements but sounds like a volume swell followed by a fade, not a percussive attack with timbral evolution.

**How to detect:** Listen for SPECTRAL change during the attack, not just AMPLITUDE change. Natural overshoot has bright modes dying quickly, leaving a warmer sustain. Artificial overshoot inflates all modes equally, so the spectrum stays flat during the attack.

**Metric to add:** Spectral centroid at 5ms vs 20ms should show a decrease. If centroid is flat, overshoot is artificial.

### 11.3 Pitfall: Symmetric Nonlinearity Producing Correct THD

**What happens:** `tanh(x)` produces reasonable THD levels but generates only odd harmonics (H3, H5, H7). The ear perceives this as "harsh" or "buzzy" rather than "warm" or "barky."

**How to detect:** Always check H2/H3 ratio, never just THD. If H3 > H2 at any velocity, the nonlinearity is symmetric and must be replaced with an asymmetric function.

### 11.4 Pitfall: Compressed Dynamic Range With Correct Per-Velocity Metrics

**What happens:** If mf and ff produce nearly identical output levels (because the preamp saturates at mf), per-velocity metric checks may pass (each velocity has correct H2/H1 for that velocity) while the instrument sounds completely unresponsive.

**How to detect:** Measure absolute dBFS at pp vs ff. Target: >= 15 dB dynamic range. The previous iteration had 1.3 dB mf-to-ff range -- immediately audible as "dead."

### 11.5 Pitfall: Flat Register Variation With Correct Per-Register Metrics

**What happens:** Heavy preamp compression flattens level differences between registers. Each individual note may have metrics within its register's target range, but all notes sound the same loudness and timbre.

**How to detect:** Compare dBFS across the full range at mf. Bass should be louder than treble (before speaker rolloff). Register spread should be >= 5 dB across the keyboard. Previous iteration had 0.2 dB.

### 11.6 Pitfall: Correct Spectrum at Wrong Temporal Resolution

**What happens:** Spectral analysis over 150-800ms averages out all temporal evolution. A note that has correct AVERAGE spectrum but wrong temporal SHAPE will have correct metrics but wrong sound.

**How to detect:** Measure spectra at MULTIPLE time points (5ms, 20ms, 50ms, 100ms, 300ms, 1s). The spectrum should evolve: bright at 5ms, progressively darker through 1s.

---

## 12. Key Questions Answered

### Q1: Is H2>H3 Really Universal for Wurlitzer 200A?

**Answer: Yes, with high confidence, for clean isolated notes at mf and above.**

Evidence: 13/13 (100%) compliance in the OBM source, which is the most reliable data (isolated notes, high-quality recording). Cross-validated with 75% compliance in two lower-quality sources (BiG and Improv), where the non-compliant cases are attributable to polyphonic overlap contamination or very soft dynamics.

**Caveat:** At pp dynamics, where the preamp is operating nearly linearly, H2 and H3 may be comparable or H2 may be only marginally above H3. The asymmetric nonlinearity that generates H2 is signal-level-dependent -- at very low levels, the preamp is nearly linear and produces little harmonic content of any kind. This is physically correct and does not invalidate the rule -- at pp, the entire harmonic content is so low (-30 to -50 dB) that whether H2 or H3 is marginally larger is perceptually irrelevant.

**Does it depend on the specific unit's setup?** The H2>H3 ordering should hold for any functioning Wurlitzer 200A because it is a property of the exponential BJT transfer function, which is inherently asymmetric. A unit with reversed polarity on a capacitor, or a significantly different transistor substitution, might behave differently, but these would be malfunction states.

### Q2: What Are the Perceptually Most Important Spectral Features?

**Answer: In order of importance:**

1. **H2/H1 ratio and its velocity dependence** -- this IS the Wurlitzer sound. The even-harmonic warmth that increases with velocity is the defining spectral feature.
2. **Spectral centroid trajectory** (bright attack -> warm sustain) -- more important than any single harmonic measurement.
3. **Overall spectral slope** (how quickly higher harmonics roll off) -- determines whether the sound is "warm" or "bright."
4. **Inharmonicity in the attack** -- subtle but contributes to the "complex" transient character.

### Q3: How Important Is the Attack Transient vs. the Sustain Character?

**Answer: The attack is more important for instrument recognition; the sustain is more important for musical quality.**

Research on piano synthesis (DDSP-Piano HAL 2023, Sines-Transient-Noise Frontiers 2025, Hybrid Architecture ScienceDirect 2026) consistently finds that the attack phase is the hardest to model accurately and the most perceptually salient for instrument identification. Listeners can identify instruments from attack transients alone in under 50 ms. Notably, the STN model achieved perceptual accuracy for sustain and trichords but had limitations in the attack phase.

However, for a PLAYABLE instrument (not just a recognizable one), the sustain character determines whether musicians want to keep playing it. A model with perfect attacks but dead sustains will be recognized as "Wurlitzer" but rejected as "lifeless."

For this project: prioritize getting the attack RIGHT (natural modal superposition, not artificial envelope), then ensure the sustain has correct timbral evolution and decay rate.

### Q4: What Does a Typical Wurlitzer Frequency Spectrum Look Like?

**Answer: See Section 3.3 for detailed spectral template.** Key features:
- H1 dominant (0 dB reference)
- H2 elevated above the otherwise decreasing harmonic series (-1 to -15 dB depending on register)
- H3 and above decreasing approximately exponentially
- Register-dependent: bass has strong H2 (near H1 level), treble has weak H2 (-20 to -30 dB)
- Velocity-dependent: ff has stronger H2 than pp by ~8 dB at the same note

### Q5: What Are the Biggest Perceptual Differences Between Real Wurlitzer and Current Digital Models?

**Answer: Based on forum consensus and expert evaluation:**

1. **Velocity-bark transition:** Real instruments have an abrupt "knee" where bark kicks in. Most models smooth this out.
2. **Attack complexity:** Real instruments have a multi-component attack (hammer noise + modal superposition). Models often simplify to a single envelope.
3. **Polyphonic preamp interaction:** Real instruments compress and distort differently when multiple notes hit the preamp simultaneously. Most models process notes independently.
4. **"Organic" quality from unit variation:** Real instruments have per-note personality from reed/pickup differences. Models are too uniform.
5. **Mechanical sounds:** Key clack, damper thump, reed rattle add realism. Most physical models lack these entirely.

### Q6: How Much Unit-to-Unit Variation Exists?

**Answer: Substantial. See Section 9.1 for detailed breakdown.**

Summary: +/- 5-8 dB H2/H1 variation, +/- 30-50% decay rate variation, +/- 3 dB overshoot variation between well-maintained instruments. This is driven by reed manufacturing period, transistor type, component aging, setup/maintenance, and speaker type.

**Recommendation: Model a "generic well-maintained late-1970s 200A" rather than a specific unit.** We lack sufficient data to accurately model any specific unit, and the generic target encompasses the range of "sounds like a 200A" that musicians recognize. The ML correction layer can later be trained on specific instrument recordings to capture unit-specific character.

---

## 13. Sources and References

### Academic Papers

- Pfeifle, F. (2017). "Real-Time Physical Model of a Wurlitzer and Rhodes Electric Piano." DAFx-17. https://www.dafx.de/paper-archive/2017/papers/DAFx17_paper_79.pdf
- Pfeifle, F. & Bader, R. (2016). "Tone Production of the Wurlitzer and Rhodes E-Pianos." Springer.
- arXiv 2407.17250: "Reduction of Nonlinear Distortion in Condenser Microphones." https://arxiv.org/html/2407.17250v1
- Frontiers (2023). "Physics-informed differentiable method for piano modeling." https://www.frontiersin.org/articles/10.3389/frsip.2023.1276748/full
- ScienceDirect (2026). "A hybrid architecture combining physical modeling and neural networks for piano sound synthesis." https://www.sciencedirect.com/science/article/pii/S2772941925002546
- Frontiers (2025). "Sines, transient, noise neural modeling of piano notes." https://www.frontiersin.org/articles/10.3389/frsip.2024.1494864/full
- HAL (2023). "DDSP-Piano: A Neural Sound Synthesizer Informed by Instrument Knowledge." https://hal.science/hal-04073770
- EUSIPCO (2024). "Correlation of Frechet Audio Distance With Human Perception." https://eurasip.org/Proceedings/Eusipco/Eusipco2024/pdfs/0000056.pdf
- Torcoli et al. (2021). "Objective Measures of Perceptual Audio Quality Reviewed." https://arxiv.org/pdf/2110.11438
- Microsoft (2024). "Adapting Frechet Audio Distance for Generative Music Evaluation." https://arxiv.org/abs/2311.01616
- Kilgour et al. (2019). "Frechet Audio Distance: A Reference-free Metric for Evaluating Music." INTERSPEECH.
- HAL (2024). "Power-balanced Vactrol Modeling." https://hal.science/hal-04452215/document

### Technical Resources

- GroupDIY: Wurlitzer 200A Preamp thread. https://groupdiy.com/threads/wurlitzer-200a-preamp.44606/
- Busted Gear: 200A Transistor Specs. https://www.bustedgear.com/res_Wurlitzer_200A_transistors.html
- Tropical Fish: How Does a Wurlitzer Electronic Piano Work. https://www.tropicalfishvintage.com/blog/2019/5/27/how-does-a-wurlitzer-electronic-piano-work
- Tropical Fish: 200 vs 200A. https://www.tropicalfishvintage.com/blog/2019/5/27/what-is-the-difference-between-a-wurlitzer-200-and-a-wurlitzer-200a
- Chicago Electric Piano: Wurlitzer 200 vs 200A. https://chicagoelectricpiano.com/wurlitzer/wurlitzer-200-vs-200a/
- DocWurly: Wurlitzer Electric Piano Models. https://docwurly.com/wurlitzer-ep-history/wurlitzer-electric-piano-models-a-list/
- Vintage Vibe: Wurlitzer Electric Piano Reeds Case Study. https://www.vintagevibe.com/blogs/news/wurlitzer-electric-piano-reeds-case-study
- EP-Forum: Wurlitzer 200 Reed Dimensions. https://ep-forum.com/smf/index.php?topic=8418.0
- EP-Forum: Wurlitzer Tremolo Rate. https://ep-forum.com/smf/index.php?topic=4412.0
- Strymon: Amplifier Tremolo Technology White Paper. https://www.strymon.net/amplifier-tremolo-technology-white-paper/
- SignalWires: Wurlitzer 200A Schematic Analysis. https://signalwires.com/wurlitzer-200a-schematic

### Schematics

- 200 Series: https://www.bustedgear.com/images/schematics/Wurlitzer_200_series_schematics.pdf
- 200A Series: https://www.bustedgear.com/images/schematics/Wurlitzer_200A_series_schematics.pdf

### Recording/Sample References

- OldBassMan Wurlitzer 200A (Freesound pack 5726, CC-BY 4.0). https://freesound.org/people/OldBassMan/packs/5726/
- Unplugged 200A (Pianobook, free). https://www.pianobook.co.uk/packs/unplugged-200a/
- VReeds (Acoustic Samples). https://www.acousticsamples.net/vreeds
- PM-200 (Puremagnetik). https://puremagnetik.com/products/pm-200-wurlitzer-piano-ableton-live-pack-kontakt-instrument-apple-logic-samples
- Keyboard Waves 200A. https://www.keyboardwaves.com/wurlitzer-200a/

### Plugin Reviews and Comparisons

- Sound on Sound: Arturia Wurlitzer V. https://www.soundonsound.com/reviews/arturia-wurlitzer-v
- Sound on Sound: Lounge Lizard EP-5. https://www.soundonsound.com/reviews/applied-acoustic-systems-lounge-lizard-ep-5
- Happy Mag: Lounge Lizard EP-5 Review. https://happymag.tv/lounge-lizard-ep-5-review/
- KVR Audio: Best Synthesized Wurlitzer discussion. https://www.kvraudio.com/forum/viewtopic.php?t=616778
- KVR Audio: AudioThing Wurly discussion. https://www.kvraudio.com/forum/viewtopic.php?t=611149
- Gearspace: AudioThing Wurly thread. https://gearspace.com/board/new-product-alert-2-older-threads/1430373-audiothing-wurly-vintage-electric-piano-wurlitzer-plugin.html
- MusicRadar: Reed200 review. https://www.musicradar.com/news/reed200-is-30mb-wurlitzer-plugin-that-sounds-like-a-million-bucks
- Native Instruments: 10 Iconic Wurlitzer Songs. https://blog.native-instruments.com/wurlitzer-songs/
- Abbey Road Institute: Classic Keys -- The Wurlitzer. https://abbeyroadinstitute.com/miami/blog/classic-keys-the-wurlitzer-electronic-piano/
- Sweetwater: Recording a Wurlitzer Electric Piano. https://www.sweetwater.com/insync/recording-a-wurlitzer-electric-piano/
- Adam Monroe Music: Best Wurlitzer VST Plugin. https://adammonroemusic.com/blog/best_wurlitzer_vst_plugin.html

### Condenser Microphone / Electrostatic Pickup Physics

- DPA Microphones: Basics About Distortion in Mics. https://www.dpamicrophones.com/mic-university/technology/the-basics-about-distortion-in-mics/
- HAL: Measurement of Nonlinear Distortion of MEMS Microphones. https://hal.science/hal-03493526/document
- ResearchGate: Nonlinear Effects in MEMS Capacitive Microphone Design. https://www.researchgate.net/publication/4028531

---

## Appendix A: Quick-Reference Target Card

For rapid evaluation during development. Minimum viable test: 3 notes (C3, C4, C5) at 2 velocities (mf=80, ff=120).

```
=== MUST PASS (all tests) ===
[ ] H2 > H3 on every note at every velocity
[ ] Decay rate increases with pitch
[ ] Dynamic range pp-to-ff >= 15 dB
[ ] No clipping at mf
[ ] Attack overshoot >= 1 dB at mf

=== SHOULD MATCH (4/5 minimum) ===
[ ] H2/H1 within 5 dB of: -0.48*MIDI + 17.5
[ ] Decay rate within 30% of: 0.26*exp(0.049*MIDI)
[ ] Register spread >= 5 dB
[ ] Centroid drift negative for bass/mid (atk > sus)
[ ] H2 velocity coupling >= 5 dB (pp-to-ff)

=== LISTEN FOR (cannot automate) ===
[ ] Bark knee at ~vel 0.6 (not gradual, not binary)
[ ] Percussive attack (not volume swell)
[ ] Warm sustain (not buzzy or harsh)
[ ] Register character variation (bass/mid/treble distinct)
[ ] Natural temporal evolution (bright -> warm)
[ ] Progressive release (not amplitude gate)
[ ] Polyphonic chord compression (not clipping)
```

## Appendix B: Calibration Data Provenance Chain

```
Real Wurlitzer 200A (unknown condition)
    |
    v
Lundahl transformer (+1-3 dB H2, phase shift, 15-20 kHz rolloff)
    |
    v
Tube preamp (+1-3 dB H2, triode asymmetry)
    |
    v
ADC (48 kHz / 24-bit, no significant coloration)
    |
    v
OBM WAV files (raw measurements)
    |
    v
Recording chain correction (-3 dB H2, -1 dB H3 estimated)
    |
    v
Corrected calibration data (this document)
    |
    v
+/- 5 dB per-note uncertainty (unit-to-unit variation)
+/- 2 dB systematic uncertainty (recording chain correction)
= Total uncertainty: +/- 7 dB on absolute H2/H1 values
```

This uncertainty is large. However, the H2/H1 SLOPE across registers (-0.48 dB/semitone) is more reliable because the recording chain correction is approximately constant across the range. The slope uncertainty is approximately +/- 0.1 dB/semitone.

## Appendix C: Comparison of Objective Audio Quality Metrics

For agent reference when selecting evaluation metrics:

| Metric | Correlation with Perception | Computation Cost | Best Use Case | Limitations |
|--------|----------------------------|-----------------|---------------|-------------|
| FAD (VGGish) | ~0.52 (moderate) | High (neural embeddings) | Distribution-level quality | Poor for temporal structure; sample-size sensitive |
| FAD (CLAP) | Better than VGGish for music | High | Music-specific generation | Newer, less validated |
| MCD | -0.31 Spearman (weak) | Low | Speech synthesis only | Not designed for music |
| Multi-scale spectral loss | Good for training | Medium | Training objective | Not directly interpretable as quality score |
| Log spectral distance | Moderate | Low | Quick spectral comparison | Misses temporal evolution |
| Spectral convergence | Moderate | Low | Training objective | Same as above |
| MUSHRA listening test | Ground truth | Very high (requires humans) | Final quality assessment | Expensive; needs 15+ listeners |
| KAD (Kernel Audio Distance) | Better alignment with perception than FAD | Medium | Emerging alternative to FAD | Very new (2024), less validated |

## Appendix D: OBM A/B Comparison Results (Feb 2026)

Systematic comparison of 13 clean OBM single notes (Freesound Pack 5726, MIDI 50-98) against synthesized notes at matching pitch/velocity, using `wurli_compare.py --tier3`.

### Tier Selection

OBM recordings pass through the full real 200A chain (preamp → power amp → speaker → room → mic). Synth renders must match: **always use `--tier3`** (full chain with power amp, speaker, volume 0.40). Tier 2 renders (no power amp, no speaker) produce 39% inflated harmonic distance.

### Key Results

| Metric | Tier 2 | Tier 3 |
|--------|--------|--------|
| Mean harmonic distance | 55.2 dB | **33.8 dB** |
| Median | 44.0 dB | **28.4 dB** |
| Best match | 24.7 dB (A4) | **11.6 dB (F4)** |

Best matches (octave 3-4): F4=11.6 dB, A4=14.8 dB, A3=15.4 dB. Upper register (octave 6-7) inflated by room resonance artifacts in OBM recordings (see anomalous notes below).

### Anomalous OBM Notes

5 of 13 notes have harmonic content inconsistent with a single vibrating reed. These should be **excluded from automatic calibration** and weighted down in perceptual evaluation:

- **D6 (MIDI 86)**: H6 = +5.6 dB above H1 (ratio 1.91x). Room resonance at ~7 kHz.
- **D7 (MIDI 98)**: H6 = +13.1 dB above H1 (ratio 4.50x). Room resonance.
- **A6 (MIDI 93)**: Flat harmonic tail -6.8 to -16.2 dB across H2-H10. Noise floor, not instrument.
- **D5 (MIDI 74)**: H5/H6 at -10.5/-12.5 dB but H2 at -23.1 dB. Resonance artifact.
- **D4 (MIDI 62)**: H2 nearly absent at -53.7 dB. Possible dead spot or very light strike.

### Actionable Findings

1. **Decay rate 2-3x too fast in bass (octaves 3-4)**: **DONE (2026-02-19).** Old `3.0*(midi/48)^4.5` replaced with OBM-calibrated frequency power law `0.005*f^1.22` (floored 3.0 dB/s). F#3 deficit: 1.6x→1.03x. All notes within ±30% of OBM.

2. **H2/H1 too clean by 5-8 dB at mid-register**: A4 shows -8.2 dB real vs -16.1 dB synth (7.9 dB deficit). Confirms the MLP ds_correction saturation finding. **DONE (2026-02-19):** DS_AT_C4 increased from 0.70 to 0.85 (+2.69 dB H2/H1 analytically), trim recalibrated. MLP retrain deferred.

3. **Speaker LPF possibly too gentle for upper register**: Real treble centroids at 0.44-0.48x of f0 suggest the 4"x8" ceramic speakers roll off well below 7500 Hz. Caveat: partly mic/room coloration. Needs DI recording to confirm.
