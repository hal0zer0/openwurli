# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Modal reed oscillator with 7 inharmonic modes and per-note frequency/amplitude tables
- Electrostatic pickup model with 1/(1-y) nonlinearity (primary bark source)
- Pickup RC high-pass filter at 2312 Hz
- Hammer dwell filter modeling finite felt-on-reed contact duration
- Register-dependent onset ramp (2-3 periods, clamped 2-60 ms)
- Attack noise burst (bandpass-filtered, exponentially decaying)
- Per-mode frequency jitter via Ornstein-Uhlenbeck process
- Per-note detuning and amplitude variation (deterministic pseudo-random)
- DK method preamp circuit solver (8-node coupled MNA, Newton-Raphson)
- Tremolo via LDR feedback modulation inside the preamp loop
- Class AB power amplifier model (VAS gain + crossover distortion + rail clipping)
- Speaker cabinet simulation (variable HPF/LPF + Hammerstein polynomial + tanh Xmax)
- 2x polyphase IIR half-band oversampler for preamp processing
- 4th-order Bessel subsonic high-pass filter (eliminates bass onset ringing)
- Volume control with audio taper (real attenuator model)
- 12-voice polyphony with voice stealing and 5 ms crossfade
- CLAP and VST3 plugin formats via nih-plug
- Standalone reed renderer CLI tool
- Preamp validation bench CLI tool (gain, sweep, harmonics, tremolo, render)
- Comprehensive technical documentation (10 reference documents)
- ngspice testbenches for circuit validation
- ML pipeline for per-note correction (experimental)
