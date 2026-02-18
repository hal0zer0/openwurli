---
name: wurli-audit
description: Generate comprehensive test audio and analysis data from the OpenWurli signal chain. Produces WAV files, harmonic analysis, frequency response, and a structured report that multiple review agents can evaluate from different perspectives (circuit accuracy, perceptual quality, DSP fidelity).
user-invocable: true
argument-hint: "[quick|full|render-only] [--notes 60,72] [--velocities 64,100,127] [--tremolo] [--output-dir /tmp/wurli-audit] [--model dk|ebers-moll]"
allowed-tools: Read, Grep, Glob, Bash, Write, Edit, Task
---

# Wurli-Audit: Signal Chain Test Battery & Multi-Agent Review

You are running the OpenWurli signal chain audit. This generates test audio, runs objective analyses, and produces a structured report that multiple specialist agents can review.

## Step 0: Parse Arguments

Arguments: $ARGUMENTS

Parse the mode from arguments:
- **`quick`** (default if no args): Fast sanity check. 5 notes, 2 velocities, core analyses only. ~30 seconds.
- **`full`**: Comprehensive audit. Chromatic scale, 5 velocities, all analyses, tremolo variants. ~2 minutes.
- **`render-only`**: Just generate WAV files, skip numeric analysis. For listening sessions.
- **`--notes N,N,N`**: Override note selection (MIDI numbers).
- **`--velocities V,V,V`**: Override velocity selection.
- **`--tremolo`**: Include tremolo-on variants (doubles render count).
- **`--output-dir PATH`**: Override output directory (default: `/tmp/wurli-audit`).
- **`--review`**: After generating, dispatch to review agents automatically.
- **`--review-only`**: Skip generation, review existing output directory.
- **`--model dk|ebers-moll`**: Select preamp model. Default: `dk` (DK circuit solver). Use `ebers-moll` for A/B comparison with the legacy model.

## Step 1: Setup

1. Set the output directory (default `/tmp/wurli-audit`). Create subdirectories:
   ```
   $OUTPUT_DIR/
   ├── data/          # Numeric analysis (text, CSV)
   ├── wav/
   │   ├── single/    # Individual note renders
   │   ├── velocity/  # Same note at multiple velocities
   │   └── tremolo/   # Tremolo-on variants
   └── report.md      # The main analysis report
   ```

2. Build the workspace:
   ```bash
   cargo build --workspace -p preamp-bench -p reed-renderer 2>&1
   ```
   If build fails, stop and report. Do not proceed with stale binaries.

3. Record the git hash for reproducibility:
   ```bash
   git -C /home/homeuser/dev/openwurli rev-parse --short HEAD
   ```

## Step 2: Define Test Matrix

### Quick Mode
```
NOTES=(36 48 60 72 84)         # C2, C3, C4, C5, C6
VELOCITIES=(64 127)            # mf, ff
ANALYSIS_NOTES=(36 48 60 72 84)
```

### Full Mode
```
NOTES=(33 36 40 45 48 52 57 60 64 69 72 76 81 84 88 93 96)  # Full range, ~chromatic sampling
VELOCITIES=(30 64 90 110 127)  # pp, mp, mf, f, ff
ANALYSIS_NOTES=(33 36 40 45 48 52 57 60 64 69 72 76 81 84 88 93 96)
```

Override with `--notes` and `--velocities` if provided.

## Step 3: Run Numeric Analyses

Skip this step if mode is `render-only`.

Run these analyses **sequentially** (they share the DSP pipeline and stdout):

### 3a. Bark Audit (H2/H1 per processing stage)
```bash
cargo run -p preamp-bench -- bark-audit \
  --notes $(IFS=,; echo "${ANALYSIS_NOTES[*]}") \
  --velocities $(IFS=,; echo "${VELOCITIES[*]}") \
  > $OUTPUT_DIR/data/bark_audit.txt 2>&1
```
**What this reveals:** Whether the pickup nonlinearity is generating the right H2 content and whether the preamp is preserving or destroying it. Each stage should be additive, not subtractive.

### 3b. Frequency Response Sweep
```bash
cargo run -p preamp-bench -- sweep --start 20 --end 20000 --points 60 \
  --csv $OUTPUT_DIR/data/freq_response_notrem.csv \
  > $OUTPUT_DIR/data/freq_response_notrem.txt 2>&1
```
If `--tremolo` or `full` mode:
```bash
cargo run -p preamp-bench -- sweep --start 20 --end 20000 --points 60 --ldr 19000 \
  --csv $OUTPUT_DIR/data/freq_response_trem.csv \
  > $OUTPUT_DIR/data/freq_response_trem.txt 2>&1
```
**What this reveals:** Whether the preamp's frequency shaping matches SPICE targets. The DK model has preamp-only BW ~15.5 kHz (nearly constant regardless of Rldr). Full-chain BW (including input network): ~11.8 kHz (no trem) / ~9.7 kHz (trem bright). The input coupling network's interaction with Miller capacitances accounts for the difference.

### 3c. Harmonic Distortion at Key Notes
For notes [36, 48, 60, 72, 84] (or ANALYSIS_NOTES in full mode), at amplitude 0.001 (mf) and 0.005 (ff).
Convert MIDI to Hz: `freq = 440 * 2^((midi - 69) / 12)`. Key frequencies:
- MIDI 36 (C2) = 65.4 Hz, 48 (C3) = 130.8 Hz, 60 (C4) = 261.6 Hz, 72 (C5) = 523.3 Hz, 84 (C6) = 1046.5 Hz

```bash
# Example for one note — loop over all ANALYSIS_NOTES and both amplitudes
cargo run -p preamp-bench -- harmonics --freq $FREQ_HZ --amplitude $AMP \
  >> $OUTPUT_DIR/data/harmonics.txt 2>&1
```
Before each result, echo a header line to the file: `echo "=== MIDI $NOTE ($FREQ_HZ Hz) amp=$AMP ===" >> ...`

**What this reveals:** Whether H2 > H3 at every note/velocity (MUST-PASS). Whether H2/H1 follows the expected slope of -0.48 dB/semitone. Whether THD is reasonable. **Note:** The DK preamp with ideal BJTs produces near-zero THD (~0.007%) — nearly all harmonic content comes from the pickup nonlinearity, not the preamp. This is correct behavior: the Wurlitzer's bark is a pickup phenomenon, not preamp distortion.

### 3d. Tremolo Gain Sweep
```bash
cargo run -p preamp-bench -- tremolo-sweep --ldr-min 19000 --ldr-max 1000000 --steps 25 \
  --csv $OUTPUT_DIR/data/tremolo_sweep.csv \
  > $OUTPUT_DIR/data/tremolo_sweep.txt 2>&1
```
**What this reveals:** Gain modulation range. Should be ~6.1 dB (from 6.0 dB no-trem to 12.1 dB trem-bright).

### 3e. Single-Frequency Gain Checks
```bash
# No tremolo
cargo run -p preamp-bench -- gain --freq 1000 > $OUTPUT_DIR/data/gain_1k_notrem.txt 2>&1
# Tremolo bright
cargo run -p preamp-bench -- gain --freq 1000 --ldr 19000 > $OUTPUT_DIR/data/gain_1k_trem.txt 2>&1
```

## Step 4: Render WAV Files

For each note in NOTES and each velocity in VELOCITIES:

```bash
cargo run -p preamp-bench -- render \
  --note $NOTE --velocity $VEL --duration 2.5 \
  --volume 0.40 --speaker 1.0 \
  --output $OUTPUT_DIR/wav/single/note${NOTE}_v${VEL}.wav
```

Use `--speaker 0.7` for a moderately authentic cabinet simulation (not flat, not extreme).

If `--tremolo` or `full` mode, also render with tremolo enabled for a subset of notes.
Use `--ldr` to set tremolo brightness — lower resistance = more feedback shunted = brighter/louder.
The LDR oscillates between ~19kOhm (bright peak) and ~1MOhm (dark trough) in the real circuit.
Render at the bright peak to capture maximum tremolo effect:
```bash
cargo run -p preamp-bench -- render \
  --note $NOTE --velocity $VEL --duration 3.0 \
  --volume 0.40 --speaker 1.0 --ldr 19000 \
  --output $OUTPUT_DIR/wav/tremolo/note${NOTE}_v${VEL}_trem_bright.wav
```
And at a mid-cycle point for typical tremolo character:
```bash
cargo run -p preamp-bench -- render \
  --note $NOTE --velocity $VEL --duration 3.0 \
  --volume 0.40 --speaker 1.0 --ldr 100000 \
  --output $OUTPUT_DIR/wav/tremolo/note${NOTE}_v${VEL}_trem_mid.wav
```
**Note:** These are static LDR snapshots, not modulated tremolo. The real tremolo LFO (~5.63 Hz) is not yet integrated into the render command. These captures show the timbral extremes.

### Velocity Demonstration Renders
Always render middle C (60) at all velocities in VELOCITIES, with 3-second duration:
```bash
cargo run -p preamp-bench -- render \
  --note 60 --velocity $VEL --duration 3.0 \
  --volume 0.40 --speaker 1.0 \
  --output $OUTPUT_DIR/wav/velocity/C4_v${VEL}.wav
```

### Reed-Only Reference (no preamp/power amp/speaker)
Render a few reed-only tones for comparison:
```bash
cargo run -p reed-renderer -- -n 60 -v 64 -d 2.0 -o $OUTPUT_DIR/wav/single/reed_only_C4_v64.wav
cargo run -p reed-renderer -- -n 60 -v 127 -d 2.0 -o $OUTPUT_DIR/wav/single/reed_only_C4_v127.wav
```

## Step 5: Generate Analysis Report

Write `$OUTPUT_DIR/report.md` with this structure:

```markdown
# OpenWurli Signal Chain Audit Report
**Generated:** [timestamp]
**Git revision:** [hash]
**Mode:** [quick|full]
**Notes tested:** [list]
**Velocities tested:** [list]

## Executive Summary
[2-3 sentences: overall health of the signal chain. Any MUST-PASS failures?]

## Tier 1: Must-Pass Results

### H2 > H3 Compliance
[For every note/velocity combo in the bark audit, check if H2/H1 > H3/H1.
Report: X/Y passed (Z% compliance). Target: 100%.]

### Decay Rate vs Pitch Correlation
[If renderable from data, note whether higher pitches decay faster.]

### Dynamic Range
[Measure peak amplitude difference between lowest and highest velocity at same note.
Target: >= 15 dB.]

### No Clipping at mf
[Check if any mf renders show digital clipping (samples at +/- 1.0).]

### Attack Overshoot
[If measurable from WAV peak vs sustain RMS. Target: >= 1 dB at mf.]

## Tier 2: Quality Indicators

### H2/H1 Slope
[From bark audit data, fit H2_dB vs MIDI note.
Expected: slope ~ -0.48 dB/semitone, intercept ~ 17.5 dB.
Report actual slope and delta.]

### Frequency Response Shape
[From sweep data: -3dB points, peak frequency, peak gain.
DK model: preamp-only BW ~15.5 kHz (constant). Full-chain BW ~11.8 kHz (no trem) / ~9.7 kHz (trem bright).]

### Tremolo Modulation Depth
[From tremolo sweep: min gain, max gain, range in dB.
Expected: ~6.1 dB range.]

### Preamp Gain Accuracy
[1 kHz gain vs SPICE target.
No tremolo: expected 6.0 dB. Tremolo bright: expected 12.1 dB.]

## Raw Data Summary

### Bark Audit
[Paste or summarize bark_audit.txt — the per-stage H2/H1 table]

### Frequency Response
[Key points from sweep: -3dB low, peak freq, peak gain, -3dB high]

### Harmonic Distortion
[Table: Note | Freq | H1 | H2 | H3 | H4 | H5 | THD% | H2/H3 dB]

### Tremolo Sweep
[Table: LDR (ohm) | Gain (dB)]

## WAV File Manifest
[Table listing every WAV file generated, with note, velocity, duration, and file size.
Group by category: single/, velocity/, tremolo/, reed-only.]

## Issues & Recommendations
[List any failures, anomalies, or areas needing attention.
Prioritize by tier (must-pass failures first).]

## Review Prompts
[Pre-written prompts for each review agent — see Step 6.]
```

**IMPORTANT:** Actually parse the output files and compute the metrics. Don't just describe what *should* be there — read the data files, extract numbers, and report actual values.

## Step 6: Multi-Agent Review Dispatch

If `--review` was specified, or if the user asks for review after generation, dispatch to these agents **in parallel** using the Task tool:

### Dr Dawgg PhDope (Perceptual/Musical Review)
Launch as a `general-purpose` agent with this prompt:
```
You are Dr Dawgg PhDope — [include full persona from MEMORY.md].

Review the OpenWurli signal chain audit at $OUTPUT_DIR.

1. Read $OUTPUT_DIR/report.md for context and metrics.
2. Listen to (read and analyze) the WAV files in $OUTPUT_DIR/wav/.
   Focus on: velocity/C4_v*.wav for dynamic range feel,
   single/note*_v127.wav for bark character across registers,
   single/reed_only_*.wav vs single/note60_v*.wav for preamp contribution.
3. If tremolo renders exist, evaluate tremolo authenticity.

Evaluate and score (1-10) each category:
- **Attack Feel**: Is it percussive? Does it have that Wurli "thwack"?
- **Bark Character**: Does harder playing produce the right kind of growl?
- **Dynamic Range**: Does pp feel delicate and ff feel aggressive?
- **Timbral Evolution**: Does the attack brighten then the sustain warm up?
- **Register Balance**: Bass warm, mids present, treble sparkly but not harsh?
- **Tremolo** (if present): Asymmetric? Timbral, not just volume?
- **Overall "Gig Factor"**: Would you play this at a gig? What's missing?

Be specific about what notes/velocities sound wrong and WHY.
Write your review to $OUTPUT_DIR/review_dr_dawgg.md.
```

### Mr Schemey (Circuit Accuracy Review)
Launch as a `mr-schemey` agent with this prompt:
```
Review the OpenWurli signal chain audit at $OUTPUT_DIR.

1. Read $OUTPUT_DIR/report.md for the full analysis.
2. Read $OUTPUT_DIR/data/freq_response_notrem.txt and compare to SPICE predictions.
3. Read $OUTPUT_DIR/data/harmonics.txt and verify H2/H3 ratios match circuit asymmetry predictions.
4. Read $OUTPUT_DIR/data/tremolo_sweep.txt and verify gain modulation matches feedback topology.
5. Read $OUTPUT_DIR/data/bark_audit.txt and check each stage's H2 contribution.

Cross-reference all values against docs/preamp-circuit.md and docs/output-stage.md.

For each measurement, report:
- **Expected** (from SPICE/circuit analysis)
- **Actual** (from audit data)
- **Delta** and whether it's within acceptable tolerance
- **Root cause** if delta is large — which DSP component is likely responsible?

Write your review to $OUTPUT_DIR/review_mr_schemey.md.
```

### Circuit Spice (SPICE Validation)
Launch as a `circuit-spice` agent with this prompt:
```
Review the OpenWurli signal chain audit at $OUTPUT_DIR.

1. Read $OUTPUT_DIR/report.md for DSP measurements.
2. Compare the frequency response data against existing SPICE simulations in spice/testbench/.
3. If significant discrepancies exist, propose a SPICE testbench that would isolate the difference.
4. Check the harmonic distortion data — does the H2/H1 ratio at each stage match what SPICE predicts for the pickup nonlinearity and preamp asymmetry?

Focus on: Are we modeling the circuit correctly, or has the DSP drifted from the analog truth?

Write your review to $OUTPUT_DIR/review_circuit_spice.md.
```

## Step 7: Summary

After all analyses and renders complete, print a summary to the user:
```
Wurli Audit Complete
====================
Output: $OUTPUT_DIR
Report: $OUTPUT_DIR/report.md
WAV files: [count] rendered
Analyses: [list of analyses run]

Quick Results:
- H2>H3 compliance: X/Y (Z%)
- Gain @1kHz: X.X dB (target: 6.0 dB)
- Freq response peak: XXX Hz (target: 447 Hz)
- Dynamic range: XX dB (target: >= 15 dB)
- Tremolo range: X.X dB (target: 6.1 dB)

[If --review: "Review agents dispatched. Results will be in $OUTPUT_DIR/review_*.md"]
```

If any Tier 1 (must-pass) tests failed, highlight them prominently with recommendations for which DSP module to investigate.

## Future: Chord Rendering (Not Yet Implemented)
When single-voice quality is validated, add a `--chords` option to render polyphonic test cases:
- Major triad (C4-E4-G4) at mf and ff — check chord compression and intermodulation
- Octave (C3-C4) — beating and phase interaction
- Dense voicing (C4-E4-G4-Bb4) — worst-case summing
- Stacked fifths across registers — bass vs treble interaction
This is a Tier 3 fine-tuning concern. Get individual voices right first.

## Notes for the Operator

- **Calibration targets** come from `docs/calibration-and-evaluation.md` and SPICE simulations — NOT from OBM recordings. The recordings are off-limits per project rules.
- **Preamp model**: The default preamp is `dk` (DK circuit solver — 8-node MNA with 2×2 Newton-Raphson). The legacy `ebers-moll` model is available via `--model ebers-moll` for A/B comparison. Both the plugin and preamp-bench default to DK.
- **DK preamp characteristics**: Near-zero THD with ideal BJTs. BW ~15.5 kHz independent of Rldr (key improvement over legacy model). Gain: 6.13 dB (no trem) / 12.17 dB (trem bright) vs SPICE targets of 6.0/12.1 dB.
- **Preamp-bench `render` subcommand** handles the full signal chain: reed -> pickup -> preamp (2x oversampled) -> power amp -> speaker -> WAV.
- **Reed-renderer** is the isolated reed+pickup (no preamp/amp/speaker) for A/B comparison.
- If a subcommand doesn't support a flag (e.g., `--tremolo-rate`), check the current CLI args in `tools/preamp-bench/src/main.rs` and adapt. The DSP evolves — don't assume flags exist without checking.
- All WAV files are 24-bit 44.1 kHz mono.
