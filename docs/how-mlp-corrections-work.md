# How the MLP Corrections Work

A non-technical overview of the per-note neural network that fine-tunes OpenWurli's physical model against real Wurlitzer recordings.

## The Problem

The physical model (reed oscillator, pickup, preamp, etc.) captures the *general* behavior of a Wurlitzer 200A — the right frequency response, the right distortion character, the right tremolo interaction. But real instruments have per-note quirks that pure physics can't predict:

- Manufacturing tolerances in reed dimensions and solder tuning masses
- Slight variations in reed-to-pickup alignment across the keyboard
- Accumulated wear, oxidation, and aging of individual reeds
- Resonances from the reed bar assembly that amplify or dampen specific notes

These are the differences between a model that sounds *like* a Wurlitzer and one that sounds like *this particular* Wurlitzer.

## The Approach

We use a tiny neural network — a Multi-Layer Perceptron (MLP) — to learn per-note corrections from real recordings. Think of it as auto-tuning the model to match a specific instrument.

**Training data:** 13 isolated single-note recordings from a real Wurlitzer 200A (OldBassMan's Freesound pack). These span MIDI 38-86 (D2 to D6) at their **true** pitch: the pack's filenames run one octave high, so each recording sits 12 semitones below its label. All were played at velocity ~80.

**What we measure:** For each recorded note, we analyze the harmonic spectrum — how strong is the fundamental vs the 2nd harmonic vs the 3rd, and so on. We run the same notes through our model and compare. The differences are the "residuals" the MLP learns to correct.

## The Network

```
Input (2 values)
  |
  v
Hidden layer 1 (16 neurons, ReLU activation)
  |
  v
Hidden layer 2 (16 neurons, ReLU activation)
  |
  v
Output (11 values)
```

**Inputs:**
- MIDI note number (normalized to 0-1)
- Velocity (normalized to 0-1)

**Outputs (11 values):**
- 5 frequency offsets in cents (for harmonics 2-6) — nudges each partial's pitch
- 5 decay rate multipliers (for harmonics 2-6) — adjusts how fast each partial fades
- 1 displacement-scale value — **retired 2026-09-27** and ignored by the engine (the network still emits it so the weights load)

Only the H2 and H3 heads have training data, so only those are applied; the H4-H6 heads emit identity.

Total parameters: 507 weights and biases.

## When It Runs

The MLP fires **once at note-on** — when you press a key. It takes the MIDI note number and velocity, runs a forward pass through the two hidden layers (just matrix multiplies and ReLU), and produces 11 correction values. These are applied to the reed oscillator's mode frequencies and decay rates before the note starts sounding.

Zero ongoing CPU cost — the corrections are baked into the voice parameters at note-on and never recomputed.

## Safety Rails

The MLP was trained on only 13 notes. To prevent it from producing wild corrections for notes it's never seen:

- **Clamping:** Frequency corrections are limited to +/-100 cents (one semitone). Decay multipliers are clamped to 0.3-3.0x.
- **Fade-out:** Outside the training range (MIDI 38-86), corrections linearly fade to identity over 12 semitones. Below MIDI 26 and above MIDI 98, the MLP has zero effect — the physics model runs unassisted.
- **Default off:** The "MLP Corrections" plugin parameter is off by default; turn it on to hear the corrections.

## What It Fixes

The corrections now touch only the pitch and decay rate of the H2 and H3 partials:

- **Frequency:** compensates for the beam model not exactly capturing each reed's inharmonicity.
- **Decay:** adjusts how fast those partials fade relative to the model.

Without the MLP, the model is the physics alone. With it, the H2/H3 pitch and decay move toward the one instrument in the training recordings — one sample, not "the" Wurlitzer, which is why it ships off.

## Training Pipeline

The full pipeline lives in `ml/` and runs in Python:

1. **Extract** isolated notes from OBM recordings
2. **Analyze** harmonic content of each note (using Goertzel algorithm)
3. **Render** the same notes through the Rust model
4. **Compute residuals** — the difference between real and modeled harmonics
5. **Train** the MLP on these residuals (PyTorch)
6. **Export** weights to Rust source code (`mlp_weights.rs`)

The exported weights are committed directly into the Rust source — no runtime file loading, no Python dependency in the plugin.

## Further Reading

- [Calibration and Evaluation](reference/calibration-and-evaluation.md) — how we measure accuracy against real recordings
- [Signal Chain Architecture](research/signal-chain-architecture.md) — where the MLP fits in the overall signal chain
- [`ml/README.md`](../ml/README.md) — the Python training pipeline in detail
