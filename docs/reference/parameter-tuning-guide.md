# Parameter Tuning Guide

## Purpose

Map how changing one DSP parameter cascades into others, so we stop playing
whack-a-mole. Written after a calibration session exposed tight coupling between
the reed's swing at the pickup, output normalization, register trim and speaker
saturation.

## Primary Parameters (The Knobs)

| Parameter | Location | Current | Role |
|-----------|----------|---------|------|
| `pickup_swing_mm` | tables.rs | per note (provisional ff swing table) | Swing at the pickup: physics-set, NOT a voicing knob |
| `target_db` (`CalibrationConfig`) | tables.rs | −29.55 | Level anchor: holds C4 v100 at the preamp input |
| `voicing_slope` (`CalibrationConfig`) | tables.rs | −0.04 | Treble roll-off (dB/semitone above C4) |
| Register trim `ANCHORS` | tables.rs (`register_trim_db`) | 13 anchors | Per-note empirical correction |
| Speaker Xmax | speaker.rs | tanh(polynomial) | Cone excursion limiting (speaker character > 0 only) |
| Volume default | plugin params.rs | 0.80 | The drawn volume pot (audio taper) |

## Known Interaction Chains

### Swing → everything

The per-note ff swing (`tables::pickup_swing_mm`, provisional, physics-derived) is NOT a voicing
knob: it is set by hammer physics and replaced by measurement. `CalibrationConfig::swing_scale`
exists for diagnostic sweeps only. Register balance belongs to the output mapping.

```
swing ↑
  → per-note swing ↑ (mm at the pickup)
  → reed enters the finger band deeper ↑ (more harmonics)
  → pickup output ↑
  → output_scale proxy tries to compensate
    → BUT proxy doesn't model preamp Cin HPF or speaker
    → register trim (calibrated at the old swing) is now WRONG
  → pre-speaker level ↑
    → speaker tanh compresses mid-register ff
    → target_db adjustments absorbed by tanh (non-linear)
    → bass/treble shift linearly, mid doesn't
```

### Register trim domain mismatch

```
Trim calibrated at Tier 3 (speaker ON, tanh active)
  → mid-register measurements compressed by tanh
  → trim values "see through" tanh distortion
  → changing the swing shifts pre-tanh levels
  → old trim + new swing = wrong corrections
  → fix requires recalibration, but recalibration
    itself is distorted by tanh (iterative convergence)
```

## Sweep Methodology — `calibrate` and `sensitivity` Subcommands

Implemented as Rust-native preamp-bench subcommands. No source patching, no rebuilds.
`CalibrationConfig` in `tables.rs` makes the swing (pick + diagnostic scale) and trim overridable at runtime.

### Single-config measurement: `calibrate`

Renders each (note, velocity) pair and measures at 5 tap points in the signal chain:

| Tap | What | Measured from |
|-----|------|---------------|
| T1  | Raw reed displacement | `reed.render()` output, peak |
| T2  | After pickup NL + HPF | Full pickup output |
| T3  | After output_scale | T2 × output_scale_with_config |
| T4  | After preamp | Oversampled DK preamp |
| T5  | After volume + PA + speaker | Full Tier 3 chain |

> **Note:** T1 (raw reed displacement) is computed internally but is **not** included as `t1_*` columns in the CSV output. Only T2-T5 appear as CSV columns. The reed peak is captured as `y_peak` (reed_peak × swing_mm).

Metrics per tap: `peak_db`, `rms_db`, `h2_h1_db` (where applicable).
Measurement window: 100-400ms of a 500ms render.

```bash
cargo run -p preamp-bench -- calibrate \
    --notes 36,40,44,48,52,56,60,64,68,72,76,80,84 \
    --velocities 40,80,127 \
    --swing-scale 1.0 \
    --volume 0.50 --speaker 0.0 \
    --zero-trim \
    --output /tmp/calibrate.csv
```

### Swing-scale sweep: `sensitivity` (diagnostic)

Iterates over multipliers on the provisional swing, runs calibrate at each. A diagnostic of how
level and harmonics respond to swing — not a way to choose the swing.

```bash
cargo run -p preamp-bench -- sensitivity \
    --notes 36,48,54,60,66,72,78,84 \
    --velocities 40,80,127 \
    --swing-range 0.5,0.75,1.0,1.5,2.0 \
    --scale-mode track \
    --output /tmp/sensitivity.csv
```

Scale modes:
- `track` (default): Override the swing scale + keep current trim → "how much trim error remains?"
- `zero-trim`: Override the swing scale + zero trim → "what's the natural imbalance?"
- `freeze`: Shipping swing + current trim → "raw level change from swing alone"

Grid: 5 swing scales (default `--swing-range`) × 8 notes × 3 velocities = 120 renders.

### Analysis: `tools/analyze_calibration.py`

Reads CSV from either subcommand. Computes (text tables, no graphs):

1. Register spread at each swing scale
2. The swing scale that minimizes spread (diagnostic only: the swing is physics-set, never picked from this)
3. New trim anchor values (ready-to-paste `const ANCHORS` array)
4. Tanh compression ceiling map
5. Sensitivity coefficients (∂metric/∂swing scale per note)
6. Dynamic range per note at each swing scale

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

2. **Swing sensitivity (diagnostic)**
   ```bash
   cargo run -p preamp-bench -- sensitivity --swing-range 0.5,0.75,1.0,1.5,2.0 --zero-trim
   python tools/analyze_calibration.py /tmp/sensitivity.csv
   ```
   - Shows how register spread responds to swing. Do NOT pick the swing from it: the swing is
     physics-set; fix register balance in the output mapping (trim anchors).

3. **Apply and verify**
   ```bash
   # Update the register-trim ANCHORS in tables.rs from script output
   cargo run -p preamp-bench -- calibrate
   python tools/analyze_calibration.py /tmp/calibrate.csv
   ```
   - Verify spread < 5 dB, tanh compression < 3 dB at mf, DR 18-22 dB mid-register

4. **Full audit**
   ```bash
   cargo run -p preamp-bench -- h2-audit
   ```

## Open Questions

- Should the proxy model the preamp Cin HPF? Would reduce trim magnitudes.
- Should trim be calibrated pre-speaker and post-speaker separately?
- Is the speaker tanh ceiling the right model, or should Xmax scale with frequency?
- Would a pre-speaker limiter (before tanh) give better control than post-hoc calibration?
