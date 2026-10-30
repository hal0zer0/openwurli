#!/usr/bin/env python3
"""Treble pickup swing against hammer speed from an SMC posterior (plan §5l, step 3 groundwork; report only).

Particles are resampled by weight from one or more weighed generations. Each strike mirrors keyboard3b: the note's
felt grade (z*, p, m_h of the treble grade for these notes), the fillet-floor strike law with the per-hammer scatter,
and one FE variant per particle (the measured-ratio variant where the survey has one). Speeds are fractions of the
stage's v_ff. Reported per note and speed: the swing re the same particle's ff swing (p10 / 50 / 90 dB), the share
falling more than NULL_DB below ff, the share whose swing does not fall monotonically with speed, and the contact
time in T1.

Usage:
    python tools/hammer_velocity_report.py SURVEY.json PARTICLES.json [PARTICLES.json ...] --stage 5 --draws 300
"""

import argparse
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hammer_contact import (ANCHORS, CROWN_MM, MM_PER_M, STRIKE_FLOOR_BY_ANCHOR, STRIKE_FLOOR_MM,  # noqa: E402
                            STRIKE_SCATTER_MM, grade_of, reed_data, strike_w_chunked, variants)
import hammer_contact_smc as H  # noqa: E402

NOTES = (84, 90, 96)                 # the F4 notes: C6, F#6, C7 (plan §5h)
SPEED_FRACTIONS = (1.0, 0.5, 0.25, 0.125)  # report grid: ff, v/2, v/4, v/8
NULL_DB = 40.0                       # report: a strike more than this far below its ff swing counts as a null
ORIGIN = "fillet-floor"              # the stage-5 strike geometry (plan §5j)
SPLIT = "damper-felt"                # grade lookup; the F4 notes are top grade in every split


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("survey")
    ap.add_argument("particles", nargs="+")
    ap.add_argument("--stage", type=int, default=5)
    ap.add_argument("--draws", type=int, required=True, help="particles resampled by weight, per file")
    ap.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    lay = H.LAYOUTS[a.stage]
    rng = np.random.default_rng(a.seed)
    measured = {r["midi"]: r["onset"]["ratio"] for r in json.load(open(a.survey))
                if r.get("onset", {}).get("status") == "line"}
    d_ref, L_ref = ANCHORS[ORIGIN]
    floor_mm = STRIKE_FLOOR_BY_ANCHOR.get(ORIGIN, STRIKE_FLOOR_MM)
    for path in a.particles:
        p = json.load(open(path))
        x, W = np.array(p["x"]), np.array(p["W"])
        inst = lay.to_inst(x[rng.choice(len(W), size=a.draws, p=W / W.sum())])
        print(f"== {os.path.basename(path)}: {a.draws} draws by weight (ESS {H.ess(W):.0f})")
        for midi in NOTES:
            g = grade_of(midi, SPLIT)
            vs = variants(midi)
            rds = [reed_data(midi, t_tip, t_root) for _, t_tip, t_root in vs]
            if midi in measured:
                pick = np.full(a.draws, int(np.argmin([abs(r["om"][1] / r["om"][0] - measured[midi]) for r in rds])))
            else:
                pick = rng.integers(0, len(vs), a.draws)
            scatter = rng.uniform(-STRIKE_SCATTER_MM, STRIKE_SCATTER_MM, a.draws)
            mh = inst["m_h"][:, g] if inst["m_h"].ndim == 2 else inst["m_h"]
            swing = np.zeros((a.draws, len(SPEED_FRACTIONS)))
            tc = np.zeros_like(swing)
            for kv, rd in enumerate(rds):
                sel = np.where(pick == kv)[0]
                if sel.size == 0:
                    continue
                L_mm = rd["L"] * MM_PER_M
                d_s = np.clip(d_ref * (L_mm / L_ref) ** inst["k"][sel] + scatter[sel], floor_mm, L_mm - CROWN_MM / 2)
                for j, frac in enumerate(SPEED_FRACTIONS):
                    r = strike_w_chunked(rd, mh[sel], inst["v"][sel] * frac, inst["z_star"][sel, g],
                                         inst["p"][sel, g], d_s)
                    swing[sel, j] = np.abs(r["pick_mm"][:, 0])
                    tc[sel, j] = r["t_contact_T1"]
            rel = 20 * np.log10(swing / swing[:, :1])
            mono = np.all(np.diff(swing, axis=1) <= 0, axis=1)
            print(f"  MIDI {midi}: ff swing p10/50/90 {'/'.join(f'{v:.3f}' for v in np.percentile(swing[:, 0], (10, 50, 90)))} mm"
                  f" | non-monotonic in speed {np.mean(~mono) * 100:.0f}%")
            for j, frac in enumerate(SPEED_FRACTIONS[1:], start=1):
                print(f"    v x {frac:<5}: re ff p10/50/90 {'/'.join(f'{v:6.1f}' for v in np.percentile(rel[:, j], (10, 50, 90)))} dB"
                      f" | below -{NULL_DB:.0f} dB {np.mean(rel[:, j] < -NULL_DB) * 100:3.0f}% | contact p50 "
                      f"{np.median(tc[:, j]):.2f} T1 (ff {np.median(tc[:, 0]):.2f})")


if __name__ == "__main__":
    main()
