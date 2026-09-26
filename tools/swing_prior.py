#!/usr/bin/env python3
"""Provisional per-note ff reed swing at the pickup: an input prior conditioned on the falsifiers.

The reed swing after an ff strike follows a short-contact impulse law on each reed's first mode:

    V_tip = (1 + e) m_h v_h D / (phi_s (m_h + M1 / phi_s^2)),  capped at v_h sqrt(m_h / M1)
    A_p   = phi_p V_tip / (2 pi f1)                       (swing at the pickup's finger region)

Per-note reed data (tools/data/reed_swing_inputs.json) comes from an 80-element Euler-Bernoulli
model of each reed, with the tip solder solved to put the reed on pitch: tip-normalised modal mass
M1, the mode shape averaged over the hammer patch (phi_s, at strike points 0.40-0.60 L) and over
the finger region (phi_p). The hammer inputs have no measurement, so each gets an independent prior
(STATED HERE, before any render):

    m_h  hammer mass              log-uniform 1 - 5 g
    v_h  ff hammer speed          log-uniform 1.4 - 3.5 m/s   (service manual action ratio)
    e    restitution              log-uniform 0.3 - 0.7
    D    contact-spread factor    log-uniform 0.33 - 0.8
    x_s  strike point             uniform 0.40 - 0.60 L
    bass blank (reeds 1-20)       50/50 mixture of the two disputed thicknesses (.020" / .026")
    tip thickness (reeds 21-64)   uniform 0.51 - 0.66 mm, reed data interpolated

The falsifiers then condition the prior: samples whose swing exceeds the note's dynamic admitted
edge (the largest ff swing that passes the registered decay criteria, bisected per note on the
engine) are discarded. Pick = median of the survivors; band = their p10 / p90. A note where fewer
than 10 % of samples survive is reported: its prior and its falsifier are in conflict.

Usage:
    python tools/swing_prior.py [--n 200000] [--rust]
"""

import argparse
import json
import os

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))

# Dynamic admitted edges, mm at the finger region: the largest ff swing passing criteria 2 and 6
# of docs/research/pickup-overlap-law.md (crit 6 as re-registered in §9: rise <= 6 dB, no timing),
# bisected per knot by tools/swing_edges.py on the four-range (140B side gap) tables. Crit 6 binds at
# every knot up to 72. MIDI 75-96: at least this (the search stops inside the table's u range).
EDGES = {33: 3.23, 36: 2.87, 39: 3.01, 42: 3.26, 45: 3.11, 48: 2.96, 51: 2.71, 52: 2.93,
         53: 2.21, 54: 2.13, 57: 2.20, 60: 2.27, 63: 2.29, 66: 2.42, 69: 2.48, 72: 2.32,
         75: 2.89, 78: 2.89, 81: 2.89, 84: 2.89, 87: 2.89, 90: 2.89, 93: 2.89, 96: 2.89}

XS = np.array([0.40, 0.45, 0.50, 0.55, 0.60])


def log_uniform(rng, lo, hi, n):
    return np.exp(rng.uniform(np.log(lo), np.log(hi), n))


def reed_params(rng, knot, n):
    """Per-sample (M1 g, phi_s, phi_p, f1) for one knot, drawing the reed-geometry priors."""
    x_s = rng.uniform(0.40, 0.60, n)
    variants = sorted(knot)
    if variants[0].startswith("bass"):
        pick = rng.integers(0, 2, n)
        rows = [knot[variants[i]] for i in pick]
        M1 = np.array([r["M1_g"] for r in rows])
        phi_p = np.array([r["phi_p"] for r in rows])
        phi_tab = np.array([[r["phi_s"][f"{x:.2f}"] for x in XS] for r in rows])
        phi_s = np.array([np.interp(x, XS, p) for x, p in zip(x_s, phi_tab)])
        f1 = rows[0]["f1_Hz"]
    else:
        tips = np.array([knot[v]["t_tip_mm"] for v in variants])
        order = np.argsort(tips)
        tips = tips[order]
        vs = [knot[variants[i]] for i in order]
        t = rng.uniform(tips[0], tips[-1], n)
        M1 = np.interp(t, tips, [v["M1_g"] for v in vs])
        phi_p = np.interp(t, tips, [v["phi_p"] for v in vs])
        # phi_s: bilinear in (tip thickness, strike point)
        grid = np.array([[v["phi_s"][f"{x:.2f}"] for x in XS] for v in vs])
        per_tip = np.array([np.interp(x_s, XS, row) for row in grid])  # (tips, n)
        phi_s = np.array([np.interp(t[i], tips, per_tip[:, i]) for i in range(n)])
        f1 = vs[0]["f1_Hz"]
    return M1, phi_s, phi_p, f1


def swing_samples(rng, knot, n):
    M1, phi_s, phi_p, f1 = reed_params(rng, knot, n)
    m_h = log_uniform(rng, 1.0, 5.0, n)
    v_h = log_uniform(rng, 1.4, 3.5, n)
    e = log_uniform(rng, 0.3, 0.7, n)
    D = log_uniform(rng, 0.33, 0.8, n)
    v_tip = (1 + e) * m_h * v_h * D / (phi_s * (m_h + M1 / phi_s ** 2))
    v_tip = np.minimum(v_tip, v_h * np.sqrt(m_h / M1))
    return 1e3 * phi_p * v_tip / (2 * np.pi * f1)  # mm


def conditioned(knot, edge, n, seed):
    a = swing_samples(np.random.default_rng(seed), knot, n)
    keep = a[a <= edge]
    return keep.size / a.size, np.percentile(keep, [10, 50, 90]), np.percentile(a, [10, 50, 90])


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--n", type=int, default=200_000)
    ap.add_argument("--rust", action="store_true", help="print PROVISIONAL_FF_SWING_MM entries")
    a = ap.parse_args()
    data = json.load(open(os.path.join(HERE, "data", "reed_swing_inputs.json")))
    rows = []
    print(f"{'MIDI':>4} {'edge':>5} {'surv':>6} | {'p10':>5} {'med':>5} {'p90':>5} | unconditioned"
          f" p10/med/p90 | seed-to-seed max change")
    for midi in sorted(int(k) for k in data):
        s1, q1, u1 = conditioned(data[str(midi)], EDGES[midi], a.n, 1)
        s2, q2, _ = conditioned(data[str(midi)], EDGES[midi], a.n, 2)
        flag = "  <10% SURVIVE" if s1 < 0.10 else ""
        print(f"{midi:4d} {EDGES[midi]:5.2f} {s1:6.1%} | {q1[0]:5.2f} {q1[1]:5.2f} {q1[2]:5.2f} | "
              f"{u1[0]:5.2f}/{u1[1]:5.2f}/{u1[2]:5.2f} | {np.max(np.abs(q1 - q2)):.4f} mm{flag}")
        rows.append((midi, q1[1], q1[0], q1[2]))
    if a.rust:
        for midi, med, p10, p90 in rows:
            print(f"    ({midi}, {med:.2f}, {p10:.2f}, {p90:.2f}),")


if __name__ == "__main__":
    main()
