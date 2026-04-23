# Vendored Melange Circuits

These `.cir` files are vendored from [melange-audio/circuits](https://github.com/melange-audio/circuits).

OpenWurli's DK solvers are generated from specific circuit versions. These local
copies are pinned to match the Rust code in `crates/openwurli-dsp/`. Do not update
them without regenerating the corresponding solver.

## Canonical locations

| Local file | Upstream path |
|---|---|
| `wurli-preamp.cir` | `testing/preamp/wurli-preamp.cir` |
| `wurli-tremolo.cir` | `testing/modules/wurli-tremolo.cir` |
| `wurli-power-amp.cir` | `testing/amp/wurli-power-amp.cir` |

## Last synced

2026-04-09 — identical to upstream at initial circuits repo creation (pre-first-commit).
