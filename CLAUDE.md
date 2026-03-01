# CLAUDE.md
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
Guidance for Claude Code and AI-assisted contributors working on this codebase.
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
## Project Overview
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
OpenWurli is a CLAP/VST3 virtual instrument plugin that models the Wurlitzer 200A electric piano through analog circuit simulation. Physically accurate sound from first principles — no samples, no IRs, no curve-fitting.
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
## PR Guidelines
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
PRs must be clean, minimal, and follow software engineering best practices. The key question: **"If I was the project maintainer, would I want to review this?"**
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
- **Small and focused.** One concern per PR. If a PR touches unrelated code, split it.
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
- **Tested.** All existing tests must pass (`cargo test --workspace`). New DSP code needs unit tests. No "I'll add tests later."
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
- **No placeholder DSP.** Every signal processing block must be derived from actual circuit analysis, not "close enough" approximations. If the circuit behavior isn't understood, research it first — see `docs/`.
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
- **Formatted and lint-clean.** `cargo fmt --check` and `cargo clippy --workspace -- -D warnings` must pass.
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
- **Python is for tooling only.** The plugin is Rust. Python is exclusively for offline analysis, ML training, and test utilities.
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
- **It's OK to push back.** If a request would produce a large, messy, or untested PR, say so. Quality over velocity - ALWAYS.
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
## Domain Knowledge
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
Read `docs/` before making DSP decisions. Key 200A characteristics:
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
- **Solid-state preamp** — two-stage direct-coupled NPN CE amplifier (2N5089), asymmetric clipping (Stage 1: 5.3:1 sat/cutoff ratio)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
- **Capacitive pickup** — reed vibration modulates capacitance (not electromagnetic like Rhodes). The 1/(1-y) nonlinearity is the primary source of "bark"
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
- **Tremolo inside feedback loop** — LDR shunts the preamp's emitter feedback path, modulating gain and timbre (not just volume)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
- **Velocity is mechanical** — hammer force on reed, not electronic scaling
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
The authoritative schematic is `#203720-S-3` (serial 102905+). See `docs/SCHEMATIC_SOURCE.md` for how to obtain it. DO NOT use other Wurlitzer schematics — different models have different topology. The 200 is not the 200A.
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
## Build & Test
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
```bash
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
cargo build --workspace          # Build everything
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
cargo test --workspace           # Run all tests (~144)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
cargo fmt --check                # Check formatting
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
cargo clippy --workspace -- -D warnings  # Lint
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
cargo xtask bundle openwurli --release   # Bundle CLAP + VST3
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
# Preamp validation CLI
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
cargo run -p preamp-bench -- render --note 60 --velocity 100 --duration 2.0 --output /tmp/test.wav
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
cargo run -p preamp-bench -- sweep --start 20 --end 20000 --points 50
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
cargo run -p preamp-bench -- bark-audit --notes 36,48,60,72,84
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
# Reed-only renderer
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
cargo run -p reed-renderer -- -n 60 -v 100 -d 1.0 -o /tmp/reed.wav
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
# Release (maintainer only — write CHANGELOG entry first)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
./scripts/release.sh 0.2.3 SomeCodename --dry-run
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
```
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
## Project Structure
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
```
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
crates/
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
  openwurli-dsp/src/          # DSP library (pure math, no framework deps)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    reed.rs                   #   Modal oscillator (7 modes)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    hammer.rs                 #   Dwell filter + attack noise + onset ramp
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    pickup.rs                 #   Electrostatic pickup (time-varying RC)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    voice.rs                  #   Voice assembly (reed + hammer + pickup)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    tables.rs                 #   Per-note parameters (freq, decay, mode ratios)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    variation.rs              #   Per-note detuning and amplitude variation
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    filters.rs                #   Filter primitives (1-pole, biquad, DC blocker)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    oversampler.rs            #   2x polyphase IIR half-band
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    dk_preamp.rs              #   DK method 8-node MNA preamp solver
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    bjt_stage.rs              #   Single BJT CE stage (NR + soft-clip)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    mlp_correction.rs         #   Per-note MLP inference (2->8->8->11)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    mlp_weights.rs            #   Trained MLP weights (195 params)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    preamp.rs                 #   PreampModel trait
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    tremolo.rs                #   LFO + CdS LDR + feedback modulation
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    power_amp.rs              #   Class AB crossover + rail clipping
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    speaker.rs                #   HPF/LPF cabinet simulation
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
  openwurli-plugin/src/       # nih-plug CLAP+VST3 plugin
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    lib.rs                    #   Plugin entry, process callback, voice mgmt
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
    params.rs                 #   Parameter definitions
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
tools/
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
  preamp-bench/               # DSP validation CLI
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
  reed-renderer/              # Standalone WAV renderer
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
scripts/
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
  release.sh                  # Automated release pipeline
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
docs/                         # Technical reference (circuit analysis, DSP specs)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
spice/                        # ngspice netlists and testbenches
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
ml/                           # MLP training pipeline (Python/PyTorch)
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
xtask/                        # nih-plug bundler
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
```
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
## Toolchain
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
| Tool | Purpose |
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
|------|---------|
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
| Rust (2024 edition) | Plugin and DSP |
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
| nih-plug (git HEAD) | CLAP + VST3 framework |
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
| Python 3.12+ | Offline analysis and ML training only |
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
| ALSA dev / JACK | Linux audio backends |
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
## Testing Tiers
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
| Tier | Scope | What it tests |
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
|------|-------|---------------|
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
| 1 | Circuit | Sine -> preamp -> stop. Gain, sweep, harmonics, tremolo-sweep |
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
| 2 | Voice + MLP | Reed -> pickup -> preamp -> stop. OBM comparison |
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
| 3 | Full plugin | Complete chain with power amp, speaker, volume |
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
## Version Codenames
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |

| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
Each release gets a codename from Wurlitzer song lyrics. See `docs/release-codenames.md` for the full list. Format: `## [x.y.z] "Codename" - YYYY-MM-DD`
| v0.2.3 | TwoTurntables | "Two turntables" — Where It's At, Beck |
