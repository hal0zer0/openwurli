# Signal Chain Architecture: Wurlitzer 200A Physical Model

Complete specification for a physically-accurate Wurlitzer 200A electric piano plugin using modal synthesis, DK-method preamp circuit simulation, and per-note ML correction. Every processing stage is fully specified with formulas, parameter values, and implementation guidance.

---

## Table of Contents

1. [Real Instrument Signal Flow](#1-real-instrument-signal-flow)
2. [Plugin Architecture Overview](#2-plugin-architecture-overview)
3. [Stage 1: MIDI Input and Voice Allocation](#3-stage-1-midi-input-and-voice-allocation)
4. [Stage 2: Modal Reed Oscillator (Per-Voice)](#4-stage-2-modal-reed-oscillator-per-voice)
5. [Stage 3: Hammer Dwell Filter (Per-Voice)](#5-stage-3-hammer-dwell-filter-per-voice)
6. [Stage 4: Attack Noise Burst (Per-Voice)](#6-stage-4-attack-noise-burst-per-voice)
7. [Stage 5: Per-Note Variation (Per-Voice)](#7-stage-5-per-note-variation-per-voice)
8. [Stage 6: Electrostatic Pickup (Per-Voice)](#8-stage-6-electrostatic-pickup-per-voice)
9. [Stage 7: Voice Summation (Per-Voice to Mono)](#9-stage-7-voice-summation-per-voice-to-mono)
10. [Stage 8: Oversampling and Preamp (Mono, 2x Rate)](#10-stage-8-oversampling-and-preamp-mono-2x-rate)
11. [Tremolo — Integrated in Preamp Emitter Feedback Loop](#11-tremolo--integrated-in-preamp-emitter-feedback-loop)
12. [Stage 10: Volume Control (Mono, 2x Rate)](#12-stage-10-volume-control-mono-2x-rate)
13. [Stage 11: Power Amplifier (Mono, 2x Rate)](#13-stage-11-power-amplifier-mono-2x-rate)
14. [Stage 12: Speaker Cabinet (Mono, Base Rate)](#14-stage-12-speaker-cabinet-mono-base-rate)
15. [Stage 13: Output Limiter and Stereo (Mono to Stereo)](#15-stage-13-output-limiter-and-stereo-mono-to-stereo)
16. [Gain Staging Analysis](#16-gain-staging-analysis)
17. [Oversampling Strategy](#17-oversampling-strategy)
18. [Anti-Aliasing Considerations](#18-anti-aliasing-considerations)
19. [Sample Rate Support](#19-sample-rate-support)
20. [Complete Parameter List](#20-complete-parameter-list)
21. [Damper and Release Model](#21-damper-and-release-model)
22. [Polyphony and Voice Management](#22-polyphony-and-voice-management)
23. [Lessons from Previous Implementation (OpenWurli)](#23-lessons-from-previous-implementation-openwurli)
24. [Comparison with Existing Plugins](#24-comparison-with-existing-plugins)
25. [CLAP Plugin Requirements](#25-clap-plugin-requirements)
26. [References](#26-references)

---

## 1. Real Instrument Signal Flow

The Wurlitzer 200A is a 64-key electrostatic reed piano (A1/MIDI 33 to C7/MIDI 96). The physical signal path from keypress to speaker output is:

```
Keypress
  -> Hammer mechanism (felt-tipped wooden hammer rises, strikes steel reed)
  -> Reed vibrates (cantilevered spring steel with solder tuning mass at free end)
  -> Electrostatic pickup (reed + shared pickup plate = variable capacitor)
     - Polarizing voltage: ~147V DC via half-wave rectifier
     - Polarizing/bias network (revised 2026-09): R-2 (1M) from the +150V line lands on the PICKUP side of the input cap; TR-1's base is biased by R-3's (470K) DC-feedback return from TR-2's emitter divider — no base divider from the supply exists
     - C-2 (220 pF) at the base — a real 200A part; folded into the pickup corner (C-1 is a short at audio), not a separate filter
     - ALL 64 reeds share ONE common pickup plate (reed bar assembly)
     - Total system capacitance: ~240 pF at preamp input
  -> Preamp (separate PCB mounted on reed bar in 200A)
     - Two direct-coupled NPN common-emitter stages (TR-1, TR-2)
     - Originally 2N2924, later replaced with 2N5089 (hFE >= 450)
     - +14.5V DC regulated supply (manual text; drawing marks +15V)
     - Collector-base feedback caps C-3 = C-4 = 100 pF
     - Pickup RC HPF at ≈880 Hz (revised 2026-09; one-pole fit to the corrected network, 0.23 dB max in-band error; scales with the LOW-confidence C_TOTAL)
     - Closed-loop gain ≈15.9 dB at the divider floor (matches Avenson's ~15 dB); tremolo swings ≈7.9 dB p-p at full depth
     - Output: 2-7 mV AC at volume pot
  -> Tremolo (LDR optocoupler modulates preamp emitter feedback)
     - LFO (~5.6 Hz twin-T oscillator, TR-3/TR-4) drives LED inside LG-1 optocoupler
     - R-10 (56K) feeds back from output to fb_junct; Ce1 (4.7 MFD) couples fb_junct to TR-1 emitter
     - LDR (LG-1) shunts fb_junct to ground via the 50K VIBRATO pot wired as a loaded divider (18K top→wiper, LDR directly on the wiper branch — see output-stage.md §2.3)
     - Modulates preamp GAIN (not post-preamp volume): series-series emitter feedback topology
     - LED ON → LDR low → fb_junct shunted to ground → feedback can't reach emitter → higher gain
     - LED OFF → LDR high → full feedback reaches emitter via Ce1 → lower gain
     - This is gain modulation, producing timbral variation through the tremolo cycle
     - Rate ≈5.56 Hz (measured); depth ladder 0/1.4/2.6/4.0/7.6 dB ("6 dB" folklore figure = one aged unit, see output-stage.md §2.3)
  -> Volume potentiometer
     - Between preamp output and power amp input
     - Output at pot: 2-7 mV AC
  -> Power amplifier (~18-20W Class AB push-pull)
     - TIP35C (NPN) / TIP36C (PNP) output transistors
     - +/-24V rails
     - 0.47 ohm emitter degeneration resistors
     - ~10 mA quiescent bias
  -> Speaker (two 4"x8" oval ceramic drivers in ABS plastic lid; see output-stage.md)
     - Open-backed baffle (NOT sealed), bass rolloff ~85-100 Hz
     - Cone breakup rolloff ~7-8 kHz
```

### Critical Topology Facts

**All reeds share a single pickup plate.** The reed bar is a long metal assembly where all 64 reeds sit in machined grooves of one continuous pickup plate. Each reed forms its own variable capacitor with the shared plate, but all capacitors sum into a single electrical output. This means:

- The pickup output is inherently the SUM of all active reeds
- Per-reed signal levels are microvolt-scale (tiny capacitance changes)
- The preamp sees the combined signal from all reeds simultaneously
- Polyphonic interaction happens at the electrical level, not the acoustic level

**The pickup's nonlinearity — the reed swinging into the finger band — is the primary source of even harmonics (H2) at normal dynamics.** The model's pickup tap reads H2/H1 ≈ −10 dB at C4 v64 (overlap law, 2026-09), while the preamp at millivolt input levels produces THD < 0.01%. The preamp's asymmetric clipping headroom (2.05V vs 10.9V, ratio 5.3:1) contributes additional H2 at extreme ff dynamics where it enters saturation. Both the pickup and preamp contribute to the characteristic "bark," but the pickup dominates at normal playing levels.

**Direct coupling between stages is the defining preamp feature.** TR-1 collector connects directly to TR-2 base with no coupling capacitor. This means TR-1's DC operating point sets TR-2's bias. Signal-dependent bias modulation creates compression, transient sag, and velocity-dependent timbral change -- all absent from AC-coupled models.

**The tremolo operates WITHIN the preamp stage.** R-10 (56K) feeds back from the output to TR-1's emitter via Ce1 (4.7 MFD coupling cap). The LDR (LG-1) shunts this feedback junction to ground, modulating how much feedback reaches the emitter and thus the closed-loop gain. This is series-series emitter feedback. The signal flow is: preamp (with integrated tremolo gain modulation via emitter feedback) -> volume pot -> power amplifier -> speaker. The volume pot sits after the preamp output and before the power amp.

---

## 2. Plugin Architecture Overview

```
MIDI note-on (key, velocity, channel, note_id)
  -> Voice Allocator (64 voices, oldest-first stealing, releasing-first preference)
     Per-voice processing (base sample rate):
       [A] Modal Reed Oscillator (7 modes, Euler-Bernoulli + tip mass)
       [B] Gaussian Dwell Filter (hammer contact time spectral shaping, sigma=8.0)
       [C] Attack Noise Burst (felt-on-steel impact, 2-5 ms)
       [D] Per-Note Variation (deterministic +-3 cents freq, +-8% amp)
       [E] Electrostatic Pickup (field-solved overlap law, capacitive)
     -> Sum all voices (mono)

  Mono shared processing:
     -> 2x Upsample (6-coefficient (3+3) allpass polyphase IIR, ~28 dB rejection at 30 kHz)
        [F] Preamp WITH INTEGRATED TREMOLO
            Default: hand-written 9-node MNA solver (DK method, dk_preamp_legacy.rs)
            Opt-in:  melange-generated 12-node solver (--features melange-preamp)
            Tremolo: LDR (LG-1) + R-10 (56K) modulate feedback ratio
        [I] Volume network as drawn: R-11 trimmer → 10K pot at the user's position → amp input, loaded (see §12)
        [K] Power Amplifier (Class AB, crossover distortion at low signal levels)
     -> 2x Downsample (matching allpass polyphase IIR)
     [L] Speaker Cabinet (variable: bypass to authentic HPF 30 Hz subsonic + LPF 5.5 kHz)
     [M] Output (no separate limiter — handled by power amp tanh and speaker tanh Xmax)
     -> Mono to Stereo duplication
     -> float32 output buffers
```

### Linear vs. Nonlinear Stages

| Stage | Linear? | Needs Oversampling? | Rate |
|-------|---------|-------------------|------|
| Modal Oscillator | Yes (sinusoidal sum) | No (bandlimited by construction) | Base |
| Dwell Filter | Yes (amplitude scaling at note-on) | No (runs once) | N/A |
| Noise Burst | Yes (filtered noise, envelope) | No (broadband, no harmonics generated) | Base |
| Pickup | NO (overlap-law nonlinearity, primary H2 source) | No (runs at base rate; harmonics stay within audio band) | Base |
| Voice Sum | Yes (addition) | No | Base |
| Preamp Stage 1 | NO (exponential + asymmetric soft-clip) | YES | 2x |
| Miller LPF 1 | Yes (1st order) | No (already at 2x) | 2x |
| Preamp Stage 2 | NO (exponential + asymmetric soft-clip) | YES | 2x |
| Miller LPF 2 | Yes (1st order) | No (already at 2x) | 2x |
| DC Block | Yes (handled internally by DK preamp) | No (already at 2x) | 2x |
| Tremolo (in preamp feedback) | Mildly nonlinear (modulates preamp gain/distortion) | YES (inside preamp oversampled block) | 2x |
| Volume | Yes (gain scaling) | No | Base |
| Power Amp | Mildly nonlinear (crossover distortion) | Marginal (2x sufficient) | Base |
| Speaker | NO (biquad filters + Hammerstein polynomial waveshaper a2=0.2/a3=0.6 + tanh Xmax limiting + thermal voice coil compression) | Marginal (low-order distortion at speaker stage) | Base |
| Output Limiter | Not a separate stage (handled by power amp tanh + speaker tanh Xmax) | N/A | N/A |

**Conclusion:** The preamp is the stage that needs oversampling; the volume network and power amp share the 2x bus (`engine.rs`). 2x is sufficient because the preamp's input signal is already bandlimited by the pickup's natural bandwidth and the preamp's own Miller-effect rolloff. The preamp generates harmonics, but the highest-energy harmonics that could alias are well below Nyquist at 2x.

---

## 3. Stage 1: MIDI Input and Voice Allocation

### Input Events

The plugin accepts both CLAP native note events and MIDI 1.0. CLAP note events carry floating-point velocity [0.0, 1.0]; MIDI velocity bytes are normalized by dividing by 127.

### Voice Pool

- **64 pre-allocated voices** (zero heap allocation in audio callback)
- States: `FREE` -> `HELD` -> `RELEASING` -> `FREE`
- All voice data is pre-allocated in a fixed-size array

### Allocation Strategy

1. Search for first `FREE` voice
2. If none free, steal the voice with the smallest `age` value (oldest note)
3. Prefer stealing `RELEASING` voices over `HELD` voices
4. On steal: move stolen voice into a 5ms linear crossfade-out slot, initialize new note in its place

### Note-On Processing

At note-on, the following happens once (not per-sample):
1. Compute mode frequencies from fundamental and interpolated mode ratios
2. Apply Gaussian dwell filter to mode amplitudes
3. Apply per-note variation (deterministic hash)
4. Compute decay rates
5. Initialize pickup gap
6. Optionally: run MLP correction network to adjust parameters

### Note-Off Processing

At note-off:
1. Transition voice to `RELEASING` state
2. Bake current amplitude (fold elapsed decay into base amplitudes)
3. Reset time counter
4. Compute per-mode damper rates
5. Top 5 keys (MIDI >= 92): no damper, natural decay only

### Voice Death Detection

A voice is dead and can be freed when:
- All mode amplitudes < 1e-4 (-80 dB) — applies to both `HELD` and `RELEASING` voices
- OR damper is active AND release time > 10.0s (safety timeout)

---

## 4. Stage 2: Modal Reed Oscillator (Per-Voice)

The reed is modeled as 7 exponentially decaying sinusoids. This is physically justified because a cantilevered beam's vibration decomposes into normal modes, each of which rings independently and decays exponentially due to internal steel damping.

### Why Modal Synthesis (Not DDSP, Not Waveguide)

- **DDSP harmonic oscillator** is strictly harmonic (integer frequency ratios). Wurlitzer reeds have inharmonic mode ratios (6.3x, 17.9x, etc.) from Euler-Bernoulli beam physics with solder tip mass. DDSP cannot represent this.
- **Waveguide** is designed for quasi-1D resonators (strings, tubes). A cantilevered beam with a point mass has a complex boundary condition that doesn't map cleanly to delay-line topologies.
- **Modal synthesis** directly represents each vibration mode as an independent sinusoid with its own frequency, amplitude, and decay. This is the natural basis for Euler-Bernoulli beam physics.

### Mode Frequencies

```
f_mode[m] = f_fundamental * ratio[m] * (1 + variation[m])
```

Mode ratios are computed dynamically per note from the Euler-Bernoulli characteristic equation:

```
1 + cos(L)cosh(L) + L*mu*(cos(L)sinh(L) - sin(L)cosh(L)) = 0
```

The tip-mass ratio `mu = tip_mass_ratio(midi)` varies per note (heavier solder on bass reeds, lighter on treble). The code solves for eigenvalues numerically and derives ratios from them. Bare beam ratios (6.267, 17.55, 34.39, 56.84) are the minimum; solder mass increases ratios above these values.

**Important:** Mode frequencies above 0.45 * sampleRate should be zeroed to prevent aliasing. This primarily affects modes 5-6 at the highest notes.

### Mode Amplitudes

Base amplitudes (OBM-calibrated, single table for all registers, before dwell filter and velocity scaling):

```
[1.0, 0.005, 0.0035, 0.0018, 0.0011, 0.0007, 0.0005]
```

These are OBM-calibrated values derived from OldBassMan 200A recordings. The previous 1/omega_n Euler-Bernoulli values were 20-37 dB too hot vs OBM data. Real Wurlitzer reeds (solder tip mass, non-uniform geometry) suppress upper modes far below ideal beam theory. The characteristic "bark" (H2) comes from the pickup's nonlinearity generating H2 at 2x the fundamental, NOT from physical mode 2 at 6.3x the fundamental.

### Velocity Scaling

Velocity scaling uses a register-dependent exponent: a bell curve centered at MIDI 62 (sigma=15) from 1.3 (extremes) to 1.7 (mid-range), applied as `velMapped = velocity^exp`. This shapes the dynamic response to match the mechanical leverage differences across the keyboard.

Example values at mid-range (exp ~ 1.7):
- pp (vel=0.3): velMapped = 0.09
- mf (vel=0.7): velMapped = 0.47
- ff (vel=0.95): velMapped = 0.91

Timbral brightening at ff comes from two sources that do NOT require per-mode velocity exponents:
1. Shorter dwell time at ff -> dwell filter passes more upper partials
2. Pickup nonlinearity -> a larger swing enters the finger band further and generates more harmonics

Per-mode velocity exponents double-count with the dwell filter's velocity-dependent brightening. Do not use per-mode exponents.

### Decay

```
base_decay = 0.005 * freq^1.22  // with MIN_DECAY_RATE = 3.0 dB/s floor
decay_rate[m] = base_decay * ratio[m]^2.0          // power-law per-mode scaling (MODE_DECAY_EXPONENT = 2.0)
```

- Base decay rate follows a frequency power law calibrated to OldBassMan 200A recordings (see reed-and-hammer-physics.md Section 5.7)
- The 3.0 dB/s floor prevents unrealistically long bass sustain
- Per-mode scaling uses a `ratio^2.0` power law, chosen by ear to cut the mode-2 "plink" short: higher modes (with larger frequency ratios) decay faster. It is unsourced: thermoelastic (Zener) loss is flat, not ∝ ω², above ~75 Hz (reed-and-hammer-physics.md §5.5), and a real 200A's C4 mode 2 rings for seconds
- This replaces the previous fixed decay scales array `[1.0, 0.20, 0.08, ...]` with a physics-derived power law
- Higher modes decay faster -> timbre darkens over time (bright attack, sine-like tail)

Calibration target: `base_decay = 0.005 * freq^1.22` with MIN_DECAY_RATE = 3.0 dB/s floor, +/-30% tolerance.

### Per-Sample Rendering

```rust
// Quadrature oscillator: each Mode struct holds (s, c) sine/cosine state.
// Phase advance via 2x2 rotation matrix — 0 sin() calls per sample per voice.
// Mode struct is AoS (array of structs, 616 bytes/voice) for L1 cache locality.
//
// Multiplicative decay: precomputed decay_mult[m] = exp(-alpha/sr),
// running envelope[m] *= decay_mult[m] each sample.
// Mathematically identical to exp(-alpha*n) to ~15 decimal places.
for each sample:
    signal = 0.0
    for each mode m:
        // Quadrature output: amplitude * sin_state * onset * envelope
        signal += mode[m].amplitude * mode[m].s * onset * mode[m].envelope

        // Quadrature rotation
        let ci = mode[m].cos_inc   // precomputed cos(2π * freq * dt)
        let si = mode[m].sin_inc   // precomputed sin(2π * freq * dt)
        let s_new = mode[m].s * ci + mode[m].c * si
        let c_new = mode[m].c * ci - mode[m].s * si
        mode[m].s = s_new
        mode[m].c = c_new

    // Apply decay (multiply only — no transcendentals)
    for each mode m:
        mode[m].envelope *= decay_mult[m]
        if damper_active && damper_ramp_done:
            mode[m].envelope *= damper_mult[m]

    // Renormalize quadrature state every 1024 samples (amortized cost)
    if sample_counter % 1024 == 0:
        for each mode m:
            let norm = 1.0 / sqrt(mode[m].s² + mode[m].c²)
            mode[m].s *= norm
            mode[m].c *= norm

    // Onset powf branching: exp≈1 direct multiply, exp≈2 squared (avoids powf)
```

### Phase Initialization

Reed starts at zero displacement (hammer imparts velocity, not displacement). All mode phases start at 0. A raised cosine onset ramp models the gradual buildup of reed vibration during hammer contact:

```
onset_envelope(t) = 0.5 * (1 - cos(PI * t / T_onset))
```

The onset ramp time `T_onset` is register-dependent: `(periods / f0).max(2ms)`, where `periods` ranges from **1.0 (ff) to 2.0 (pp)** (2026-07: shortened from 2.5/5.0). No upper clamp. The envelope shape is `cosine^(1 + (1-velocity))` -- ff gets a raised cosine, pp gets Hann-squared (softer onset). The physical anchor is the hammer contact (dwell ≈ 0.75 cycle, Miessner): the reed reaches near-peak by the end of contact, ~1 cycle -- so at ff the low bass now **cracks near the strike** (C2 ff ~15 ms) instead of swelling in over ~38 ms, which had made the bass growl fade up rather than hit. Because the ramp is period-scaled, this primarily speeds up the bass; treble already sits at the 2 ms floor and is unchanged. (The earlier "90% at cycle 2.0, matching OBM" target came from room-coupled OBM whose smeared attack over-reads the rise time -- the ear wanted the crack.)

### Attack Overshoot: Let Physics Handle It

With physically accurate 1/omega mode amplitudes, all modes start in-phase at t=0 and upper modes decay faster. The sum of all modes at t=0 is larger than the sustained fundamental-only signal. This naturally produces 2-4 dB overshoot at mf and 4-8 dB at ff without any artificial envelope. The attack character emerges from modal superposition, which is exactly how it works in the real instrument.

Do NOT add an artificial overshoot envelope. If natural overshoot is insufficient, the mode amplitude ratios or dwell filter parameters are wrong.

---

## 5. Stage 3: Hammer Dwell Filter (Per-Voice)

The hammer contact time creates a finite-duration force pulse that spectrally shapes the initial mode excitation.

### Dwell Time

```
cycles = 0.75 + 0.25 * (1 - velocity)  // Miessner patent: 3/4 to 1 cycle of f0
t_dwell = cycles / f0                   // clamped [0.3ms, 20ms]
```

- ff (vel ~0.95): ~0.76 cycles -> short contact, brighter
- mf (vel ~0.7): ~0.83 cycles
- pp (vel ~0.3): ~0.93 cycles -> longer contact, darker
- Clamped to [0.3ms, 20ms] (Miessner patent US 2,932,231)

### Force Pulse Shape: Gaussian (NOT Rectangular, NOT Half-Sine)

**This is critical.** The choice of force pulse model determines the spectral envelope of mode excitation:

| Model | Spectral Envelope | Nulls? | Appropriateness |
|-------|------------------|--------|-----------------|
| Rectangular pulse | sinc: `abs(sin(pi*f*T)/(pi*f*T))` | 40-60 dB deep at integer f*T | WRONG. No real hammer has a perfectly rectangular force profile. The deep nulls forced 20x mode amp compensation in the previous project, which destroyed attack transients. |
| Half-sine pulse | `abs(cos(pi*f*T)) / abs(1-(2*f*T)^2)` | Nulls at f*T = 1.5, 2.5, 3.5... | REJECTED. Although closer to felt physics, the nulls near f*T = 2.5 and 3.5 cause mode 2 attenuation to swing from -3 dB to -40+ dB depending on velocity and note, creating the same instability as the sinc model. |
| Gaussian pulse | `exp(-dwell_arg^2 / (2 * sigma^2))` | NO nulls, monotonic rolloff | CORRECT for felt-tipped hammer. Smooth, progressive attenuation of upper modes. No artifacts. |

**Use the Gaussian model with sigma = 8.0:**

```
sigma_sq = 8.0^2  // = 64.0; sigma^2 in (f*T)^2 units
dwell_arg = freq[m] * t_dwell  // dimensionless f*T product
dwell_filter = exp(-dwell_arg^2 / (2 * sigma_sq))

// Normalize to fundamental
if m == 0:
    dwell_filter_f0 = dwell_filter
    attenuation = 1.0
else:
    attenuation = dwell_filter / dwell_filter_f0
```

The Gaussian sigma parameter controls how aggressively upper modes are attenuated. Larger sigma = more upper modes pass through = brighter overall timbre.

With sigma=8.0, attenuation at mf (C4, t_dwell=1.9ms) is: mode 2 = -0.8 dB, mode 3 = -6.4 dB, mode 4 = -25.6 dB. This preserves mode 3's "metallic clang" contribution while rolling off negligible higher modes. See reed-and-hammer-physics.md Sections 4.3.3-4.3.4 for the full analysis.

### Why Normalization to Fundamental Matters

Without normalization, the dwell filter would also attenuate the fundamental, changing the overall volume with velocity. Normalizing to the fundamental ensures mode 0 always passes at unity, and the filter only shapes the relative amplitudes of upper modes.

---

## 6. Stage 4: Attack Noise Burst (removed)

Earlier versions added a bandpass noise burst at note-on (0.025·vel², 3 ms decay, 15 ms, centred
at 5×f0) for the hammer's impact. Every number in it was hand-set, and no 200A source shows an
impact transient in the electrical output: the electrostatic pickup senses the reed's motion, and
the documented electrical transients are faults (a reed shorting against the pickup, debris). The
service manual's "clicking" (§5D, cured by softening hard hammer felt) does not say whether it is
heard through the amplifier. The burst was removed; the hammer reaches the signal only through
the reed's modes (dwell filter and onset ramp). It had also been one cause of the chord-attack
cusps: its first sample stepped to full level, and simultaneous voices summed the steps.

---

## 7. Stage 5: Per-Note Variation (Per-Voice)

Real Wurlitzers have per-note personality from manufacturing tolerance: solder placement, reed alignment, gap variation. This is deterministic (the same note always sounds slightly different from its neighbors, but consistent across strikes).

```
// Deterministic hash seeded by note number (NOT random per strike)
freq_detune = 1.0 + hash(key) * 0.00173     // +/-3 cents fundamental detuning
amp_variation[m] = 1.0 + hash(key, m) * 0.08 // +/-8% per mode
```

- Fundamental IS detuned: +/-3 cents (0.00173 ratio), matching factory tuning tolerance per US Patent 2,919,616 (Miessner, 1960)
- No per-mode frequency spread — only the fundamental is detuned (applied as a multiplier to all mode frequencies via the detuned fundamental)
- All modes get +/-8% amplitude variation
- Hash function must be deterministic: same note always gets same variation

---

## 8. Stage 6: Electrostatic Pickup (Per-Voice)

### Operating Principle

Each reed + the shared pickup plate forms a small variable capacitor. A 147V DC polarizing voltage charges this capacitor. As the reed vibrates, the capacitance changes, inducing a signal voltage.

### Per-Reed vs. System Capacitance

This is a nuanced point with significant implications:

- **Per-reed capacitance:** ~5-20 pF (geometric estimate: plate ~3mm x 8mm, gap ~0.23mm)
- **System capacitance:** ~240 pF at preamp input (all 64 reeds in parallel + wiring + parasitics)
- **Per-reed RC corner:** f_c = 1/(2*PI*287k*10pF) >> 20 kHz -> constant-charge at all audio frequencies
- **System RC corner:** f_c ≈ 880 Hz (revised 2026-09 — one-pole fit to the corrected network; the old 287K/2312 Hz figure was built on the misread bias divider; see pickup-system.md §3.7)

The per-reed constant-charge approximation is a defensible engineering tradeoff given the ≈880 Hz system corner. The pickup model places its nonlinearity in the SOURCE (the moving reed's charge injection — 2026-09 restructure), and that source is the primary origin of even-harmonic "bark" at normal dynamics.

### Overlap Pickup Law (2026-09 revision)

The 200A pickup is a through-slot comb: each reed swings between two fingers across a small side clearance, resting just outside the finger band (about a sheet of paper; bass reeds below it, treble reeds above). Capacitance follows the reed-to-finger **overlap**, fringe-rounded at the band edges. It is a field-solved table per keyboard section, not the parallel-plate `C₀/(1−y)` used before. Full spec: [pickup-overlap-law.md](pickup-overlap-law.md).

```rust
// u = penetration past the near finger face (mm); rest sits just outside the band
let u = band_sign * amplitude_mm * sample - PICKUP_REST_OFFSET_MM;
// Source: the section's field-solved law, zero at rest. THE nonlinearity.
let s = law.eval(u) - c_rest;
// Network: one moving reed on a ~240 pF node, ~LTI one-pole high-pass at PICKUP_FC ≈ 880 Hz
// (exact trapezoidal discretization — see pickup.rs)
```

The reed enters the band on one half of its swing only, so the source is strongly one-sided and even harmonics dominate. At the pickup tap for C4 (after the HPF), H2/H1 is −27 dB at v20, −10 dB at v64, −7 dB at v100 and −6.5 dB at v127. The law is linear in swing up to about 0.45 mm, so bark grows with how deep the reed enters the band, not with a singularity.

### Swing Scale

Per-note ff swing at the pickup (mm) = `tables::pickup_swing_mm(midi)`, a **PROVISIONAL** table from a bottom-up hammer-impact model (reed mass and frequency per note; hammer mass, ff hammer speed from the service manual's action dimensions, restitution and contact factor at the geometric centre of their bands). The velocity curve scales it down from ff.

```
A1 0.99 mm | C3 0.95 | E3 0.88 || F3 1.25 | C4 1.19 | C5 0.82 | C6 0.45 | C7 0.22
(bass: Rule B stopgap inside the fundamental-swell bound, see pickup-overlap-law.md §8;
 band ends carried alongside)
```

It replaced the recording-fitted `DS_AT_C4 × compliance^0.75` scaling, whose C4 anchor (0.45 mm) sat below every plausible hammer-physics combination. The velocity curve is still recording-fitted and goes with the excitation revision (hammer–felt contact integration).

### Parameters

| Parameter | Default | Purpose |
|-----------|---------|---------|
| `pickup_swing_mm` | per note (C4 1.19 mm) | Provisional ff swing at the pickup, bottom-up; replaced by bench measurements |
| `PICKUP_REST_OFFSET_MM` | 0.085 | Rest offset outside the finger band (0.07–0.10 mm evidence band) |
| Section laws | bass 1–20, middle 21–42, treble 43–64 | Field-solved C(u), generated by `tools/pickup_field2d.py` |
| `PICKUP_SENSITIVITY` | 1.8375 V | V_pol · C_REED0 / C_TOTAL = 147 · 3/240 |
| `PICKUP_FC` | ≈880 Hz | One-pole fit to the drawn input network (C_TOTAL + C-2) |

---

## 9. Stage 7: Voice Summation (Per-Voice to Mono)

All active voices render into a shared mono buffer via addition:

```rust
// Zero the mono buffer
mono_buffer.fill(0.0);
for voice in active_voices {
    voice.render_block(&mut mono_buffer, frames, ...);  // += into buffer
}
```

This matches the real 200A topology: all reeds sum into one pickup plate, producing a single mono signal that feeds the preamp.

### Signal Level After Summation

At mf with a single voice, the summed output is approximately 0.05-0.15 (arbitrary units). With 6 voices (chord), the sum is 0.3-0.9. These levels need to be scaled to the correct range for the preamp input.

---

## 10. Stage 8: Oversampling and Preamp (Mono, 2x Rate)

This is the most complex processing stage. The preamp adds harmonic coloring at high dynamics and provides the tremolo-modulated gain that defines the instrument's character. (The pickup's nonlinearity is the primary bark source at normal dynamics; the preamp contributes at extreme ff.)

### DECISION: Trait-Based A/B Architecture

The preamp implements a `PreampModel` trait with `process_sample()`, `set_ldr_resistance()`, `reset()`. Two implementations exist behind this interface:
1. **DkPreampLegacy** (`dk_preamp_legacy.rs`) — hand-written 9-node MNA solver on the 2026-09 drawn topology. **Default since v0.5.2.** Measured (v0.5.2 A/B) to be tonally indistinguishable from the melange solver (±0.18 dB gain offset, identical THD/H2/H3, identical tremolo range 6.10 dB) while being 1.6-7× faster depending on workload. The former `EbersMollPreamp` was deleted in v0.3.0.
2. **DkPreamp** (melange-generated 12-node MNA solver using the DK method, `--features melange-preamp`) — full-precision implementation. Models the full two-stage circuit with direct coupling, Miller caps, and emitter feedback as a single coupled nonlinear system. See `dk-preamp-derivation.md`. Opt-in if a future analysis surfaces a tonal difference the v0.5.2 A/B missed.

### Oversampling Wrapper

The preamp runs at 2x the base sample rate inside a polyphase IIR oversampler:

1. **Upsample:** 6-coefficient (3+3) allpass polyphase half-band upsampler (~28 dB rejection at 30 kHz)
2. **Process:** Run the preamp (default: 9-node MNA solver), volume network and power amp at 2x rate
3. **Downsample:** Matching allpass polyphase half-band downsampler

The oversampler uses allpass IIR filters (custom Rust implementation in `oversampler.rs`). The ~28 dB rejection is sufficient because the preamp's Miller-effect rolloff naturally limits harmonic energy above ~15 kHz.

### Input Drive

The DkPreamp receives the voice summation output directly. Because the DK method models the full circuit with physical component values, no artificial input drive scaling is needed -- the circuit's gain, impedances, and nonlinear behavior emerge naturally from the MNA equations. The voice output is scaled by `output_scale()` (physics-based, computed from displacement scale and pickup geometry) to approximate millivolt-level signals before entering the preamp.

### Preamp Circuit Model

The preamp is solved as the drawn circuit, not as per-stage behavioural blocks (the Ebers-Moll stage model and envelope-follower coupling that used to be described here were deleted in v0.3.0). Topology, component values, stage gains and headroom: [preamp-circuit.md](preamp-circuit.md). Solver derivation: [dk-preamp-derivation.md](dk-preamp-derivation.md).

### DC Block

Handled internally by the DK preamp's coupled MNA solver -- no separate DC block stage is needed. The DK method's circuit equations naturally account for DC operating points and coupling.

---

## 11. Tremolo — Integrated in Preamp Emitter Feedback Loop

The 200A tremolo modulates the preamp's closed-loop gain via an LDR (LG-1) that shunts the emitter feedback junction to ground. See preamp-circuit.md Section 7 for detailed analysis. R-10 (56K) feeds back from the output to fb_junct; Ce1 (4.7 MFD) AC-couples fb_junct to TR-1's emitter. The LDR path (cable Pin 1 → 50K VIBRATO → 18K → LG-1 → GND) diverts feedback current away from the emitter. This is **gain modulation**, not simple amplitude modulation — the distortion character changes through the tremolo cycle.

### LFO (Twin-T Oscillator, TR-3/TR-4)

The oscillator is a twin-T (parallel-T) notch filter oscillator. SPICE-validated at 5.63 Hz with 11.8 Vpp output swing. See `spice/subcircuits/tremolo_osc.cir` and `output-stage.md` Section 2.1 for full topology.

```
// The Twin-T oscillator circuit (melange-generated, ~5.6 Hz, SPICE-validated) is
// simulated directly; the LED current read from its taps drives the light law.
light = led_light_law(i_led(osc_taps))   // 0..1, LED conducts forward only
```

### LDR Response (Asymmetric Attack/Release)

```
// Exponential smoothing with asymmetric time constants
if led_drive > ldr_envelope:
    coeff = exp(-1 / (0.003 * sample_rate))  // 3ms attack (LED on -> resistance drops fast)
else:
    coeff = exp(-1 / (0.050 * sample_rate))  // 50ms release (LED off -> resistance recovers slowly)

ldr_envelope = led_drive + coeff * (ldr_envelope - led_drive)
```

### CdS Nonlinearity and Emitter Feedback Modulation

```
// LDR resistance from CdS log-interpolation response
// Real CdS cells span ~4 decades (50 ohm fully lit to 1M ohm dark).
// log(R) interpolates between log(R_max) and log(R_min) as drive increases,
// with gamma controlling the knee of the response curve.
drive = ldr_envelope.clamp(0, 1)
log_r = log(R_max) + (log(R_min) - log(R_max)) * drive^gamma  // gamma = 0.9
R_ldr = exp(log_r)    // R_min=9k, R_max=1M (weakly driven cell)

// Shunt seen by fb_junct: the 50K VIBRATO pot is a 3-terminal divider
// (top = fb_junct, bottom = GND, wiper -> LDR); 18K bridges top -> wiper.
Z_shunt = (R_upper ∥ 18k) + (R_lower ∥ R_ldr)
R_upper = 50k·(1 − depth),  R_lower = 50k·depth   // depth 0 -> ≈13 kΩ fixed

// Emitter feedback: R-10 (56K) from output to fb_junct, Ce1 couples to emitter
// LDR path shunts fb_junct to ground, diverting feedback away from emitter
// When LDR path low (LED on): fb_junct grounded -> emitter AC-grounded via Ce1 -> higher gain
// When LDR path high (LED off): full feedback reaches emitter -> lower gain

// Modulate preamp emitter feedback with LDR path impedance
// At low preamp drive: gain modulation ≈ amplitude modulation
// At high preamp drive: gain modulation also changes distortion character
```

### Character

The asymmetric attack/release creates a "choppy" effect: fast dips (2.5 ms), slow recovery (35 ms). This is distinctly different from a smooth sine tremolo and is immediately recognizable as Wurlitzer.

**Timbral modulation:** At the high-gain phase (LDR lit — the low-resistance shunt diverts feedback from the emitter), the preamp's gain is higher, amplifying the pickup-generated harmonics more and pushing the preamp closer to its own saturation threshold. At the low-gain phase (LDR dark, full feedback reaches the emitter), the preamp operates more linearly with less harmonic amplification. This subtle but important timbral variation distinguishes the real 200A tremolo from a simple volume multiplier.

### Implementation Note

Because the tremolo modulates the preamp's emitter feedback (via the LDR shunt at fb_junct), it must be implemented INSIDE the preamp processing block (within the 2x oversampled domain), not as a separate post-preamp stage. The LDR state updates at the base sample rate, but the emitter feedback modulation applies per-sample at 2x rate.

### Parameters

| Parameter | Default | Range | Notes |
|-----------|---------|-------|-------|
| Depth | 0.5 | 0.0-1.0 | 0=off, 0.5 ~ 4.5 dB dip, 1.0 ~ 9 dB dip |

---

## 12. Stage 10: Volume Control (Mono, 2x Rate)

### DECISION: Model as Real Attenuator Between Preamp and Power Amp

In the real 200A, the 3K audio-taper volume potentiometer sits between the preamp output and the power amplifier input. The plugin must place the volume control at this exact point in the signal chain — NOT as a final output gain.

**Why placement matters on the real instrument:** at low pot settings the power amp input drops into the crossover region and the distortion character changes (more odd harmonics from the Class AB dead zone).

**What the model does (2026-09-23, the drawn network):** user volume is the pot position, and the network is solved loaded:

```
v_oc   = preamp_out * open_circuit_output_factor()          // back to the R-9 terminal
drive  = v_oc * volume_pot_gain(vol)                         // R-9 → R-11 → 10K pot → wiper ∥ R-27
output = speaker(power_amp(drive)) * POST_SPEAKER_GAIN       // PSG = 0 dB: rail = full scale
```

`volume_pot_gain` (tables.rs) uses R-11 at mid-travel and a standard 15 %-at-center audio taper, both assumptions pending a bench measurement. Consequence: the amp's operating point follows the pot as on the instrument — the crossover residual is proportionally larger at low settings and the clip knee is reachable at full pot with a hot R-11. From 2026-04-26 to 2026-09-23 the drive was pinned at 0.25 and user volume was a post-speaker multiplier; that departure from the drawing was retired on the maintainer's ruling after the 2026-09-22 external circuit review (output-stage §3.2 has the history).

---

## 13. Stage 11: Power Amplifier (Mono, 2x Rate)

**Priority: MEDIUM. The power amp is transparent at moderate levels but matters for ff polyphonic saturation and aged-instrument character.**

The real 200A has a ~18-20W quasi-complementary push-pull Class AB output stage:

- Input differential pair: 2N5087 (PNP)
- Vbe multiplier for bias: MPSA06
- Output: TIP35C (NPN) / TIP36C (PNP), +/-24V rails
- Emitter degeneration: 0.47 ohm
- Quiescent bias: ~10 mA

### Melange-generated 7-BJT Circuit Solver (opt-in: build with `--no-default-features`)

The power amp is modeled by a melange-generated DK/Nodal circuit solver compiled
from `spice/melange/wurli-power-amp.cir`. Every one of the 7 transistors (Q7/Q8
input diff pair, Q14 VAS, Q9 Vbe multiplier, Q10/Q12 Sziklai drivers, Q11/Q13
TIP35C/TIP36C output pair) uses a full Gummel-Poon model with datasheet values
(IS, BF, VAF, IKF, ISE, NE, BR, VAR, IKR, ISC, NC, RB, RE, RC, CJE, VJE, MJE,
CJC, VJC, MJC, TF). No `.linearize` hints — every nonlinearity is solved exactly
at runtime by the Newton-Raphson iteration inside the generated code.

**Solver shape (at openwurli pin `7ecb36c`, regenerated 2026-09-23 after the R-28 rail / C-13-C-14 / Zobel corrections):** N=20 nodes, M=14 nonlinear
dimensions, auto-routed to Nodal with Backward Euler integration (ρ = 1.0041
makes trapezoidal marginally unstable; BE is L-stable). Generated with
`--output-clamp 30` so melange's post-DC-block limiter passes the natural
±22 V rail swing instead of hard-clipping at the default ±10 V "Signal Level
Contract" ceiling.

Adapter divides the raw output by `HEADROOM = 22 V` to give ±1.0-ish output
for the downstream speaker model. Closed-loop gain, rail clipping, crossover
suppression, and level-dependent distortion all emerge from the circuit
simulation — no separate hand-written nonlinearity model.

**Regression-measured behavior** (full tests in `power_amp::tests`):

- Closed-loop gain: ~10 dB normalized (≈69× × 22 V / 22 V) at 1 kHz, 0.001 V —
  matches `1 + R31/R30 = 1 + 15 k/220 = 69×` (37 dB raw) within 1 dB of ngspice
- Rail clipping: peak ≥ 0.85 normalized (≈18.7 V) at 5 V 100 Hz input, with the
  bootstrap C12 tracking output above Vp so the VAS can swing the Sziklai
  rail-to-rail
- H3 < −30 dB at 440 Hz, 0.001 V — crossover suppression by feedback
- Output always finite and bounded for any input in ±5 V

### Behavioral closed-loop NR model (`legacy-power-amp`, in the default feature set — the SHIPPING model)

Models the loop `y = f(A_ol × (input − β(s) × y))` where `f()` = Gaussian-dead-zone
crossover followed by `rail × tanh(v / rail)`, and β(s) is the drawn R-31 / (R-30 +
C-10) feedback divider — 220/15220 in band, rising to 1 at DC. That leg is the
circuit's only bass roll-off (the amp is split-rail and DC-coupled): −3 dB at 33 Hz,
−1.3 dB at A1. It is a bilinear first-order section inside the NR loop (added
2026-09-22 after an external circuit review found the model memoryless):

| Constant | Value | Derivation |
|----------|-------|------------|
| `OPEN_LOOP_GAIN` | 19,000 | Diff pair × VAS × output, from schematic bias-point analysis |
| `FEEDBACK_BETA` | 0.01445 | R30/(R30+R31) = 220/15 220 |
| `HEADROOM` | 22.0 V | ±24 V rails − 2 V Vce_sat |
| `CROSSOVER_VT` | 0.013 V | Thermal voltage at lightly-aged 5–7 mA bias |
| `QUIESCENT_GAIN` | 0.1 | Output gain at zero signal |
| `NR_MAX_ITER` | 8 | 2–4 iterations typical |

Crossover uses a C∞ Gaussian `q + (1 − q)(1 − exp(−v² / vt²))` instead of a
piecewise dead zone so the NR Jacobian is well-defined everywhere. It produces
physically plausible harmonic content at typical drive levels but can't capture
level-dependent device nonlinearity that naturally emerges from full Gummel-Poon.
The melange solver is the higher-fidelity path; it is opt-in on CPU grounds.

User volume: the drawn pot network between preamp and amp (§12); no post-speaker user gain.

---

## 14. Stage 12: Speaker Cabinet (Mono, Base Rate)

The 200A uses two 4"x8" oval ceramic speakers in an open-backed ABS plastic lid (NOT sealed), 16 ohm each (part #202243). See output-stage.md for details.

### DECISION: Variable Speaker Emulation

The speaker HPF/LPF are physical limitations, not design choices. Expose a "Speaker Character" parameter that blends from **bypass** (full-range, flat) to **authentic** (full HPF + LPF). This lets players who want more bass or extended treble dial back the speaker emulation.

### Model (at "Authentic" Position)

Two variable-cutoff biquad filters (Direct Form II Transposed) with smoothed coefficient updates:

1. **Open-baffle bass rolloff:** was a single HPF at 95 Hz, Q=0.75 — lowered to 30 Hz subsonic in 2026-07 (cabinet bass roll-off belongs to the downstream host)
   - Physics-motivated: combination of speaker resonance + open baffle cancellation (~12 dB/oct)
   - Attenuates C2 fundamental (65 Hz) by ~5.4 dB
   - Leaves H2 (130 Hz) nearly untouched
   - Significant contributor to bass register H2/H1 balance

2. **Cone breakup rolloff:** 2nd-order LPF at 5500 Hz, Q=0.707 (Butterworth)
   - OBM A/B comparison shows real 4"x8" ceramic speakers roll off well below 7500 Hz
   - Treble centroids at 0.44-0.48x of f0 consistent with ~5500 Hz cutoff
   - Models speaker cone's own breakup, not preamp bandwidth

At "Bypass" position: both filters disabled (flat passthrough). Intermediate positions interpolate cutoff frequencies toward their extremes (HPF -> 20 Hz, LPF -> 20 kHz).

Hammerstein nonlinearity: normalized polynomial `(x + a2*x^2 + a3*x^3) / (1 + a2 + a3)` so that y(1)=1, plus tanh Xmax limiting.

Thermal voice coil compression: a slow power envelope follower (5s time constant) tracks the average input power and applies gain reduction via `thermal_gain = 1 / (1 + thermal_coeff * thermal_state.sqrt())`. This models the real speaker's voice coil heating under sustained loud passages, producing 0.5-2 dB of gradual compression.

### Coefficient Computation

Use the Audio EQ Cookbook (Robert Bristow-Johnson) formulas. Recompute coefficients when sample rate changes (in `activate()`) and when the Speaker Character parameter changes (with per-block smoothing).

---

## 15. Stage 13: Output Limiter and Stereo (Mono to Stereo)

### Soft Limiter

**Not implemented as a separate stage** -- limiting is handled by the power amp's tanh soft-clip (headroom=22V, see Section 13) and the speaker's tanh Xmax saturation. No additional output limiter is needed because these two stages already provide gradual saturation at the correct points in the signal chain.

### Stereo Output

The Wurlitzer 200A is a mono instrument. The plugin duplicates the mono signal to both stereo channels:

```rust
out_l[i] = mono_signal as f32;
out_r[i] = mono_signal as f32;
```

Optional enhancement: slight stereo widening via a short decorrelation delay (e.g., 0.2ms on one channel) or mid-side processing. But the authentic sound is mono.

---

## 16. Gain Staging Analysis

This section traces signal levels through the entire chain. Note: the DkPreamp uses physical component values directly and does not require artificial input drive scaling (see "Input Drive (Historical)" below).

### Real 200A Signal Levels

| Point in Chain | Signal Level | Source |
|---------------|-------------|--------|
| Reed displacement | ~0.1-0.5 mm peak | Mechanical measurement |
| Pickup AC voltage | ~1-10 uV per reed | Tiny capacitance change * 147V bias |
| Summed pickup (all reeds) | ~10-100 uV | Multiple reeds in parallel |
| After preamp (volume pot) | 2-7 mV AC | Brad Avenson measurement |
| Power amp input | 0-7 mV (volume dependent) | After pot |
| Speaker drive | ~1-5V peak | 18-20W into 4-8 ohm |

### Plugin Signal Levels (Current)

The power amp is driven through the drawn volume network. Measured with the
engine's drive-headroom probe (`power_amp_drive_headroom_probe`, 2026-09-23,
R-11 (engine setting) at its 17.2K default): at the default pot position (0.80),
single ff notes 37–70 mV peak at the amp input (11–22 % of the 319 mV clip
knee) and a worst-phase ff chord 117 mV (37 %); at vol 1.0, 51–98 mV single
(16–31 %) and 164 mV chord (51 %). The output mapping is `FULL_SCALE_VOLTS` =
the amp's 22 V rail = 1.0 at the engine output; the OpenWurli plugin adds a fixed +14 dB output alignment after the engine (0.9.0, see output-stage.md). *The figures below are 2026-09-23 engine-output measurements and predate the 0.9.0 pickup overlap law (ff 5–9 dB lower).*

| Point in Chain | Level | Notes |
|---------------|-------|-------|
| Single voice, mf | ~0.05-0.15 | After pickup |
| 6-voice chord, ff | ~0.3-0.9 | Sum of voices |
| After output_scale() | target_db=-35 dBFS | Into DkPreamp |
| After preamp | ~50 mV RMS (C4 ff) | ≈14 dB closed-loop gain at the idle shunt |
| After the volume network, vol 0.80 (default) | 7.6–8.3 mV RMS single ff, 17.5 mV RMS ff chord | 11–22 % / 37 % of the 319 mV clip knee |
| After the volume network, vol 1.0 | 10.6–11.5 mV RMS single ff, 24 mV RMS ff chord | 16–31 % / 51 % of the knee (R-11 17.2K) |
| After power amp | ~0.5 V RMS single ff at the default (69×) | Clean; crossover residual proportionally larger at low drive |
| After speaker | physics-level output | speaker character defaults to 0 (true passthrough) |
| Output (22 V rail = 0 dBFS) | ≈ −18.8 dBFS peak single ff, −11.6 ff chord, at the defaults | the pot has ~3 dB in hand; R-11 (engine setting, not a plugin parameter since 0.9.0) ~9 dB more |

Polyphonic headroom (measured 2026-09-23, six-note ff chord, speaker off):
- −11.6 dBFS peak at the default pot position (0.80); −8.6 at full volume; −1.3 at full volume with R-11 at 0 Ω (the amp at its rail = full scale by construction)

---

## 17. Oversampling Strategy

### What Needs Oversampling

The preamp is the main stage that generates harmonics above the input signal's bandwidth; the power amp also runs in the 2x bus (`engine.rs`; the aliasing gate is `alias_audit.rs`).

The pickup's nonlinearity generates harmonics but does so at the base sample rate; its output bandwidth stays within the audio band. The power amp crossover distortion generates only low-order odd harmonics at small signal levels.

### Why 2x Is Sufficient

The preamp's input is naturally bandlimited by the pickup's RC HPF (~880 Hz, `PICKUP_FC`) and the modal oscillator's finite mode count. This means:
- The highest-energy input component is around 2-4 kHz (fundamental of mid/treble register, or H2 of bass)
- The preamp generates harmonics at 2x, 3x, 4x, ... of this input
- At 48 kHz base rate, 2x oversampling gives 96 kHz processing rate with 48 kHz Nyquist
- H8 of a 4 kHz input = 32 kHz, safely below 48 kHz Nyquist
- H12 of a 4 kHz input = 48 kHz, at Nyquist -- but H12 is typically -50 dB or lower

For 44.1 kHz base rate, 2x gives 88.2 kHz with 44.1 kHz Nyquist. Still adequate given the natural input bandwidth.

### Conditional Bypass at High Sample Rates

At host sample rates >= 88.2 kHz, the 2x oversampling is bypassed entirely. The preamp runs at the native host rate. Rationale: the preamp's bandwidth (HF −3 dB ≈ 16.8 kHz in the SPICE reference) is well below the native Nyquist frequency (44.1+ kHz at 88.2 kHz host rate), so the oversampler provides no meaningful anti-aliasing benefit. Bypassing saves approximately 50% of the DK preamp solver cost (which dominates CPU usage).

The plugin checks `sample_rate < 88200.0` at initialization and sets `oversample` accordingly. The preamp-bench CLI supports `--sample-rate` to test rendering at non-default rates; oversampling is auto-bypassed when the specified rate is >= 88.2 kHz.

### Filter Choice: Allpass Polyphase IIR Half-Band

- Architecture: Polyphase IIR half-band filter using two allpass branches (3 coefficients each, 6 total)
- Stopband rejection: ~28 dB at 30 kHz (sufficient given the preamp's Miller-effect rolloff limits harmonic energy above ~15 kHz)
- Phase: Allpass (constant group delay within each branch)
- CPU cost: Very efficient -- only multiply-accumulate operations, no table lookups
- Implementation: Custom Rust port in `oversampler.rs`, not the HIIR library

### Alternative: ADAA (Anti-Derivative Anti-Aliasing)

ADAA can reduce aliasing without oversampling by computing the antiderivative of the nonlinear function and using it to perform continuous-time convolution. Research shows 2x oversampling + ADAA provides aliasing suppression comparable to 6x oversampling without ADAA.

However, ADAA requires the nonlinear function to have a closed-form antiderivative. The DkPreamp's coupled MNA solver with Newton-Raphson iteration is too complex for straightforward ADAA application. Allpass polyphase 2x oversampling is simpler and sufficient.

---

## 18. Anti-Aliasing Considerations

### Modal Oscillator

The oscillator is alias-free by construction: each mode is a pure sinusoid at a known frequency. Modes above 0.45 * sampleRate are zeroed at note-on. No anti-aliasing required.

### Pickup Nonlinearity

The pickup law generates significant even harmonics (H2/H1 ≈ −10 dB at C4 mf) but these are low-order harmonics within the audio band. Since the pickup operates at the base sample rate and its harmonic content is bounded by the reed's modal frequencies, aliasing is not a concern (checked 2026-09: pickup harmonics identical at 44.1 and 176.4 kHz to 0.1 dB).

### Preamp

Addressed by 2x oversampling (Section 17). The ~28 dB rejection at 30 kHz is sufficient because the preamp's Miller-effect rolloff naturally limits harmonic energy at high frequencies, so aliased components are well below the audible signal.

### Output Limiter

There is no separate output limiter (see §15).

### Denormal Protection

After the preamp, decaying voices produce very small signal values that can become denormal floating-point numbers, causing CPU spikes on x86 processors. Set FTZ (Flush-to-Zero) and DAZ (Denormals-Are-Zero) bits in the MXCSR register at the start of the process callback:

In Rust/nih-plug, denormal protection is typically handled by the framework or via inline assembly. The nih-plug `process()` callback runs with FTZ/DAZ already set by the host in most DAWs. If needed, Rust's `std::arch::x86_64` intrinsics can be used directly.

---

## 19. Sample Rate Support

The plugin must support at minimum: 44100, 48000, 88200, 96000 Hz. Higher rates (176400, 192000) are desirable but not critical.

### What Changes With Sample Rate

| Component | Sample Rate Dependence |
|-----------|----------------------|
| Oscillator phase increment | `2*PI*freq/sampleRate` |
| All filters (HPF, LPF, biquads) | Coefficients recomputed in `activate()` |
| Oversampler | 2x at base rates < 88.2 kHz; bypassed at >= 88.2 kHz |
| Preamp (inside oversampler) | Filters prepared at 2x sampleRate |
| Decay rates | Time-domain rates are sample-rate-independent (expressed in seconds) |
| Tremolo LFO | Phase increment: `rate * dt` |

### At Higher Sample Rates

At 96 kHz base rate (above the 88.2 kHz threshold), 2x oversampling is bypassed — the preamp's bandwidth (HF −3 dB ≈ 16.8 kHz in the SPICE reference) is well below the native 48 kHz Nyquist. The preamp runs at the native 96 kHz rate, saving approximately 50% of DK solver cost. Filter coefficients are recomputed for the native rate in `activate()`.

At 44.1 kHz, the 2x oversampler runs at 88.2 kHz. The pickup RC HPF (~880 Hz) attenuates the preamp input below that corner, so even H12 of a 4 kHz input (48 kHz) is below the 44.1 kHz Nyquist of the oversampled domain. Adequate.

---

## 20. Complete Parameter List

### User-Facing Parameters (Exposed in DAW)

| ID | Name | Module | Min | Max | Default | Purpose |
|----|------|--------|-----|-----|---------|---------|
| "volume" | Volume | output | 0% | 100% | 80% | The drawn volume pot (audio taper) between preamp and power amp |
| "trem_depth" | Tremolo Depth | tremolo | 0% | 100% | 50% | Modulation amount |
| "speaker" | Speaker Character | speaker | 0% | 100% | 0% | 0%=bypass (full range), 100%=authentic (HPF+LPF+waveshaper) |
| "mlp" | MLP Corrections | dsp | off / on | — | off | Per-note ML corrections (H2/H3 mode tuning and decay) |

The plugin also applies a fixed +14 dB output alignment (`OUTPUT_ALIGNMENT_DB`) after the engine. R-11 (reed-bar volume trim) is an engine setting, not a plugin parameter.

All other parameters (decay rates, pickup gap, preamp component values, mode amplitudes, velocity curve, attack overshoot) are hardcoded internally based on physical circuit analysis and OBM calibration data. They are not exposed to the user.

### Internal Constants (Not Exposed)

| Constant | Value | Purpose |
|----------|-------|---------|
| DC block | N/A | Handled internally by DK preamp |
| Speaker HPF (authentic) | 30 Hz, Q=0.75 | Subsonic only (2026-07; the 95 Hz cabinet roll-off moved to the downstream host) |
| Speaker LPF (authentic) | 5500 Hz, Q=0.707 | Cone breakup (was 7500 Hz; calibrated at the octave-mislabelled OBM pitch — unverified) |
| Dwell sigma^2 | 64.0 | Gaussian dwell filter width (sigma=8.0) |
| kNumModes | 7 | Modal oscillator mode count |
| kMaxVoices | 64 | Maximum simultaneous voices (matches real 200A key count) |

---

## 21. Damper and Release Model

### DECISION: Full Three-Phase Progressive Model

Implement the complete three-phase damper with release velocity sensitivity. The damper is a critical part of the playing experience — half-damping techniques are used expressively on the real instrument.

At note-off, a felt damper progressively contacts the reed. This is NOT an amplitude gate -- it progressively increases decay rates, with higher modes dying first. Frequency-dependent damping means upper modes damp first (felt absorbs high frequencies more efficiently), producing a brief "darkening" during release before silence.

### Damper Rate Computation

```
base_damper_rate = 55.0 * max(2^((key - 60) / 24), 0.5)

for each mode m:
    damper_factor = min(base_damper_rate * 3^m, 2000)
```

### Three-Phase Release Envelope

```
if release_time < damper_ramp:
    // Phase 1-2: Progressive contact (quadratic ramp)
    damper_decay = damper_rates[m] * release_time^2 / (2 * damper_ramp)
else:
    // Phase 3: Full contact
    damper_decay = damper_rates[m] * (release_time - damper_ramp / 2)

envelope = amps[m] * exp(-decay_rates[m] * time - damper_decay)
```

### Register-Dependent Ramp Time

| Register | Ramp time | Behavior |
|----------|-----------|----------|
| Bass (key < 48) | 50 ms | Slow felt engagement, residual ring |
| Mid (48-72) | 25 ms | Medium |
| Treble (key >= 72) | 8 ms | Fast damping, minimal ring |

### Special Cases

- **Top 5 keys (MIDI >= 92):** No damper. Natural decay only.
- **Safety envelope:** After 10 seconds of release, force voice to FREE.

---

## 22. Polyphony and Voice Management

### Real Instrument Context

The Wurlitzer 200A has 64 keys but practical polyphony is limited by the player and the instrument's nature (attack-focused, moderate sustain). The preamp naturally compresses polyphonic signals because it saturates harder with more simultaneous notes.

### Plugin Voice Count: 64

The real 200A has 64 mechanically independent reeds sharing a single pickup plate — all 64 keys can sound simultaneously with no artificial limit. The plugin matches this with 64 voice slots. Voice stealing only occurs if all 64 are active (practically impossible with 10 fingers, but an arpeggiator with sustain could approach it).

### Voice Allocation Algorithm

```
1. Scan for first FREE voice -> return it
2. If no FREE voice found:
   a. Prefer RELEASING voices over HELD
   b. Among candidates, steal the oldest (smallest age counter)
3. Set stolen voice to FREE, then initialize new note on it
```

### Voice Stealing Behavior

When a voice is stolen:
- A 5ms linear crossfade is applied (stolen voice fades out while new voice fades in)
- The host receives a NOTE_END event for the stolen voice

### CPU Considerations

- All voice memory is pre-allocated (no `new`/`delete` in audio thread)
- Voice rendering is the most CPU-intensive per-voice work
- Multiplicative decay (`envelope *= decay_mult`) replaces per-sample `exp(-alpha*n)` — saves 7 exp/sample/voice
- BJT function fusion: `bjt_ic_gm()` computes one exp() for both ic and gm in the NR loop
- Tremolo ln() caching: `ln(r_min)`, `ln(r_max)` precomputed at construction
- The oversampler and preamp run ONCE (shared), not per-voice
- NaN guard in preamp: `result.is_finite()` check prevents permanent state corruption
- Plugin-level NaN output guard: final `is_finite()` check after power amp + speaker, resets both on NaN
- **v0.2.0 optimizations:**
- Quadrature oscillator: 7 sin() → 0 transcendentals/sample/voice (Mode AoS struct, 616 bytes fits L1)
- Conditional oversampling: 2x bypassed at host rates >= 88.2 kHz (saves ~50% DK preamp cost)
- Filter precomputes: melange-primitives Biquad coefficients cached at construction/set_type()

### Per-Sample Budget (at 48 kHz)

| Component | Operations per sample | Notes |
|-----------|----------------------|-------|
| N voices x 7 modes | ~28N multiply-adds (quadrature rotation), 0 sin | Per-voice oscillator (multiplicative decay, no exp) |
| Pickup per voice | ~N max/add | Minimal |
| Voice sum | ~N additions | Trivial |
| 2x oversampler up | ~12 multiply-adds (bypassed at >= 88.2 kHz) | Allpass polyphase filter |
| Preamp (2 samples at 2x) | ~2 x (3 NR iterations x 2 fused exp) = 12 exp calls | Most expensive shared stage |
| 2x oversampler down | ~12 multiply-adds (bypassed at >= 88.2 kHz) | Allpass polyphase filter |
| Tremolo | ~2 multiply-adds + 1 exp + 1 powf | Cached ln values |
| Speaker (2 biquads) | ~10 multiply-adds | Trivial |

**Total: approximately 0 sin + 12-24 exp + ~600 multiply-adds per base-rate sample.** Previously ~84 sin + 12-24 exp + ~300 multiply-adds (v0.1.x).

---

## 23. Lessons from Previous Implementation (OpenWurli)

Key failure patterns from the previous project (40+ tuning rounds without convergence):

1. **No fudge factors.** Every parameter must trace to a physical quantity. If a compensation knob is needed, find and fix the underlying modeling error.
2. **Gaussian dwell filter only.** Sinc (rectangular pulse) and half-sine models have deep spectral nulls that forced 20x mode amplitude compensation, destroying the attack-to-sustain ratio.
3. **Miller cap polarity matters.** The cap provides MORE feedback at HF (low impedance), LESS at LF. Inverting this breaks register-dependent distortion.
4. **No artificial drive scaling.** The DkPreamp uses physical component values. If the sound is wrong, fix the circuit model, not the input scaling.
5. **No per-mode velocity exponents.** They double-count with the dwell filter's velocity-dependent brightening. Use a single register-dependent exponent (1.3-1.7).
6. **DAW state override.** Changing parameter defaults requires users to re-add the plugin. Consider a version check in `stateLoad()`.

---

## 24. Comparison with Existing Plugins

### Pianoteq (Modartt) - Gold Standard for Physical Modeling

**Strengths:**
- Most advanced physical modeling piano engine, including Rhodes and Wurlitzer modules
- Full key-by-key customization of tuning, voicing, damping
- Extremely small install size (~50 MB vs. 80+ GB for sample libraries)
- Supports Linux natively (as of Pianoteq 9)
- Continuous velocity response with no layer boundaries
- Updated MKII and Reeds packs in 2025 with improved dynamic response and ff grit

**Weaknesses:**
- Closed-source, commercial ($149-449)
- Wurlitzer model reportedly lacks the "grittiness" of the real instrument at ff
- No direct-coupled preamp modeling (inferred from tonal characteristics)

**What we can learn:** Pianoteq demonstrates that physical modeling can compete with sampling. Their success comes from modeler-friendly parameterization (physical quantities, not abstract DSP knobs).

### Lounge Lizard (Applied Acoustics) - Pioneer Physical Modeling EP

**Strengths:**
- Early physical modeling EP with both Rhodes and Wurlitzer
- Good parameter control for timbral sculpting
- Reasonable CPU usage

**Weaknesses:**
- Aging codebase (version 4 is several years old)
- Wurlitzer model lacks authenticity at extreme dynamics
- No Linux support

### MrTramp (GSi) - Free Wurlitzer Physical Model

**Strengths:**
- Free, physically modeled Wurlitzer
- Demonstrates that physical modeling for Wurlitzer is feasible

**Weaknesses:**
- Limited parameter control
- Sound quality below commercial alternatives
- Windows only

### Keyscape (Spectrasonics) - Sampling Gold Standard

**Strengths:**
- Deep-sampled real Wurlitzer 200A with round-robin and multiple velocity layers
- Extremely authentic at sampled dynamic levels
- Industry standard for EP sounds

**Weaknesses:**
- 80+ GB install
- Audible velocity layer boundaries (fundamental limitation of sampling)
- No half-damping or partial key release
- No Linux support
- $399

### Our Advantage as Open Source Physical Model

1. **No velocity layers** -- continuous register-dependent velocity curve with pickup nonlinearity providing natural timbral transition
2. **True half-damping** -- the progressive damper model supports partial key release
3. **Open source + Linux** -- no iLok, no 80 GB download, full source access
4. **CLAP native** -- forward-looking plugin format
5. **Tiny install** -- the entire plugin is < 1 MB
6. **Customizable** -- users can modify the preamp model, speaker, tremolo

---

## 25. CLAP Plugin Requirements

### Minimum CLAP Implementation

The plugin must implement these CLAP extensions:

| Extension | Purpose |
|-----------|---------|
| `clap_plugin_audio_ports` | Declare stereo output |
| `clap_plugin_note_ports` | Accept CLAP notes + MIDI |
| `clap_plugin_params` | Expose automatable parameters |
| `clap_plugin_state` | Save/load parameter state |

### Audio Thread Safety

- Zero heap allocation in `process()` callback
- All buffers pre-allocated at initialization
- nih-plug handles parameter thread safety via its `Params` derive macro and `AtomicFloat` types
- FTZ/DAZ typically set by the host DAW

### Event Processing

CLAP requires sample-accurate event processing. The process callback must:

1. Split input events by timestamp
2. Render audio in sub-blocks between events
3. Handle note-on and note-off events (note-choke and NOTE_END are NOT yet implemented)
4. Handle param-value events via nih-plug's smoothed parameter framework

### Parameter Threading

Parameters are managed by nih-plug's `Params` derive macro, which provides thread-safe access via `AtomicFloat` and `AtomicCell` types. The audio thread reads smoothed parameter values; the main thread writes via the framework's parameter handling. No manual atomics or locks needed.

### State Format

State serialization uses nih-plug's built-in state persistence mechanism (JSON-based, handled automatically by the framework). No custom serialization format is needed.

---

## 26. References

### Papers

- Pfeifle, F. (2017). "Real-Time Physical Model of a Wurlitzer and Rhodes Electric Piano." DAFx-17. [PDF](https://www.dafx.de/paper-archive/2017/papers/DAFx17_paper_79.pdf)
- Pfeifle & Bader (2016). "Tone Production of the Wurlitzer and Rhodes E-Pianos." Springer.
- arXiv 2407.17250: "Reduction of Nonlinear Distortion in Condenser Microphones"
- Jatin Chowdhury, "Antiderivative Antialiasing for Nonlinear Waveshaping." [CCRMA](https://ccrma.stanford.edu/~jatin/Notebooks/adaa.html)
- Aalto University: "Oversampling for Nonlinear Waveshaping: Choosing the Right Filters." [PDF](https://aaltodoc.aalto.fi/items/3d3a2f3d-022a-4b48-98a5-a172c79dfb7a)

### Schematics

- [200 Series Schematic](https://www.bustedgear.com/images/schematics/Wurlitzer_200_series_schematics.pdf)
- [200A Series Schematic](https://www.bustedgear.com/images/schematics/Wurlitzer_200A_series_schematics.pdf)

### Circuit Analysis

- [GroupDIY: 200A Preamp](https://groupdiy.com/threads/wurlitzer-200a-preamp.44606/)
- [Busted Gear: 200A Transistors](https://www.bustedgear.com/res_Wurlitzer_200A_transistors.html)
- [DIY Stompboxes: Wurlitzer 200A Preamp Clone](https://www.diystompboxes.com/smfforum/index.php?topic=113560.0)

### Mechanical / Reed

- [Tropical Fish: How Does a Wurlitzer Work](https://www.tropicalfishvintage.com/blog/2019/5/27/how-does-a-wurlitzer-electronic-piano-work)
- [Tropical Fish: 200 vs 200A](https://www.tropicalfishvintage.com/blog/2019/5/27/what-is-the-difference-between-a-wurlitzer-200-and-a-wurlitzer-200a)
- [EP-Forum: Reed Dimensions](https://ep-forum.com/smf/index.php?topic=8418.0)
- [Vintage Vibe: Reed Case Study](https://www.vintagevibe.com/blogs/news/wurlitzer-electric-piano-reeds-case-study)
- [Jupiter Vintage Pianos: Pickup Encyclopedia](https://www.jupitervintagepianos.com/encyclopedia/pickup-wurlitzer/)

### Tremolo / LDR

- [Strymon: Amplifier Tremolo Technology](https://www.strymon.net/amplifier-tremolo-technology-white-paper/)
- [Vactrol Technical Data](https://richardsholmes.com/topics/synth/vactrol-information/)

### DSP / Anti-Aliasing

- [HIIR Library (Laurent de Soras)](https://github.com/music-dsp-collection/hiir)
- [ADAA Experiments (Jatin Chowdhury)](https://github.com/jatinchowdhury18/ADAA)
- [KVR: Oversampling for Nonlinear Waveshaping](https://www.kvraudio.com/forum/viewtopic.php?t=500251)

### Plugin Format

- [CLAP Audio Plugin API](https://github.com/free-audio/clap)
- [CLAP Helpers (C++ wrapper)](https://github.com/free-audio/clap-helpers)

### Competing Products

- [Pianoteq (Modartt)](https://www.modartt.com/pianoteq) -- Physical modeling, includes Wurlitzer
- [Keyscape (Spectrasonics)](https://www.spectrasonics.net/products/keyscape/) -- Sampling, includes Wurlitzer 200A
- [Lounge Lizard (Applied Acoustics)](https://www.applied-acoustics.com/lounge-lizard-ep-4/) -- Physical modeling EP
- [MrTramp (GSi)](https://www.genuinesoundware.com/) -- Free physical model Wurlitzer
