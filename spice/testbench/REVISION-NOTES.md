# Testbench migration state — 2026-09 drawn-topology revision

Branch `revision/drawn-topology`. All benches include the REVISED
`subcircuits/preamp.cir` / `tremolo_osc.cir` automatically; every supply
line is migrated 15 → 14.5 V. Per-bench status below. "RE-BASELINED" =
run on the revised topology and header targets updated; "PENDING" = runs
but expectation comments still quote pre-revision numbers — re-baseline
before trusting a failure; "SUPERSEDED" = the phenomenon it probes was an
artifact of the old topology.

| Bench | Status | Notes |
|---|---|---|
| tb_preamp_dc | RE-BASELINED | Matches the independent cross-check deck < 1 mV every node (14.5 V, BF 1434). |
| tb_tremolo_osc | RE-BASELINED | 5.70 Hz, 11.88 Vpp, LED drop 1.50 V. Lib GP card gives more swing than the melange deck's IS/BF card — harmonize in Phase 2. |
| tb_preamp_ac | RE-BASELINED | 15.54 dB @1 kHz (R_ldr 12K, loaded out); HF −3 dB 16.79 kHz (triple-confirmed: independent SPICE 16.7k, matched-drive derivation 17.1k); mild LF rise to 16.4 dB @31 Hz (stage-2 zero/pole + loop-gain shaping — real physics, not a defect). |
| tb_pump_emit | RE-BASELINED → **now a C-6 regression guard** | Pump = 0.000 mV pp with LDR cycling 19K↔1M @5.63 Hz. Any nonzero pump here means C-6 went missing again. |
| tb_pump_c8_detail, tb_pump_loaded, tb_pump_pot_effect, tb_preamp_pump_transient, tremolo_pump | SUPERSEDED | The DC pump they dissect was an artifact of the missing C-6 (output DC-coupled into the LDR leg). Kept for history; do not re-baseline. The Rust shadow-pump compensation they motivated becomes physically baseless in Phase 2. |
| verify_dc_bias | NEEDS REWRITE | Builds INLINE circuit copies of the OLD topology ("CONFIG A: R2=2MEG…") rather than including the subcircuit. Rewrite as old-vs-drawn comparison, or retire in favor of tb_preamp_dc. |
| tb_dk_ac_extract, tb_dk_validation | PENDING (Phase 2) | Validate the Rust DK solver against SPICE; re-cut when the revised solver exists. |
| tb_preamp_transfer, tb_preamp_clipping, tb_preamp_harmonic, tb_preamp_tran, tb_preamp_tran_analysis, preamp_transient, tb_preamp_ac_sweep_ldr, preamp_ldr_sweep, tb_preamp_dc_vs_rldr, tb_variable_gbw, tb_harmonic_audit, tb_real_thd, tb_rldr_transient, tb_tremolo_register, tb_full_chain | PENDING | Topology comes in via the include; rails migrated; expectation comments stale. Re-baseline opportunistically — cross-check references: gain 15.65 dB @13k shunt unloaded node_c6 (BF 1434), stage split A1·vbe≈14×/A2≈131 (BF 1434); knee tables in the maintainer's coordination record. |
| tb_pickup | PENDING (Phase 2, decision ②a) | Pickup network moves to a current-source interface; corner target ≈900 Hz (C-2 grounded), non-one-pole (−17.3 dB/dec asymptote). |
| tb_power_amp, tb_power_amp_harmonics, tb_power_supply | UNAFFECTED | Power-amp side; no preamp topology or 14.5 V rail involvement. |
