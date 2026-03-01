# CLAUDE.md

Guidance for Claude Code and AI-assisted contributors working on this codebase.

## Project Overview

OpenWurli is a CLAP/VST3 virtual instrument plugin that models the Wurlitzer 200A electric piano through analog circuit simulation. Physically accurate sound from first principles — no samples, no IRs, no curve-fitting.

## PR Guidelines

PRs must be clean, minimal, and follow software engineering best practices. The key question: **"If I was the project maintainer, would I want to review this?"**

- **Small and focused.** One concern per PR. If a PR touches unrelated code, split it.
- **Tested.** All existing tests must pass (`cargo test --workspace`). New DSP code needs unit tests. No "I'll add tests later."
- **No placeholder DSP.** Every signal processing block must be derived from actual circuit analysis, not "close enough" approximations. If the circuit behavior isn't understood, research it first — see `docs/`.
- **Formatted and lint-clean.** `cargo fmt --check` and `cargo clippy --workspace -- -D warnings` must pass.
- **Python is for tooling only.** The plugin is Rust. Python is exclusively for offline analysis, ML training, and test utilities.
- **It's OK to push back.** If a request would produce a large, messy, or untested PR, say so. Quality over velocity - ALWAYS.

## Domain Knowledge

Read `docs/` before making DSP decisions. Key 200A characteristics:

- **Solid-state preamp** — two-stage direct-coupled NPN CE amplifier (2N5089), asymmetric clipping (Stage 1: 5.3:1 sat/cutoff ratio)
- **Capacitive pickup** — reed vibration modulates capacitance (not electromagnetic like Rhodes). The 1/(1-y) nonlinearity is the primary source of "bark"
- **Tremolo inside feedback loop** — LDR shunts the preamp's emitter feedback path, modulating gain and timbre (not just volume)
- **Velocity is mechanical** — hammer force on reed, not electronic scaling

The authoritative schematic is `#203720-S-3` (serial 102905+). See `docs/SCHEMATIC_SOURCE.md` for how to obtain it. DO NOT use other Wurlitzer schematics — different models have different topology. The 200 is not the 200A.

## Build & Test

```bash
cargo build --workspace          # Build everything
cargo test --workspace           # Run all tests (~144)
cargo fmt --check                # Check formatting
cargo clippy --workspace -- -D warnings  # Lint

cargo xtask bundle openwurli --release   # Bundle CLAP + VST3

# Preamp validation CLI
cargo run -p preamp-bench -- render --note 60 --velocity 100 --duration 2.0 --output /tmp/test.wav
cargo run -p preamp-bench -- sweep --start 20 --end 20000 --points 50
cargo run -p preamp-bench -- bark-audit --notes 36,48,60,72,84

# Reed-only renderer
cargo run -p reed-renderer -- -n 60 -v 100 -d 1.0 -o /tmp/reed.wav

# Release (maintainer only — uses /release skill)
# /release 0.2.4 SomeCodename
```

## Project Structure

```
crates/
  openwurli-dsp/src/          # DSP library (pure math, no framework deps)
    reed.rs                   #   Modal oscillator (7 modes)
    hammer.rs                 #   Dwell filter + attack noise + onset ramp
    pickup.rs                 #   Electrostatic pickup (time-varying RC)
    voice.rs                  #   Voice assembly (reed + hammer + pickup)
    tables.rs                 #   Per-note parameters (freq, decay, mode ratios)
    variation.rs              #   Per-note detuning and amplitude variation
    filters.rs                #   Filter primitives (1-pole, biquad, DC blocker)
    oversampler.rs            #   2x polyphase IIR half-band
    dk_preamp.rs              #   DK method 8-node MNA preamp solver
    bjt_stage.rs              #   Single BJT CE stage (NR + soft-clip)
    mlp_correction.rs         #   Per-note MLP inference (2->8->8->11)
    mlp_weights.rs            #   Trained MLP weights (195 params)
    preamp.rs                 #   PreampModel trait
    tremolo.rs                #   LFO + CdS LDR + feedback modulation
    power_amp.rs              #   Class AB crossover + rail clipping
    speaker.rs                #   HPF/LPF cabinet simulation
  openwurli-plugin/src/       # nih-plug CLAP+VST3 plugin
    lib.rs                    #   Plugin entry, process callback, voice mgmt
    params.rs                 #   Parameter definitions
tools/
  preamp-bench/               # DSP validation CLI
  reed-renderer/              # Standalone WAV renderer
docs/                         # Technical reference (circuit analysis, DSP specs)
spice/                        # ngspice netlists and testbenches
ml/                           # MLP training pipeline (Python/PyTorch)
xtask/                        # nih-plug bundler
```

## Toolchain

| Tool | Purpose |
|------|---------|
| Rust (2024 edition) | Plugin and DSP |
| nih-plug (git HEAD) | CLAP + VST3 framework |
| Python 3.12+ | Offline analysis and ML training only |
| ALSA dev / JACK | Linux audio backends |

## Testing Tiers

| Tier | Scope | What it tests |
|------|-------|---------------|
| 1 | Circuit | Sine -> preamp -> stop. Gain, sweep, harmonics, tremolo-sweep |
| 2 | Voice + MLP | Reed -> pickup -> preamp -> stop. OBM comparison |
| 3 | Full plugin | Complete chain with power amp, speaker, volume |

## Version Codenames

Each release gets a codename from Wurlitzer song lyrics. Format in CHANGELOG: `## [x.y.z] "Codename" - YYYY-MM-DD`
