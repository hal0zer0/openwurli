# OpenWurli

An *obsessively* physically-modeled Wurlitzer 200A electric piano plugin for CLAP and VST3 hosts. NOT IN ANY WAY OFFICIAL OR AFFILIATED WITH ANYONE.


()
## What Is This?

OpenWurli models the complete analog signal chain of a Wurlitzer 200A electric piano from first principles — no samples, no impulse responses, no curve-fitted approximations. Every stage is derived from the actual circuit schematic and validated against SPICE simulations. Which, by the way, was REALLY REALLY HARD. 

That's probably why there are so few decent Wurlitzer style plugins out there. We learned (the hard way, after several false starts) that you cannot come close to approximating the sound of a Wurli without modeling *EVERYTHING* that makes it sound the way it does. We drew from the real 200A schematic diagram to model every resistor, every diode, even the little 4x8 ceramic cone speakers. 

As far as we can tell, this is both the most accurate and best sounding open source Wurli EP plugin in existence. Though there's still room for improvement. 

The signal chain:

1. **Modal reed oscillator** — 7-mode synthesis with per-note frequency ratios, decay rates, and inharmonicity from Euler-Bernoulli beam theory
2. **Electrostatic pickup** — capacitive 1/(1-y) nonlinearity (the primary source of Wurlitzer "bark") with RC high-pass filter
3. **Hammer model** — Gaussian dwell filter, register-dependent onset ramp, impact noise burst
4. **DK method preamp** — coupled 8-node Modified Nodal Analysis with Newton-Raphson solving, modeling the 200A's two-stage direct-coupled NPN amplifier
5. **Tremolo** — LDR feedback modulation inside the preamp loop (timbral, not just volume)
6. **Power amplifier** — Class AB with crossover distortion and rail clipping
7. **Speaker cabinet** — variable HPF/LPF with Hammerstein polynomial nonlinearity

## Features

- Physically accurate sound derived from component-level circuit analysis
- 12-voice polyphony with voice stealing and crossfade
- 2x oversampled preamp processing
- Tremolo that modulates timbre (not just amplitude), matching the real 200A topology
- Per-note MLP corrections trained on real Wurlitzer recordings (experimental)
- Per-note variation in tuning and amplitude (no two notes sound identical)
- Per-mode frequency jitter to break digital coherence
- CLAP and VST3 plugin formats

## Install

Download the latest release from the [Releases](https://github.com/hal0zer0/openwurli/releases) page, then copy the plugin to your host's search path:

| Format | Linux | macOS | Windows |
|--------|-------|-------|---------|
| CLAP | `~/.clap/` | `~/Library/Audio/Plug-Ins/CLAP/` | `%LOCALAPPDATA%\Programs\Common\CLAP\` |
| VST3 | `~/.vst3/` | `~/Library/Audio/Plug-Ins/VST3/` | `%COMMONPROGRAMFILES%\VST3\` |

## Build from Source

Requires [Rust](https://rustup.rs/) (stable toolchain).

```bash
# Clone
git clone https://github.com/hal0zer0/openwurli.git
cd openwurli

# Build and bundle the plugin (release mode)
cargo xtask bundle openwurli --release

# Output: target/bundled/openwurli.clap and target/bundled/openwurli.vst3
```

### Linux Dependencies

```bash
# Debian/Ubuntu
sudo apt install libasound2-dev

# Fedora
sudo dnf install alsa-lib-devel
```

### Run Tests

```bash
cargo test --workspace
```

## Parameters

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| Volume | 0-100% | 40% | Attenuator between preamp and power amp (audio taper) |
| Tremolo Rate | 0.1-15.0 Hz | 5.63 Hz | LFO rate (stock 200A: 5.63 Hz) |
| Tremolo Depth | 0-100% | 50% | Modulation depth (0 = off) |
| Speaker Character | 0-100% | 100% | 0% = flat bypass, 100% = authentic cabinet response |

## Documentation

Detailed technical documentation is in [`docs/`](docs/):

- [Signal Chain Architecture](docs/signal-chain-architecture.md) — complete DSP specification
- [Preamp Circuit Reference](docs/preamp-circuit.md) — component values, DC bias, harmonic analysis
- [DK Preamp Derivation](docs/dk-preamp-derivation.md) — Discretization-K method math
- [DK Preamp Testing](docs/dk-preamp-testing.md) — five-layer test pyramid strategy
- [Output Stage](docs/output-stage.md) — power amp, tremolo, speaker
- [Pickup System](docs/pickup-system.md) — electrostatic pickup physics
- [Reed and Hammer Physics](docs/reed-and-hammer-physics.md) — modal synthesis parameters
- [Calibration and Evaluation](docs/calibration-and-evaluation.md) — test methodology and targets
- [SPICE-Rust Mapping](docs/spice-rust-mapping.md) — SPICE-to-Rust translation reference
- [Schematic Source](docs/SCHEMATIC_SOURCE.md) — how to obtain the Wurlitzer 200A schematic

## License

This project is licensed under the [GNU General Public License v3.0](LICENSE).

## Acknowledgments

- **OldBassMan** (Freesound) — Wurlitzer 200A recordings used for calibration
- **BustedGear** — Wurlitzer service documentation archive
- **Robbert van der Helm** — [nih-plug](https://github.com/robbert-vdh/nih-plug) plugin framework
- The Wurlitzer 200A repair and enthusiast community
