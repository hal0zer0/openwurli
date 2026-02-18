# MLP Per-Note Correction Pipeline

A small neural network that runs once at note-on to correct per-note spectral characteristics, trained on real Wurlitzer 200A recordings.

## Overview

The physical model produces good spectral shape overall, but individual notes have small systematic errors in harmonic levels, tuning, and decay rates that vary across the keyboard. This pipeline extracts those residuals from real recordings and trains an MLP to correct them.

**Architecture:** 2 inputs (MIDI note, velocity) -> 8 hidden (ReLU) -> 8 hidden (ReLU) -> 22 outputs. Total: 294 parameters. Runtime: <10 us per note-on.

**Outputs (22 values):**
- H2-H8 amplitude offsets (dB) -- 7 values
- H2-H8 frequency offsets (cents) -- 7 values
- H2-H8 decay ratio offsets -- 7 values
- Displacement scale multiplier -- 1 value

**Training data:** 9 OBM gold-tier isolated notes (MIDI 65-97, velocity ~80) from [Freesound pack 5726](https://freesound.org/people/OldBassMan/packs/5726/) (CC-BY 4.0).

## Pipeline Stages

```
1. extract_notes.py      Extract note events from OBM recordings
2. score_isolation.py    Score isolation quality, filter candidates
3. extract_harmonics.py  Goertzel-based harmonic analysis (H1-H8)
4. render_model_notes.py Render matching notes via preamp-bench
5. compute_residuals.py  Compute OBM-vs-model residuals -> training_data.npz
6. train_mlp.py          Train the MLP (PyTorch)
7. generate_rust_weights.py  Export weights -> mlp_weights.rs
```

## Usage

```bash
# Activate the Python environment
source .venv/bin/activate

# Full extraction pipeline (stages 1-5) — requires OBM recordings
python ml/pipeline.py

# OBM isolated notes only (fast, no polyphonic extraction)
python ml/pipeline.py --obm-only

# Resume from a specific stage
python ml/pipeline.py --from-stage 3

# Full pipeline including training and weight export (stages 1-7)
python ml/pipeline.py --train

# Dry run — show what would be done
python ml/pipeline.py --dry-run
```

## Prerequisites

- Python 3.12+ with dependencies from `tools/requirements.txt`
- PyTorch (`pip install torch`) for training (stage 6)
- OBM Wurlitzer 200A recordings (Freesound pack 5726) in the expected location
- Built `preamp-bench` tool (`cargo build -p preamp-bench --release`) for rendering model notes

## Output

The final artifact is `crates/openwurli-dsp/src/mlp_weights.rs`, containing the trained weights as Rust constants. This file is included by `mlp_correction.rs` at compile time.

## Integration

At note-on, `mlp_correction::compute_corrections(midi, velocity)` runs the MLP forward pass and returns an `MlpCorrections` struct. The voice module applies these corrections to mode amplitudes, frequencies, decay rates, and displacement scale before synthesis begins.

Outside the training range (MIDI 65-97), corrections fade linearly to identity over 12 semitones, ensuring graceful degradation at keyboard extremes.

## Training Details

- Seed: 9, Hidden size: 8, Weight decay: 1e-4, Epochs: 10K
- Best loss: 0.074
- SNR-filtered: H4-H8 always masked (below noise floor), H2/H3 filtered by 10 dB SNR threshold + anomaly detection
- H2 mean deficit: +0.73 dB -> 0.29 dB MAE after MLP
- H3 mean deficit: +8.75 dB -> 0.94 dB MAE after MLP
- Frequency: ~2.5 cents MAE
- Decay H2: 0.32 ratio MAE

## Files

| File | Purpose |
|------|---------|
| `pipeline.py` | Orchestrator — runs all stages in sequence |
| `extract_notes.py` | Stage 1: note event extraction |
| `score_isolation.py` | Stage 2: isolation quality scoring |
| `extract_harmonics.py` | Stage 3: Goertzel harmonic analysis |
| `goertzel_utils.py` | Shared Goertzel DFT utilities |
| `render_model_notes.py` | Stage 4: model note rendering via preamp-bench |
| `compute_residuals.py` | Stage 5: residual computation |
| `train_mlp.py` | Stage 6: MLP training (PyTorch) |
| `generate_rust_weights.py` | Stage 7: weight export to Rust |
| `h3_analysis.py` | H3 harmonic deep-dive analysis |
| `h3_analysis_v2.py` | H3 analysis with improved filtering |
