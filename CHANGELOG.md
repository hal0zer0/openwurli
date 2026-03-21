# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- **Melange circuit solvers are now the default.** The preamp (12-node DK with
  Sherman-Morrison pot correction) and tremolo oscillator (Twin-T circuit) are
  now melange-generated from SPICE netlists. The legacy hand-written solvers
  remain available behind `--features legacy-preamp` and `--features
  legacy-tremolo` for A/B testing.
- **Gain staging recalibrated from circuit measurements.** POST_SPEAKER_GAIN
  reduced from +13 dB to +10.5 dB after audit showed accumulated drift from
  prior modeling revisions. Preamp output (3 mV RMS) matches Brad Avenson's
  2-7 mV measurement of a real 200A. Volume default lowered from 63% to 50%.
  16-voice ff chords at full volume now peak at -1 dBFS instead of clipping.
- MLP v2 retrained against melange preamp (loss 0.129 → 0.090).
- `melange-primitives` dependency switched from local path to
  [github.com/hal0zer0/melange](https://github.com/hal0zer0/melange).

### Removed
- Deleted dead filter types (`OnePoleLpf`, `OnePoleHpf`, `TptLpf`, `DcBlocker`)
  superseded by melange-primitives — 370 lines removed.
- Deleted `bjt_stage.rs` (single BJT CE stage), superseded by DK method preamp.
- Removed `melange-preamp` and `melange-tremolo` feature flags — melange is now
  the default, not an opt-in.

### Added
- `--cargo-features` flag for `ml/render_model_notes.py` and `ml/pipeline.py`
  to support feature-gated MLP training renders.

## [0.2.4] "AndAMicrophone" - 2026-03-01

### Fixed
- **CLAUDE.md corruption**: release script's codename table insertion used a `sed`
  append with an empty line address, injecting garbage after every line in CLAUDE.md.
  Root cause: condensed CLAUDE.md no longer has a codename table, so the awk match
  returned empty, and `sed "a\..."` with no address inserts after every line.

### Changed
- Replaced `scripts/release.sh` with `/release` skill (Claude Code command). Same
  checklist (version bump, fmt, clippy, test, bundle, commit, tag, push) but executed
  by the AI agent instead of fragile bash sed. No more commit-ordering bugs.
- Untracked `docs/release-codenames.md` — personal reference, not a project file.

## [0.2.3] "TwoTurntables" - 2026-03-01

### Added
- **[How the Circuit Modeling Works](docs/how-circuit-modeling-works.md)** — non-technical
  overview of the DK method preamp solver and full signal chain
- **[How the MLP Corrections Work](docs/how-mlp-corrections-work.md)** — non-technical
  overview of the per-note neural network correction layer

### Changed
- Reorganized `docs/` into user guides, `docs/research/` (200A circuit analysis),
  and `docs/reference/` (agent/developer working docs)
- Condensed CLAUDE.md from 230 to 107 lines with PR guidelines for contributors
- Fixed 21 stale claims across 6 technical docs
- Updated ml/README.md to reflect MLP v2 architecture (11 outputs, 195 params)
- Enhanced release script: commit after validation, idempotent re-runs

## [0.2.2] "GoBackJack" - 2026-03-01

### Fixed
- **Tremolo depth click**: adjusting tremolo depth caused an audible click from
  shadow preamp bypass toggling (solver on/off discontinuity). Shadow preamp now
  runs unconditionally — cost of one extra DK step is negligible vs 64 reed
  oscillators. Pump cancellation is always exact.
- **Tremolo automation clicks**: tremolo rate and depth parameters now smoothed
  per-sample via `nih_plug::smoothed.next()`, preventing zipper noise during
  DAW automation or manual knob sweeps.
- **NaN freeze**: tremolo depth changes could trigger NaN in the DK solver that
  persisted indefinitely. NaN guard now resets solver state on divergence.

### Changed
- Codebase simplification: -195 lines, zero behavior changes. Replaced manual
  loops with `core::array::from_fn`, extracted `DkState::at_dc()` constructor,
  simplified LDR log-space interpolation, removed dead `set_shadow_bypass()` API.

## [0.2.1] "YouMakeMeLive" - 2026-02-28

### Fixed
- **Speaker character click**: moving the Speaker Character slider caused a loud
  click from biquad filter coefficient discontinuities (HPF 20→95 Hz,
  LPF 20k→5.5k Hz jumped in one buffer step). Now smoothed per-sample via
  `.smoothed.next()` with coarser update threshold (0.002) to limit
  recomputation rate.
- **Volume slider feel**: Skewed range (`skew_factor(2.0)` = factor 4.0) applied
  a fourth-root UI mapping on top of the circuit-accurate vol² audio taper,
  compressing the entire 0–3 dB range into the top half of the slider. Changed
  to Linear range — vol² alone gives standard audio pot feel (−12 dB at half
  slider). Circuit model unchanged.
- **Output too quiet**: POST_SPEAKER_GAIN raised from +10 dB to +13 dB. Applied
  after all analog circuit stages (models mic/DI level), so distortion and
  frequency response are unaffected. New levels: single ff at max vol = −3.3
  dBFS, 4-voice ff chord at max vol = −4.3 dBFS.

## [0.2.0] "GoodbyeMary" - 2026-02-23

### Changed
- Reed oscillator: quadrature rotation replaces per-sample `sin()` calls —
  7 transcendentals per sample per voice reduced to 0. Mode struct (AoS)
  replaces parallel arrays.
- Reed jitter: subsample update (every 16 samples instead of every sample).
  OU correlation time τ=20ms >> 0.36ms update interval — perceptually identical.
- Pickup: algebraic division elimination — `beta / c_n` → `beta * (1-y)`,
  `q / c_n` → `q * (1-y)`. 2 divisions per sample per voice removed.
- Conditional oversampling: 2x oversampling skipped at host rates ≥ 88.2 kHz
  (preamp BW ~15.5 kHz is below 44.1 kHz Nyquist at 96 kHz). Saves ~50%
  of DK preamp cost at high sample rates.
- Full 64-voice polyphony (matches real 200A's 64 mechanically independent
  reeds). Was 12 voices — no physical basis for that limit.
- Volume parameter smoother: Logarithmic → Linear (fixes NaN at zero volume)
- Iterator idioms: module-level `needless_range_loop` allows removed from
  tables.rs, voice.rs, variation.rs; loops converted to `std::array::from_fn`,
  `iter_mut().enumerate()`, and `zip()` chains.
- `powf(MODE_DECAY_EXPONENT)` → `x * x` in mode_decay_rates (exponent is 2.0)
- Filter precomputes: `OnePoleLpf` caches `one_minus_alpha`, `TptLpf` caches
  `g_denom = g / (1 + g)` — avoids recomputation per sample.

### Added
- `bench-reed` subcommand in preamp-bench: isolated reed microbenchmark
  (voices × duration, reports realtime ratio)
- `--sample-rate` flag for preamp-bench render: enables 96 kHz rendering
  with automatic oversampling bypass
- Plugin-level NaN output guard: `is_finite()` check after power amp + speaker,
  resets both stages on NaN to prevent permanent state corruption

### Fixed
- **PipeWire audio engine crash** when volume swept to zero and back with
  arpeggiator running. Root cause: `SmoothingStyle::Logarithmic` produces
  NaN at volume=0.0 (`log(0) = -inf`), NaN cascades through biquad filter
  state permanently, PipeWire kills non-finite audio streams. Fix: Linear
  smoother + output NaN guard as safety net.

### Performance
- Batch offline render (15 notes × 3 velocities): 4.4% wall-clock improvement
- 8-voice polyphonic stress test: 12.1% wall-clock improvement
- Reed-only microbenchmark: 451x realtime (64 voices × 1 second)

## [0.1.5] "MountUp" - 2026-02-22

### Changed
- Pickup model: time-varying RC replaces static `y/(1-y)` + separate HPF.
  Bilinear-discretized charge dynamics couple 1/(1-y) nonlinearity with
  RC high-pass filtering in a single step. Self-limiting — no hard clamp
  needed below y=0.98
- Displacement scale reshaped: DS_AT_C4=0.75, EXP=0.75, CLAMP=[0.02, 0.82].
  Bass cap reduced 0.92→0.82 to prevent extreme nonlinearity (y_peak 0.89→0.80)
- Speaker LPF lowered 7500→5500 Hz per OBM A/B comparison (real 4x8"
  ceramic speakers roll off well below 7500 Hz)
- MLP retrained against RC pickup model: loss 0.355→0.101 (3.5x improvement),
  ds correction MAE 0.13, frequency MAE 2.5 cents
- Register trim recalibrated for new gain staging
- Rename confusing `t_dwell`/`dwell_time_s` to `onset_time`/`onset_time_s`
  in voice.rs and reed.rs (onset ramp ≠ spectral dwell filter)

### Fixed
- Gain staging: target_db -19→-35 dBFS so power amp sees realistic signal
  levels (~5-10% headroom at ff, was 57%). Post-speaker gain (+10 dB)
  models mic/DI stage. Single ff note: -14.6 dBFS, 6-note ff chord:
  -9.8 dBFS — no more DAW clipping on polyphonic material
- preamp-bench render/calibrate/centroid-track used linear volume instead
  of audio taper (vol²), mismatching the plugin's signal chain
- preamp-bench calibrate default ds_at_c4 was 0.85, now matches code (0.75)

### Added
- Version codename convention: lyric fragments from Wurlitzer songs
- `POST_SPEAKER_GAIN` constant (tables.rs): +10 dB post-speaker output gain,
  applied in plugin and all preamp-bench render commands
- `tools/strip_pedal.py`: strip sustain pedal from MIDI files, extending
  note durations to compensate (for testing without pedal support)

## [0.1.4] - 2026-02-21

### Changed
- Reed oscillator: multiplicative decay (`envelope *= decay_mult`) replaces
  per-sample `exp(-α·n)` — saves 7 exp() calls per sample per voice
- Reed jitter: scaled uniform noise replaces Box-Muller transform in render
  loop — saves 3 transcendentals per mode per sample (ln + sqrt + cos).
  OU filter ensures Gaussian-distributed output via CLT regardless.
- DK preamp: fused `bjt_ic_gm()` computes one exp() for both collector
  current and transconductance (was two separate exp() calls per BJT per
  NR iteration)
- Tremolo: cache `ln(r_min)` and `ln(r_max)` at construction instead of
  recomputing per sample

### Added
- Shadow preamp bypass: skip shadow DK solver when tremolo depth < 0.001
  (R_ldr is constant → shadow output is constant DC, saves ~50% preamp cost)
- NaN guard in DK preamp: `process_sample()` checks `result.is_finite()`
  and calls `reset()` on NR divergence to prevent permanent state corruption

### Fixed
- Round-2 doc audit: fix stale BW numbers in preamp-circuit.md Section 5.6,
  remove C20 from 200A signal level calc, mark p=1.5 decay exponent as historical
- Fix stale code comments in speaker.rs (architecture, normalization symmetry)
- Align pickup.rs fallback DISPLACEMENT_SCALE with DS_AT_C4 (0.70 → 0.85)
- Fix README: correct parameter defaults, power amp description, add MLP toggle

## [0.1.3] - 2026-02-20

### Added
- Release script (`scripts/release.sh`) — mirrors CI pipeline locally
  (fmt, clippy, test, bundle, install) before tagging and pushing
- YouTube demo thumbnail in README

### Fixed
- Fix clippy warnings and fmt issues in preamp-bench (collapsible if, eprint)
- Sync all 7 docs with code (65 discrepancies found and fixed):
  - signal-chain-architecture.md: pickup model rewrite, freq variation,
    LDR gamma/formula, depth polarity, volume taper, parameter IDs,
    MLP toggle, voice death, shadow preamp, C20 annotations
  - output-stage.md: bandwidth values, GBW claim, normalization math,
    R-30 table entry, Hammerstein description, volume formula
  - dk-preamp-derivation.md: pseudocode return, beta, DC solve details
  - preamp-circuit.md: FLOPs count, tremolo frequency
  - pickup-system.md: C20 is 206A-only annotations
  - reed-and-hammer-physics.md: tip mass note, 200A thickness values
  - parameter-tuning-guide.md: stale line numbers, grid count, tap points
- Fix stale preamp-bench defaults: calibrate DS 0.70→0.85, sensitivity
  range includes 0.85, freeze mode DS 0.70→0.85
- Fix stale code comments (oversampler 100→28 dB, hammer 4x→5x,
  tremolo gamma 0.7→1.1, pickup default 0.35→0.70)

## [0.1.2] - 2026-02-20

### Added
- Power amp closed-loop negative feedback NR solver (replaces open-loop model)
  - Gaussian C∞ crossover with quiescent gain
  - tanh soft-clip rail saturation (not hard clamp)
  - 8 NR iterations/sample, models R-31/R-30 feedback loop (T=275 at DC)
- MIDI file rendering (`preamp-bench render-midi`)
- Polyphonic rendering (`preamp-bench render-poly`) with intermod analysis

### Changed
- Power amp: open-loop gain→crossover→tanh replaced with closed-loop
  feedback solver (eliminates polyphonic intermod buzz)

### Fixed
- Massive polyphonic intermodulation distortion (audible as buzzy crackle)
  caused by open-loop power amp model — feedback linearizes at normal levels

## [0.1.1] - 2026-02-19

### Added
- Plugin integration tests (11 tests, 139 total)
- MLP v2 per-note corrections (2→8→8→11, 195 params, trained on 8 OBM notes)
- Calibration sweep tooling (`preamp-bench calibrate`, `sensitivity`)
- OBM A/B comparison tooling (`wurli_compare.py`)

### Changed
- Decay rate: fixed array → frequency power law `0.005*f^1.22` (floor 3.0 dB/s)
- Pickup displacement scale DS_AT_C4: 0.70 → 0.85
- Plugin defaults: speaker 0% (bypass), MLP corrections ON
- Velocity exponent: max 2.2→1.7, min 1.4→1.3 (less cliff-like)
- vel_blend: `v^2.0` → `v^1.3` (more trim at sub-ff velocities)

### Fixed
- Power amp tanh soft-clip (was hard clamp)
- Speaker polynomial normalization `/(1+a2+a3)` (was boosting 80% → clipping)

## [0.1.0] - 2026-02-19

### Added
- Modal reed oscillator — 7-mode synthesis with per-note frequency ratios,
  decay rates, and inharmonicity from Euler-Bernoulli beam theory
- Electrostatic pickup model — capacitive 1/(1-y) nonlinearity with
  RC high-pass filter at 2312 Hz
- Hammer model — Gaussian dwell filter, register-dependent onset ramp,
  impact noise burst (5×f0, Q=0.7, 3ms decay)
- Per-mode frequency jitter via Ornstein-Uhlenbeck process (0.7 cents, τ=20ms)
- Per-note detuning (±3 cents) and amplitude variation (±8%)
- DK method preamp — coupled 8-node MNA with Newton-Raphson solving,
  modeling the 200A's two-stage direct-coupled NPN amplifier
- Shadow preamp pump cancellation (tremolo pump: -25 dBFS → -120 dBFS)
- Tremolo — LDR feedback modulation inside the preamp loop (timbral, not volume)
- Speaker cabinet — HPF (95 Hz) + LPF (5500 Hz) + Hammerstein polynomial
  nonlinearity + tanh Xmax + thermal voice coil compression
- 2× polyphase IIR half-band oversampler for preamp processing
- Volume control with audio taper (vol², default 63%)
- 64-voice polyphony with voice stealing and 5ms crossfade
- CLAP and VST3 plugin formats via nih-plug
- Standalone reed renderer CLI tool
- Preamp validation bench (gain, sweep, harmonics, tremolo-sweep, render,
  bark-audit, calibrate, sensitivity, render-poly, render-midi)
- 10 technical reference documents in docs/
- ngspice testbenches for circuit validation
- GitHub Actions CI (test, bundle, clippy, fmt) + release pipeline
  (Linux, macOS x64/arm64/universal, Windows)
- GPL-3.0 license

[Unreleased]: https://github.com/hal0zer0/openwurli/compare/v0.2.4...HEAD
[0.2.4]: https://github.com/hal0zer0/openwurli/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/hal0zer0/openwurli/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/hal0zer0/openwurli/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/hal0zer0/openwurli/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/hal0zer0/openwurli/compare/v0.1.5...v0.2.0
[0.1.5]: https://github.com/hal0zer0/openwurli/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/hal0zer0/openwurli/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/hal0zer0/openwurli/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/hal0zer0/openwurli/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/hal0zer0/openwurli/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/hal0zer0/openwurli/releases/tag/v0.1.0
