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

- **Solid-state preamp** — two-stage direct-coupled NPN CE amplifier; stage 2 is the high-gain stage (C-7 AC-grounds its emitter) and closed-loop gain is set by the R-10/Ce1 feedback divider; DC bias comes from an R-3 feedback servo, not a base divider (2026-09 drawn-topology revision — see docs/research/preamp-circuit.md)
- **Capacitive pickup** — reed vibration modulates capacitance (not electromagnetic like Rhodes). It is a through-slot comb: the reed swings into the finger band, and that field-solved overlap law (not a parallel-plate 1/(1-y)) is the primary harmonic source (see docs/research/pickup-overlap-law.md). "Bark" is a player's word for a percept; which harmonics carry it has not been established, so name the harmonics you measure
- **Tremolo inside feedback loop** — LDR shunts the preamp's emitter feedback path, modulating gain and timbre (not just volume)
- **Velocity is mechanical** — hammer force on reed, not electronic scaling

The authoritative schematic is `#203720-S-3` — specifically the titled archive scan; the Tropical Fish redraw is a different print (see `docs/SCHEMATIC_SOURCE.md` for provenance and how to obtain it). DO NOT use other Wurlitzer schematics — different models have different topology. The 200 is not the 200A.

## Build & Test

```bash
cargo build --workspace          # Build everything
cargo test --workspace           # Run all tests
cargo fmt --check                # Check formatting
cargo clippy --workspace -- -D warnings  # Lint

cargo xtask bundle openwurli --release   # Bundle CLAP + VST3

# Preamp validation CLI
cargo run -p preamp-bench -- render --note 60 --velocity 100 --duration 2.0 --output /tmp/test.wav
cargo run -p preamp-bench -- sweep --start 20 --end 20000 --points 50
cargo run -p preamp-bench -- h2-audit --notes 36,48,60,72,84

# Reed + pickup only (no preamp, amp or speaker)
cargo run -p preamp-bench -- render --note 60 --velocity 100 --duration 1.0 --no-preamp --no-poweramp --speaker 0 --no-mlp --output /tmp/reed.wav

# Release (maintainer only — uses /release skill)
# /release 0.2.4 SomeCodename
```

## Project Structure

```
crates/
  openwurli-dsp/src/          # DSP library (pure math, no framework deps)
    reed.rs                   #   Modal oscillator (7 modes)
    hammer.rs                 #   Dwell filter + onset ramp
    pickup.rs                 #   Capacitive pickup (field-solved overlap law + RC network)
    pickup_tables.rs          #   Generated overlap-law capacitance tables
    voice.rs                  #   Voice assembly (reed + hammer + pickup)
    engine.rs                 #   Engine: voices, shared chain, parameter smoothing
    tables.rs                 #   Per-note parameters (freq, decay, mode ratios)
    variation.rs              #   Per-note detuning and amplitude variation
    filters.rs                #   Biquad wrapper (backed by melange-primitives)
    oversampler.rs            #   2x polyphase IIR half-band
    dk_preamp/                #   Feature-toggled preamp (hand 9-node default; --features melange-preamp for 12-node)
    dk_preamp_legacy.rs       #   Hand-written 9-node MNA preamp solver (default since v0.5.2; drawn topology since v0.7.0)
    gen_preamp.rs             #   Melange-generated 12-node preamp solver (opt-in via --features melange-preamp)
    mlp_correction.rs         #   Per-note MLP inference (2->16->16->11)
    mlp_weights.rs            #   Trained MLP weights (507 params)
    preamp.rs                 #   PreampModel trait
    tremolo.rs                #   CdS LDR + melange Twin-T oscillator
    gen_tremolo.rs            #   Melange-generated Twin-T oscillator solver
    power_amp.rs              #   Power amp; behavioral NR is the shipping default (feature legacy-power-amp, in default set)
    gen_power_amp.rs          #   Melange-generated 7-BJT Class AB solver (opt-in: build with --no-default-features)
    speaker.rs                #   HPF/LPF cabinet simulation
    alias_audit.rs            #   Rate-convergence alias gate
  openwurli-plugin/src/       # nih-plug CLAP+VST3 plugin
    lib.rs                    #   Plugin entry, process callback, voice mgmt
    params.rs                 #   Parameter definitions
tools/
  preamp-bench/               # DSP validation CLI
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
| 2 | Voice + preamp | Reed -> pickup -> preamp -> stop. Voice + preamp renders |
| 3 | Full plugin | Complete chain with power amp, speaker, volume |

## Version Codenames

Each release gets a codename from Wurlitzer song lyrics. Format in CHANGELOG: `## [x.y.z] "Codename" - YYYY-MM-DD`
