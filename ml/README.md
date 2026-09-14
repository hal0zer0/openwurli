# MLP Per-Note Correction Pipeline

A small neural network that runs once at note-on to correct per-note spectral characteristics, trained on real Wurlitzer 200A recordings.

## Overview

The physical model produces good spectral shape overall, but individual notes have small systematic errors in harmonic levels, tuning, and decay rates that vary across the keyboard. This pipeline extracts those residuals from real recordings and trains an MLP to correct them.

**Architecture (v2):** 2 inputs (MIDI note, velocity) -> 16 hidden (ReLU) -> 16 hidden (ReLU) -> 11 outputs. Total: 507 parameters. Runtime: <10 us per note-on. (The hidden width was 8 / 195 parameters before the v0.5.0 retrain; `--hidden` still defaults to 8, so pass `--hidden 16` to reproduce the shipped model.)

**Outputs (11 values):**
- H2-H6 frequency offsets (cents) -- 5 values
- H2-H6 decay ratio multipliers -- 5 values
- Displacement scale correction -- 1 value

v2 removed amplitude offsets (harmonic-vs-mode domain mismatch) and fixed a sign bug in the displacement scale correction that had v1 pushing corrections in the wrong direction.

**Training data:** 13 OBM isolated notes (MIDI 50-98, all velocity ~80, all gold tier) from [Freesound pack 5726](https://freesound.org/people/OldBassMan/packs/5726/) (CC-BY 4.0).

Note which stage-5 entry point you use: `pipeline.py` calls `assemble_dataset`
without an SNR cache, so only the H4+ mask and the anomaly detector apply and
all 13 notes survive. Running `compute_residuals.py` directly adds
inter-harmonic SNR filtering and drops to 8 notes. The shipped weights were
trained via `pipeline.py`, i.e. the 13-note set.

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

At note-on, `MlpCorrections::infer(midi, velocity)` runs the MLP forward pass and returns an `MlpCorrections` struct. The voice module applies these corrections to mode frequencies, decay rates, and displacement scale before synthesis begins.

Outside the training range (MIDI 65-97), corrections fade linearly to identity over 12 semitones, ensuring graceful degradation at keyboard extremes.

## Training Details

Shipped model (topology-revision retrain): hidden 16, 507 parameters, seed
chosen by sweeping {7, 42, 123, 999} at `--epochs 3000 --lr 3e-3 --patience 150`
(Huber delta 5.0, weight decay 1e-4) and taking the lowest validation loss.
With only 13 observations `load_data` trains on all of them and reports
validation on the same set — the "validation" loss is a fit quality number,
not a generalisation estimate.

- Best loss: 0.045 · freq_H2 1.67 cents MAE · decay_H2 0.69 ratio MAE ·
  ds_corr 0.09 MAE
- H4-H8 targets are always masked (below the OBM noise floor); H2/H3 are kept
  subject to anomaly detection
- Reproduce: `python pipeline.py --from-stage 4 --through-stage 5 --obm-only`,
  then `train_mlp.py --hidden 16 --epochs 3000 --seed <s>`, then
  `generate_rust_weights.py`, then `cargo fmt -p openwurli-dsp`

### Known limitations of the target set

- **Three of the five decay outputs are never trained.** `MAX_RELIABLE_HARMONIC`
  keeps only H2 and H3, so decay_H4-H6 have an all-false mask, a zero
  normalisation mean and unit std. Weight decay drives them to ~0, and the
  runtime lower clamp turns that into a 0.3 decay multiplier — i.e. modes 4-6
  decay 3.3x faster whenever corrections are enabled. That is a pipeline
  defect, not a fit problem: retraining cannot fix it, and it reproduces on
  every seed and on the pre-revision weights.
- **Single velocity.** Every OBM observation is velocity ~80, so anything the
  network does away from that velocity is extrapolation, and it is not
  constrained to move in the physically correct direction.
- **Room-coupled reference.** The OBM recordings are speaker-in-a-room
  captures, not DI. Absolute residuals against them mix instrument error with
  room and speaker colour and must not be read as circuit-fidelity figures.

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
