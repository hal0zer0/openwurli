# clap-validator: known failures, and why we do not fix them

**Triaged 2026-09-14.** `clap-validator` reports **4 failures** against our CLAP bundle.
All four were traced to the plugin framework's CLAP wrapper, **not to OpenWurli code**.
This note exists so the next person to run the validator does not spend a day re-deriving
that.

```
44 tests run, 32 passed, 4 failed, 0 warnings, 8 skipped
```

## The decisive test: they reproduce on a stock framework example

The framework ships a minimal `gain` example — one gain parameter, one nested parameter, no
OpenWurli code of any kind. Built from the pinned framework revision and validated:

| test | OpenWurli | stock `gain` example |
|---|---|---|
| `state-invalid-random` | **CRASHED** (SIGABRT) | **CRASHED** (SIGABRT) |
| `state-reproducibility-basic` | FAILED | FAILED |
| `state-reproducibility-binary` | FAILED | FAILED |
| `state-reproducibility-buffered` | FAILED | FAILED |

Identical failures, identical signatures. **Nothing here is ours.** Our parameter set is
plain — six parameters, unique string ids, no `#[persist]` fields, no custom state
serialisation, `params()` returns a clone of the one `Arc` — and none of the suspected
OpenWurli-side mechanisms (per-voice seeding, warm-up state, smoother state, the tremolo
oscillator's free-running phase) is implicated by the evidence below.

## Failure 1 — `state-invalid-random` (CRASH). Category (b): real, upstream, precisely located

> *Loads 3x1MB chunks of random bytes via `clap_plugin_state::load()` and asserts that the
> plugin doesn't crash.*

Aborts with `memory allocation of 1025222176999353387 bytes failed` (0.89 EiB). Backtrace
lands in the framework's `Wrapper::<OpenWurli>::ext_state_load`, and the mechanism is a
**length prefix taken from the stream and used unbounded**:

```rust
let length = u64::from_le_bytes(length_bytes);
let mut read_buffer: Vec<u8> = Vec::with_capacity(length as usize);
```

Random bytes give a random `u64`; `with_capacity` asks the allocator for it; the allocator
refuses; the process aborts. The reported 1025222176999353387 is literally the first eight
random bytes read as a little-endian `u64`.

**Why it matters beyond the validator:** this is a host-process abort triggered by a
malformed state chunk — a truncated or corrupted project file or preset, not just a fuzzer.

**Why we are not fixing it:** the code is in the framework's CLAP wrapper, which we consume
as a pinned git dependency and do not modify. The entry point is the wrapper's, so there is
no OpenWurli-side seam to guard — our code never sees the stream. The upstream fix is small
(bound `length` against a sane maximum before allocating, or read incrementally), and this
note is the report-ready description of it.

## Failures 2-4 — `state-reproducibility-{basic,binary,buffered}`. Category (b), upstream

> *Randomizes a plugin's parameters, saves its state, recreates the plugin instance, reloads
> the state, and then checks whether the parameter values are the same [...] The parameter
> values are updated using the process function.*

After reload, every parameter reads its **default** instead of the randomised value:

```
 - Speaker Character - 0 %   (0.0000) vs 18 %   (0.1821)
 - Volume            - 50 %  (0.5000) vs 55 %   (0.5452)
 - Noise Level       - 1.0x  (0.0333) vs 15.4x  (0.5135)
 - Tremolo Depth     - 50 %  (0.5000) vs 40 %   (0.4012)
 - Authentic Noise   - Off   (0.0000) vs On     (1.0000)
```

The stock `gain` example produces the same shape — `Gain 0.00 dB (0.5000) vs -30.00 dB
(0.0556)`, `Unused Nested Parameter 2.00 (0.0000) vs 2.26 (0.1821)` — so the randomised
values are not surviving into the saved state in the framework's wrapper either.

(Our `mlp` parameter is absent from the list only because a random boolean matched its
default by chance; all six are equally affected.)

**Not classified further on purpose.** Deciding whether this is an upstream defect or a
deliberate divergence from what the validator expects requires reading the framework's
intent, not just its behaviour, and that is upstream's call. What is settled is that it is
not ours. Reproduce in one line:

```
clap-validator validate -t "state" <path to any stock framework example>.clap
```

## Relationship to the CLAP-silence-after-sample-rate-change issue

**Flagged, not chased.** Different code path: that investigation concerns the activate /
`start_processing` lifecycle, whereas everything above is `ext_state_load` and parameter
state. No shared mechanism is implicated by this triage.

The one connection worth remembering is remedial rather than causal: both live inside the
same framework CLAP wrapper, so **a framework pin bump is a candidate remedy for both** and
should be evaluated against both at once rather than separately.

## What passes

The 32 passing tests include every audio-processing, parameter-fuzzing, note-handling and
transport test — `param-fuzz-modulation`, `param-fuzz-sample-accurate`,
`process-random-block-sizes`, `transport-null`, `transport-fuzz`,
`transport-fuzz-sample-accurate`, `state-invalid-empty` and the rest. `transport-null` is
listed as failing in some older notes; it **passes** as of this triage.

## Re-running

```
cargo xtask bundle openwurli --release
clap-validator validate target/bundled/openwurli.clap
```
