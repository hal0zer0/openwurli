# Mr Schemey — Agent Memory

## Signal Chain Audit (Feb 2026, rev fdb835a, DK Preamp, displacement_scale=0.60)

### Status: Healthy (watch MIDI 36 H2/H3)
- Gains: +0.69 dB systematic offset (both trem states). Tremolo range: 6.10 dB (target 6.1). H2>H3: 10/10.
- **MIDI 36 H2/H3 margin: +0.2 dB** -- barely passing. Degraded from +2.4 dB (ds=0.35). Root cause: higher y pushes H3 growth faster than H2.
- **BW: 11.25 kHz measured vs 11.8 kHz SPICE (no-trem, -4.3%), 9.4 vs 9.7 kHz (trem-bright, -3.1%).**
  Root cause: bilinear discretization warp at 88.2 kHz. Predicted by formula: f_d=11,151 Hz. Fix: pre-warp C3/C4.
- BW ratio trem/notrem: 0.833 measured vs 0.823 SPICE -- confirms nested-loop topology captured correctly.
- Pickup 1/(1-y) confirmed as dominant bark source (>98% of H2). DK preamp transparent at mV levels.
- THD amplitude-independent (perfect linearity). Preamp adds <2.3 pp H2 (C2 ff only).
- **C2 ff bark: y=0.57, H2/H1=58.7%** -- very aggressive. Consider register-dependent displacement_scale.
- Power amp model still minimal (crossover + rail clip, no gain/feedback/freq response).
- No renders clip (fixed from previous 6/20).
- displacement_scale=0.60 (up from 0.35). Per-mode jitter (OU, sigma=0.0004, tau=20ms). Faster attack tau.

### DK Preamp Verification (full review: /tmp/wurli-diagnostics/review_mr_schemey.md)
- Gain accuracy: +0.69 dB systematic (no trem and trem bright identical offset)
- Tremolo sweep: smooth, monotonic, 6.10 dB range (exact match to SPICE 6.1 dB)
- Amplitude-independent THD: confirms preamp linearity at millivolt signals
- Bark audit: preamp adds < 2.3 pp H2 (only C2 ff shows meaningful preamp H2)
- Preamp accuracy unchanged from rev 267856a (all deltas identical within measurement noise)

### Key File Locations
- DSP sources: `crates/openwurli-dsp/src/`
- Plugin wiring: `crates/openwurli-plugin/src/lib.rs`
- SPICE reference: `spice/subcircuits/preamp.cir`, `power_amp.cir`, `pickup.cir`
- Docs: `docs/preamp-circuit.md`, `docs/output-stage.md`, `docs/pickup-system.md`

### Preamp Signal Flow (DK model, current)
**DSP:** voice_sum -> upsample -> [DkPreamp: Cin-R1 companion -> 8-node MNA + 2x2 NR -> DC_block] -> downsample
**Real:** voice_sum -> [preamp: Cin+R1 -> base1 -> stage1(with R-10/Ce1 fb) -> direct_couple -> stage2 -> R-9] -> volume
