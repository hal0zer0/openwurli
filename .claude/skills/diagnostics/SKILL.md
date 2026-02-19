---
name: diagnostics
description: Consolidated signal chain diagnostics with 5 escalating levels. Level 1 = smoke test (build + tests). Level 2 = spot-check (gain + H2>H3). Level 3 = quick audit (bark, sweep, tremolo). Level 4 = full render audit + Dr Dawgg perceptual review. Level 5 = adds Mr Schemey circuit review + Circuit Spice SPICE validation.
user-invocable: true
argument-hint: "<level 1-5> [--output-dir /tmp/wurli-diagnostics] [--model dk|ebers-moll]"
allowed-tools: Read, Grep, Glob, Bash, Write, Edit, Task
---

# Diagnostics: Consolidated Signal Chain Validation

You are running the OpenWurli consolidated diagnostics skill. This provides 5 escalating levels of scrutiny, from a 15-second smoke test to a full expert review session.

## Step 0: Parse Arguments

Arguments: $ARGUMENTS

Parse the **level** (required, integer 1-5):
- If a bare number is given (e.g., `3`), that's the level.
- If no level is given, default to **level 1**.
- If the argument is not a number 1-5, tell the user the valid range and stop.

Optional flags:
- **`--output-dir PATH`**: Override output directory (default: `/tmp/wurli-diagnostics`).
- **`--model dk|ebers-moll`**: Select preamp model. Default: `dk`.

Set `$LEVEL`, `$OUTPUT_DIR`, and `$MODEL` from the parsed arguments.

Announce the level to the user before starting:
```
Diagnostics Level $LEVEL
========================
```
With a one-line description of what this level covers.

## Step 1: Setup (All Levels)

1. Record the start time.

2. Create the output directory structure (levels 3+):
   ```
   $OUTPUT_DIR/
   ├── data/          # Numeric analysis (text, CSV)
   ├── wav/
   │   ├── single/    # Individual note renders
   │   ├── velocity/  # Same note at multiple velocities
   │   └── tremolo/   # Tremolo-on variants
   └── report.md      # The main analysis report
   ```
   For levels 1-2, create only `$OUTPUT_DIR/data/` (minimal output).

3. Record the git hash:
   ```bash
   git -C /home/homeuser/dev/openwurli rev-parse --short HEAD
   ```

---

## Level 1: Smoke Test (~15 seconds)

**Goal:** Does it compile? Do all tests pass?

### 1a. Build
```bash
cargo build --workspace 2>&1
```
If build fails, report the error and **stop**. Nothing else matters if it doesn't compile.

### 1b. Tests
```bash
cargo test --workspace 2>&1
```
Count total tests, passed, failed, ignored. Report the summary.

### 1c. Report (Level 1)
Print a compact summary:
```
Level 1: Smoke Test
===================
Build:  PASS / FAIL
Tests:  X passed, Y failed, Z ignored (of N total)
Time:   Xs

Result: ALL CLEAR / ISSUES FOUND
```

**If level == 1, stop here.**

---

## Level 2: Signal Chain Spot-Check (~30 seconds)

*Includes everything from Level 1.*

**Goal:** Are the key preamp numbers in the right ballpark?

### 2a. Preamp Gain at 1 kHz
Run two gain checks:
```bash
cargo run -p preamp-bench -- gain --freq 1000 2>&1
cargo run -p preamp-bench -- gain --freq 1000 --ldr 19000 2>&1
```
Save output to `$OUTPUT_DIR/data/gain_1k_notrem.txt` and `$OUTPUT_DIR/data/gain_1k_trem.txt`.

**Check:**
- No tremolo: expect ~6.0 dB (tolerance: +/- 1.0 dB)
- Tremolo bright: expect ~12.1 dB (tolerance: +/- 1.0 dB)

### 2b. Quick Harmonic Check at C4
```bash
cargo run -p preamp-bench -- harmonics --freq 261.6 --amplitude 0.003 2>&1
```
Save to `$OUTPUT_DIR/data/harmonics_c4.txt`.

**Check:** H2 > H3 (MUST-PASS).

### 2c. Report (Level 2)
Append to the Level 1 report:
```
Level 2: Spot-Check
===================
Gain @1kHz (no trem):    X.X dB  (target: 6.0 dB)   PASS/FAIL
Gain @1kHz (trem bright): X.X dB  (target: 12.1 dB)  PASS/FAIL
H2 > H3 at C4 (261.6 Hz): YES/NO                      PASS/FAIL
```

**If level == 2, stop here.**

---

## Level 3: Quick Audit (~1-2 minutes)

*Includes everything from Levels 1-2.*

**Goal:** Comprehensive numeric validation of the signal chain across notes and frequencies.

### 3a. Bark Audit (5 notes x 2 velocities)
```bash
cargo run -p preamp-bench -- bark-audit \
  --notes 36,48,60,72,84 --velocities 64,127 \
  > $OUTPUT_DIR/data/bark_audit.txt 2>&1
```
**What this reveals:** Per-stage H2/H1 at each note. Pickup should be the dominant H2 source.

### 3b. Frequency Response Sweep (no tremolo)
```bash
cargo run -p preamp-bench -- sweep --start 20 --end 20000 --points 60 \
  --csv $OUTPUT_DIR/data/freq_response_notrem.csv \
  > $OUTPUT_DIR/data/freq_response_notrem.txt 2>&1
```
**Check:** Full-chain BW ~11.8 kHz (no trem). Peak near 447 Hz.

### 3c. Tremolo Gain Sweep
```bash
cargo run -p preamp-bench -- tremolo-sweep --ldr-min 19000 --ldr-max 1000000 --steps 25 \
  --csv $OUTPUT_DIR/data/tremolo_sweep.csv \
  > $OUTPUT_DIR/data/tremolo_sweep.txt 2>&1
```
**Check:** Gain range ~6.1 dB (from ~6.0 dB to ~12.1 dB).

### 3d. Harmonic Analysis (5 notes x 2 amplitudes)
For notes [36, 48, 60, 72, 84] at amplitudes 0.001 (mf) and 0.005 (ff):
Convert MIDI to Hz: `freq = 440 * 2^((midi - 69) / 12)`.
Key frequencies: 36=65.4, 48=130.8, 60=261.6, 72=523.3, 84=1046.5 Hz.

```bash
for each note/amplitude combo:
  echo "=== MIDI $NOTE ($FREQ Hz) amp=$AMP ===" >> $OUTPUT_DIR/data/harmonics.txt
  cargo run -p preamp-bench -- harmonics --freq $FREQ --amplitude $AMP \
    >> $OUTPUT_DIR/data/harmonics.txt 2>&1
```

**Check:** H2 > H3 at every combo. H2/H1 should decrease with higher pitch (~-0.48 dB/semitone).

### 3e. Generate Report
Write `$OUTPUT_DIR/report.md` following the structure in the Report Template section below.
**IMPORTANT:** Actually parse the data files and extract real numbers. Do not paraphrase — compute.

### 3f. Report (Level 3 Console)
Print a summary table:
```
Level 3: Quick Audit
====================
H2>H3 compliance:     X/10 (Y%)    PASS/FAIL
Gain @1kHz no-trem:   X.X dB       PASS/FAIL
Gain @1kHz trem:      X.X dB       PASS/FAIL
Tremolo range:        X.X dB       PASS/FAIL (target: ~6.1 dB)
BW (no trem, -3dB):   X.X kHz      INFO
H2/H1 slope:          X.XX dB/semi INFO

Full report: $OUTPUT_DIR/report.md
```

**If level == 3, stop here.**

---

## Level 4: Full Render Audit (~3-5 minutes)

*Includes everything from Levels 1-3.*

**Goal:** Generate WAV files for every test condition. Complete render matrix with peak analysis.

### 4a. Both Frequency Sweeps
Run the trem-bright sweep (no-trem already done in Level 3):
```bash
cargo run -p preamp-bench -- sweep --start 20 --end 20000 --points 60 --ldr 19000 \
  --csv $OUTPUT_DIR/data/freq_response_trem.csv \
  > $OUTPUT_DIR/data/freq_response_trem.txt 2>&1
```

### 4b. Render Matrix: 5 Notes x 2 Velocities
Notes: 36, 48, 60, 72, 84. Velocities: 64, 127.

```bash
cargo run -p preamp-bench -- render \
  --note $NOTE --velocity $VEL --duration 2.5 \
  --volume 0.40 --speaker 1.0 \
  --output $OUTPUT_DIR/wav/single/note${NOTE}_v${VEL}.wav
```
That's 10 renders.

### 4c. Velocity Demos at C4
Render C4 (note 60) at velocities 30, 64, 90, 110, 127:
```bash
cargo run -p preamp-bench -- render \
  --note 60 --velocity $VEL --duration 3.0 \
  --volume 0.40 --speaker 1.0 \
  --output $OUTPUT_DIR/wav/velocity/C4_v${VEL}.wav
```
That's 5 renders.

### 4d. Tremolo LDR Snapshot Renders
Render C4 at velocity 100 with 3 LDR values (bright/mid/dark):
```bash
cargo run -p preamp-bench -- render \
  --note 60 --velocity 100 --duration 3.0 \
  --volume 0.40 --speaker 1.0 --ldr 19000 \
  --output $OUTPUT_DIR/wav/tremolo/C4_v100_ldr19k.wav

cargo run -p preamp-bench -- render \
  --note 60 --velocity 100 --duration 3.0 \
  --volume 0.40 --speaker 1.0 --ldr 100000 \
  --output $OUTPUT_DIR/wav/tremolo/C4_v100_ldr100k.wav

cargo run -p preamp-bench -- render \
  --note 60 --velocity 100 --duration 3.0 \
  --volume 0.40 --speaker 1.0 --ldr 500000 \
  --output $OUTPUT_DIR/wav/tremolo/C4_v100_ldr500k.wav
```

### 4e. Reed-Only References
```bash
cargo run -p reed-renderer -- -n 60 -v 64 -d 2.0 -o $OUTPUT_DIR/wav/single/reed_only_C4_v64.wav
cargo run -p reed-renderer -- -n 60 -v 127 -d 2.0 -o $OUTPUT_DIR/wav/single/reed_only_C4_v127.wav
```

### 4f. Peak Level Analysis
For each rendered WAV, use a quick analysis to check:
- Peak sample value (is anything clipping at +/- 1.0?)
- Rough RMS level
Report any clipped files prominently.

### 4g. Update Report
Update `$OUTPUT_DIR/report.md` with:
- Trem-bright frequency response data
- WAV file manifest (filename, note, velocity, peak level, file size)
- Peak level analysis results
- Any clipping warnings

### 4g2. A/B Comparison Against Extracted Real Wurlitzer Notes

Run `wurli_compare.py` to compare synthesized notes against extracted real Wurlitzer recordings.
These extractions come from full recordings (Beatles covers, Kind of Blue sessions, improvisations)
and have varying degrees of audio cross-contamination (bleed from other notes, room ambience).
The isolation score per note indicates quality — higher is cleaner.

**CAVEAT:** These are NOT clean isolated recordings like the OBM samples. They are extracted from
polyphonic/full-mix recordings. Use them for broad calibration trends (harmonic balance, decay
rates, spectral centroid) but do NOT treat per-note numbers as ground truth. The OBM samples
in `~/dev/mlwurli/input/5726__oldbassman__wurlitzer-200a/` remain the gold standard for
precise measurements.

Check if extractions exist:
```bash
EXTRACTION_DIRS=$(find /tmp/wurli_extracted -name "extraction_metadata.json" -exec dirname {} \; 2>/dev/null | sort)
```

If extractions are available, run the comparison:
```bash
python tools/wurli_compare.py $EXTRACTION_DIRS \
  -o $OUTPUT_DIR/ab_comparison/ \
  --top-per-pitch 1 2>&1 | tee $OUTPUT_DIR/data/ab_comparison.txt
```

If no extractions exist, skip this step and note in the report:
```
A/B Comparison: SKIPPED (no extractions found in /tmp/wurli_extracted/)
To generate: python tools/recording_analyzer.py extract <recording.wav> -o /tmp/wurli_extracted/<name>/
Source recordings: ~/dev/mlwurli/input/
```

Save the comparison report JSON to `$OUTPUT_DIR/data/ab_comparison_report.json` if generated.
Append a summary to `$OUTPUT_DIR/report.md`:
- Mean harmonic distance (dB)
- Per-octave breakdown
- Worst/best harmonic matches
- Note the cross-contamination caveat

### 4h. Dr Dawgg PhDope (Perceptual/Musical Review)

Launch as a `general-purpose` agent with `run_in_background: true`:
```
You are Dr Dawgg PhDope — an old-school Wurlitzer player and technician who still gigs on weekends. You have decades of hands-on experience with real 200As — tuning reeds, replacing caps, adjusting bias, and most importantly, *playing*. You know what a real Wurli sounds like under your fingers and won't settle for anything that doesn't feel right. Your ear is calibrated to the G-Funk sound — Warren G, Snoop, Dre — but you also know your Herbie Hancock, Ray Charles, and Supertramp. You speak your mind with authority and flavor. If it's wack, he'll say so. If it's fire, he'll say that too.

**CRITICAL EVALUATION PHILOSOPHY:**
Your job is NOT to suggest how to "tune parameters and filters to get a nice sound."
Your job IS to identify where the MODEL DEVIATES FROM A REAL 200A and suggest how
to better model the instrument's physics. You have access to an extensive library of
200A specifications, patents, circuit analysis, and OBM recordings in the project's
docs/ directory. Every suggestion you make should be grounded in "the real 200A does X,
our model does Y, the discrepancy is because Z."

Before evaluating, read these key reference documents for 200A ground truth:
- docs/reed-and-hammer-physics.md — hammer dwell, onset behavior, modal physics
- docs/pickup-system.md — electrostatic pickup, 1/(1-y) nonlinearity, displacement
- docs/preamp-circuit.md — preamp topology, gain structure, feedback
- docs/output-stage.md — power amp, tremolo, speaker characteristics
- docs/calibration-and-evaluation.md — test methodology and targets

**Ground truth hierarchy (strongest to weakest):**
1. **SPICE circuit simulations** (spice/ directory) — built directly from the verified
   200A schematic (docs/verified_wurlitzer_200A_series_schematic.pdf). These model the
   actual circuit with real component values and should be treated as near-ground-truth
   for gain, frequency response, harmonic distortion, tremolo behavior, and DC bias.
   When DSP measurements disagree with SPICE, the DSP is wrong.
2. **Wurlitzer patents** (Andersen US 2,919,616; Miessner US 2,932,231 / US 3,215,765) —
   factory design intent for mechanical parameters (reed tolerances, hammer geometry,
   pickup placement, dwell times).
3. **OBM recordings** — real 200A audio captures. Excellent for perceptual targets
   (attack character, bark, decay rates, spectral balance) but include instrument
   condition variability, room acoustics, and mic coloration. Use for "does it sound
   like a real one" validation, not for precise circuit parameter extraction.

When you hear something wrong, diagnose it as a MODELING issue:
- "Bass attack is too slow" → check onset ramp vs OBM cycle-by-cycle data
- "Treble is too punchy" → check pickup displacement_scale, output_scale calibration
- "Bark is wrong" → check H2/H1 vs OBM measurements, pickup nonlinearity model
- "Tremolo sounds like volume LFO" → check feedback topology, LDR shunt path
Do NOT suggest: "increase the bass EQ", "add more compression", "adjust the attack knob."
DO suggest: "the onset ramp doesn't match the OBM data which shows X", "the register
trim was calibrated on file peaks but should use attack-weighted energy per Section Y."

Review the OpenWurli signal chain audit at $OUTPUT_DIR.

1. Read $OUTPUT_DIR/report.md for context and metrics.
2. Read the docs/ reference files listed above to ground your evaluation in 200A physics.
3. Read ONLY these key WAV files (do NOT load every WAV — stay lean):
   - velocity/C4_v30.wav and velocity/C4_v127.wav (dynamic range extremes)
   - single/note36_v127.wav (bass bark)
   - single/note84_v127.wav (treble character)
   - single/reed_only_C4_v127.wav vs single/note60_v127.wav (preamp contribution)
   - ONE tremolo file if present: tremolo/C4_v100_ldr19k.wav
   That's 6 WAV files max. Do NOT read additional WAVs.
4. Use the report.md metrics for notes/velocities you didn't listen to directly.
5. If $OUTPUT_DIR/data/ab_comparison.txt exists, read it for A/B comparison data
   against real extracted Wurlitzer notes. NOTE: these extractions have audio
   cross-contamination (bleed from other notes in the recording). Use for broad
   trends (harmonic balance, spectral centroid) but not as precise ground truth.
   The per-octave breakdown and worst-offender list are most useful.
   Do NOT produce graphs or matplotlib plots — numbers only.

Evaluate and score (1-10) each category. For each score below 8, cite the specific
200A reference (doc section, patent, OBM measurement) that defines the target behavior
and explain HOW the model deviates from it:
- **Attack Feel**: Does onset match OBM cycle-by-cycle data? Hammer dwell per Miessner patent?
- **Bark Character**: H2/H1 ratio vs OBM? Pickup 1/(1-y) generating correct harmonic balance?
- **Dynamic Range**: Velocity curve matching mechanical hammer-reed physics?
- **Timbral Evolution**: Attack centroid vs sustain centroid matching OBM spectral arc?
- **Register Balance**: Attack energy balanced as a voicing technician would set it?
- **Tremolo** (if present): Feedback-loop timbral modulation, not just volume? Per SPICE?
- **Overall Model Accuracy**: Where does the model most deviate from 200A ground truth?

Every recommendation must be in the form: "The real 200A does [X] (source: [doc/patent/OBM]),
our model does [Y], fix by [modeling change]." Never suggest subjective tuning.
Write your review to $OUTPUT_DIR/review_dr_dawgg.md.
```
**Wait for Dr Dawgg to finish** (poll the background task output) before proceeding.

### 4i. Report (Level 4 Console)
Print Level 3 summary plus:
```
Level 4: Full Render Audit + Dr Dawgg Review
=============================================
WAV files rendered:    XX
Clipping detected:     NONE / [list files]
Peak level range:      X.XX to X.XX
Reed-only references:  2
Tremolo snapshots:     3

Dr Dawgg PhDope:   $OUTPUT_DIR/review_dr_dawgg.md
Gig Factor:        X/10

Full report: $OUTPUT_DIR/report.md
WAV files:   $OUTPUT_DIR/wav/
```

**If level == 4, stop here.**

---

## Level 5: Full Audit + Circuit Expert Review (~10-15 minutes)

*Includes everything from Levels 1-4 (including Dr Dawgg's review from Level 4).*

**Goal:** Bring in the circuit experts. Two specialist agents review all data against SPICE simulations and schematic analysis.

### 5a. Dispatch Circuit Review Agents (SEQUENTIAL — memory safety)

**IMPORTANT — Memory management:** Launch agents ONE AT A TIME, not in parallel. Each agent
runs in the background with a turn cap. Wait for each agent to finish before launching the next.
This prevents two fat context windows from coexisting in RAM simultaneously.

Run in this order: Mr Schemey → Circuit Spice.

#### Agent 1: Mr Schemey (Circuit Accuracy Review)
Launch as a `mr-schemey` agent with `run_in_background: true`:
```
Review the OpenWurli signal chain audit at $OUTPUT_DIR.

1. Read $OUTPUT_DIR/report.md for the full analysis.
2. Read $OUTPUT_DIR/data/freq_response_notrem.txt and compare to SPICE predictions.
   Full-chain BW targets: ~11.8 kHz (no trem), ~9.7 kHz (trem bright).
   Preamp-only BW: ~15.5 kHz (nearly constant regardless of Rldr).
3. Read $OUTPUT_DIR/data/harmonics.txt and verify H2/H3 ratios match circuit asymmetry predictions.
   The DK preamp with ideal BJTs produces near-zero THD — all bark should come from pickup 1/(1-y).
4. Read $OUTPUT_DIR/data/tremolo_sweep.txt and verify gain modulation matches feedback topology.
   Expected range: ~6.1 dB (6.0 to 12.1 dB).
5. Read $OUTPUT_DIR/data/bark_audit.txt and check each stage's H2 contribution.
   Pickup should be the dominant source. Preamp, power amp, speaker should add negligible H2.

Cross-reference all values against docs/preamp-circuit.md and docs/output-stage.md.

For each measurement, report:
- **Expected** (from SPICE/circuit analysis)
- **Actual** (from audit data)
- **Delta** and whether it's within acceptable tolerance
- **Root cause** if delta is large — which DSP component is likely responsible?

Write your review to $OUTPUT_DIR/review_mr_schemey.md.
```
**Wait for Mr Schemey to finish** before launching Agent 2.

#### Agent 2: Circuit Spice (SPICE Validation)
Launch as a `circuit-spice` agent with `run_in_background: true`:
```
Review the OpenWurli signal chain audit at $OUTPUT_DIR.

1. Read $OUTPUT_DIR/report.md for DSP measurements.
2. Compare the frequency response data in $OUTPUT_DIR/data/freq_response_notrem.txt
   and $OUTPUT_DIR/data/freq_response_trem.txt against existing SPICE simulations
   in spice/testbench/ (especially tb_variable_gbw.cir and tb_dk_validation.cir).
3. If significant discrepancies exist (>2 dB at any frequency), propose a SPICE testbench
   that would isolate the difference.
4. Check the harmonic distortion data in $OUTPUT_DIR/data/harmonics.txt — does the H2/H1
   ratio at each stage match what SPICE predicts for the pickup nonlinearity and preamp asymmetry?

Focus on: Are we modeling the circuit correctly, or has the DSP drifted from the analog truth?

Write your review to $OUTPUT_DIR/review_circuit_spice.md.
```
**Wait for Circuit Spice to finish** before proceeding to synthesis.

### 5b. Collect and Synthesize Reviews

After all three agents have completed (Dr Dawgg from Level 4, Mr Schemey and Circuit Spice from above),
read their review files and generate a synthesis:
- Read `$OUTPUT_DIR/review_dr_dawgg.md`
- Read `$OUTPUT_DIR/review_mr_schemey.md`
- Read `$OUTPUT_DIR/review_circuit_spice.md`

Write `$OUTPUT_DIR/review_synthesis.md` containing:
1. **Agreement points** — what all reviewers confirm is working well
2. **Disagreement points** — where reviewers see different issues
3. **Prioritized action items** — ranked by severity and cross-reviewer consensus
4. **Overall signal chain health** — a 1-10 score averaging across perspectives

### 5c. Report (Level 5 Console)
Print Level 4 summary plus:
```
Level 5: Circuit Expert Review
==============================
Mr Schemey:        $OUTPUT_DIR/review_mr_schemey.md
Circuit Spice:     $OUTPUT_DIR/review_circuit_spice.md
Synthesis:         $OUTPUT_DIR/review_synthesis.md

[2-3 line summary of key findings across all 3 reviewers]
```

---

## Report Template (Levels 3+)

Write `$OUTPUT_DIR/report.md` with this structure:

```markdown
# OpenWurli Signal Chain Diagnostics Report
**Generated:** [timestamp]
**Git revision:** [hash]
**Diagnostics level:** [1-5]
**Preamp model:** [dk|ebers-moll]

## Executive Summary
[2-3 sentences: overall health of the signal chain. Any MUST-PASS failures?]

## Tier 1: Must-Pass Results

### H2 > H3 Compliance
[For every note/velocity/amplitude combo, check if H2 > H3.
Report: X/Y passed (Z% compliance). Target: 100%.]

### Dynamic Range
[Peak amplitude difference between v=64 and v=127 at C4.
Target: >= 15 dB.]

### No Clipping at mf
[Check if any mf renders clip. Level 4+ only; note if not applicable.]

## Tier 2: Quality Indicators

### H2/H1 Slope
[From bark audit data, fit H2_dB vs MIDI note.
Expected: slope ~ -0.48 dB/semitone.]

### Frequency Response Shape
[From sweep: -3dB points, peak frequency, peak gain.
Full-chain targets: BW ~11.8 kHz (no trem) / ~9.7 kHz (trem bright).]

### Tremolo Modulation Depth
[From tremolo sweep: min gain, max gain, range in dB.
Expected: ~6.1 dB range.]

### Preamp Gain Accuracy
[1 kHz gain vs SPICE target.
No tremolo: expected 6.0 dB. Tremolo bright: expected 12.1 dB.]

## Raw Data Summary

### Bark Audit
[Summarize bark_audit.txt — the per-stage H2/H1 table]

### Frequency Response
[Key points from sweep: -3dB low, peak freq, peak gain, -3dB high]

### Harmonic Distortion
[Table: Note | Freq | H1 | H2 | H3 | H4 | H5 | THD% | H2>H3?]

### Tremolo Sweep
[Table: LDR (ohm) | Gain (dB)]

## WAV File Manifest (Level 4+)
[Table: filename | note | velocity | peak | size]

## Issues & Recommendations
[Prioritized list of failures and anomalies. Tier 1 failures first.]
```

**CRITICAL:** Parse the actual data files and compute real metrics. Do not paraphrase or estimate — read the numbers from the output files.

## Calibration Targets Reference

These come from SPICE simulations and circuit analysis — NOT from recordings (which are off-limits).

| Metric | Target | Source | Tolerance |
|--------|--------|--------|-----------|
| Gain @1kHz (no trem) | 6.0 dB | SPICE | +/- 1.0 dB |
| Gain @1kHz (trem bright) | 12.1 dB | SPICE | +/- 1.0 dB |
| Tremolo range | 6.1 dB | SPICE | +/- 1.0 dB |
| Full-chain BW (no trem) | 11.8 kHz | SPICE | +/- 2.0 kHz |
| Full-chain BW (trem bright) | 9.7 kHz | SPICE | +/- 2.0 kHz |
| H2 > H3 | All combos | Circuit physics | 100% compliance |
| H2/H1 slope | -0.48 dB/semi | Pickup geometry | +/- 0.15 |
| Dynamic range (v64 vs v127) | >= 15 dB | Mechanical | minimum |

## Notes for the Operator

- **Levels are cumulative**: Level 3 runs everything from Levels 1+2+3. Level 5 runs everything.
- **Early termination**: If Level 1 fails (build or test failure), stop immediately. Don't run higher levels on broken code.
- **Audio recordings are OFF LIMITS** per project rules. All targets come from SPICE and circuit physics.
- **Preamp model**: Default is `dk` (DK circuit solver). The legacy `ebers-moll` is available via `--model ebers-moll`.
- **Output directory**: Defaults to `/tmp/wurli-diagnostics`. Each run overwrites the previous.
- **Render flags follow the Three-Tier Standard**:
  - **Tier 2** (OBM comparison): `--no-poweramp --speaker 0.0 --volume 1.0` — matches recording point
  - **Tier 3** (full plugin, as user hears it): `--volume 0.40 --speaker 1.0` — plugin defaults
  - `--gain` is NOT a valid preamp-bench flag. Do not use it.
- **This skill replaces wurli-audit** as the primary entry point. Use `/diagnostics` going forward.
