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
24. [Lessons from Previous Implementation (OpenWurli)](#24-lessons-from-previous-implementation-openwurli)
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
     - Bias network: R-2=2M, R-3=470k -> R_total = R-2||R-3 = 380k
     - C20 shunt cap: 220 pF — NOTE: 206A ONLY, NOT present on the 200A
     - ALL 64 reeds share ONE common pickup plate (reed bar assembly)
     - Total system capacitance: ~240 pF at preamp input
  -> Preamp (separate PCB mounted on reed bar in 200A)
     - Two direct-coupled NPN common-emitter stages (TR-1, TR-2)
     - Originally 2N2924, later replaced with 2N5089 (hFE >= 450)
     - +15V DC supply
     - Collector-base feedback caps C-3 = C-4 = 100 pF
     - Pickup RC HPF at ~2312 Hz (C20 at 1903 Hz is 206A only, NOT 200A)
     - Total gain 6.0 dB (2.0x) no tremolo / 12.1 dB (4.0x) tremolo bright
     - Output: 2-7 mV AC at volume pot
  -> Tremolo (LDR optocoupler modulates preamp emitter feedback)
     - LFO (~5.6 Hz twin-T oscillator, TR-3/TR-4) drives LED inside LG-1 optocoupler
     - R-10 (56K) feeds back from output to fb_junct; Ce1 (4.7 MFD) couples fb_junct to TR-1 emitter
     - LDR (LG-1) shunts fb_junct to ground via cable Pin 1 → 50K VIBRATO → 18K → LG-1
     - Modulates preamp GAIN (not post-preamp volume): series-series emitter feedback topology
     - LED ON → LDR low → fb_junct shunted to ground → feedback can't reach emitter → higher gain
     - LED OFF → LDR high → full feedback reaches emitter via Ce1 → lower gain
     - This is gain modulation, producing timbral variation through the tremolo cycle
     - Rate: ~5.5-6 Hz, depth: ~6 dB modulation range at max vibrato
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

**The pickup's 1/(1-y) nonlinearity is the primary source of even harmonics (H2) at normal dynamics.** SPICE simulation confirms H2/H1 ~ -21 dB (THD ~ 8.7%) from the pickup at mf (y=0.10), while the preamp at millivolt input levels produces THD < 0.01%. The preamp's asymmetric clipping headroom (2.05V vs 10.9V, ratio 5.3:1) contributes additional H2 at extreme ff dynamics where it enters saturation. Both the pickup and preamp contribute to the characteristic "bark," but the pickup dominates at normal playing levels.

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
       [D] Per-Note Variation (deterministic +-3 cents freq, +-8% amp)
       [E] Electrostatic Pickup (1/(1-y) capacitive nonlinearity)
     -> Sum all voices (mono)

  Mono shared processing:
     -> 2x Upsample (6-coefficient (3+3) allpass polyphase IIR, ~28 dB rejection at 30 kHz)
        [F] DkPreamp (8-node coupled MNA solver) WITH INTEGRATED TREMOLO
            Tremolo: LDR (LG-1) + R-10 (56K) modulate feedback ratio
            DkPreamp: 8-node coupled MNA solver (DK method)
              -> Stage 1: Miller pole ~23 Hz open-loop
              -> Direct coupling
              -> Stage 2: Miller pole ~81 kHz
        [H] DC Block (handled internally by DK preamp)
     -> 2x Downsample (matching allpass polyphase IIR)
     [I] Volume Control (real attenuator, audio taper, between preamp and power amp)
     [K] Power Amplifier (Class AB, crossover distortion at low signal levels)
     [L] Speaker Cabinet (variable: bypass to authentic HPF 85-100 Hz + LPF 7-8 kHz)
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
| Pickup | NO (1/(1-y) nonlinearity, primary H2 source) | No (runs at base rate; harmonics stay within audio band) | Base |
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

**Conclusion:** Only the preamp requires oversampling. 2x is sufficient because the preamp's input signal is already bandlimited by the pickup's natural bandwidth and the preamp's own Miller-effect rolloff. The preamp generates harmonics, but the highest-energy harmonics that could alias are well below Nyquist at 2x.

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

These are OBM-calibrated values derived from OldBassMan 200A recordings. The previous 1/omega_n Euler-Bernoulli values were 20-37 dB too hot vs OBM data. Real Wurlitzer reeds (solder tip mass, non-uniform geometry) suppress upper modes far below ideal beam theory. The characteristic "bark" (H2) comes from the pickup's 1/(1-y) nonlinearity generating H2 at 2x the fundamental, NOT from physical mode 2 at 6.3x the fundamental.

### Velocity Scaling

Velocity scaling uses a register-dependent exponent: a bell curve centered at MIDI 62 (sigma=15) from 1.3 (extremes) to 1.7 (mid-range), applied as `velMapped = velocity^exp`. This shapes the dynamic response to match the mechanical leverage differences across the keyboard.

Example values at mid-range (exp ~ 1.7):
- pp (vel=0.3): velMapped = 0.09
- mf (vel=0.7): velMapped = 0.47
- ff (vel=0.95): velMapped = 0.91

Timbral brightening at ff comes from two sources that do NOT require per-mode velocity exponents:
1. Shorter dwell time at ff -> dwell filter passes more upper partials
2. Pickup 1/(1-y) nonlinearity -> louder signal generates more harmonics

Per-mode velocity exponents double-count with the dwell filter's velocity-dependent brightening. Do not use per-mode exponents.

### Decay

```
base_decay = 0.005 * freq^1.22  // with MIN_DECAY_RATE = 3.0 dB/s floor
decay_rate[m] = base_decay * ratio[m]^2.0          // power-law per-mode scaling (MODE_DECAY_EXPONENT = 2.0)
```

- Base decay rate follows a frequency power law calibrated to OldBassMan 200A recordings (see reed-and-hammer-physics.md Section 5.7)
- The 3.0 dB/s floor prevents unrealistically long bass sustain
- Per-mode scaling uses a `ratio^2.0` power law (Zener damping, proportional to omega squared): higher modes (with larger frequency ratios) decay faster
- This replaces the previous fixed decay scales array `[1.0, 0.20, 0.08, ...]` with a physics-derived power law
- Higher modes decay faster -> timbre darkens over time (bright attack, sine-like tail)

Calibration target: `base_decay = 0.005 * freq^1.22` with MIN_DECAY_RATE = 3.0 dB/s floor, +/-30% tolerance.

### Per-Sample Rendering

```rust
for each sample:
    signal = 0.0
    for each mode m:
        envelope = amps[m] * exp(-decay_rates[m] * time)
        // Add damper if releasing (see Section 21)
        signal += envelope * sin(phases[m])
        phases[m] += 2*PI * freqs[m] * dt
        if phases[m] >= 2*PI { phases[m] -= 2*PI }
```

### Phase Initialization

Reed starts at zero displacement (hammer imparts velocity, not displacement). All mode phases start at 0. A raised cosine onset ramp models the gradual buildup of reed vibration during hammer contact:

```
onset_envelope(t) = 0.5 * (1 - cos(PI * t / T_onset))
```

The onset ramp time `T_onset` is register-dependent: `(periods / f0).clamp(2ms, 30ms)`, where `periods` ranges from 2.0 (ff) to 4.0 (pp). The envelope shape is `cosine^(1 + (1-velocity))` -- ff gets a raised cosine, pp gets Hann-squared (softer onset). This models reed mechanical inertia -- bass reeds take more time to reach full amplitude than treble reeds.

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

## 6. Stage 4: Attack Noise Burst (Per-Voice)

The felt-tipped hammer striking a steel reed produces a broadband impact noise that lasts 2-5 ms. This is separate from the modal vibration.

```
noise_amp = 0.025 * vel^2
noise_decay = 1/0.003  // 3 ms time constant
noise_cutoff = (5 * f0).clamp(200, 2000)  // tracks fundamental, not velocity
```

- No floor or register scaling -- amplitude scales purely with velocity squared
- Noise center frequency tracks the fundamental (5x f0), clamped to 200-2000 Hz, Q=0.7
- LCG pseudo-random generator -> bandpass at `noise_cutoff` -> exponential decay envelope (3ms decay, 15ms duration)
- Added to the voice signal BEFORE the pickup model

The noise burst is subtle but contributes to the "woody" percussive attack character. Without it, notes have an artificially pure, synthesizer-like onset.

---

## 7. Stage 5: Per-Note Variation (Per-Voice)

Real Wurlitzers have per-note personality from manufacturing tolerance: solder placement, reed alignment, gap variation. This is deterministic (the same note always sounds slightly different from its neighbors, but consistent across strikes).

```
// Deterministic hash seeded by note number (NOT random per strike)
freq_detune = 1.0 + hash(key) * 0.00173     // +/-3 cents fundamental detuning
amp_variation[m] = 1.0 + hash(key, m) * 0.08 // +/-8% per mode
```

- Fundamental IS detuned: +/-3 cents (0.00173 ratio), matching factory tuning tolerance per US Patent 2,919,616 (Andersen, 1960)
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
- **System RC corner:** f_c = 1/(2*PI*287k*240pF) = 2312 Hz -> bass fundamentals in constant-voltage regime (R_total = R_feed||(R-1+R_bias) = 1M||402K = 287K; see pickup-system.md Section 3.7)

The per-reed constant-charge approximation is a defensible engineering tradeoff because the system RC HPF at 2312 Hz provides similar bass rolloff to the system-level RC dynamics. (Note: C20 at 1903 Hz is 206A only, NOT 200A.) The pickup model includes the full 1/(1-y) nonlinearity, which is the primary source of even-harmonic "bark" at normal dynamics (H2/H1 ~ -21 dB at mf from SPICE).

### 1/(1-y) Nonlinear Pickup Model

The reed-plate capacitance varies as C(y) = C_0 / (1-y), where y = x/d_0 is the normalized displacement fraction. This produces a signal voltage proportional to y/(1-y), which is the primary source of even-harmonic "bark":

```rust
// Convert reed model displacement to physical fraction y = x/d_0
let y = (sample * displacement_scale).clamp(-MAX_Y, MAX_Y);

// Nonlinear capacitance: C(y) = C_0/(1-y) → signal ∝ y/(1-y)
// Positive y (toward plate) amplified more than negative → generates H2
let nonlinear = y / (1.0 - y);

// Scale to voltage: V = V_hv * C_0/(C_0+C_p) * y/(1-y)
let v = nonlinear * SENSITIVITY;   // SENSITIVITY = 1.8375 V

// Pickup RC highpass at 2312 Hz (R_total=287K, C=240pF)
let output = hpf.process(v);       // OnePoleHpf at 2312 Hz
```

The 1/(1-y) nonlinearity generates H2 that scales with displacement amplitude:
- y=0.02 (pp): THD 1.7%, H2 = -35 dB
- y=0.10 (mf): THD 8.7%, H2 = -21 dB
- y=0.20 (f): THD 17.6%, H2 = -15 dB

The HPF also amplifies H2 relative to H1 (since H2 is at 2f, where the HPF has higher gain), adding ~1.9x boost to the H2/H1 ratio.

### Displacement Scale

The `displacement_scale` parameter converts reed model output (normalized, fundamental amplitude = 1.0) to the physical displacement fraction y = x/d_0. It is the single biggest tuning knob for bark intensity and is set per-note from `tables::pickup_displacement_scale()`:

```
displacement_scale(midi) = DS_AT_C4 * 2^((midi - 60) * DS_EXPONENT / 12)
// DS_AT_C4 = 0.85, DS_EXPONENT = 0.65, clamp [0.02, 0.85]
```

Bass reeds have wider gaps and larger displacements; treble reeds have tighter gaps. The exponential curve captures this register dependence.

### Parameters

| Parameter | Default | Range | Purpose |
|-----------|---------|-------|---------|
| `displacement_scale` | Per-note (DS_AT_C4=0.85) | 0.02-0.85 | Converts model units to physical y = x/d_0 |
| `MAX_Y` | 0.90 | — | Safety clamp (y=1.0 is a singularity) |
| `SENSITIVITY` | 1.8375 V | — | V_hv * C_0 / (C_0 + C_p) = 147 * 3/240 |
| HPF corner | 2312 Hz | — | 1-pole HPF from pickup RC (R_total=287K, C=240pF) |

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

This is the most complex processing stage. The preamp adds harmonic coloring at high dynamics and provides the tremolo-modulated gain that defines the instrument's character. (The pickup's 1/(1-y) nonlinearity is the primary bark source at normal dynamics; the preamp contributes at extreme ff.)

### DECISION: Trait-Based A/B Architecture

The preamp implements a `PreampModel` trait with `process_sample()`, `set_ldr_resistance()`, `reset()`. Two implementations exist behind this interface:
1. **DkPreamp** (8-node coupled MNA solver using the DK method) — the shipping implementation. Models the full two-stage circuit with direct coupling, Miller caps, and emitter feedback as a single coupled nonlinear system. See `docs/dk-preamp-derivation.md`.
2. **EbersMollPreamp** — legacy reference with independent per-stage NR solvers. Retained for comparison only; not used in production.

### Oversampling Wrapper

The preamp runs at 2x the base sample rate inside a polyphase IIR oversampler:

1. **Upsample:** 6-coefficient (3+3) allpass polyphase half-band upsampler (~28 dB rejection at 30 kHz)
2. **Process:** Run DkPreamp (coupled 8-node MNA solver) at 2x rate
3. **Downsample:** Matching allpass polyphase half-band downsampler

The oversampler uses allpass IIR filters (custom Rust implementation in `oversampler.rs`). The ~28 dB rejection is sufficient because the preamp's Miller-effect rolloff naturally limits harmonic energy above ~15 kHz.

### Input Drive

The DkPreamp receives the voice summation output directly. Because the DK method models the full circuit with physical component values, no artificial input drive scaling is needed -- the circuit's gain, impedances, and nonlinear behavior emerge naturally from the MNA equations. The voice output is scaled by `output_scale()` (physics-based, computed from displacement scale and pickup geometry) to approximate millivolt-level signals before entering the preamp.

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

The collector-base capacitor creates Miller-effect negative feedback:
- At HIGH frequencies: cap impedance is low -> MORE current from collector to base -> MORE feedback -> LESS gain
- At LOW frequencies: cap impedance is high -> LESS feedback -> FULL gain

The correct model:
```
// Cap state tracks the DIFFERENCE between output and input (AC component)
// At HF: cap can track fast changes -> provides feedback
// At LF: cap charges fully -> no AC feedback

// Corner frequency from Miller multiplication:
// f_miller = 1 / (2*PI * Ccb * (1+Av) * R_source)
// For Stage 1: C-3=100pF, Av=420 -> C_miller=42,100pF -> f_dominant ~23 Hz

// Implementation:
hf_feedback = output - fbCapState  // HF component (what cap can't track)
fbCapState += fbCapCoeff * (output - fbCapState)  // LPF tracks output

// Apply as degeneration:
effectiveRe = re + feedbackBeta * (something proportional to HF content)
```

Target corner frequencies based on physical Miller multiplication (C-3 = C-4 = 100 pF):
- Stage 1: ~23 Hz open-loop dominant pole (C-3=100pF × (1+420) = 42,100 pF Miller-multiplied)
- Stage 2: ~81 kHz (C-4=100pF × (1+2.2) = 320 pF, into low source impedance from Stage 1 output)
- Closed-loop bandwidth: **~10 kHz** (no tremolo) / **~8.3 kHz** (tremolo bright)

### Miller LPF (After Each Stage)

First-order LPF modeling Miller-effect bandwidth limitation. With C-3 = C-4 = 100 pF:
- After Stage 1: dominant pole at ~23 Hz open-loop
- After Stage 2: ~81 kHz (Stage 2 has low gain of ~2.2, so Miller multiplication is mild)

Stage 1's Miller pole at ~23 Hz is the dominant open-loop pole. The DkPreamp's coupled MNA solver handles both stages and their feedback interactions as a single system, so separate per-stage Miller LPF modeling is not needed. Full-chain BW: ~15.5 kHz (preamp only), ~11.8 kHz (no trem), ~9.7 kHz (trem bright). See preamp-circuit.md Section 5.5.1 for full analysis.

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

Handled internally by the DK preamp's coupled MNA solver -- no separate DC block stage is needed. The DK method's circuit equations naturally account for DC operating points and coupling.

---

## 11. Tremolo — Integrated in Preamp Emitter Feedback Loop

The 200A tremolo modulates the preamp's closed-loop gain via an LDR (LG-1) that shunts the emitter feedback junction to ground. See preamp-circuit.md Section 7 for detailed analysis. R-10 (56K) feeds back from the output to fb_junct; Ce1 (4.7 MFD) AC-couples fb_junct to TR-1's emitter. The LDR path (cable Pin 1 → 50K VIBRATO → 18K → LG-1 → GND) diverts feedback current away from the emitter. This is **gain modulation**, not simple amplitude modulation — the distortion character changes through the tremolo cycle.

### LFO (Twin-T Oscillator, TR-3/TR-4)

The oscillator is a twin-T (parallel-T) notch filter oscillator. SPICE-validated at 5.63 Hz with 11.8 Vpp output swing. See `spice/subcircuits/tremolo_osc.cir` and `docs/output-stage.md` Section 2.1 for full topology.

```
lfo = sin(2*PI * rate * t)
led_drive = max(0, lfo)  // half-wave rectified (LED only conducts forward)
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
log_r = log(R_max) + (log(R_min) - log(R_max)) * drive^gamma  // gamma = 1.1
R_ldr = exp(log_r)    // R_min=50, R_max=1M

// LDR path impedance: fb_junct -> Pin 1 -> 50K VIBRATO -> 18K -> LDR -> GND
R_ldr_path = 18000 + 50000 * (1 - depth) + R_ldr

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

**Timbral modulation:** At the high-gain phase (LDR dark), the preamp's gain is higher, amplifying the pickup-generated harmonics more and pushing the preamp closer to its own saturation threshold. At the low-gain phase (LDR lit), the preamp operates more linearly with less harmonic amplification. This subtle but important timbral variation distinguishes the real 200A tremolo from a simple volume multiplier.

### Implementation Note

Because the tremolo modulates the preamp's emitter feedback (via the LDR shunt at fb_junct), it must be implemented INSIDE the preamp processing block (within the 2x oversampled domain), not as a separate post-preamp stage. The LDR state updates at the base sample rate, but the emitter feedback modulation applies per-sample at 2x rate.

### Shadow Preamp Pump Cancellation

R_ldr modulation at 5.63 Hz creates a ~4.5V pp pump at the preamp output via Ce1 transient dynamics (confirmed by SPICE). The pump has harmonics at 28-200+ Hz that overlap bass fundamentals -- no HPF can separate them without cutting bass. Solution: a second `DkState` ("shadow") runs in parallel with zero audio input but the same R_ldr modulation. Its output is pure pump. Subtracting the shadow output from the main output cancels all pump harmonics at every frequency. Pump level after subtraction: < -120 dBFS. CPU cost: ~60% more (the shadow DK solver converges faster with zero input).

### Parameters

| Parameter | Default | Range | Notes |
|-----------|---------|-------|-------|
| Rate | 5.63 Hz | 0.1-15.0 | Most real instruments 5.3-7 Hz |
| Depth | 0.5 | 0.0-1.0 | 0=off, 0.5 ~ 4.5 dB dip, 1.0 ~ 9 dB dip |

---

## 12. Stage 10: Volume Control (Mono, Base Rate)

### DECISION: Model as Real Attenuator Between Preamp and Power Amp

In the real 200A, the 3K audio-taper volume potentiometer sits between the preamp output and the power amplifier input. The plugin must place the volume control at this exact point in the signal chain — NOT as a final output gain.

**Why placement matters:** At low volume settings, the signal level at the power amp input drops into the crossover distortion region, changing the distortion character (more odd harmonics from the Class AB dead zone). This interaction between volume and power amp behavior is audible and contributes to the instrument's character at low volumes.

```
// Audio taper: skewed range (display mapping) + squared multiplier (audio path)
pot_position = user_volume_param  // 0.0 to 1.0 (FloatRange::Skewed, factor 2.0)
output = input * pot_position * pot_position  // vol^2 in the audio path
// -> feeds into power amplifier stage
```

The effective taper combines two stages: `FloatRange::Skewed` with factor 2.0 (which controls the DAW display/automation curve) plus the `vol * vol` squared multiplier in the audio path. The default volume of 0.63 produces an effective gain of ~0.40 (0.63^2). In the real instrument, the volume pot output is measured at 2-7 mV AC.

---

## 13. Stage 11: Power Amplifier (Mono, Base Rate)

**Priority: MEDIUM. The power amp is transparent at moderate levels but matters for ff polyphonic saturation and aged-instrument character.**

The real 200A has a ~18-20W quasi-complementary push-pull Class AB output stage:

- Input differential pair: 2N5087 (PNP)
- Vbe multiplier for bias: MPSA06
- Output: TIP35C (NPN) / TIP36C (PNP), +/-24V rails
- Emitter degeneration: 0.47 ohm
- Quiescent bias: ~10 mA

### Closed-Loop NR Feedback Solver

The power amp is modeled as a closed-loop feedback amplifier solved by Newton-Raphson iteration:

```
// Feedback parameters (from schematic: R-31=15K, R-30=220 ohm)
A_ol = 19000          // open-loop gain
beta = 0.01445        // feedback fraction (R-30 / (R-30 + R-31))
loop_gain T = 275     // A_ol * beta
closed_loop_gain = A_ol / (1 + T) ≈ 69  // 1 + R-31/R-30 = 37 dB

// Crossover distortion (dead zone between NPN/PNP conduction)
crossover_vt = 0.013     // thermal voltage; effective dead zone ≈ ±2*vt = ±26 mV

// Rail clipping: tanh soft-clip (gradual saturation, not hard clamp)
headroom = 22.0  // ±24V rails minus ~2V Vce_sat
output = headroom * tanh(input / headroom)
```

The crossover model uses a C-infinity Gaussian gain function instead of a piecewise linear dead zone:
```
cross_gain = q + (1 - q) * (1 - exp(-v^2 / vt^2))
```
where `q = QUIESCENT_GAIN = 0.1` is the residual gain at zero signal (from quiescent bias current) and `vt = 0.013` is the thermal voltage. At v=0 the gain is q (not zero -- physically realistic); at |v| >> vt the gain approaches 1.0. This smooth function avoids NR convergence issues and produces the correct odd-harmonic crossover distortion signature.

The code models crossover/feedback behavior generically, not specific transistor parameters (the TIP35C/TIP36C reference describes the real instrument).

At mf single notes, the power amp is nearly transparent -- the preamp dominates tonal character. At ff polyphonic, the power amp's tanh soft-clip adds compression and gradual saturation (Yeh/Abel/Smith 2007). With aging, bias drifts, increasing crossover distortion (odd harmonics from the dead zone).

Audio taper volume control: `vol^2` (quadratic, skew +2.0), default position 0.63 (effective ~0.40).

---

## 14. Stage 12: Speaker Cabinet (Mono, Base Rate)

The 200A uses two 4"x8" oval ceramic speakers in an open-backed ABS plastic lid (NOT sealed), 16 ohm each (part #202243). See output-stage.md for details.

### DECISION: Variable Speaker Emulation

The speaker HPF/LPF are physical limitations, not design choices. Expose a "Speaker Character" parameter that blends from **bypass** (full-range, flat) to **authentic** (full HPF + LPF). This lets players who want more bass or extended treble dial back the speaker emulation.

### Model (at "Authentic" Position)

Two variable-cutoff biquad filters (Direct Form II Transposed) with smoothed coefficient updates:

1. **Open-baffle bass rolloff:** Single HPF at 95 Hz, Q=0.75
   - Physics-motivated: combination of speaker resonance + open baffle cancellation (~12 dB/oct)
   - Attenuates C2 fundamental (65 Hz) by ~5.4 dB
   - Leaves H2 (130 Hz) nearly untouched
   - Significant contributor to bass register H2/H1 balance

2. **Cone breakup rolloff:** 2nd-order LPF at 7500 Hz, Q=0.707 (Butterworth)
   - Set above the preamp Miller LPFs to avoid stacking
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

| Point in Chain | Signal Level (arbitrary units) | Notes |
|---------------|-------------------------------|-------|
| Single voice, mf | ~0.05-0.15 | After pickup |
| 6-voice chord, ff | ~0.3-0.9 | Sum of voices |
| After output_scale() | scaled to mV range | Into DkPreamp |
| After preamp Stage 1 | ~0.5-11 | Depends on gain/feedback |
| After preamp Stage 2 | ~0.5-6.5 | Clipped by soft-limits |
| After preampGain (0.7x) | ~0.35-4.5 | Into tremolo/speaker |
| After masterVol (0.63, effective ~0.40x) | ~0.14-1.80 | Into power amp |

### Input Drive (Historical)

**Note:** The `kPreampInputDrive` scaling factor discussed here is historical and applies only to the legacy `EbersMollPreamp`. The shipping `DkPreamp` implementation uses physical component values directly (resistances, capacitances, transistor parameters) and does not require artificial input drive scaling. The DK method MNA solver operates on the actual circuit equations, so signal levels are determined by the component values themselves.

SPICE-measured closed-loop gain: 6.0 dB (2.0x) without tremolo, 12.1 dB (4.0x) at tremolo bright peak. BW: ~15.5 kHz preamp-only, ~11.8 kHz full-chain (no trem) / ~9.7 kHz (trem bright).

---

## 17. Oversampling Strategy

### What Needs Oversampling

Only the preamp requires oversampling. It is the only significantly nonlinear stage that generates harmonics above the input signal's bandwidth.

The pickup's 1/(1-y) nonlinearity generates harmonics but does so at the base sample rate; its output bandwidth stays within the audio band. The output limiter operates on an already band-limited signal at low levels. The power amp crossover distortion generates only low-order odd harmonics at small signal levels.

### Why 2x Is Sufficient

The preamp's input is naturally bandlimited by the pickup's RC HPF (~2312 Hz) and the modal oscillator's finite mode count. This means:
- The highest-energy input component is around 2-4 kHz (fundamental of mid/treble register, or H2 of bass)
- The preamp generates harmonics at 2x, 3x, 4x, ... of this input
- At 48 kHz base rate, 2x oversampling gives 96 kHz processing rate with 48 kHz Nyquist
- H8 of a 4 kHz input = 32 kHz, safely below 48 kHz Nyquist
- H12 of a 4 kHz input = 48 kHz, at Nyquist -- but H12 is typically -50 dB or lower

For 44.1 kHz base rate, 2x gives 88.2 kHz with 44.1 kHz Nyquist. Still adequate given the natural input bandwidth.

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

The 1/(1-y) pickup model generates significant even harmonics (H2/H1 ~ -21 dB at mf) but these are low-order harmonics within the audio band. Since the pickup operates at the base sample rate and its harmonic content is bounded by the reed's modal frequencies, aliasing is not a concern.

### Preamp

Addressed by 2x oversampling (Section 17). The ~28 dB rejection at 30 kHz is sufficient because the preamp's Miller-effect rolloff naturally limits harmonic energy at high frequencies, so aliased components are well below the audible signal.

### Output Limiter

The tanh limiter at the output operates on a signal that has already been through the speaker cabinet LPF (8 kHz cutoff). Any harmonics generated by the tanh are above 16 kHz and inaudible. No oversampling needed.

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
| Oversampler | Operates at 2x whatever the base rate is |
| Preamp (inside oversampler) | Filters prepared at 2x sampleRate |
| Decay rates | Time-domain rates are sample-rate-independent (expressed in seconds) |
| Tremolo LFO | Phase increment: `rate * dt` |

### At Higher Sample Rates

At 96 kHz base rate, the 2x oversampler runs at 192 kHz. This provides even more anti-aliasing headroom. The preamp's harmonics have more room before Nyquist. No special handling needed -- just recompute filter coefficients.

At 44.1 kHz, the 2x oversampler runs at 88.2 kHz. The pickup RC HPF limits the preamp input to ~2312+ Hz, so even H12 of a 4 kHz input (48 kHz) is below the 44.1 kHz Nyquist of the oversampled domain. Adequate.

---

## 20. Complete Parameter List

### User-Facing Parameters (Exposed in DAW)

| ID | Name | Module | Min | Max | Default | Purpose |
|----|------|--------|-----|-----|---------|---------|
| "volume" | Volume | output | 0% | 100% | 63% | Audio taper attenuator between preamp and power amp |
| "trem_rate" | Tremolo Rate | tremolo | 0.1 | 15.0 Hz | 5.63 Hz | LFO frequency |
| "trem_depth" | Tremolo Depth | tremolo | 0% | 100% | 50% | Modulation amount |
| "speaker" | Speaker Character | speaker | 0% | 100% | 0% | 0%=bypass (full range), 100%=authentic (HPF+LPF+waveshaper) |
| "mlp" | MLP Corrections | dsp | off / on | — | on | Per-note ML corrections for freq/decay/displacement |

All other parameters (decay rates, pickup gap, preamp component values, mode amplitudes, velocity curve, attack overshoot) are hardcoded internally based on physical circuit analysis and OBM calibration data. They are not exposed to the user.

### Internal Constants (Not Exposed)

| Constant | Value | Purpose |
|----------|-------|---------|
| B (thermal voltage) | 38.5 | 1/(n*Vt) for BJT Ebers-Moll |
| Stage 1 gain | 420 (max) | gm1 × Rc1 = 2.80 mA/V × 150K (open-loop, fb_junct grounded) |
| Stage 1 satLimit | 10.9 V | Vcc - Vc1 = 15 - 4.1 |
| Stage 1 cutoffLimit | 2.05 V | Vc1 - Ve1 - Vce_sat = 4.1 - 1.95 - 0.1 |
| Stage 2 gain | 238 | gm2 × Rc2 = 132 mA/V × 1.8K (open-loop) |
| Stage 2 satLimit | 6.2 V | Vcc - Vc2 = 15 - 8.8 |
| Stage 2 cutoffLimit | 5.3 V | Vc2 - Ve2 - Vce_sat = 8.8 - 3.4 - 0.1 |
| Stage 2 re | 0.456 | Re2_unbypassed / Rc2 = 820Ω / 1.8K |
| Miller pole 1 (open-loop) | ~23 Hz | Stage 1 dominant pole (C-3=100pF, Miller-multiplied) |
| Miller pole 2 | ~81 kHz | Stage 2 (C-4=100pF, low Miller multiplication) |
| Full-chain bandwidth | ~11800 Hz (no trem) / ~9700 Hz (trem bright) | Preamp-only ~15.5 kHz; full chain includes speaker rolloff |
| DC block | N/A | Handled internally by DK preamp |
| Speaker HPF (authentic) | 95 Hz, Q=0.75 | Open-baffle resonance + bass cancellation |
| Speaker LPF (authentic) | 7500 Hz, Q=0.707 | Cone breakup |
| Noise decay | 1/0.003 = 333 Hz | 3ms attack noise time constant |
| Dwell sigma^2 | 64.0 | Gaussian dwell filter width (sigma=8.0) |
| kNumModes | 7 | Modal oscillator mode count |
| kMaxVoices | 12 | Maximum simultaneous voices |

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
- A 5ms linear crossfade is applied (stolen voice fades out while new voice fades in)
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
| 2x oversampler up | ~12 multiply-adds | Allpass polyphase filter |
| Preamp (2 samples at 2x) | ~2 x (3 NR iterations x 2 stages) = 12 exp calls | Most expensive shared stage |
| 2x oversampler down | ~12 multiply-adds | Allpass polyphase filter |
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

### Phase 2: SPICE Validation (Complete)

All critical analog subcircuits validated in ngspice before DSP implementation:

1. **Preamp** -- `spice/subcircuits/preamp.cir`: Two-stage CE amp with emitter feedback via Ce1. Closed-loop gain 6.0 dB (no tremolo) to 12.1 dB (tremolo bright). THD < 0.04% at normal levels.
2. **Tremolo oscillator** -- `spice/subcircuits/tremolo_osc.cir`: Twin-T oscillator, TR-3/TR-4 shared collector. Freq=5.63 Hz, Vpp=11.82V.
3. **LDR behavioral model** -- `spice/models/ldr_behavioral.lib`: VTL5C3-like power-law with asymmetric time constants.
4. **LDR sweep** -- `spice/testbench/topology_b_ldr_sweep.cir`: 6.1 dB gain modulation range across LDR sweep.

### Phase 3: Pickup and Summation (1 day)

1. Constant-charge pickup model per voice
2. Gap scaling by register
3. Voice summation into mono buffer

**Test:** Verify pickup doesn't alter pitch, minGap clamp works at extreme ff.

### Phase 4: Oversampler and Preamp (3-5 days)

This is the most complex and sonically important stage. Component values and topology were validated in SPICE (Phase 2).

1. Build allpass polyphase oversampler wrapper
2. Implement DkPreamp (8-node coupled MNA solver using DK method)
3. Miller caps and direct coupling handled within DK circuit equations
4. Wire up DkPreamp with oversampler and DC block
5. Implement emitter feedback path: R-10 (56K) -> fb_junction -> Ce1 (4.7µF) -> TR-1 emitter
6. Validate DkPreamp gain against SPICE targets (2.0x no-trem, 4.0x trem-bright)

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

### Phase 8: ML Correction (v2 Deployed)

Per-note MLP corrections run at note-on. Architecture: 2 inputs -> 8 hidden -> 8 hidden -> 11 outputs. 195 parameters, <10 us inference, zero per-sample cost.

**v2 (deployed Feb 2026):** Retrained with reduced outputs after v1 harmonic-vs-mode domain mismatch:
- **Outputs [0:5]: freq_offsets** — per-note mode frequency tuning (cents)
- **Outputs [5:10]: decay_offsets** — per-note mode decay adjustment (ratio)
- **Output [10]: ds_correction** — displacement scale correction from H2/H1 ratio. Runtime clamp [0.7, 1.5].

Plugin has BoolParam "MLP Corrections" (id="mlp", default ON) for real-time A/B testing.

Results: Freq 2.2 cents MAE, ds 0.16 MAE, best loss 0.158. 8 OBM training notes (MIDI 65-97, vel=80).

1. Training data: 8 OBM notes (MIDI 65-97, vel=80), SNR-filtered
2. Weights baked into `mlp_weights.rs` (no external files needed)
3. Corrections applied at note-on via `mlp_correction.rs`
4. Outside training range: corrections fade to identity over 12 semitones
5. See `ml/compute_residuals.py` and `ml/train_mlp.py` for training pipeline

---

## 24. Lessons from Previous Implementation (OpenWurli)

Key failure patterns from the previous project (40+ tuning rounds without convergence):

1. **No fudge factors.** Every parameter must trace to a physical quantity. If a compensation knob is needed, find and fix the underlying modeling error.
2. **Gaussian dwell filter only.** Sinc (rectangular pulse) and half-sine models have deep spectral nulls that forced 20x mode amplitude compensation, destroying the attack-to-sustain ratio.
3. **Miller cap polarity matters.** The cap provides MORE feedback at HF (low impedance), LESS at LF. Inverting this breaks register-dependent distortion.
4. **No artificial drive scaling.** The DkPreamp uses physical component values. If the sound is wrong, fix the circuit model, not the input scaling.
5. **No per-mode velocity exponents.** They double-count with the dwell filter's velocity-dependent brightening. Use uniform vel^2 scaling.
6. **DAW state override.** Changing parameter defaults requires users to re-add the plugin. Consider a version check in `stateLoad()`.

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
