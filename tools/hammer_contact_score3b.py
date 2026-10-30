#!/usr/bin/env python3
"""Score a pass-3b keyboard run (hammer_contact.py keyboard3b) with the rules registered in the plan (§5h).

- F1 (mode 2): unchanged from pass 3: random offset at v_ff, calibrated, robust per grade.
- F2 + F3 (voiced, at v_max): at MIDI <= 60 a note passes if ANY offset in the voiced set gives both
  crown-end swing >= the let-off gap (for the lowest n_b reeds) and tip swing <= 12 mm. Above 60, F3
  uses the random offset at v_max. n_b in {7, 10, 13}, gap in {2.4, 3.2, 4.0} mm (nuisances;
  primary 10 and 3.2).
- F4 (treble voicing, operator-approved): pickup swing at v_ff >= 0.012 mm at the top-grade sample notes
  (MIDI 84, 90, 96), the lenient end of the 20 dB bound. 6 and 10 dB are reported.
- Validity flag: treble-grade peak felt compression <= 1.5 mm.

Usage:
    python tools/hammer_contact_score3b.py KB3B.json CALIBRATION.json SURVEY.json
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
F4_NOTES = (84, 90, 96)    # top-grade sample notes for the treble voicing check (plan §5h)
F4_MIN_SWING_MM = 0.012    # 20 dB within C4 at best voicing, lenient end of 0.012-0.015 mm (operator-approved)
# Minimum top-treble pickup swing within 6 / 10 / 20 dB of C4: midpoints of the physics review's bands.
REPORT_SWING_MM = {6: 0.07, 10: 0.045, 20: 0.0135}
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
    notes = {int(m): {k: (np.array(v, dtype=float) if isinstance(v, list) else v) for k, v in d.items()}
             for m, d in kb["notes"].items()}
    inst = {k: np.array(v) for k, v in kb["instruments"].items()}

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

    def f23(n_b, gap):
        ok = np.ones(n, bool)
        for m, v in notes.items():
            if v["voiced"]:
                tip, end = v["voiced_tip_mm"], v["voiced_crown_end_mm"]
                good = tip <= SHIELD_MM
                if m - 32 <= n_b:
                    good &= end >= gap
                ok &= good.any(axis=1)
            else:
                ok &= v["tip_vmax_mm"] <= SHIELD_MM
        return ok

    f3_only = f23(0, 0.0)  # shield alone: no reed carries the let-off bound
    f4 = np.ones(n, bool)
    for m in F4_NOTES:
        if m in notes:
            f4 &= notes[m]["swing_mm"] >= F4_MIN_SWING_MM
    valid = np.ones(n, bool)
    for m, v in notes.items():
        if v["grade"] == TREBLE_GRADE:
            valid &= v["d_peak_mm"] <= TREBLE_DPEAK_MAX_MM

    prim = f23(*PRIMARY)
    print(f"pass 3b | origin {kb['origin']}, split {kb['split']}, {n} instruments")
    print(f"  alone: F1 {f1.mean()*100:.1f}% | F2+F3 voiced {PRIMARY} {prim.mean()*100:.1f}% | F3 alone "
          f"{f3_only.mean()*100:.1f}% | F4 {f4.mean()*100:.1f}% | valid {valid.mean()*100:.1f}%")
    print(f"  F1+F4+valid (no let-off): {(f1 & f4 & valid & f3_only).sum()} | F2+F3+F4+valid (no mode-2): "
          f"{(prim & f4 & valid).sum()}")
    grid = " ".join(f"{nb}/{gap}:{(f1 & f23(nb, gap) & f4 & valid).sum()}" for nb in LETOFF_NB for gap in LETOFF_GAP_MM)
    print(f"  ALL, survivors by (n_b/gap): {grid}")
    ok = f1 & prim & f4 & valid
    print(f"  PRIMARY (all, {PRIMARY[0]} / {PRIMARY[1]} mm): {ok.sum()} ({ok.mean()*100:.1f}%)")
    if ok.sum() == 0:
        return
    for gi, gname in enumerate(("bass", "middle", "treble")):
        print(f"    z*_{gname}: {q(inst['z_star'][ok, gi]*1e3)} mm (prior {q(inst['z_star'][:, gi]*1e3)}); "
              f"p {q(inst['p'][ok, gi])}")
    print(f"    k {q(inst['k'][ok])} | m_h {q(inst['m_h'][ok]*1e3)} g | v_ff {q(inst['v'][ok])} | v_max "
          f"{q(inst['v_max'][ok])} m/s | bass blank {q(inst['t_bass_in'][ok]*1e3)} mil")
    print("    per note (v_ff, random offset): d_s mm (frac L) | tip mm | pickup swing mm | contacts==1 % | contact T1")
    for m, v in sorted(notes.items()):
        L = v["L_mm"]
        print(f"      MIDI {m:3d} g{v['grade']}: {np.median(v['d_s_mm'][ok]):5.1f} ({np.median(v['d_s_mm'][ok])/L:.2f}) | "
              f"{np.median(v['tip_mm'][ok]):5.2f} | {np.median(v['swing_mm'][ok]):6.3f} | "
              f"{np.mean(v['contacts'][ok]==1)*100:3.0f} | {np.median(v['t_contact_T1'][ok]):4.2f}")
    for m in F4_NOTES:
        if m in notes:
            sw = notes[m]["swing_mm"][ok]
            print(f"    REPORT voicing MIDI {m}: within 6 dB {np.mean(sw >= REPORT_SWING_MM[6])*100:.0f}%, 10 dB "
                  f"{np.mean(sw >= REPORT_SWING_MM[10])*100:.0f}%, 20 dB {np.mean(sw >= REPORT_SWING_MM[20])*100:.0f}%")


if __name__ == "__main__":
    main()
