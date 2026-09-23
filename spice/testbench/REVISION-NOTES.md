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
| verify_dc_bias | **RETIRED 2026-09-14** | Deleted in favour of `tb_preamp_dc`. See the tombstone below. |
| tb_dk_ac_extract, tb_dk_validation | PENDING (Phase 2) | Validate the Rust DK solver against SPICE; re-cut when the revised solver exists. |
| tb_preamp_transfer, tb_preamp_clipping, tb_preamp_harmonic, tb_preamp_tran, tb_preamp_tran_analysis, preamp_transient, tb_preamp_ac_sweep_ldr, preamp_ldr_sweep, tb_preamp_dc_vs_rldr, tb_variable_gbw, tb_real_thd, tb_rldr_transient, tb_tremolo_register | PENDING | Topology comes in via the include; rails migrated; expectation comments stale. Re-baseline opportunistically — cross-check references: gain 15.65 dB @13k shunt unloaded node_c6 (BF 1434), stage split A1·vbe≈14×/A2≈131 (BF 1434); knee tables in the maintainer's coordination record. |
| tb_full_chain | PENDING (rewired 2026-09-22) | External circuit review, 2026-09-22: pickup moved to the current-source interface (no Vhv, plate is an AC-only node; the duplicated 1MEG feed + 0.022 µF cap are gone). DC op verified: plate 0.000, base1 2.651, emit1 2.093, coll1 4.270, emit2a 3.602, emit2b 2.710, coll2 8.561, preamp_out 0.000, fb_junct 0.000 V. Runs to completion. Gain/level expectations still pre-revision; note Rldr = 1M is not the idle shunt under the revised topology. The old ddt() pickup railed the speaker at 3520 Hz (38–41 V pp, numerical plate kick) — gone with the charge-capacitor pickup (3.72 V pp). |
| tb_harmonic_audit | PENDING (rewired 2026-09-22) | Section 1's hand-built 22K/380K/220P load replaced by R-1 into its own `wurli_preamp` instance (the real input network); pickup on the new pins. Runs to completion. Section 1 now matches tb_pickup exactly (y=0.10: 71.4 mV pp, H2/H1 7.88%). Section 3's 33 mV "equivalent level" arithmetic assumed the old 380K load — marked superseded in-file, not re-derived. |
| tb_pickup | RE-BASELINED 2026-09-22 | External circuit review: `subcircuits/pickup.cir` is now `wurli_pickup plate reed_disp gnd` — displacement-modulated charge source only, V_pol = 147 V internal, no feed resistor or coupling cap (the preamp owns R-2/C-1). Bench loads it with R-1 into a real `wurli_preamp`. DC: plate 0 V, base1 2.651 V. Physical jω drive at base1: −3 dB at **836 Hz** re 10 kHz (844 Hz re in-band peak) vs the ≈880–900 Hz target (−5 to −7%; plate-node corner is 987 Hz, so the doc's 897 depends on probe point/reference — not pinned). Non-one-pole confirmed: flat-current transimpedance falls −17.7 dB/dec (1k→10k), −19.8 dB/dec (2k→20k); jω-drive slope 55–1760 Hz +15.4 dB/dec (doc: +15.6). The ddt() B-source form was replaced by an equivalent charge-capacitor form (ddt() kicked numerically in tran and is not linearized in .ac). |
| tb_power_amp, tb_power_amp_harmonics, tb_power_supply | UNAFFECTED | Power-amp side; no preamp topology or 14.5 V rail involvement. |

## Tombstone: `verify_dc_bias.cir` (retired 2026-09-14)

533 lines, four inline configurations, **all of them the superseded topology**
(R-2 from Vcc to `base1`, R-3 returning to ground). It was written to investigate
one question, stated in its own header:

> *Previous SPICE run showed Vb1=2.80V vs schematic 2.45V (350mV gap). This
> netlist tests THREE R2 configurations to investigate.*

**That question is settled, and not by any of the configurations it tested.** The
350 mV gap was never about R-2's value — it was R-3's return node. R-3 goes to the
R-7/R-8 junction, not to ground, and under the drawn topology TR-1's base has no
other DC path at all. The old arrangement is not a badly-biased circuit, it is
**not a working circuit**; see `docs/research/preamp-circuit.md` §4.3 and §11.
With R-3 corrected, the printed 2.45 V and the hardware 2.447 V both fall out
exactly, and `tb_preamp_dc` matches the independent cross-check deck to under
1 mV at every node.

**Why retired rather than rewritten as an old-vs-drawn comparison.** A comparison
deck has to instantiate the wrong circuit to compare against it, which is how this
file went stale in the first place — inline copies that no include could keep in
step. The thing such a deck would demonstrate (the old reading cannot reach the
anchors) is already pinned in two places that cannot drift: `tb_preamp_dc` against
the subcircuit, and `test_dc_operating_point` in `dk_preamp_legacy.rs`, which
asserts the R-3 drop sits in 30–90 mV and so fails immediately if base current
ever vanishes from the kernel again. Keeping a fourth copy of the superseded
topology in the tree is the exact hazard §11 exists to prevent.

Nothing unique was lost: the GroupDIY hardware anchor it printed (base 2.447 V)
lives in `preamp-circuit.md` §4.1 and in `tb_preamp_dc`.

**Use `tb_preamp_dc.cir` for DC verification.** Verified green at retirement —
base1 2.651 / emit1 2.093 / coll1 4.270 / emit2a 3.602 / emit2b 2.710 /
coll2 8.561 V, node_c6 at exactly 0 V.
