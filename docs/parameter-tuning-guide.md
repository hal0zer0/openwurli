# Parameter Tuning Guide

## Purpose

Map how changing one DSP parameter cascades into others, so we stop playing
whack-a-mole. Built from the DS_AT_C4 0.70→0.80 session that exposed tight
coupling between displacement, output normalization, register trim, and
speaker saturation.

## Primary Parameters (The Knobs)

| Parameter | Location | Current | Role |
|-----------|----------|---------|------|
| `DS_AT_C4` | tables.rs | 0.85 | Pickup displacement anchor at C4 |
| `DS_CLAMP` | tables.rs | [0.02, 0.85] | Physical limit on displacement |
| `DS_EXPONENT` | tables.rs | 0.65 | Compliance-to-displacement curve shape |
| `TARGET_DB` | tables.rs | -13.0 | Absolute output level target |
| `VOICING_SLOPE` | tables.rs | -0.04 | Treble roll-off (dB/semitone above C4) |
| Register trim `ANCHORS` | tables.rs | 13 anchors | Per-note empirical correction |
| Speaker Xmax | speaker.rs | tanh(polynomial) | Cone excursion limiting |
| Volume default | params.rs | 0.63 | Audio-taper attenuator |

## Known Interaction Chains

### DS_AT_C4 → everything

```
DS_AT_C4 ↑
  → per-note DS ↑ (via compliance curve, clamped)
  → y_peak ↑ (pickup displacement)
  → 1/(1-y) nonlinearity ↑ (more bark, more harmonics)
  → pickup output ↑
  → output_scale proxy tries to compensate
    → BUT proxy doesn't model preamp Cin HPF or speaker
    → register trim (calibrated for old DS) is now WRONG
  → pre-speaker level ↑
    → speaker tanh compresses mid-register ff
    → TARGET_DB adjustments absorbed by tanh (non-linear)
    → bass/treble shift linearly, mid doesn't
```

### TARGET_DB in the tanh shadow

```
TARGET_DB ↓ (trying to reduce clipping)
  → bass/treble: shift by expected amount (linear region)
  → mid ff: barely moves (tanh ceiling)
  → mf all notes: shift by expected amount (linear region)
  → net effect: wider register spread at ff, quieter mf
  → WRONG direction for both problems
```

### Register trim domain mismatch

```
Trim calibrated at Tier 3 (speaker ON, tanh active)
  → mid-register measurements compressed by tanh
  → trim values "see through" tanh distortion
  → changing DS shifts pre-tanh levels
  → old trim + new DS = wrong corrections
  → fix requires recalibration, but recalibration
    itself is distorted by tanh (iterative convergence)
```

## Sweep Methodology — `calibrate` and `sensitivity` Subcommands

Implemented as Rust-native preamp-bench subcommands. No source patching, no rebuilds.
`CalibrationConfig` in `tables.rs` makes DS_AT_C4 and trim overridable at runtime.

### Single-config measurement: `calibrate`

Renders each (note, velocity) pair and measures at 5 tap points in the signal chain:

| Tap | What | Measured from |
|-----|------|---------------|
| T1  | Raw reed displacement | `reed.render()` output, peak |
| T2  | After pickup NL + HPF | Full pickup output |
| T3  | After output_scale | T2 × output_scale_with_config |
| T4  | After preamp | Oversampled DK preamp |
| T5  | After volume + PA + speaker | Full Tier 3 chain |

> **Note:** T1 (raw reed displacement) is computed internally but is **not** included as `t1_*` columns in the CSV output. Only T2-T5 appear as CSV columns. The reed peak is captured as `y_peak` (reed_peak × ds_actual).

Metrics per tap: `peak_db`, `rms_db`, `h2_h1_db` (where applicable).
Measurement window: 100-400ms of a 500ms render.

```bash
cargo run -p preamp-bench -- calibrate \
    --notes 36,40,44,48,52,56,60,64,68,72,76,80,84 \
    --velocities 40,80,127 \
    --ds-at-c4 0.85 \
    --volume 0.40 --speaker 1.0 \
    --zero-trim \
    --output /tmp/calibrate.csv
```

### Multi-DS grid sweep: `sensitivity`

Iterates over a range of DS_AT_C4 values, runs calibrate at each.

```bash
cargo run -p preamp-bench -- sensitivity \
    --notes 36,48,54,60,66,72,78,84 \
    --velocities 40,80,127 \
    --ds-range 0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85 \
    --scale-mode track \
    --output /tmp/sensitivity.csv
```

Scale modes:
- `track` (default): Override DS + keep current trim → "how much trim error remains?"
- `zero-trim`: Override DS + zero trim → "what's the natural imbalance?"
- `freeze`: Original DS=0.85 + current trim → "raw level change from DS alone"

Grid: 8 DS × 8 notes × 3 velocities = 192 renders. ~90 seconds.

### Analysis: `tools/analyze_calibration.py`

Reads CSV from either subcommand. Computes (text tables, no graphs):

1. Register spread at each DS
2. Optimal DS search (minimizes spread)
3. New trim anchor values (ready-to-paste `const ANCHORS` array)
4. Tanh compression ceiling map
5. Sensitivity coefficients (∂metric/∂DS per note)
6. Dynamic range per note at each DS

```bash
python tools/analyze_calibration.py /tmp/sensitivity.csv
```

### CSV columns (both subcommands)

```
midi, note_name, velocity, ds_at_c4, ds_actual, y_peak,
t2_peak_db, t2_rms_db, t2_h2_h1_db,
t3_peak_db, t3_rms_db,
t4_peak_db, t4_rms_db, t4_h2_h1_db,
t5_peak_db, t5_rms_db, t5_h2_h1_db,
proxy_db, trim_db, proxy_error_db, tanh_compression_db
```

## Calibration Protocol (revised)

Based on today's lessons, now automated via `calibrate`/`sensitivity`:

1. **Measure natural imbalance** (zero-trim mode)
   ```bash
   cargo run -p preamp-bench -- calibrate --zero-trim --output /tmp/natural.csv
   python tools/analyze_calibration.py /tmp/natural.csv
   ```
   - Shows true proxy error without trim distortion

2. **Find optimal DS** (sensitivity sweep)
   ```bash
   cargo run -p preamp-bench -- sensitivity --ds-range 0.60,0.65,0.70,0.75,0.80,0.85 --zero-trim
   python tools/analyze_calibration.py /tmp/sensitivity.csv
   ```
   - Script reports optimal DS (minimizes register spread) + suggested trim anchors

3. **Apply and verify**
   ```bash
   # Update DS_AT_C4 and ANCHORS in tables.rs from script output
   cargo run -p preamp-bench -- calibrate --ds-at-c4 <new_ds>
   python tools/analyze_calibration.py /tmp/calibrate.csv
   ```
   - Verify spread < 5 dB, tanh compression < 3 dB at mf, DR 18-22 dB mid-register

4. **Full audit**
   ```bash
   cargo run -p preamp-bench -- bark-audit
   ```

## Open Questions

- Should the proxy model the preamp Cin HPF? Would reduce trim magnitudes.
- Should trim be calibrated pre-speaker and post-speaker separately?
- Is the speaker tanh ceiling the right model, or should Xmax scale with frequency?
- Would a pre-speaker limiter (before tanh) give better control than post-hoc calibration?
