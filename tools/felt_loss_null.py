#!/usr/bin/env python3
"""The stage-6 C4 prediction's statistic at FIXED felt (plan §5n): what felt loss alone does to it, with no
re-identification of the felt.

Stage-5 posterior instruments (identified lossless) are resampled by weight. Each is struck at C4 twice, with the same
FE variant, scatter and strike: lossless, and with tau_d drawn log-uniform on the stage-6 prior. Reported: the
registered statistic, 1 - median(contact | tau_d >= 0.1 ms) / median(contact | tau_d <= 0.02 ms), with a bootstrap
interval; and the paired change per tau_d bin.

Usage:
    python tools/felt_loss_null.py STAGE5_PARTICLES.json SURVEY.json --draws 1500
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

ORIGIN = "fillet-floor"  # the stage-5/6 strike geometry (plan §5j)
SPLIT = "code"           # grade lookup only: C4 is in the middle grade under every split
BOOT = 1000              # bootstrap resamples (90 % interval)
BOOT_PCT = (5, 95)       # the 90 % interval's percentiles
BINS_MS = (0.01, 0.02, 0.05, 0.1, 0.2, 0.3)  # report bins spanning the tau_d prior (plan §5m)
REPORT_PCT = (10, 50, 90)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("particles"), ap.add_argument("survey")
    ap.add_argument("--draws", type=int, required=True)
    ap.add_argument("--seed", type=int, default=5)
    a = ap.parse_args()
    rng = np.random.default_rng(a.seed)
    p = json.load(open(a.particles))
    if p.get("stage") != 5:
        sys.exit(f"{a.particles}: stage {p.get('stage')}; this takes a stage-5 (lossless) posterior")
    x, W = np.array(p["x"]), np.array(p["W"])
    inst = H.LAYOUTS[5].to_inst(x[rng.choice(len(W), size=a.draws, p=W / W.sum())])
    measured = {r["midi"]: r["onset"]["ratio"] for r in json.load(open(a.survey))
                if r.get("onset", {}).get("status") == "line"}
    midi = H.PREDICT_MIDI
    g = grade_of(midi, SPLIT)
    vs = variants(midi)
    rds = [reed_data(midi, t_tip, t_root) for _, t_tip, t_root in vs]
    kv = int(np.argmin([abs(r["om"][1] / r["om"][0] - measured[midi]) for r in rds]))  # as keyboard3b picks it
    rd = rds[kv]
    d_ref, L_ref = ANCHORS[ORIGIN]
    floor_mm = STRIKE_FLOOR_BY_ANCHOR.get(ORIGIN, STRIKE_FLOOR_MM)
    L_mm = rd["L"] * MM_PER_M
    scatter = rng.uniform(-STRIKE_SCATTER_MM, STRIKE_SCATTER_MM, a.draws)
    d_s = np.clip(d_ref * (L_mm / L_ref) ** inst["k"] + scatter, floor_mm, L_mm - CROWN_MM / 2)
    td = np.exp(rng.uniform(*H.LNT, a.draws))
    strike = (rd, inst["m_h"][:, g], inst["v"], inst["z_star"][:, g], inst["p"][:, g], d_s)
    r0, r1 = strike_w_chunked(*strike), strike_w_chunked(*strike, tau_d=td)
    t0, t1 = r0["t_contact_T1"], r1["t_contact_T1"]
    c0, c1 = r0["n_contacts"], r1["n_contacts"]
    lo, hi = td <= H.TAU_D_LOW_MAX_S, td >= H.TAU_D_HIGH_MIN_S
    stat = 1 - np.median(t1[hi]) / np.median(t1[lo])
    boot = []
    for _ in range(BOOT):
        i = rng.integers(0, a.draws, a.draws)
        boot.append(1 - np.median(t1[i][hi[i]]) / np.median(t1[i][lo[i]]))
    b_lo, b_hi = np.percentile(boot, BOOT_PCT)
    print(f"C4 (MIDI {midi}, variant {vs[kv][0]}), {a.draws} stage-5 posterior draws, felt FIXED:")
    print(f"  lossless contact p10/50/90 " + "/".join(f"{v:.3f}" for v in np.percentile(t0, REPORT_PCT))
          + f" T1; single contact {np.mean(c0 == 1) * 100:.0f} % lossless, {np.mean(c1 == 1) * 100:.0f} % lossy")
    print(f"  registered statistic at fixed felt: {stat * 100:.1f} % shorter [90 %: {b_lo * 100:.1f}, {b_hi * 100:.1f}]"
          f" (n {lo.sum()} at tau_d <= {H.TAU_D_LOW_MAX_S * 1e3:g} ms, {hi.sum()} at >= {H.TAU_D_HIGH_MIN_S * 1e3:g} ms)")
    rel = t1 / t0 - 1
    for b0, b1 in zip(BINS_MS[:-1], BINS_MS[1:]):
        s = (td >= b0 * 1e-3) & (td < b1 * 1e-3)
        same = s & (c0 == c1)
        print(f"  tau_d {b0:.2f}-{b1:.2f} ms (n {s.sum()}): paired change p10/50/90 "
              + "/".join(f"{v * 100:+.1f}" for v in np.percentile(rel[s], REPORT_PCT))
              + f" % | same contact count {same.sum()}: p50 {np.median(rel[same]) * 100:+.1f} %")


if __name__ == "__main__":
    main()
