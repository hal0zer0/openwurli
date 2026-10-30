#!/usr/bin/env python3
"""Score a pass-3 keyboard run (hammer_contact.py keyboard3) with the rules registered in the plan (§5f).

Each falsifier is applied on its own and in combination, so their effects (and agreement) stay visible:
- F1 (mode 2): the §5d per-note rule at the own-line notes, chain-identically calibrated
  (hammer_contact_score.py's calibration), robust per grade.
- F2 (let-off, practitioner): for the lowest n_b reeds, the swing at the crown's tip-ward end on a
  loud strike must reach the let-off gap. n_b in {7, 10, 13}, gap in {2.4, 3.2, 4.0} mm (nuisances;
  the primary is 10 and 3.2).
- F3 (shield): tip swing <= 12 mm at every note.
- Validity flag: in the treble grade, peak felt compression <= half the measured felt thickness
  (3.0-3.3 mm, so <= 1.5 mm). Bass and middle are uncapped.

Report only (pending operator decisions): the top-treble swing at the pickup against the voicing-bound
thresholds (6 / 10 / 20 dB within C4).

Usage:
    python tools/hammer_contact_score3.py KB3.json CALIBRATION.json SURVEY.json
"""

import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hammer_contact_score import ALLOW_DB, MAX_FAIL_FRACTION, SCORED, offsets, q

SHIELD_MM = 12.0           # grounded RF shield ~1/2" above the comb: tip swing limit (plan §5f, F3)
TREBLE_DPEAK_MAX_MM = 1.5  # validity: half of the 3.0-3.3 mm crown read off a photo (superseded in pass 3c)
TREBLE_GRADE = 2
# Minimum top-treble pickup swing within 6 / 10 / 20 dB of C4 at the best voicing: midpoints of the
# physics review's bands (0.06-0.08 / 0.04-0.05 / 0.012-0.015 mm).
VOICING_MIN_SWING_MM = {6: 0.07, 10: 0.045, 20: 0.0135}
# Let-off nuisance grid (plan §5f): the lowest n_b reeds must reach the gap; manual §12 sets the gap at
# 1/8" +- 1/32" (2.4-4.0 mm); the reed count ("the last 10 bass notes or so") is carried as 7-13.
LETOFF_NB = (7, 10, 13)
LETOFF_GAP_MM = (2.4, 3.2, 4.0)
PRIMARY = (10, 3.2)        # the registered primary point of the grid



def main():
    kb = json.load(open(sys.argv[1]))
    fits = offsets(json.load(open(sys.argv[2])))
    rec = {r["midi"]: r["onset"]["re_h1_db"] for r in json.load(open(sys.argv[3]))
           if r.get("onset", {}).get("status") == "line"}
    n = kb["n"]
    notes = {int(m): {k: (np.array(v) if isinstance(v, list) else v) for k, v in d.items()}
             for m, d in kb["notes"].items()}
    inst = {k: np.array(v) for k, v in kb["instruments"].items()}

    # F1
    fails, cnt = {}, {}
    for m in SCORED:
        if m not in notes or m not in rec:
            continue
        v = notes[m]
        a, b = fits.get(m, (0.0, 0.0))
        chain = v["m2_out_db"] + a * v["swing_mm"] + b
        g = v["grade"]
        fails.setdefault(g, np.zeros(n, int))
        cnt[g] = cnt.get(g, 0) + 1
        fails[g] += chain > rec[m] + ALLOW_DB
    f1 = np.ones(n, bool)
    for g, f in fails.items():
        f1 &= f <= MAX_FAIL_FRACTION * cnt[g]

    # F2 over the nuisance grid
    def f2(n_b, gap):
        ok = np.ones(n, bool)
        for m, v in notes.items():
            if m - 32 <= n_b:
                ok &= v["crown_end_mm"] >= gap
        return ok

    # F3 and the validity flag
    f3 = np.ones(n, bool)
    valid = np.ones(n, bool)
    for m, v in notes.items():
        f3 &= v["tip_mm"] <= SHIELD_MM
        if v["grade"] == TREBLE_GRADE:
            valid &= v["d_peak_mm"] <= TREBLE_DPEAK_MAX_MM

    f2p = f2(*PRIMARY)
    print(f"origin {kb['origin']}, split {kb['split']}, {n} instruments")
    print(f"  pass alone: F1 {f1.mean()*100:.1f}% | F2({PRIMARY[0]}, {PRIMARY[1]} mm) {f2p.mean()*100:.1f}% | F3 {f3.mean()*100:.1f}% | "
          f"valid {valid.mean()*100:.1f}%")
    grid = " ".join(f"{nb}/{gap}:{(f1 & f2(nb, gap) & f3 & valid).sum()}" for nb in LETOFF_NB for gap in LETOFF_GAP_MM)
    print(f"  all four, survivors by (n_b/gap): {grid}")
    print(f"  F1 + F3 + valid (no let-off term): {(f1 & f3 & valid).sum()} | F2 + F3 + valid (no mode-2 term): "
          f"{(f2p & f3 & valid).sum()}")
    ok = f1 & f2p & f3 & valid
    print(f"  PRIMARY (all four at {PRIMARY[0]} / {PRIMARY[1]} mm): {ok.sum()} ({ok.mean()*100:.1f}%)")
    if ok.sum() == 0:
        return
    for gi, gname in enumerate(("bass", "middle", "treble")):
        print(f"    z*_{gname}: {q(inst['z_star'][ok, gi]*1e3)} mm (prior {q(inst['z_star'][:, gi]*1e3)}); "
              f"p {q(inst['p'][ok, gi])}")
    print(f"    k: {q(inst['k'][ok])} (prior 0-2) | m_h {q(inst['m_h'][ok]*1e3)} g | v_ff {q(inst['v'][ok])} m/s")
    print("    per note: d_s mm (frac L) | tip mm | pickup swing mm | crown-end mm | contacts==1 % | contact T1 | d_peak mm")
    for m, v in sorted(notes.items()):
        L = v["L_mm"]
        print(f"      MIDI {m:3d} g{v['grade']}: {np.median(v['d_s_mm'][ok]):5.1f} ({np.median(v['d_s_mm'][ok])/L:.2f}) | "
              f"{np.median(v['tip_mm'][ok]):5.2f} | {np.median(v['swing_mm'][ok]):5.3f} | {np.median(v['crown_end_mm'][ok]):5.2f} | "
              f"{np.mean(v['contacts'][ok]==1)*100:3.0f} | {np.median(v['t_contact_T1'][ok]):4.2f} | {np.median(v['d_peak_mm'][ok]):4.2f}")
    top = max(notes)
    sw = notes[top]["swing_mm"][ok]
    print(f"    REPORT (voicing, MIDI {top} pickup swing p10/50/90 {q(sw)} mm): within 6 dB of C4 "
          f"{np.mean(sw >= VOICING_MIN_SWING_MM[6])*100:.0f}%, 10 dB {np.mean(sw >= VOICING_MIN_SWING_MM[10])*100:.0f}%, "
          f"20 dB {np.mean(sw >= VOICING_MIN_SWING_MM[20])*100:.0f}%")


if __name__ == "__main__":
    main()
