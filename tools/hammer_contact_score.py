#!/usr/bin/env python3
"""Score a hammer_contact.py keyboard pass with the rules registered in the plan (§5d and its amendments).

Inputs:
- the keyboard run JSON;
- the calibration JSON (`hammer_contact.py calibrate`);
- the survey JSON (`mode2_survey.py`).

Per note, the model's chain-identical mode-2 level re H1 is the proxy plus a calibrated offset. The
offset is linear in swing, fitted on that note's chain renders (with >= 5 resolved lines; otherwise
zero).

An instrument is falsified if any of these holds:
- any note's ff swing exceeds its section's admitted edge;
- within any felt grade, more than a quarter of the scored notes put the model's mode 2 above the
  recording's level + 10 dB.

Scored notes: every note with an own line in the survey, except the exclusions registered in §5d.

Usage:
    python tools/hammer_contact_score.py KB.json CALIBRATION.json SURVEY.json
"""

import json
import sys

import numpy as np

# Swing prior's dynamic admitted edges, mm at the finger region (tools/swing_prior.py EDGES): the pickup
# tables' reach, a NUMERICAL limit; retired as a falsifier from pass 3 on.
EDGE_MM = {"bass": 4.29, "middle": 2.90, "treble": 2.89}
# Notes with a clean own mode-2 line in the reference-set survey (plan §5c/§5d; exclusions listed there).
SCORED = (47, 48, 50, 59, 60, 65, 67, 69, 71, 72, 74, 76, 77, 79, 81, 83, 84, 86, 88)
ALLOW_DB = 10.0            # falsifier 10a's allowance above the recording (plan §5, registered)
MAX_FAIL_FRACTION = 0.25   # robust rule: at most a quarter of a grade's scored notes may fail (plan §5d)
MIN_CAL_POINTS = 5         # chain renders needed to fit a note's calibration line; otherwise no offset


def section(m):
    return "bass" if m <= 52 else ("middle" if m <= 72 else "treble")


def offsets(cal):
    fits = {}
    for m in {r["midi"] for r in cal}:
        pts = [(r["swing_mm"], r["chain_db"] - r["proxy_db"]) for r in cal if r["midi"] == m and r["status"] == "line"]
        if len(pts) >= MIN_CAL_POINTS:
            x, y = np.array(pts).T
            fits[m] = np.polyfit(x, y, 1) if np.ptp(x) > 0 else np.array([0.0, np.mean(y)])
    return fits


def q(a):
    return "/".join(f"{np.percentile(a, p):.2f}" for p in (10, 50, 90)) if len(a) else "-"


def main():
    kb = json.load(open(sys.argv[1]))
    fits = offsets(json.load(open(sys.argv[2])))
    rec = {r["midi"]: r["onset"]["re_h1_db"] for r in json.load(open(sys.argv[3]))
           if r.get("onset", {}).get("status") == "line"}
    n = kb["n"]
    notes = {int(m): v for m, v in kb["notes"].items()}
    ok = np.ones(n, bool)
    edge_fail = np.zeros(n, bool)
    for m, v in notes.items():
        e = np.array(v["swing_mm"]) > EDGE_MM[section(m)]
        edge_fail |= e
    fails_by_grade = {}
    scored_by_grade = {}
    chain = {}
    for m in SCORED:
        if m not in notes or m not in rec:
            continue
        v = notes[m]
        sw = np.array(v["swing_mm"])
        a, b = fits.get(m, (0.0, 0.0))
        chain[m] = np.array(v["m2_out_db"]) + a * sw + b
        g = v["grade"]
        fails_by_grade.setdefault(g, np.zeros(n, int))
        scored_by_grade[g] = scored_by_grade.get(g, 0) + 1
        fails_by_grade[g] += chain[m] > rec[m] + ALLOW_DB
    m2_fail = np.zeros(n, bool)
    for g, f in fails_by_grade.items():
        m2_fail |= f > MAX_FAIL_FRACTION * scored_by_grade[g]
    ok = ~edge_fail & ~m2_fail
    inst = {k: np.array(v) for k, v in kb["instruments"].items()}
    print(f"split {kb['split']}: {n} instruments | swing-edge falsified {edge_fail.mean()*100:.0f}% | mode-2 rule "
          f"falsified {m2_fail.mean()*100:.0f}% | SURVIVORS {ok.sum()} ({ok.mean()*100:.1f}%)")
    print(f"  scored notes per grade: {dict(sorted(scored_by_grade.items()))}")
    if ok.sum() == 0:
        return
    for gi, gname in enumerate(("bass", "middle", "treble")):
        print(f"  z*_{gname} (mm) p10/50/90: prior {q(inst['z_star'][:, gi]*1e3)} | survivors {q(inst['z_star'][ok, gi]*1e3)};"
              f" p survivors {q(inst['p'][ok, gi])}")
    print(f"  m_h (g): prior {q(inst['m_h']*1e3)} | survivors {q(inst['m_h'][ok]*1e3)}")
    print(f"  v_ff (m/s): prior {q(inst['v'])} | survivors {q(inst['v'][ok])}")
    print("  per note (survivors): swing p50 mm | single-contact % | contact p50 T1 | model mode2 chain p50 dB | recording dB")
    for m, v in sorted(notes.items()):
        sw = np.array(v["swing_mm"])[ok]
        c = np.array(v["contacts"])[ok]
        tc = np.array(v["t_contact_T1"])[ok]
        mc = f"{np.median(chain[m][ok]):6.1f}" if m in chain else "     -"
        rr = f"{rec[m]:6.1f}" if m in rec else "     -"
        print(f"    MIDI {m:3d} (grade {v['grade']}): {np.median(sw):5.2f} | {np.mean(c==1)*100:3.0f} | {np.median(tc):5.2f} | {mc} | {rr}")


if __name__ == "__main__":
    main()
