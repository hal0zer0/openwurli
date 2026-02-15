# Signal Chain Architecture: Wurlitzer 200A Physical Model

Complete specification for a physically-accurate Wurlitzer 200A electric piano plugin using modal synthesis, Ebers-Moll preamp modeling, and per-note ML correction. Designed for AI agent consumption: every processing stage is fully specified with formulas, parameter values, and implementation guidance.

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
11. [Stage 9: Tremolo (Mono, Base Rate)](#11-stage-9-tremolo-mono-base-rate)
12. [Stage 10: Volume Control (Mono, Base Rate)](#12-stage-10-volume-control-mono-base-rate)
13. [Stage 11: Power Amplifier (Mono, Base Rate)](#13-stage-11-power-amplifier-mono-base-rate)
14. [Stage 12: Speaker Cabinet (Mono, Base Rate)](#14-stage-12-speaker-cabinet-mono-base-rate)
15. [Stage 13: Output Limiter and Stereo (Mono to Stereo)](#15-stage-13-output-limiter-and-stereo-mono-to-stereo)
16. [Gain Staging Analysis](#16-gain-staging-analysis)
17. [Oversampling Strategy](#17-oversampling-strategy)
18. [Anti-Aliasing Considerations](#18-anti-aliasing-considerations)
19. [Sample Rate Support](#19-sample-rate-support)
20. [Complete Parameter List](#20-complete-parameter-list)
21. [Damper and Release Model](#21-damper-and-release-model)
22. [Polyphony and Voice Management](#22-polyphony-and-voice-management)
23. [Implementation Order](#23-implementation-order)
24. [Lessons from Previous Implementation (Vurli)](#24-lessons-from-previous-implementation-vurli)
25. [Comparison with Existing Plugins](#25-comparison-with-existing-plugins)
26. [CLAP Plugin Requirements](#26-clap-plugin-requirements)
27. [References](#27-references)

---

## 1. Real Instrument Signal Flow

The Wurlitzer 200A is a 64-key electrostatic reed piano (A1/MIDI 33 to C7/MIDI 96). The physical signal path from keypress to speaker output is:

```
Keypress
  -> Hammer mechanism (felt-tipped wooden hammer rises, strikes steel reed)
  -> Reed vibrates (cantilevered spring steel with solder tuning mass at free end)
  -> Electrostatic pickup (reed + shared pickup plate = variable capacitor)
     - Polarizing voltage: ~147V DC via half-wave rectifier
     - Bias network: R-2=2M (RESOLVED Feb 2026), R-3=470k -> R_total = R-2||R-3 = 380k
     - C20 shunt cap: 220 pF (RESOLVED from schematic; see preamp-circuit.md Section 2)
     - ALL 64 reeds share ONE common pickup plate (reed bar assembly)
     - Total system capacitance: ~240 pF at preamp input
  -> Preamp (separate PCB mounted on reed bar in 200A)
     - Two direct-coupled NPN common-emitter stages (TR-1, TR-2)
     - Originally 2N2924, later replaced with 2N5089 (hFE >= 450)
     - +15V DC supply
     - Collector-base feedback caps C-3 = C-4 = 100 pF (verified from schematic)
     - C20 HPF at input (~1903 Hz)
     - Total gain 6.0 dB (2.0x) no tremolo / 12.1 dB (4.0x) tremolo bright [SPICE-measured Feb 2026]
     - Output: 2-7 mV AC at volume pot (Avenson measurement; his replacement design has ~15 dB gain)
  -> Tremolo (LDR optocoupler modulates preamp emitter feedback)
     - LFO (~5.6 Hz twin-T oscillator, TR-3/TR-4) drives LED inside LG-1 optocoupler
     - R-10 (56K) feeds back from output to fb_junct; Ce1 (4.7 MFD) couples fb_junct to TR-1 emitter
     - LDR (LG-1) shunts fb_junct to ground via cable Pin 1 → 50K VIBRATO → 18K → LG-1
     - Modulates preamp GAIN (not post-preamp volume): series-series emitter feedback topology
     - LED ON → LDR low → fb_junct shunted to ground → feedback can't reach emitter → higher gain
     - LED OFF → LDR high → full feedback reaches emitter via Ce1 → lower gain
     - This is gain modulation, producing timbral variation through the tremolo cycle
     - Rate: ~5.5-6 Hz, depth: ~6 dB (SPICE-measured modulation range at max vibrato)
  -> Volume potentiometer
     - Between preamp output and power amp input
     - Output at pot: 2-7 mV AC
  -> Power amplifier (~18-20W Class AB push-pull)
     - TIP35C (NPN) / TIP36C (PNP) output transistors
     - +/-24V rails
     - 0.47 ohm emitter degeneration resistors
     - ~10 mA quiescent bias
  -> Speaker (two oval ceramic drivers in ABS plastic lid; 4"x6" per schematic, commonly cited as 4"x8" — see output-stage.md)
     - Sealed enclosure, resonance ~85 Hz
     - Cone breakup rolloff ~8 kHz
```

### Critical Topology Facts

**All reeds share a single pickup plate.** The reed bar is a long metal assembly where all 64 reeds sit in machined grooves of one continuous pickup plate. Each reed forms its own variable capacitor with the shared plate, but all capacitors sum into a single electrical output. This means:

- The pickup output is inherently the SUM of all active reeds
- Per-reed signal levels are microvolt-scale (tiny capacitance changes)
- The preamp sees the combined signal from all reeds simultaneously
- Polyphonic interaction happens at the electrical level, not the acoustic level

**The preamp is the primary source of even harmonics (H2).** The electrostatic pickup in constant-charge regime is approximately linear (V proportional to gap displacement). The asymmetric clipping of the two-stage direct-coupled BJT preamp -- with different headroom to Vcc vs. Vce_sat -- produces the characteristic even-harmonic "bark." From condenser microphone literature, pickup-generated H2 is approximately -26 dB at mf, well below preamp contribution.

**Direct coupling between stages is the defining preamp feature.** TR-1 collector connects directly to TR-2 base with no coupling capacitor. This means TR-1's DC operating point sets TR-2's bias. Signal-dependent bias modulation creates compression, transient sag, and velocity-dependent timbral change -- all absent from AC-coupled models.

**The tremolo operates WITHIN the preamp stage.** R-10 (56K) feeds back from the output to TR-1's emitter via Ce1 (4.7 MFD coupling cap). The LDR (LG-1) shunts this feedback junction to ground, modulating how much feedback reaches the emitter and thus the closed-loop gain. This is series-series emitter feedback. The signal flow is: preamp (with integrated tremolo gain modulation via emitter feedback) -> volume pot -> power amplifier -> speaker. The volume pot sits after the preamp output and before the power amp.

---

## 2. Plugin Architecture Overview

```
MIDI note-on (key, velocity, channel, note_id)
  -> Voice Allocator (12 voices, oldest-first stealing, releasing-first preference)
     Per-voice processing (base sample rate):
       [A] Modal Reed Oscillator (7 modes, Euler-Bernoulli + tip mass)
       [B] Gaussian Dwell Filter (hammer contact time spectral shaping, sigma=8.0)
       [C] Attack Noise Burst (felt-on-steel impact, 2-5 ms)
       [D] Per-Note Variation (deterministic +-0.8% freq, +-8% amp)
       [E] Electrostatic Pickup (constant-charge, linear with minGap clamp)
     -> Sum all voices (mono)

  Mono shared processing:
     -> 2x Upsample (HIIR polyphase IIR, 12 coefficients, ~100 dB rejection)
        [F] Input Coupling HPF (C20, ~1903 Hz, 1st order)
        [G] Two-Stage Ebers-Moll Preamp WITH INTEGRATED TREMOLO
            Tremolo: LDR (LG-1) + R-10 (56K) modulate feedback ratio
            Stage 1: gain=420, B=38.5, satLimit=10.9V, cutoffLimit=2.05V
              -> Miller pole (~25 Hz open-loop; ~9.9 kHz closed-loop BW no trem)
              -> Direct coupling
            Stage 2: gain=238, B=38.5, satLimit=6.2V, cutoffLimit=5.3V, re=0.456
              -> Miller pole ~3.3 kHz
        [H] DC Block HPF (20 Hz)
     -> 2x Downsample (HIIR polyphase IIR)
     [I] Volume Control (user parameter)
     [K] Power Amplifier (Class AB, optional -- priority MEDIUM)
     [L] Speaker Cabinet (HPF 85 Hz Q=0.75 + LPF 8 kHz Butterworth)
     [M] Output Limiter (soft saturation)
     -> Mono to Stereo duplication
     -> float32 output buffers
```

### Linear vs. Nonlinear Stages

| Stage | Linear? | Needs Oversampling? | Rate |
|-------|---------|-------------------|------|
| Modal Oscillator | Yes (sinusoidal sum) | No (bandlimited by construction) | Base |
| Dwell Filter | Yes (amplitude scaling at note-on) | No (runs once) | N/A |
| Noise Burst | Yes (filtered noise, envelope) | No (broadband, no harmonics generated) | Base |
| Pickup | Nearly linear (minGap clamp is only nonlinearity) | No (mild, rarely triggered) | Base |
| Voice Sum | Yes (addition) | No | Base |
| C20 HPF | Yes (1st order filter) | No | 2x |
| Preamp Stage 1 | NO (exponential + asymmetric soft-clip) | YES | 2x |
| Miller LPF 1 | Yes (1st order) | No (already at 2x) | 2x |
| Preamp Stage 2 | NO (exponential + asymmetric soft-clip) | YES | 2x |
| Miller LPF 2 | Yes (1st order) | No (already at 2x) | 2x |
| DC Block | Yes (1st order HPF) | No (already at 2x) | 2x |
| Tremolo (in preamp feedback) | Mildly nonlinear (modulates preamp gain/distortion) | YES (inside preamp oversampled block) | 2x |
| Volume | Yes (gain scaling) | No | Base |
| Power Amp | Mildly nonlinear (crossover distortion) | Marginal (2x sufficient) | Base |
| Speaker | Yes (biquad filters) | No | Base |
| Output Limiter | Mildly nonlinear (tanh/soft-clip) | No (at output, minimal aliasing concern) | Base |

**Conclusion:** Only the preamp requires oversampling. 2x is sufficient because the preamp's input signal is already bandlimited by the C20 HPF (~1903 Hz) and the pickup's natural bandwidth. The preamp generates harmonics, but with the C20 HPF removing fundamentals below ~1903 Hz, the highest-energy harmonics that could alias are well below Nyquist at 2x.

---

## 3. Stage 1: MIDI Input and Voice Allocation

### Input Events

The plugin accepts both CLAP native note events and MIDI 1.0. CLAP note events carry floating-point velocity [0.0, 1.0]; MIDI velocity bytes are normalized by dividing by 127.

### Voice Pool

- **12 pre-allocated voices** (zero heap allocation in audio callback)
- States: `FREE` -> `HELD` -> `RELEASING` -> `FREE`
- All voice data is pre-allocated in a fixed-size array

### Allocation Strategy

1. Search for first `FREE` voice
2. If none free, steal the voice with the smallest `age` value (oldest note)
3. Prefer stealing `RELEASING` voices over `HELD` voices
4. On steal: immediately set stolen voice to `FREE`, then initialize new note

### Note-On Processing

At note-on, the following happens once (not per-sample):
1. Compute mode frequencies from fundamental and interpolated mode ratios
2. Apply Gaussian dwell filter to mode amplitudes
3. Apply per-note variation (deterministic hash)
4. Compute decay rates
5. Initialize pickup gap
6. Initialize noise burst parameters
7. Optionally: run MLP correction network to adjust parameters

### Note-Off Processing

At note-off:
1. Transition voice to `RELEASING` state
2. Bake current amplitude (fold elapsed decay into base amplitudes)
3. Reset time counter
4. Compute per-mode damper rates
5. Top 5 keys (MIDI >= 92): no damper, natural decay only

### Voice Death Detection

A voice is dead and can be freed when:
- State is `RELEASING` AND release time > 0.1s AND all mode amplitudes < 1e-6
- OR release time > 10.0s (safety timeout)

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

Mode ratios are interpolated linearly by MIDI note between three register anchors:

| Anchor | MIDI | Ratios [mode 0-6] |
|--------|------|-------------------|
| Bass | 33 | 1.0, 6.3, 17.9, 35.4, 58.7, 88.0, 123.0 |
| Mid | 60 | 1.0, 6.8, 19.5, 39.0, 64.0, 96.0, 134.0 |
| Treble | 84 | 1.0, 6.3, 17.6, 34.5, 57.0, 85.0, 118.0 |

These ratios derive from solving the Euler-Bernoulli characteristic equation `1 + cos(L)cosh(L) + L*mu*(cos(L)sinh(L) - sin(L)cosh(L)) = 0` with estimated tip-mass ratios (mu). Bare beam ratios (6.267, 17.55, 34.39, 56.84) are the minimum; solder mass INCREASES ratios above these values.

**Important:** Mode frequencies above 0.45 * sampleRate should be zeroed to prevent aliasing. This primarily affects modes 5-6 at the highest notes.

### Mode Amplitudes

Base amplitudes by register (before dwell filter and velocity scaling):

| Register | Mode amplitudes [0-6] |
|----------|----------------------------------------------|
| Bass | 0.35, 0.10, 0.030, 0.015, 0.010, 0.006, 0.004 |
| Mid | 0.18, 0.079, 0.018, 0.010, 0.006, 0.004, 0.002 |
| Treble | 0.12, 0.057, 0.014, 0.008, 0.004, 0.002, 0.002 |

These approximate 1/omega scaling (physical modal participation from a velocity impulse at the free end). With the corrected Gaussian dwell filter (sigma=8.0), the fundamental receives negligible attenuation, so no mode 1 boost compensation is needed.

### Velocity Scaling

All modes scale uniformly with `velMapped = velocity^2`. The quadratic mapping widens the usable dynamic range:
- pp (vel=0.3): velMapped = 0.09
- mf (vel=0.7): velMapped = 0.49
- ff (vel=0.95): velMapped = 0.90

Timbral brightening at ff comes from two sources that do NOT require per-mode velocity exponents:
1. Shorter dwell time at ff -> dwell filter passes more upper partials
2. Preamp nonlinearity -> louder signal generates more harmonics

**Lesson from Vurli:** Per-mode velocity exponents (`pow(vel, 1 + (curve-1)*(m/6)^0.6)`) double-count with the dwell filter. Both brighten at ff. Remove the per-mode exponent entirely.

### Decay

```
decay_time = base_decay * min(2^((69 - key) / 12), 10.0)
decay_rate[m] = 1.0 / (decay_time * decay_scale[m])
```

- `base_decay`: 1.1 seconds (geometric mean fit to OldBassMan 200A calibration data across 11 notes; see reed-and-hammer-physics.md Section 5.7)
- Decay scales: [1.0, 0.20, 0.08, 0.05, 0.03, 0.02, 0.015]
- Higher modes decay faster -> timbre darkens over time (bright attack, sine-like tail)
- Register scaling `/12` gives approximate doubling every octave

> **Correction (Feb 2026):** base_decay reduced from 1.6s to 1.1s. The previous value gave 3.6 dB/s at D4, nearly half the measured 6.2 dB/s. With 1.1s, the model gives 5.3 dB/s at D4, within the +/-30% calibration tolerance. Decay scales updated from [1.0, 0.55, 0.30, 0.18, 0.10, 0.06, 0.035] to physics-derived values based on constant-Q damping with a mounting-loss floor (reed-and-hammer-physics.md Section 5.8). The previous values had modes 2-4 decaying 3-6x too slowly, causing upper partials to persist unnaturally long.

Calibration target from OldBassMan 200A recordings: `decay_rate_dB_per_sec = 0.26 * exp(0.049 * MIDI)` with +/-30% tolerance.

### Per-Sample Rendering

```cpp
for each sample:
    signal = 0.0
    for each mode m:
        envelope = amps[m] * exp(-decayRates[m] * time)
        // Add damper if releasing (see Section 21)
        signal += envelope * sin(phases[m])
        phases[m] += 2*PI * freqs[m] * dt
        if phases[m] >= 2*PI: phases[m] -= 2*PI
```

### Phase Initialization

Reed starts at zero displacement (hammer imparts velocity, not displacement):
```
phase[m] = fmod(PI * freq[m] * t_dwell, 2*PI)
```
The dwell offset accounts for phase accumulated during hammer contact.

### Attack Overshoot: Let Physics Handle It

The previous implementation used an artificial multiplicative envelope `(1 + X * exp(-alpha*t))` with no physical basis. This was the root cause of the R40 regression where 20x mode amplitude compensation destroyed the attack-to-sustain ratio.

**Correct approach:** With physically accurate 1/omega mode amplitudes, all modes start in-phase at t=0 and upper modes decay faster. The sum of all modes at t=0 is larger than the sustained fundamental-only signal. This naturally produces 2-4 dB overshoot at mf and 4-8 dB at ff without any artificial envelope. The attack character emerges from modal superposition, which is exactly how it works in the real instrument.

Do NOT add an artificial overshoot envelope. If natural overshoot is insufficient, the mode amplitude ratios or dwell filter parameters are wrong.

---

## 5. Stage 3: Hammer Dwell Filter (Per-Voice)

The hammer contact time creates a finite-duration force pulse that spectrally shapes the initial mode excitation.

### Dwell Time

```
t_dwell = 0.001 + 0.003 * (1.0 - velocity)
```

- ff (vel ~0.95): ~1.15 ms (shorter contact, brighter)
- mf (vel ~0.7): ~1.9 ms
- pp (vel ~0.3): ~3.1 ms (longer contact, darker)

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

> **Correction (Feb 2026):** sigma increased from 2.5 to 8.0, per the analysis in reed-and-hammer-physics.md Sections 4.3.3-4.3.4. With sigma=2.5, mode 3 was attenuated by -33 dB and mode 4 by -127 dB at mf -- effectively zeroing all modes above mode 2. This is far too aggressive for a felt hammer, which has a broad spectral envelope. With sigma=8.0, attenuation at mf (C4, t_dwell=1.9ms) is: mode 2 = -0.8 dB, mode 3 = -6.4 dB, mode 4 = -25.6 dB. This preserves mode 3's "metallic clang" contribution to the attack transient while still rolling off the negligible higher modes. The half-sine model was also considered but rejected because its spectral nulls at f*T = 2.5, 3.5, etc. cause unpredictable attenuation swings with velocity -- the same problem that plagued the sinc dwell filter (see Vurli lessons, Section 24).

### Why Normalization to Fundamental Matters

Without normalization, the dwell filter would also attenuate the fundamental, changing the overall volume with velocity. Normalizing to the fundamental ensures mode 0 always passes at unity, and the filter only shapes the relative amplitudes of upper modes.

---

## 6. Stage 4: Attack Noise Burst (Per-Voice)

The felt-tipped hammer striking a steel reed produces a broadband impact noise that lasts 2-5 ms. This is separate from the modal vibration.

```
noise_amp = max(vel^2, 0.10) * 0.15 * register_scale
noise_decay = 1/0.003  // 3 ms time constant
noise_cutoff = 2000 + 6000 * vel  // velocity-dependent bandwidth
```

- `register_scale = clamp(2^((48 - key) / 24), 0.4, 1.0)` -- bass has more mechanical energy, treble noise must not dominate the modal signal
- LCG pseudo-random generator -> one-pole LPF at `noise_cutoff` -> exponential decay envelope
- Added to the voice signal BEFORE the pickup model

The noise burst is subtle but contributes to the "woody" percussive attack character. Without it, notes have an artificially pure, synthesizer-like onset.

---

## 7. Stage 5: Per-Note Variation (Per-Voice)

Real Wurlitzers have per-note personality from manufacturing tolerance: solder placement, reed alignment, gap variation. This is deterministic (the same note always sounds slightly different from its neighbors, but consistent across strikes).

```
// Deterministic hash seeded by note number (NOT random per strike)
freq_variation[m] = 1.0 + hash(key, m, 0) * 0.008  // +/-0.8% on modes 1+
amp_variation[m] = 1.0 + hash(key, m, 1) * 0.08    // +/-8% per mode
```

- Fundamental (mode 0) stays precisely tuned (no frequency variation)
- Upper modes get +/-0.8% frequency spread (solder mass varies slightly)
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
- **System RC corner:** f_c = 1/(2*PI*287k*240pF) = 2312 Hz -> bass fundamentals in constant-voltage regime (R_total = R_feed||(R-1+R_bias) = 1M||402K = 287K; R_feed = 1 MEG RESOLVED Feb 2026, Avenson's 499K is replacement design; see pickup-system.md Section 3.7)

The per-reed constant-charge approximation is a defensible engineering tradeoff because the C20 input HPF at ~1903 Hz provides similar bass rolloff to the system-level RC dynamics. For the plugin, model the pickup as linear (constant-charge) and let the C20 HPF handle bass shaping.

### Constant-Charge Pickup Model

In constant-charge regime, V_ac is proportional to gap displacement (linear):

```cpp
d0 = pickup_d0 * gap_scale  // base gap, register-scaled
min_gap = d0 * 0.20         // reed can't hit plate (20% minimum)
gap = max(d0 + signal + offset, min_gap)
pickup = gap - d0            // = signal + offset when not clamped
output = signal * (1 - mix) + pickup * mix  // mix=1.0 for full pickup
```

The ONLY nonlinearity is the minGap clamp (reed approaching plate). In normal playing this is rarely triggered. The pickup is effectively a pass-through with a DC offset.

### Gap Scaling by Register

Real Wurlitzer bass reeds have wider pickup gaps than treble reeds:

| Register | Measured slot width | Ratio to mid |
|----------|-------------------|-------------|
| Bass (reeds 1-14) | 0.172" | 1.24x |
| Mid (reeds 21-42) | 0.139" | 1.00x |
| Treble (reeds 51-64) | 0.114" | 0.82x |

Model: `gap_scale = 2^((60 - key) / 60)` gives bass:treble ratio of ~1.74:1 (close to measured 1.51:1).

### Parameters

| Parameter | Default | Range | Purpose |
|-----------|---------|-------|---------|
| `pickup_d0` | 0.50 | 0.3-6.0 | Base capacitive gap |
| `pickup_mix` | 1.0 | 0.0-1.0 | 1.0 = full constant-charge pickup |
| `pickup_offset` | -0.10 | -0.5 to 0.5 | DC offset (asymmetry correction) |

---

## 9. Stage 7: Voice Summation (Per-Voice to Mono)

All active voices render into a shared mono buffer via addition:

```cpp
memset(mono_buffer, 0, frames * sizeof(double))
for each active voice:
    voice.renderBlock(mono_buffer, frames, ...)  // += into buffer
```

This matches the real 200A topology: all reeds sum into one pickup plate, producing a single mono signal that feeds the preamp.

### Signal Level After Summation

At mf with a single voice, the summed output is approximately 0.05-0.15 (arbitrary units). With 6 voices (chord), the sum is 0.3-0.9. These levels need to be scaled to the correct range for the preamp input.

---

## 10. Stage 8: Oversampling and Preamp (Mono, 2x Rate)

This is the most complex and sonically important processing stage. The preamp is the primary source of the Wurlitzer's characteristic even-harmonic "bark."

### Oversampling Wrapper

The preamp runs at 2x the base sample rate inside an HIIR polyphase IIR oversampler:

1. **Upsample:** HIIR 2x upsampler (12 coefficients, ~100 dB stopband rejection, transition band 0.01)
2. **Process:** Run C20 HPF + Stage 1 + Miller LPF + Stage 2 + Miller LPF + DC Block at 2x rate
3. **Downsample:** HIIR 2x downsampler (matching filter)

The oversampler uses minimum-phase IIR filters (HIIR library), which have lower latency than linear-phase FIR alternatives at the cost of slight phase distortion. For a musical instrument, the phase behavior is inaudible and the reduced latency is beneficial.

### Input Drive

The voice summation output must be scaled to the correct amplitude range for the preamp's Ebers-Moll exponential to produce physically accurate nonlinearity:

```cpp
preamp_input = voice_sum * kPreampInputDrive
preamp_output = preamp.processSample(preamp_input) * preampGain
```

**Critical gain staging issue from Vurli:** The `kPreampInputDrive` parameter is an artificial scaling factor that compensates for the mismatch between the plugin's internal signal levels and the real millivolt-scale signals in the physical circuit. In the real 200A, the pickup generates microvolt signals that the preamp amplifies by ~15 dB. In the plugin, the oscillator produces much larger signals (0.05-0.9 range), requiring a drive factor to place them in the correct regime of the Ebers-Moll curve.

**The correct approach is to ensure that at mf single-note:**
- `B * preamp_input` should be 1-3 (mild nonlinearity)
- At pp: `B * input` ~ 0.1-0.5 (nearly linear)
- At ff: `B * input` ~ 5-10 (moderate saturation)
- At ff 6-note chord: `B * input` ~ 15-30 (heavy saturation, compression)

With B = 38.5 V^-1, this means the preamp input signal should be in the range 0.003V (pp) to 0.5V (ff chord). The kPreampInputDrive value should be calibrated to produce these input levels. A value of 28.0 with the current oscillator output levels puts mf single-note in the 0.03-0.08 * 28 = 0.8-2.2 range after C20 HPF attenuation, which is reasonable.

### C20 Input Coupling HPF

First-order HPF at ~1903 Hz. Models the C20 shunt capacitor at the preamp input (C20 = 220 pF, RESOLVED from schematic at 1500 DPI; R = 380K):

```
RC = 1 / (2*PI*1903)   // f_c20 = 1903 Hz (C20 = 220 pF, R = 380K)
alpha = RC / (RC + dt)  // where dt = 1/(2*sampleRate) at 2x rate
y[n] = alpha * (y[n-1] + x[n] - x[n-1])
```

This is the primary bass rolloff mechanism. At C2 (65 Hz), attenuation is approximately -27 dB. At C4 (262 Hz), approximately -15 dB. The C20 HPF profoundly shapes the Wurlitzer's tonal balance: bass fundamentals are heavily attenuated, leaving the preamp-generated H2 (130 Hz for C2) as the dominant component in the bass register.

### BJT Stage Model

Each stage implements the Ebers-Moll exponential transfer function solved by Newton-Raphson iteration:

```
// Implicit equation:
raw = A * expm1(B * (input_eff - effectiveRe * raw))

// Where:
//   A = gain / B (normalizes small-signal gain)
//   B = 1/(n*Vt) = 38.5 V^-1 (physical thermal voltage)
//   effectiveRe = re + feedbackBeta (emitter degeneration + cap feedback)
//   input_eff = input + feedbackBeta * fbCapState (cap feedback signal)

// Newton-Raphson (3 iterations):
for iter in 0..2:
    arg = clamp(B * (input_eff - effectiveRe * raw), -20, 20)
    exp_arg = exp(arg)
    f = A * (exp_arg - 1) - raw
    df = -A * B * effectiveRe * exp_arg - 1
    raw -= f / df
```

Followed by asymmetric exponential soft-clip (collector rail limits):
```
if raw >= 0: output = satLimit * (1 - exp(-raw / satLimit))     // toward Vcc
if raw < 0:  output = -cutoffLimit * (1 - exp(raw / cutoffLimit)) // toward Vce_sat
```

**H2 mechanism:** The asymmetric soft-clip produces even harmonics because satLimit >> cutoffLimit (e.g., 10.9V vs 2.05V for Stage 1, ratio ~5.3:1). The negative side clips much harder, creating asymmetric compression whose Taylor expansion includes an x^2 term. This is the primary H2 source. The exponential nonlinearity itself is largely linearized by the NR feedback and contributes relatively little H2.

### Stage 1 Parameters

| Parameter | Value | Physical Basis |
|-----------|-------|---------------|
| gain | 420 (max) | gm1 × Rc1 = 2.80 mA/V × 150K (open-loop, fb_junct grounded — see note) |
| B | 38.5 | 1/(n*Vt), n~1.0 for 2N5089 |
| satLimit | 10.9 V | Vcc - Vc1 = 15 - 4.1 |
| cutoffLimit | 2.05 V | Vc1 - Ve1 - Vce_sat = 4.1 - 1.95 - 0.1 |
| re | depends on fb_junct Z | Ce1 (4.7 μF) couples emitter to fb_junct (NOT a simple bypass to ground); effective re depends on LDR path impedance |

### Collector-Base Feedback Caps

**IMPORTANT: The previous implementation had feedback cap polarity BACKWARDS.**

In the real circuit, the collector-base capacitor creates Miller-effect negative feedback:
- At HIGH frequencies: cap impedance is low -> MORE current from collector to base -> MORE feedback -> LESS gain
- At LOW frequencies: cap impedance is high -> LESS feedback -> FULL gain

The correct model:
```
// Cap state tracks the DIFFERENCE between output and input (AC component)
// At HF: cap can track fast changes -> provides feedback
// At LF: cap charges fully -> no AC feedback

// Corner frequency from Miller multiplication:
// f_miller = 1 / (2*PI * Ccb * (1+Av) * R_source)
// For Stage 1: C-3=100pF, Av=420 -> C_miller=42,100pF -> f_dominant ~25 Hz

// Implementation:
hf_feedback = output - fbCapState  // HF component (what cap can't track)
fbCapState += fbCapCoeff * (output - fbCapState)  // LPF tracks output

// Apply as degeneration:
effectiveRe = re + feedbackBeta * (something proportional to HF content)
```

Target corner frequencies based on physical Miller multiplication (C-3 = C-4 = 100 pF verified):
- Stage 1: ~25 Hz open-loop dominant pole (C-3=100pF × (1+420) = 42,100 pF Miller-multiplied)
- Stage 2: ~3.3 kHz (C-4=100pF × (1+2.2) = 320 pF, into 150K source impedance)
- Closed-loop bandwidth: **~10 kHz** (no tremolo) / **~8.3 kHz** (tremolo bright) [SPICE-MEASURED Feb 2026]

### Miller LPF (After Each Stage)

First-order LPF modeling Miller-effect bandwidth limitation. With verified C-3 = C-4 = 100 pF:
- After Stage 1: dominant pole at ~25 Hz open-loop; closed-loop BW ~9.9 kHz (no trem) / ~8.3 kHz (trem bright)
- After Stage 2: ~3.3 kHz (Stage 2 has low gain of ~2.2, so Miller multiplication is mild)

> **Note (Feb 2026):** The corrected values show Stage 1's Miller pole is much lower than previously estimated (~25 Hz vs ~200-500 Hz). This is the dominant open-loop pole. With the global feedback loop (R-10 = 56K via Ce1 to emitter), the SPICE-measured closed-loop bandwidth is ~9.9 kHz (no tremolo) / ~8.3 kHz (tremolo bright). GBW/Acl = ~21 kHz / 2.0 = 10.5 kHz, consistent with SPICE. The plugin's per-stage Miller LPF implementation may need restructuring to properly model this feedback topology — see preamp-circuit.md Sections 5-6 for full analysis.

### Stage 2 Parameters

| Parameter | Value | Physical Basis |
|-----------|-------|---------------|
| gain | 238 | gm2 × Rc2 = 132 mA/V × 1.8K (open-loop) |
| B | 38.5 | Same BJT thermal voltage |
| satLimit | 6.2 V | Vcc - Vc2 = 15 - 8.8 |
| cutoffLimit | 5.3 V | Vc2 - Ve2 - Vce_sat = 8.8 - 3.4 - 0.1 |
| re | 0.456 | Re2_unbypassed / Rc2 = 820Ω / 1.8K |

### Direct Coupling Dynamics

Stage 1 output feeds Stage 2 input directly. At physical millivolt signal levels, DC shifts from asymmetric clipping are small. However, for accurate dynamics, the direct coupling should produce:

1. **Signal-dependent bias modulation:** At ff, Stage 1's average collector voltage sags -> shifts Stage 2 toward cutoff -> compression
2. **Transient sag:** Hard attacks momentarily shift bias, then recover over 10-100ms
3. **Velocity-dependent timbral change:** Stage 2 operates at different gain/distortion regimes depending on Stage 1's bias shift

This can be approximated with an envelope follower on Stage 1's output that modulates Stage 2's operating point. Full physical modeling would track the actual DC operating point through the circuit, but the envelope approximation captures the audible effects.

### DC Block

First-order HPF at 20 Hz after Stage 2. Removes residual DC from asymmetric clipping.

---

## 11. Tremolo — Integrated in Preamp Emitter Feedback Loop

**CORRECTION (Feb 2026):** Previous versions of this document described the tremolo as a post-preamp "shunt-to-ground" signal divider. This was WRONG. A subsequent correction placed R-10 at node_A (shunt-feedback to the input), which was ALSO WRONG (based on the wrong 200/203 schematic). The correct 200A topology: R-10 feeds back to TR-1's EMITTER via Ce1 (series-series emitter feedback). See preamp-circuit.md Section 7 for detailed analysis.

The 200A tremolo modulates the preamp's closed-loop gain via an LDR (LG-1) that shunts the emitter feedback junction to ground. R-10 (56K) feeds back from the output to fb_junct; Ce1 (4.7 MFD) AC-couples fb_junct to TR-1's emitter. The LDR path (cable Pin 1 → 50K VIBRATO → 18K → LG-1 → GND) diverts feedback current away from the emitter. This is **gain modulation**, not simple amplitude modulation — the distortion character changes through the tremolo cycle.

### LFO (Twin-T Oscillator, TR-3/TR-4)

The oscillator is a twin-T (parallel-T) notch filter oscillator, NOT a phase-shift oscillator as previously documented. SPICE-validated at 5.63 Hz with 11.8 Vpp output swing. See `spice/subcircuits/tremolo_osc.cir` and `docs/output-stage.md` Section 2.1 for full topology.

```
lfo = sin(2*PI * rate * t)
led_drive = max(0, lfo)  // half-wave rectified (LED only conducts forward)
```

### LDR Response (Asymmetric Attack/Release)

```
if led_drive > ldr_state:
    tau = 0.003  // 3ms attack (LED on -> resistance drops fast)
else:
    tau = 0.050  // 50ms release (LED off -> resistance recovers slowly)

alpha = dt / (tau + dt)
ldr_state += alpha * (led_drive - ldr_state)
```

### CdS Nonlinearity and Emitter Feedback Modulation

```
// LDR resistance from CdS power-law response
R_ldr = R_dark * pow(ldr_state + epsilon, -gamma)  // gamma ~ 0.7-0.9

// LDR path impedance: fb_junct -> Pin 1 -> 50K VIBRATO -> 18K -> LDR -> GND
R_ldr_path = vibrato_pot * depth_setting + 18000 + R_ldr

// Emitter feedback: R-10 (56K) from output to fb_junct, Ce1 couples to emitter
// LDR path shunts fb_junct to ground, diverting feedback away from emitter
// When LDR path low (LED on): fb_junct grounded -> emitter AC-grounded via Ce1 -> higher gain
// When LDR path high (LED off): full feedback reaches emitter -> lower gain

// Modulate preamp emitter feedback with LDR path impedance
// At low preamp drive: gain modulation ≈ amplitude modulation
// At high preamp drive: gain modulation also changes distortion character
```

### Character

The asymmetric attack/release creates a "choppy" effect: fast dips (3ms), slow recovery (50ms). This is distinctly different from a smooth sine tremolo and is immediately recognizable as Wurlitzer.

**Timbral modulation:** At the high-gain phase (LDR dark), the preamp is driven harder, producing more harmonic distortion ("bark"). At the low-gain phase (LDR lit), the preamp operates more linearly. This subtle but important timbral variation distinguishes the real 200A tremolo from a simple volume multiplier.

### Implementation Note

Because the tremolo modulates the preamp's emitter feedback (via the LDR shunt at fb_junct), it must be implemented INSIDE the preamp processing block (within the 2x oversampled domain), not as a separate post-preamp stage. The LDR state updates at the base sample rate, but the emitter feedback modulation applies per-sample at 2x rate.

### Parameters

| Parameter | Default | Range | Notes |
|-----------|---------|-------|-------|
| Rate | 5.5 Hz | 0.1-15.0 | Most real instruments 5.3-7 Hz |
| Depth | 0.5 | 0.0-1.0 | 0=off, 0.5 ~ 4.5 dB dip, 1.0 ~ 9 dB dip |

---

## 12. Stage 10: Volume Control (Mono, Base Rate)

In the real 200A, the volume potentiometer sits between the preamp output and the power amplifier input. In the plugin:

```
output = input * masterVolume
```

This is a simple linear gain. The `masterVolume` parameter default of 0.05 reflects the typical attenuation needed to bring the preamp's output level into a reasonable range for the DAW. In the real instrument, the volume pot output is measured at 2-7 mV AC.

---

## 13. Stage 11: Power Amplifier (Mono, Base Rate)

**Priority: MEDIUM. The power amp is transparent at moderate levels but matters for ff polyphonic saturation and aged-instrument character.**

The real 200A has a ~18-20W quasi-complementary push-pull Class AB output stage:

- Input differential pair: 2N5087 (PNP)
- Vbe multiplier for bias: MPSA06
- Output: TIP35C (NPN) / TIP36C (PNP), +/-24V rails
- Emitter degeneration: 0.47 ohm
- Quiescent bias: ~10 mA

### Minimal Model

```
// Crossover distortion (dead zone between NPN/PNP conduction)
if abs(input) < crossover_width:
    ratio = abs(input) / crossover_width
    output = copysign(abs(input) * ratio^2, input)
else:
    output = input

// Rail clipping (asymmetric)
output = soft_clip(output, +rail_limit, -rail_limit)
```

At mf single notes, the power amp is nearly transparent -- the preamp dominates tonal character. At ff polyphonic, the power amp's own clipping adds compression and saturation. With aging, bias drifts, increasing crossover distortion (odd harmonics from the dead zone).

### Implementation Priority

For a first release, a simple soft-clip at the output is sufficient. The crossover distortion model adds realism for vintage/aged presets but is not essential for the core Wurlitzer sound.

---

## 14. Stage 12: Speaker Cabinet (Mono, Base Rate)

The 200A uses two oval ceramic speakers in the ABS plastic lid (sealed enclosure). The schematic specifies 4"x6" oval, 16Ω each (part #202243), though many online sources cite 4"x8" — see output-stage.md for discussion.

### Model

Two biquad filters (Direct Form II Transposed):

1. **Sealed-box resonance:** 2nd-order HPF at 85 Hz, Q=0.75
   - Slightly underdamped (Q=0.75 > 0.707) matches small sealed enclosure
   - Attenuates C2 fundamental (65 Hz) by ~5.4 dB
   - Leaves H2 (130 Hz) nearly untouched
   - This is a significant contributor to bass register H2/H1 balance

2. **Cone breakup rolloff:** 2nd-order LPF at 8000 Hz, Q=0.707 (Butterworth)
   - Set above the preamp Miller LPFs to avoid stacking
   - Models speaker cone's own breakup, not preamp bandwidth

### Coefficient Computation

Use the Audio EQ Cookbook (Robert Bristow-Johnson) formulas. Recompute coefficients when sample rate changes (in `activate()`).

---

## 15. Stage 13: Output Limiter and Stereo (Mono to Stereo)

### Soft Limiter

```
output = tanh(input)  // or equivalent soft saturation
```

At the signal levels reaching this point (after volume control), this is effectively transparent -- providing only safety limiting against extreme transients. The tanh function at typical signal levels (< 0.5) introduces less than 0.04 dB of compression.

**Note:** The previous implementation used `tanh(signal * masterVol)`, combining volume and limiting. This is functionally a volume control with negligible saturation, since `tanh(0.05) ~ 0.05`.

### Stereo Output

The Wurlitzer 200A is a mono instrument. The plugin duplicates the mono signal to both stereo channels:

```cpp
outL[i] = float(mono_signal)
outR[i] = float(mono_signal)
```

Optional enhancement: slight stereo widening via a short decorrelation delay (e.g., 0.2ms on one channel) or mid-side processing. But the authentic sound is mono.

---

## 16. Gain Staging Analysis

This section traces signal levels through the entire chain, identifying where levels are too high or too low and where the artificial `kPreampInputDrive` compensation originates.

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

| Point in Chain | Signal Level (arbitrary units) | Notes |
|---------------|-------------------------------|-------|
| Single voice, mf | ~0.05-0.15 | After pickup |
| 6-voice chord, ff | ~0.3-0.9 | Sum of voices |
| After kPreampInputDrive (28x) | ~1.4-25 | Into preamp |
| After C20 HPF (varies by freq) | ~0.1-5 | Bass heavily attenuated |
| After preamp Stage 1 | ~0.5-11 | Depends on gain/feedback |
| After preamp Stage 2 | ~0.5-6.5 | Clipped by soft-limits |
| After preampGain (0.7x) | ~0.35-4.5 | Into tremolo/speaker |
| After masterVol (0.05x) | ~0.02-0.22 | Into output |

### The kPreampInputDrive Problem

The artificial `kPreampInputDrive` exists because the plugin's oscillator produces signals at arbitrary scale (0.05-0.9), not at the millivolt scale of the real circuit. This is NORMAL for virtual analog -- nobody models actual millivolt signals in floating point (unnecessary precision waste).

The problem is when `kPreampInputDrive` is treated as a tuning knob. In Vurli's history:
- R39: kPreampInputDrive = 16.0 (with B=2.5 -> too linear)
- R40: kPreampInputDrive = 48.0 (with B=38.5 -> crushes dynamic range at mf)
- Current: kPreampInputDrive = 28.0 (compromise)

**The correct approach:** Set kPreampInputDrive ONCE based on the desired operating point (B * input ~ 1-3 at mf), then NEVER change it during tuning. All tonal adjustments should come from physically motivated parameters (gain, feedback cap values, soft-clip limits). If the sound is wrong, the preamp model is wrong -- do not compensate by changing the input drive.

> **Note (Feb 2026):** The preamp gain structure has been corrected and SPICE-measured: Stage 1 max open-loop gain is ~420, Stage 2 is ~2.2. Combined max open-loop gain ~912. Feedback topology: R-10 via Ce1 to TR-1 emitter (series-series emitter feedback). **SPICE-measured closed-loop gain: 6.0 dB (2.0x) without tremolo, 12.1 dB (4.0x) at tremolo bright peak. BW: ~10 kHz (no trem) / ~8.3 kHz (trem bright).** The kPreampInputDrive value of 28.0 was calibrated against the old incorrect values and **will need recalibration** once the corrected parameters are implemented.

### Recommendations

1. Normalize oscillator output so that mf single-voice peaks at ~1.0
2. Set kPreampInputDrive = 1/(B * desired_mf_input) to place mf at B*x ~ 2
3. With B=38.5, desired mf input of ~0.05 -> kPreampInputDrive ~ 0.05/1.0 = 0.05... but the C20 HPF attenuation must be factored in
4. Alternatively: scale oscillator output to match real millivolt levels and eliminate kPreampInputDrive entirely

---

## 17. Oversampling Strategy

### What Needs Oversampling

Only the preamp requires oversampling. It is the only significantly nonlinear stage that generates harmonics above the input signal's bandwidth.

The pickup is nearly linear (minGap clamp rarely triggered). The output limiter operates on an already band-limited signal at low levels. The power amp crossover distortion, if modeled, generates only low-order odd harmonics at small signal levels.

### Why 2x Is Sufficient

The preamp's input is pre-filtered by the C20 HPF at ~1903 Hz. This means:
- The highest-energy input component is around 1903-4000 Hz (fundamental of mid/treble register, or H2 of bass)
- The preamp generates harmonics at 2x, 3x, 4x, ... of this input
- At 48 kHz base rate, 2x oversampling gives 96 kHz processing rate with 48 kHz Nyquist
- H8 of a 4 kHz input = 32 kHz, safely below 48 kHz Nyquist
- H12 of a 4 kHz input = 48 kHz, at Nyquist -- but H12 is typically -50 dB or lower

For 44.1 kHz base rate, 2x gives 88.2 kHz with 44.1 kHz Nyquist. Still adequate because the C20 HPF limits input bandwidth.

### Filter Choice: HIIR Polyphase IIR

- Library: HIIR (Laurent de Soras)
- Architecture: Polyphase IIR half-band filter
- Coefficients: 12 (steep transition, ~100 dB stopband rejection)
- Transition band: 0.01 of half-band
- Phase: Minimum-phase (lower latency than linear-phase FIR)
- CPU cost: Very efficient -- only multiply-accumulate operations, no table lookups

### Alternative: ADAA (Anti-Derivative Anti-Aliasing)

ADAA can reduce aliasing without oversampling by computing the antiderivative of the nonlinear function and using it to perform continuous-time convolution. Research shows 2x oversampling + ADAA provides aliasing suppression comparable to 6x oversampling without ADAA.

However, ADAA requires the nonlinear function to have a closed-form antiderivative. The Ebers-Moll exponential with Newton-Raphson solver and feedback caps is too complex for straightforward ADAA application. HIIR 2x oversampling is simpler and sufficient.

---

## 18. Anti-Aliasing Considerations

### Modal Oscillator

The oscillator is alias-free by construction: each mode is a pure sinusoid at a known frequency. Modes above 0.45 * sampleRate are zeroed at note-on. No anti-aliasing required.

### Pickup Nonlinearity

The minGap clamp is a mild nonlinearity that generates low-level harmonics only when the reed approaches the plate (rare, only at extreme ff in bass register). The aliasing from this is negligible.

### Preamp

Addressed by 2x oversampling (Section 17). The 100 dB stopband rejection of the HIIR filter means aliased components are at -100 dB -- well below the noise floor.

### Output Limiter

The tanh limiter at the output operates on a signal that has already been through the speaker cabinet LPF (8 kHz cutoff). Any harmonics generated by the tanh are above 16 kHz and inaudible. No oversampling needed.

### Denormal Protection

After the preamp, decaying voices produce very small signal values that can become denormal floating-point numbers, causing CPU spikes on x86 processors. Set FTZ (Flush-to-Zero) and DAZ (Denormals-Are-Zero) bits in the MXCSR register at the start of the process callback:

```cpp
unsigned int old_mxcsr = _mm_getcsr();
_mm_setcsr(old_mxcsr | 0x8040);  // FTZ (0x8000) | DAZ (0x0040)
// ... process audio ...
_mm_setcsr(old_mxcsr);  // restore at end
```

---

## 19. Sample Rate Support

The plugin must support at minimum: 44100, 48000, 88200, 96000 Hz. Higher rates (176400, 192000) are desirable but not critical.

### What Changes With Sample Rate

| Component | Sample Rate Dependence |
|-----------|----------------------|
| Oscillator phase increment | `2*PI*freq/sampleRate` |
| All filters (HPF, LPF, biquads) | Coefficients recomputed in `activate()` |
| Oversampler | Operates at 2x whatever the base rate is |
| Preamp (inside oversampler) | Filters prepared at 2x sampleRate |
| Decay rates | Time-domain rates are sample-rate-independent (expressed in seconds) |
| Tremolo LFO | Phase increment: `rate * dt` |

### At Higher Sample Rates

At 96 kHz base rate, the 2x oversampler runs at 192 kHz. This provides even more anti-aliasing headroom. The preamp's harmonics have more room before Nyquist. No special handling needed -- just recompute filter coefficients.

At 44.1 kHz, the 2x oversampler runs at 88.2 kHz. The C20 HPF limits the preamp input to ~1903+ Hz, so even H12 of a 4 kHz input (48 kHz) is below the 44.1 kHz Nyquist of the oversampled domain. Adequate.

---

## 20. Complete Parameter List

### User-Facing Parameters (Exposed in DAW)

| ID | Name | Module | Min | Max | Default | Purpose |
|----|------|--------|-----|-----|---------|---------|
| 0 | Master Volume | output | 0.0 | 1.0 | 0.05 | Post-everything output level |
| 1 | Base Decay | reed | 0.2 | 10.0 | 1.1 | Fundamental decay time at A4 (seconds); corrected Feb 2026 from 1.6 per OldBassMan calibration |
| 2 | Velocity Curve | reed | 1.5 | 8.0 | 3.0 | NOT USED if per-mode exponents removed. Reserve for future use. |
| 3 | Pickup Gap | pickup | 0.3 | 6.0 | 0.50 | Base capacitive gap d0 |
| 4 | Pickup Mix | pickup | 0.0 | 1.0 | 1.0 | Blend between raw signal and pickup output |
| 5 | Pickup Offset | pickup | -0.5 | 0.5 | -0.10 | DC offset correction |
| 6 | Preamp Gain | preamp | 0.2 | 16.0 | 0.7 | Post-preamp output gain (linear) |
| 7 | Preamp Drive | preamp | 0.2 | 3.0 | 1.0 | Multiplier on Stage 1 open-loop gain |
| 8 | Asymmetry | preamp | 0.0 | 2.0 | 1.0 | Reserved (asymmetry is physical) |
| 9 | Tremolo Rate | tremolo | 0.1 | 15.0 | 5.5 | LFO frequency (Hz) |
| 10 | Tremolo Depth | tremolo | 0.0 | 1.0 | 0.5 | Modulation amount |
| 11 | Attack Overshoot | reed | 0.0 | 6.0 | 4.0 | DEPRECATED: should be removed; attack emerges from physics |

### Internal Constants (Not Exposed)

| Constant | Value | Purpose |
|----------|-------|---------|
| kPreampInputDrive | 28.0 | Scales voice sum into preamp operating range |
| B (thermal voltage) | 38.5 | 1/(n*Vt) for BJT Ebers-Moll |
| Stage 1 gain | 420 (max) | gm1 × Rc1 = 2.80 mA/V × 150K (open-loop, fb_junct grounded) |
| Stage 1 satLimit | 10.9 V | Vcc - Vc1 = 15 - 4.1 |
| Stage 1 cutoffLimit | 2.05 V | Vc1 - Ve1 - Vce_sat = 4.1 - 1.95 - 0.1 |
| Stage 2 gain | 238 | gm2 × Rc2 = 132 mA/V × 1.8K (open-loop) |
| Stage 2 satLimit | 6.2 V | Vcc - Vc2 = 15 - 8.8 |
| Stage 2 cutoffLimit | 5.3 V | Vc2 - Ve2 - Vce_sat = 8.8 - 3.4 - 0.1 |
| Stage 2 re | 0.456 | Re2_unbypassed / Rc2 = 820Ω / 1.8K |
| C20 HPF frequency | ~1903 Hz | C20 = 220 pF (RESOLVED), R = 380K |
| Miller pole 1 (open-loop) | ~25 Hz | Stage 1 dominant pole (C-3=100pF, Miller-multiplied) |
| Miller pole 2 | ~3300 Hz | Stage 2 (C-4=100pF, low Miller multiplication) |
| Closed-loop bandwidth | ~9900 Hz (no trem) / ~8300 Hz (trem bright) | SPICE-measured, combined preamp with R-10 emitter feedback |
| DC block frequency | 20 Hz | Output DC removal |
| Speaker HPF | 85 Hz, Q=0.75 | Sealed box resonance |
| Speaker LPF | 8000 Hz, Q=0.707 | Cone breakup |
| Noise decay | 1/0.003 = 333 Hz | 3ms attack noise time constant |
| Dwell sigma^2 | 64.0 | Gaussian dwell filter width (sigma=8.0; corrected Feb 2026 from 6.25/sigma=2.5) |
| kNumModes | 7 | Modal oscillator mode count |
| kMaxVoices | 12 | Maximum simultaneous voices |

---

## 21. Damper and Release Model

At note-off, a felt damper progressively contacts the reed. This is NOT an amplitude gate -- it progressively increases decay rates, with higher modes dying first.

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

### Plugin Voice Count: 12

12 voices provide sufficient polyphony for typical Wurlitzer playing:
- Solo melody: 1 voice
- Chords: 3-6 voices
- Chord + melody + sustain: 7-10 voices
- Extreme: full-hand clusters with pedal: may reach 12

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
- It is immediately silenced (set to FREE state)
- No crossfade or fadeout (the abrupt cutoff is masked by the new note's attack)
- The host receives a NOTE_END event for the stolen voice

### CPU Considerations

- All voice memory is pre-allocated (no `new`/`delete` in audio thread)
- Voice rendering is the most CPU-intensive per-voice work
- At 12 voices, each rendering 7 modes with sin() and exp() calls: ~84 trig calls per sample per voice maximum
- Optimization: skip modes with amplitude < 1e-10
- The oversampler and preamp run ONCE (shared), not per-voice

### Per-Sample Budget (at 48 kHz)

| Component | Operations per sample | Notes |
|-----------|----------------------|-------|
| 12 voices x 7 modes | ~84 sin, ~84 exp | Per-voice oscillator |
| Pickup per voice | ~12 max/add | Minimal |
| Voice sum | ~12 additions | Trivial |
| 2x oversampler up | ~12 multiply-adds | HIIR filter |
| Preamp (2 samples at 2x) | ~2 x (3 NR iterations x 2 stages) = 12 exp calls | Most expensive shared stage |
| 2x oversampler down | ~12 multiply-adds | HIIR filter |
| Tremolo | ~2 multiply-adds | Trivial |
| Speaker (2 biquads) | ~10 multiply-adds | Trivial |

**Total: approximately 100 sin/cos + 100 exp + 200 multiply-adds per base-rate sample.** This is well within real-time budget on modern CPUs.

---

## 23. Implementation Order

### Phase 0: Scaffold (1-2 days)

Build the minimum framework that compiles, loads in a DAW, and produces silence:

1. CLAP plugin entry point (descriptor, create, destroy)
2. Audio ports (stereo output)
3. Note ports (CLAP + MIDI input)
4. Parameter definitions (all 12 params)
5. State save/load
6. Empty process callback that outputs silence

**Test:** Load in Reaper/Bitwig, verify it appears and doesn't crash.

### Phase 1: Voice and Oscillator (3-5 days)

Get a playable instrument with correct pitch and basic dynamics:

1. Voice struct with state machine (FREE/HELD/RELEASING)
2. Voice allocator with stealing
3. Modal oscillator (7 modes, interpolated ratios)
4. Velocity scaling (vel^2, uniform)
5. Gaussian dwell filter
6. Decay model (base_decay, mode scales, register scaling)
7. Phase initialization
8. Per-note variation

**Test:** Play notes, verify correct pitch across full range, velocity responds, notes decay. No preamp yet -- output is the raw oscillator.

### Phase 2: SPICE Validation (COMPLETED Feb 2026)

All critical analog subcircuits validated in ngspice before DSP implementation:

1. **Preamp** — `spice/subcircuits/preamp.cir`: Two-stage CE amp with emitter feedback via Ce1. DC operating points match schematic. Closed-loop gain 6.0 dB (no tremolo) to 12.1 dB (tremolo bright). THD < 0.04% at normal levels.
2. **Tremolo oscillator** — `spice/subcircuits/tremolo_osc.cir`: Twin-T oscillator, TR-3/TR-4 shared collector. Freq=5.63 Hz, Vpp=11.82V, DC matches within 1%.
3. **LDR behavioral model** — `spice/models/ldr_behavioral.lib`: VTL5C3-like power-law with asymmetric time constants (tau_on=2.5ms, tau_off=30ms).
4. **LDR sweep** — `spice/testbench/topology_b_ldr_sweep.cir`: Verified gain modulation range of 6.1 dB across LDR sweep.

### Phase 3: Pickup and Summation (1 day)

1. Constant-charge pickup model per voice
2. Gap scaling by register
3. Voice summation into mono buffer

**Test:** Verify pickup doesn't alter pitch, minGap clamp works at extreme ff.

### Phase 4: Oversampler and Preamp (3-5 days)

This is the most complex and sonically important stage. All component values and topology are SPICE-validated (Phase 2).

1. Integrate HIIR library, build oversampler wrapper
2. Implement BjtStage class (NR solver, asymmetric soft-clip)
3. Implement C20 HPF (f_c = 1903 Hz)
4. Implement Miller LPFs (C-3 = C-4 = 100 pF)
5. Wire up Wurlitzer200APreamp (Stage 1 -> Miller -> Stage 2 -> Miller -> DC Block)
6. Implement emitter feedback path: R-10 (56K) -> fb_junction -> Ce1 (4.7µF) -> TR-1 emitter
7. Calibrate kPreampInputDrive and preampGain against SPICE gain targets (2.0x no-trem, 4.0x trem-bright)

**Test:** Verify H2 > H3 on all notes at mf. Check that pp is clean, mf has moderate bark, ff has aggressive bark. Check dynamic range: pp should be at least 15 dB quieter than ff. Cross-validate gain and THD against SPICE measurements.

### Phase 5: Post-Processing (1-2 days)

1. Tremolo LFO (twin-T oscillator model, ~5.6 Hz, mildly distorted sinusoid)
2. LDR model (asymmetric attack/release, power-law R vs illumination)
3. Feedback modulation (LDR path shunts fb_junction — gain modulation, not volume)
4. Speaker cabinet (biquad HPF + LPF)
5. Output limiter
6. Mono-to-stereo

**Test:** Full signal chain test. Compare spectra to OldBassMan recordings. Verify tremolo produces timbral modulation (not just amplitude).

### Phase 6: Release and Polish (2-3 days)

1. Damper model (progressive, per-mode)
2. Attack noise burst
3. Denormal protection (FTZ/DAZ)
4. Note-end events to host
5. Parameter automation smoothing

**Test:** Play musical passages. Verify damper release sounds natural, no CPU spikes on voice release.

### Phase 7: Tuning and Calibration (Ongoing)

1. Register balance test (10 notes, mf and ff)
2. Compare H2/H1 slope to target: `H2_dB = -0.48 * MIDI + 17.5`
3. Decay rate comparison to calibration curve
4. Dynamic range verification (pp vs ff: target 20-30 dB)
5. Polyphonic chord test (compression, intermodulation)

### Phase 8: ML Correction (Future)

1. Train MLP: `(pitch, velocity) -> (amp_offsets, freq_offsets, decay_offsets, d0_correction)`
2. Generate baked weights header file
3. Apply corrections at note-on (zero per-sample cost)
4. Deploy via RTNeural or custom inference code

---

## 24. Lessons from Previous Implementation (Vurli)

The previous project (Vurli, in `/home/homeuser/dev/mlwurli/`) went through 40+ rounds of tuning without fully converging. The following failure patterns must be avoided:

### Failure 1: Artificial Parameters That Don't Correspond to Physical Quantities

**Problem:** Parameters like `kPreampInputDrive`, `overshootCap`, and mode amplitude arrays were hand-tuned to compensate for modeling errors elsewhere. When one was changed, others had to be re-tuned, creating a fragile interdependent system.

**Solution:** Every parameter must trace to a physical quantity (voltage, capacitance, resistance, mass ratio). If a "fudge factor" is needed, it indicates a modeling error that should be found and fixed, not compensated.

### Failure 2: Sinc Dwell Filter Cascading Into Mode Amplitude Compensation

**Problem:** The rectangular-pulse sinc dwell filter had 40-60 dB deep nulls. To compensate, mode amplitudes were raised 20x. This destroyed the attack-to-sustain ratio because the artificial overshoot envelope multiplied already-elevated sustained amplitudes.

**Solution:** Use the Gaussian dwell filter (no nulls, monotonic rolloff). Mode amplitudes stay near physical 1/omega scaling. Attack overshoot emerges naturally from modal superposition. No artificial overshoot envelope.

### Failure 3: Feedback Cap Polarity Inverted

**Problem:** The model applied MORE feedback at low frequencies (cap tracks LF output) when the real Miller-effect cap provides MORE feedback at HIGH frequencies (cap's low impedance shunts HF). This inverted register-dependent distortion: bass should get MORE distortion (less HF feedback reducing gain), treble LESS.

**Solution:** Model the cap correctly: at HF, cap impedance is low, creating negative feedback that reduces gain. At LF, cap impedance is high, no feedback, full gain.

### Failure 4: kPreampInputDrive as a Tuning Knob

**Problem:** The input drive was changed from 16 to 48 to 28 across rounds, each time requiring re-tuning of preampGain, feedback cap corners, and mode amplitudes.

**Solution:** Set kPreampInputDrive once to place mf at the correct operating point. Never change it during tonal tuning.

### Failure 5: Per-Mode Velocity Exponents Double-Counting With Dwell

**Problem:** Both the dwell filter (shorter at ff -> brighter) AND per-mode velocity exponents (higher at ff for upper modes) brightened the spectrum at ff. This double-counted the physical brightening mechanism, making ff sound harsh and over-bright.

**Solution:** Remove per-mode velocity exponents. All modes scale uniformly with vel^2. Timbral brightening at ff comes from two physically correct sources: (1) shorter dwell time, (2) preamp nonlinearity on louder signal.

### Failure 6: Reaper Plugin State Override

**Problem:** When loading a Reaper project that saved old parameter values, `stateLoad()` overrode the new defaults from `params.cpp`, undoing tuning changes.

**Solution:** After changing parameter defaults, users must remove and re-add the plugin in their DAW project. Document this clearly. Consider a version check in `stateLoad()`.

---

## 25. Comparison with Existing Plugins

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

1. **No velocity layers** -- continuous vel^2 curve with preamp nonlinearity providing natural timbral transition
2. **True half-damping** -- the progressive damper model supports partial key release
3. **Open source + Linux** -- no iLok, no 80 GB download, full source access
4. **CLAP native** -- forward-looking plugin format
5. **Tiny install** -- the entire plugin is < 1 MB
6. **Customizable** -- users can modify the preamp model, speaker, tremolo

---

## 26. CLAP Plugin Requirements

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
- All buffers pre-allocated in `activate()`
- Use `std::atomic<double>` for parameter values (read in audio thread, written in main thread)
- FTZ/DAZ set at start of `process()`, restored at end

### Event Processing

CLAP requires sample-accurate event processing. The process callback must:

1. Split input events by timestamp
2. Render audio in sub-blocks between events
3. Handle note-on, note-off, note-choke, and param-value events
4. Emit NOTE_END events when voices die

### Parameter Threading

Parameters are stored as `std::atomic<double>`. The audio thread reads with `memory_order_relaxed`. The main thread writes via `handleParamValue()`. No locks needed for this simple double-exchange pattern.

### State Format

Simple text serialization: `VURLI-STATE-V1;0=0.050000;1=1.100000;...`

---

## 27. References

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
