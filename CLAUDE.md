# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

OpenWurli is a virtual instrument plugin (CLAP and VST3 formats) that models the Wurlitzer 200A electric piano through analog circuit simulation. The goal is physically accurate sound — not sample playback or generic synthesis approximations.

**Critical context:** Multiple prior attempts to build this plugin have failed. This project follows a strict phased approach: each phase must be thoroughly tested and reviewed before proceeding to the next. Do not rush ahead or skip validation steps.

## Wurlitzer 200A Domain Knowledge

Before making any design or DSP decisions, **always consult `docs/`** first. It contains research materials being assembled specifically for this project.

Key distinctions of the 200A vs the 200 (and other Wurlitzer models) that the circuit simulation must capture:
- The 200A uses a solid-state amplifier circuit (not tube-based like earlier models)
- Reed-based tone generation: steel reeds vibrate near an electrostatic pickup
- The pickup is capacitive (not electromagnetic like Rhodes) — reed vibration modulates capacitance
- **Tremolo is INSIDE the preamp feedback loop** — R-10 (56K) feeds back from the output to TR-1's emitter via Ce1 (4.7 MFD coupling cap), providing series-series negative feedback. LDR (LG-1) shunts the feedback junction to ground via the cable, modulating how much feedback reaches the emitter and thus the closed-loop gain. This produces timbral modulation (not just volume).
- The preamp is a two-stage direct-coupled NPN CE amplifier (TR-1/TR-2, 2N5089). Asymmetric clipping headroom (Stage 1: 2.05V toward saturation vs 10.9V toward cutoff, ratio ~5.3:1) is the primary source of even-harmonic "bark"
- The 200A's specific amplifier topology, EQ curve, and soft-clipping characteristics define its signature sound
- Velocity response is mechanical (hammer force on reed), not electronic

## Development Philosophy

1. **Phased development** — complete and validate each phase before starting the next
2. **Analog circuit simulation** — model actual circuit behavior (component-level where it matters), not curve-fitted approximations
3. **Test everything** — each DSP module needs unit tests comparing output against known-good reference data
4. **Consult docs/ first** — the docs folder is the primary source of truth for circuit schematics, frequency response data, and behavioral specifications

## Toolchain

| Tool | Version | Purpose |
|------|---------|---------|
| Rust / Cargo | 1.92.0 | Plugin and DSP (Rust 2024 edition) |
| nih-plug | git HEAD | CLAP + VST3 plugin framework |
| Python | 3.12.3 | Supporting tools only (analysis, schematic preprocessing) |
| ALSA dev | 1.2.11 | Linux audio backend |
| JACK (PipeWire) | available | Linux audio backend |

## Build Commands

```bash
# Build everything
cargo build --workspace

# Run all tests (57 unit + integration)
cargo test --workspace

# Bundle CLAP + VST3 plugin (release build)
cargo xtask bundle openwurli --release
# Output: target/bundled/openwurli.clap, target/bundled/openwurli.vst3

# Install to user plugin directories
cp target/bundled/openwurli.clap ~/.clap/
cp -r target/bundled/openwurli.vst3 ~/.vst3/

# Preamp validation CLI
cargo run -p preamp-bench -- gain --freq 1000
cargo run -p preamp-bench -- sweep --start 20 --end 20000 --points 50
cargo run -p preamp-bench -- harmonics --freq 440
cargo run -p preamp-bench -- tremolo-sweep
cargo run -p preamp-bench -- render --note 60 --velocity 100 --duration 2.0 --output /tmp/test.wav

# Reed renderer (standalone WAV output)
cargo run -p reed-renderer -- -n 60 -v 100 -d 1.0 -o /tmp/reed.wav

# Python virtual environment (for analysis/test tools only)
source .venv/bin/activate
```

## Project Structure

```
Cargo.toml                          # Workspace root
crates/
  openwurli-dsp/                    # Shared DSP library (pure math, no framework deps)
    src/
      lib.rs                        # Module exports
      reed.rs                       # Modal oscillator (7 modes) with damper support
      hammer.rs                     # Dwell filter + attack noise
      pickup.rs                     # Electrostatic pickup model (HPF at 2312 Hz)
      voice.rs                      # Voice assembly (reed + hammer + pickup + damper)
      tables.rs                     # Per-note parameters (frequencies, decay rates, mode ratios)
      variation.rs                  # Per-note detuning and amplitude variation
      filters.rs                    # Filter primitives (OnePoleHpf/Lpf, DcBlocker, Biquad)
      oversampler.rs                # 2x polyphase IIR half-band oversampler
      bjt_stage.rs                  # Single BJT CE stage (NR solver + asymmetric soft-clip)
      preamp.rs                     # PreampModel trait + EbersMollPreamp
      tremolo.rs                    # LFO + CdS LDR model + feedback modulation
      power_amp.rs                  # Class AB crossover distortion + rail clipping
      speaker.rs                    # Variable HPF/LPF cabinet simulation
  openwurli-plugin/                 # nih-plug CLAP+VST3 plugin (cdylib)
    src/
      lib.rs                        # Plugin entry, process callback, voice management
      params.rs                     # Parameter definitions (Volume, Gain, Tremolo, Speaker)
tools/
  reed-renderer/                    # Standalone reed → WAV renderer
  preamp-bench/                     # Preamp DSP validation CLI (gain, sweep, harmonics, render)
xtask/                              # nih-plug bundler (cargo xtask bundle)
docs/                               # Research: schematics, circuit analysis, DSP specs
spice/                              # ngspice netlists and testbenches
.venv/                              # Python virtual environment (analysis tools)
```

## Authoritative Schematic

The **only** schematic reference for this project is:

```
docs/verified_wurlitzer_200A_series_schematic.pdf
```

This is the verified Wurlitzer Model 200A Electronic Piano Schematic (#203720-S-3, starting serial 102905). **Do not download, source, or use any other schematic PDF.** Other Wurlitzer schematics (200/203/206/207 combined sheets) have different component numbering and topology that will cause errors.

## Schematic Image Reading

Claude's vision pipeline downsamples all images to **max 1568px on the long edge** (~1.15 MP). Pre-rendered tiles in `schematic_tiles/` are already processed for AI reading — use those first.

**Pre-rendered tiles** (in `schematic_tiles/`, gitignored):
- Named after their region and DPI, e.g. `preamp_600dpi.png`, `overview_150dpi.png`
- Already preprocessed: grayscale, denoised, CLAHE contrast, sharpened, resized to fit Claude's limits
- Read these directly with the Read tool — no rendering needed for standard analysis

**To re-render or create new tiles**, use `tools/schematic_preprocess.py`:

```bash
source .venv/bin/activate

# List available named regions
python tools/schematic_preprocess.py regions

# Render a named region
python tools/schematic_preprocess.py render --pdf docs/verified_wurlitzer_200A_series_schematic.pdf --region preamp

# Render a custom area (normalized 0-1 coordinates)
python tools/schematic_preprocess.py render --pdf docs/verified_wurlitzer_200A_series_schematic.pdf --rect 0.1,0.3,0.3,0.5 --dpi 900

# Enhance an existing PNG
python tools/schematic_preprocess.py enhance some_image.png
```

Output goes to `schematic_tiles/` (gitignored). The pipeline: grayscale -> denoise -> CLAHE contrast -> unsharp mask -> border crop -> resize to fit Claude's limits.

**Automatic text region detection** — find component labels without manual coordinate hunting:

```bash
# Detect all text/annotation regions in a schematic area
python tools/schematic_preprocess.py detect-text \
    --pdf docs/verified_wurlitzer_200A_series_schematic.pdf --region preamp \
    --output-dir /tmp/text_detect/

# From an existing tile image
python tools/schematic_preprocess.py detect-text --input schematic_tiles/preamp_600dpi.png \
    --output-dir /tmp/text_detect/

# Tune detection sensitivity
python tools/schematic_preprocess.py detect-text --input img.png \
    --min-area 200 --max-area 30000 --kernel-w 20 --kernel-h 8 --output-dir /tmp/td/
```

Outputs: `detected_regions.png` (annotated overview with red boxes), `detected_regions.json` (manifest), `text_region_NNN.png` (individual enhanced crops). Mr Schemey reads the overview image to locate labels, then reads individual crops to decipher values.

**Optional OCR** (requires `pip install easyocr`):

```bash
python tools/schematic_preprocess.py ocr \
    --pdf docs/verified_wurlitzer_200A_series_schematic.pdf --region preamp-detail \
    --output /tmp/ocr_results.json --annotate /tmp/ocr_annotated.png
```

OCR is supplementary — Claude's vision on enhanced crops is more reliable for schematic text. Use OCR as a cross-check or when processing many regions programmatically.

**Two-pass strategy for circuit analysis:**
1. Overview crop at low DPI (150-300) to understand topology and signal flow
2. Detail crops at higher DPI (600-900) to read specific component values
3. (Optional) `detect-text` to automatically find and crop annotation regions for closer inspection

## Rules

- **Never skip a phase.** Each phase's tests must pass and output must be reviewed before the next phase begins.
- **No placeholder DSP.** Every signal processing block must be derived from actual circuit analysis, not "close enough" approximations. If the circuit behavior isn't understood yet, research it before coding it.
- **Python is for tooling only.** The plugin itself is C++ or Rust. Python scripts in `.venv` are exclusively for offline analysis, test signal generation, frequency response comparison, and MIDI test utilities.
- **Always read docs/ before implementing a DSP component.** The research materials may contain critical details about component values, circuit topology, or measured frequency response that must inform the implementation.
