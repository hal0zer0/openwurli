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

2026-09-23 — last change to these netlists in this repo. Functional sync with the upstream
copies is checked by `crates/openwurli-dsp/tests/netlist_sync.rs` (skipped when no upstream
checkout is present); the regen manifest pins each netlist to its generated solver.
