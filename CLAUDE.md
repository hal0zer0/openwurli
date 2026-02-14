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
- **Tremolo is INSIDE the preamp feedback loop** — LDR (LG-1) + R-10 (56K) form a voltage divider in the preamp's negative feedback network, modulating closed-loop gain. This produces timbral modulation (not just volume). The "shunt-to-ground" description found in some sources is a simplification of this feedback-shunt topology.
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
| CMake | 3.28.3 | Build system |
| g++ | 13.3.0 | C++ compiler |
| Rust / Cargo | 1.92.0 | Available if Rust-based plugin framework is chosen |
| Python | 3.12.3 | Supporting tools only (analysis, MIDI/audio test scripts) |
| ALSA dev | 1.2.11 | Linux audio backend |
| JACK (PipeWire) | available | Linux audio backend |

## Build Commands

*To be filled in once the build system is established in Phase 1.*

```bash
# Python virtual environment (for analysis/test tools only)
source .venv/bin/activate
pip install <package>         # add supporting Python packages as needed
```

## Project Structure

```
docs/           # Research materials: schematics, frequency response data, Wurlitzer 200A specs
.venv/          # Python 3.12 virtual environment for supporting tools (audio/MIDI analysis)
```

*Source directories will be added as phases are implemented.*

## Rules

- **Never skip a phase.** Each phase's tests must pass and output must be reviewed before the next phase begins.
- **No placeholder DSP.** Every signal processing block must be derived from actual circuit analysis, not "close enough" approximations. If the circuit behavior isn't understood yet, research it before coding it.
- **Python is for tooling only.** The plugin itself is C++ or Rust. Python scripts in `.venv` are exclusively for offline analysis, test signal generation, frequency response comparison, and MIDI test utilities.
- **Always read docs/ before implementing a DSP component.** The research materials may contain critical details about component values, circuit topology, or measured frequency response that must inform the implementation.
