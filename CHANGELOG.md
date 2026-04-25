# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Removed
- **DI limiter — removed entirely.** The soft `tanh`-knee limiter at −1 dBFS
  ceiling / −6 dBFS threshold was a DAW-domain bandage (mic preamp / ADC
  ceiling model), not part of the 200A circuit, and `tanh` between the
  threshold and the ceiling was adding a continuous floor of odd-harmonic
  distortion on chord-ff content where the limiter engaged. Per openwurli's
  scope (raw physics and circuit only), output peak management belongs
  downstream — Vurli will run its own compressor for dynamics; standalone
  hosts can use the DAW's headroom or limiter. `WurliEngine::set_di_limiter`,
  the `DI_LIMITER_*` constants, the `di_soft_limit()` helper, the
  `di_limiter_enabled` field, and the four `di_soft_limit_*` unit tests
  are gone. The plugin shell drops the `di_limiter` `BoolParam` and the
  `test_di_limiter_default_is_on` shell test. Saved sessions with
  `di_limiter` set will lose that setting on reload — was a no-op anyway.
  **Consequence:** chord-ff peaks at default gain staging may now exceed
  0 dBFS (~+4 dBFS raw on heavy chords). Single notes and moderate playing
  stay safely under. Hosts handle the ceiling.

### Added
- **Power-amp rail sag — behavioral dynamics for the unregulated ±22 V supply,
  default ON.** New `RailDynamics` in `power_amp.rs` tracks the two rail
  magnitudes per sample with an asymmetric one-pole RC (8 ms attack / 15 ms
  release) and a load-line model calibrated to the documented service-manual
  numbers: idle ±24.5 V, sagging toward ±22 V at rated 20 W into 8 Ω. Drives
  the melange power-amp solver via two new `.runtime V` directives on V1/V2;
  `gen_power_amp.rs` regenerated from the updated `wurli-power-amp.cir` (no
  matrix changes, just two new `f64` fields + RHS additions; MAX_ITER auto-
  tuned 200 → 70). One-pole alphas precomputed at sample-rate change so
  audio-rate stepping is 4 muls + 4 adds + 4 compares per sample, no
  transcendentals. Public API on `WurliEngine`: `set_rail_sag(bool)`,
  `rail_sag_enabled()`. New SPICE artifacts pin the calibration:
  `spice/subcircuits/power_supply.cir` (full-wave CT bridge, 2× 2200 µF caps,
  1.5 A fuses, RSEC=0.5 Ω back-solved from the load line) and
  `spice/testbench/tb_power_supply.cir` (light-load 24.39 V / rated-load
  21.997 V, both within 0.5 V of spec). Topology and component values
  extracted from the schematic by Schemer; see `docs/research/output-stage.md`
  §4.3.1 and `~/dev/schemer/research/openwurli-power-supply-response.md`.
  No real 200A hardware was used — both endpoints of the load line come from
  the service manual; RSEC is the documented one-line tuning knob if a
  hardware measurement ever surfaces.

  **Honest framing of the audible effect.** Correct physics, small audible
  delta. Compared to the pre-rail-sag adapter (now toggleable via
  `set_rail_sag(false)`), rail sag narrows the chord-vs-single dynamic-range
  ratio at vol=1.0 from 7.87 dB to 7.40 dB (~0.5 dB compression) and gives
  single notes +0.32 dB more peak headroom (rails sit at ±24.5 V at idle).
  Effect is bigger at moderate-to-loud volumes where the amp gets closer to
  its rails; near-zero at low volume where rails barely engage. Compression-
  for-loudness on chord-ff peaks is largely a downstream concern (e.g., a
  Vurli-side compressor), not solvable inside the openwurli physics model.
  CPU cost measured at +0.66 % over a 30 s polyphonic chord render —
  negligible — so default-ON is the correct-physics choice. Toggle off via
  `WurliEngine::set_rail_sag(false)` for A/B against ideal rails. Seven new
  unit tests (`test_rail_sag_*`, `test_rail_dynamics_*`) guard the
  calibration anchor and the on↔off toggle semantics.

### Changed
- **`WurliEngine` extraction — synth engine now lives in `openwurli-dsp`
  instead of locked inside `openwurli-plugin`.** New module
  `openwurli_dsp::engine::WurliEngine` owns voice management (slots,
  allocation, stealing with 5 ms crossfade, sustain pedal), the shared
  signal chain (preamp → vol² → power amp → speaker → POST_SPEAKER_GAIN
  → DI limiter), parameter smoothing for the audio-rate user controls
  (vol, tremolo depth, speaker character — internal LinearSmoother over
  ~5 ms), and the NaN guard. Framework-agnostic by design: the engine
  has no nih-plug dependency, so any host (oomox/Vurli, custom DAW
  integrations, headless CLI tools) can wrap it without copying glue
  code. The OpenWurli plugin shell is now what it should be — parameter
  declarations, MIDI event splitting, stereo channel fan-out — and
  collapses from ~1,900 to ~250 lines. Behaviorally identical to the
  pre-extraction plugin: same defaults, same dynamics, same DI-limiter
  ceiling. All 23 high-value voice/sustain/NaN/divergence-guard tests
  ported from the plugin's inline test module to `engine::tests`
  against the new public API; the plugin retains 5 shell-level tests
  (instantiate, default-param values, di_limiter / noise_enable /
  noise_gain defaults). Background motivation: oomox is building Vurli,
  an oomox-native Wurlitzer plugin that consumes OpenWurli as a library
  rather than forking its DSP — see `docs/vurli-plan.md` in the oomox
  repo for the full division-of-labor sketch.

### Added
- **Phase 5: Authentic preamp noise (new `noise_enable` BoolParam,
  default OFF; new `noise_gain` FloatParam, range 0–30×, default 1.0×).**
  Wires melange's Johnson-Nyquist thermal noise stamping into the 12-node
  DK preamp solver. Each of the 11 fixed resistors contributes the
  `sqrt(4·k_B·T·R·BW)` voltage density predicted by ngspice `.NOISE`
  on the same netlist; the noise is shaped by the full two-stage
  feedback transfer function and modulated by the LDR loop gain
  (tremolo-bright = louder noise). Default OFF means existing users
  hear no change; flip ON for the character of a real 200A preamp at
  idle. `noise_gain` is a multiplier on top of the physics-correct
  level: `1.0×` = ngspice-validated (~8 µV at preamp out, ~−86 dBFS at
  DAW with default volume — mostly inaudible, like a clean DI of a
  real 200A); raise toward `30×` for an audible "vintage hiss" floor
  without changing the spectral shape. Required upstream melange fix
  (`a5dff8c`): two-draw thermal stamp `i_n = w_new + w_prev` to zero
  the Nyquist pole that single-draw injection was exciting at every
  resistor-only node, which would otherwise produce a ~200× hot,
  fs-scaling artifact instead of the band-limited audio-rate noise
  the kTC test guarantees on simpler topologies. Verified post-fix:
  raw preamp output at 88.2 kHz matches ngspice's 8.08 µV within 2 %,
  and the fs-scaling now follows the gentle √fs curve expected from
  bandwidth growth instead of linear-fs growth. New ignored
  measurement tests: `phase5_raw_noise` (preamp standalone),
  `phase5_chain_noise_floor` (full plugin chain at multiple gains).
- **DI Limiter (new `di_limiter` BoolParam, default ON).** Soft output
  ceiling at −1 dBFS with a threshold at −6 dBFS and a tanh soft-knee;
  signals below the threshold pass through bit-exact so single-note and
  mf playing is never touched. Catches the peak level ff polyphonic
  chords naturally produce (~+4 dBFS without the limiter) so the DAW
  doesn't register clipping. Models the ceiling any mic preamp / DI box
  / A-D converter imposes on a recorded 200A — not part of the analog
  circuit chain (the preamp, power amp, and speaker models all run
  identically regardless of this switch). Users who want raw un-limited
  output for post-processing can turn it off. Implemented as a stateless
  per-sample branch in the plugin's final output path; passthrough below
  threshold is a compare + return (no transcendental math), so no audible
  character on content below peak-chord dynamics.

### Fixed
- **Melange power-amp solver divergence causing +20 dBFS spikes during
  continuous polyphonic play (DAW peak-protect muting).** Root cause:
  under continuous chord transitions, the melange 7-BJT Class AB NR
  solver intermittently failed to converge, the Backward Euler fallback
  also silently diverged, and internal node state ran away to non-physical
  values (up to 1e272 V observed). The clamp-saturated rail-slam that
  produced at the output excited the speaker's HPF/LPF resonance, which
  POST_SPEAKER_GAIN amplified to +20–22 dBFS spikes — enough to trip DAW
  peak-protect muting. Fix: new divergence guard in `PowerAmp::process`
  detects solver failure by three signals (non-finite raw output, NR at
  MAX_ITER, or any `v_prev` node above 100 V) and resets the solver state
  while holding the last confirmed-good output. A 23 µs hold burst stays
  inaudible vs. a zero-silence gap that would click on longer divergence
  bursts. New regression test
  `test_no_catastrophic_output_spikes_under_continuous_play` renders 15 s
  of chord transitions (reliably trips the pre-fix divergence) and
  asserts no output sample exceeds +4 dBFS. Pre-fix peak: +22 dBFS;
  post-fix: +2.5 dBFS. The underlying melange solver robustness issue is
  an upstream bug, to be filed once a minimal reproducer is extracted.

### Changed
- **Pickup displacement scale retuned "hotter" for more bark.** `DS_AT_C4`
  0.75 → 0.85 and `DS_CLAMP` upper bound 0.82 → 0.88 after Phase 2 +
  Phase 4 + the Apr tremolo depth-curve fix nudged the effective working
  point of the pickup → preamp → output chain cooler. Measured via
  `preamp-bench bark-audit`: pickup H2/H1 now +0.9 dB at C2 ff
  (97.1 → 107.7 %), +1.8 dB at C4 ff (63.9 → 78.6 %), +1.5 dB at C5,
  modest bumps at C3/C6. Max y_peak 0.853 at C2 ff, still comfortably
  below the `PICKUP_MAX_Y = 0.98` pole. RMS level shift ~+2 dB across
  the register; peak at ff stays around −17 to −22 dBFS. DS is
  explicitly a tuning constant — the 200A's real rest gap d₀ has never
  been published — so this is a recalibration within the documented
  tuning range, not a physics change. Investigated and rejected a
  Miessner-style k-exponent on C(y) = C₀/(1-y)^k because the 200A's
  U-channel side walls contribute a symmetric capacitance that partially
  cancels any production-design asymmetry (per the docs), and raising k
  couples H1 and H2 boosts inseparably — net gain but not H2-isolated.

### Added
- `preamp-bench calibrate --ds-clamp-max <V>` so future DS sweeps can
  probe bass-clamp headroom without code edits. Default 0.82 (the
  historical in-tree constant before the Apr retune).

### Fixed
- **Tremolo depth → swing curve non-monotonic at top end.** Before this fix,
  depth=1.0 produced *less* modulation (11.25 dB RMS swing) than depth=0.75
  (12.04 dB) because `set_depth` was mixing the 50 kΩ VIBRATO pot into both
  the LED drive path (via `led_drive = osc * depth`) AND the feedback shunt
  path (via `r_series = 18_000 + 50_000 × (1 − depth)`). The double-count
  made depth=1.0's small r_series (18 kΩ) keep the dim phase partially lit,
  narrowing the swing at high settings. Fix: removed the depth-dependent
  r_series field; shunt is now a constant 680 Ω (LDR pin 5 series resistor)
  + `r_ldr`, and `R_LDR_MIN` raised 50 Ω → 18 320 Ω so the bright phase
  lands at the documented 19 kΩ bright calibration point instead of diving
  below the preamp's `.runtime R 1k 1Meg` clamp floor. Post-fix RMS swing
  curve is monotonic: 9.49 / 9.50 / 9.79 / 12.10 dB at depth
  0.25 / 0.50 / 0.75 / 1.00. New regression test `test_depth_swing_monotonic`
  guards the log-swing curve against the double-count bug.

### Changed
- **Power amp is now a melange-generated 7-BJT Class AB circuit solver.** The
  behavioral closed-loop NR approximation is preserved behind
  `--features openwurli-dsp/legacy-power-amp` for A/B diagnostics only. Every
  transistor in the power amp stage (2N5087 diff pair, MPSA06 VAS + top Sziklai
  driver + Vbe multiplier, MPSA56 bottom Sziklai driver, TIP35C/TIP36C outputs)
  runs full Gummel-Poon at runtime: N=20 nodes, M=16 nonlinear dimensions,
  Nodal Schur + Backward Euler auto-selected. Rail clipping, crossover
  suppression, and level-dependent distortion all emerge from the circuit
  simulation. Closed-loop gain stays at `1 + R31/R30 = 69×` (37 dB) within 1 dB
  of ngspice on the same netlist.
- **Melange pin `1f46b80` → `47b2702`.** Picks up upstream fixes for Nodal
  auto-route on Class AB push-pull topologies (b99f5f3), the new
  `--output-clamp <V>` CLI flag so power amps with rails above ±10 V don't
  hit melange's default "Signal Level Contract" ceiling (b7f1ba4), and the
  plugin-template smoother-priming fix that mirrors our own patch (47b2702).
  Preamp and tremolo regenerated bit-identically — all circuit-level
  measurements (sweep, gain, harmonics, bark-audit, tremolo-sweep) diff-empty.
- **Grapevine A/B vs the behavioral approximation** (210 s, full plugin chain,
  vol=0.50): sample-rate correlation 0.954, peak identical (both saturating at
  0 dBFS downstream), RMS −0.39 dB. Physics-correct drift from the closed-loop
  NR approximation, accepted as-is.

### Fixed
- **Tremolo stuck silent in DAWs that skip the framework's activate-time
  smoother priming.** `self.params.X.smoothed.next()` per-sample calls were
  returning 0.0 forever while `.value()` correctly reported the default, so
  `tremolo.set_depth(0.0)` was being called on every sample regardless of the
  visible UI value. Added a defensive `smoothed.reset(.value())` inside
  `Plugin::initialize()` via `OpenWurli::reset_param_smoothers_to_current`.
  Idempotent against the normal framework path. New regression test
  `test_tremolo_smoother_does_not_pin_depth_to_zero`.
- **`preamp-bench render --ldr`: reset clobbered the LDR setting.** Same bug
  class as the `measure_gain_at` fix in 7f33173 but missed for `render`.
  `preamp.set_ldr_resistance(r_ldr)` was called before `preamp.reset()`, so
  reset restored the cached nominal 100 kΩ state and silently wiped the
  request. Surfaced while reproducing Dr Dawgg's "tremolo timbral modulation"
  P0 — the test was running at 100 kΩ for every LDR setting.

### Closed without code changes
- **Tremolo timbral modulation P0 (Feb 2026) — correct physics.** Remeasured
  on the full melange stack at v=100 vol=0.60 and v=127 vol=1.0 across
  LDR = 19K / 100K / 500K / 1M. Gain modulates the expected ~6.1 dB, but
  H2/H1, H3/H1, etc. stay flat within 0.03 dB across the whole LDR range.
  Correct — the pickup's 1/(1−y) bark sits *before* the preamp, so LDR's
  gain scaling multiplies H1 and H2 equally and preserves the ratio. The OBM
  reference Dr Dawgg was comparing against is a room-coupled speaker
  recording, not a DI, so any level-dependent harmonic reshape on that side
  is plausibly speaker + room + mic coloration rather than instrument
  behavior. No action.

## [0.4.0] "ThisBombsForLovin" - 2026-03-31

### Changed
- **Velocity dynamics restored to full 30 dB range.** Removed VEL_COMP_BLEND
  (velocity loudness compensation) — it was compressing dynamics to match other
  plugins' loudness rather than modeling real 200A physics. The "real neoprene:
  ~10 dB" code comment had no source and contradicted the project's own
  documented targets (15-30 dB dynamic range). POST_SPEAKER_GAIN raised from
  +10.5 dB to +19.5 dB to achieve industry-standard output levels (-10 to
  -14 dBFS for single ff notes at vol=0.50) without touching any circuit model.
- **Velocity exponent min_exp reverted from 0.7 to 1.3.** The aggressive
  compression at keyboard extremes reduced attack/decay timbral contrast by
  over-driving the pickup at sub-ff velocities, making sustain uniformly thick.
- **Bass onset ramp ceiling removed.** The 30 ms upper clamp on onset ramp time
  killed all velocity dependence below ~130 Hz — C2 ff and pp had identical
  attack timing (both 30 ms). Now the physics formula runs unclamped: C2 ff
  = 38 ms, C2 pp = 77 ms. Bass attack feels alive again.
- **MLP level compensation.** Added sqrt-proxy compensation in voice.rs so
  MLP ds_correction adjusts timbre without shifting output level. Register
  spread reduced from 10.7 dB to 3.2 dB.

### Added
- `--track N` flag for `preamp-bench render-midi` to select individual MIDI
  tracks (0-based).

### Fixed
- Documentation updated to reflect current gain staging values (PSG +19.5 dB,
  Tier 3 flags `--volume 0.50 --speaker 0.0`, register-dependent velocity
  exponent).

## [0.3.1] "GoodbyeJane" - 2026-03-29

### Added
- **Sustain pedal support (CC64).** Lifts the damper rail — reeds ring freely
  while the pedal is held, matching the real 200A's mechanical sustain lever.
  Re-striking a sustained note releases the old voice first (one reed per
  pitch). Voice stealing priority: Free > Releasing > Sustained > Held.
- MIDI input upgraded from `MidiConfig::Basic` to `MidiConfig::MidiCCs` to
  receive CC events from the host.

### Fixed
- **note_off clamping mismatch**: `note_off()` did not clamp MIDI note numbers
  to the valid range, while `note_on()` did. A note-on for out-of-range note 0
  (stored as clamped 33) could never be released by note-off 0. Both paths now
  clamp identically.

## [0.3.0] "MercyMercyMercy" - 2026-03-21

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

### Fixed
- **Speaker tanh in bypass**: excursion limiter ran even at character=0,
  compressing polyphonic ff chords by 8-17% and generating odd harmonics.
  Now skipped when character < 0.001.

### Removed
- **Tremolo Rate parameter removed.** The real 200A has no rate knob — the
  Twin-T oscillator frequency (~5.6 Hz) is fixed by passive components. The
  rate slider in earlier versions was an artifact of the synthetic sine LFO.
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

[Unreleased]: https://github.com/hal0zer0/openwurli/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/hal0zer0/openwurli/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/hal0zer0/openwurli/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/hal0zer0/openwurli/compare/v0.2.4...v0.3.0
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
