#!/usr/bin/env python3
"""Registered test of the felt-loss form (plan §5m) on measured hammer loops: Stulov 1995 Fig. 2(a), Yanagisawa &
Nakamura's A1 pliant hammer at four impact speeds, as digitised from the scan.

Statistic: the gap between the loading and unloading branches at a fraction of the loop's maximum compression, as a
fraction of its maximum force; and its growth from the slowest to the fastest blow. Registered expectation (physics
review): x1.3-1.6 under Stulov's form, x2.3-3 under Hunt-Crossley; Stulov's form fails at >= x2.1.

Uncertainty, Monte Carlo over the digitisation's stated terms: each point's read uncertainty (uniform +-), a placement
scatter of up to +-0.014 mm in compression (the replotted data's disagreement with itself), and, for curves drawn with
triangles, a common force offset of up to +-0.12 N (the symbol's anchor point is unknown).

Usage:
    python tools/felt_loop_check.py DIGITISED.csv
"""

import csv
import sys

import numpy as np

FIGURE = "Fig. 2(a)"
CURVES = ("A1 pliant V=0.52", "A1 pliant V=0.86", "A1 pliant V=1.16", "A1 pliant V=1.43")  # slowest first
TRIANGLE_CURVES = ("A1 pliant V=0.86",)  # drawn with triangles (digitisation note)
PLACEMENT_SCATTER_MM = 0.014   # digitisation: the same data plotted twice disagrees by up to this much
TRIANGLE_OFFSET_N = 0.12       # digitisation: possible common offset of a triangle curve
FRACTIONS = (0.5, 0.8)         # registered at 0.5 of maximum compression; 0.8 shown beside it
STULOV_FAIL_RATIO = 2.1        # registered failure line for Stulov's form
MC_DRAWS = 4000
SEED = 1


def load(path):
    rows = [r for r in csv.DictReader(open(path)) if r["figure"] == FIGURE]
    out = {}
    for c in CURVES:
        pts = sorted((r for r in rows if r["curve"] == c), key=lambda r: (["loading", "peak", "unloading"].index(r["branch"]),
                                                                       int(r["seq"])))
        if not pts:
            sys.exit(f"{path}: no points for {FIGURE} {c}")
        out[c] = dict(u=np.array([float(r["compression_mm"]) for r in pts]),
                      f=np.array([float(r["force_N"]) for r in pts]),
                      su=np.array([float(r["read_unc_mm"]) for r in pts]),
                      sf=np.array([float(r["read_unc_N"]) for r in pts]))
    return out


def gap(u, f, frac):
    """(loading - unloading force) / F_max at frac x the maximum compression; the loop is in time order, split at its
    maximum compression. Past the unloading branch's last point the felt has left contact: force 0."""
    i = int(np.argmax(u))
    target = frac * u[i]
    fl = np.interp(target, u[:i + 1], f[:i + 1])
    ul, ff = u[i:][::-1], f[i:][::-1]
    fu = 0.0 if target < ul[0] else np.interp(target, ul, ff)
    return (fl - fu) / f.max()


def main():
    data = load(sys.argv[1])
    rng = np.random.default_rng(SEED)
    for frac in FRACTIONS:
        nominal = {c: gap(d["u"], d["f"], frac) for c, d in data.items()}
        draws = {c: [] for c in data}
        for _ in range(MC_DRAWS):
            for c, d in data.items():
                u = d["u"] + rng.uniform(-1, 1, len(d["u"])) * (d["su"] + PLACEMENT_SCATTER_MM)
                f = d["f"] + rng.uniform(-1, 1, len(d["f"])) * d["sf"]
                if c in TRIANGLE_CURVES:
                    f = f + rng.uniform(-TRIANGLE_OFFSET_N, TRIANGLE_OFFSET_N)
                draws[c].append(gap(u, f, frac))
        print(f"gap at {frac} of maximum compression (fraction of F_max): nominal, MC p5-p95")
        for c in CURVES:
            print(f"  {c}: {nominal[c]:.3f}  [{np.percentile(draws[c], 5):.3f}, {np.percentile(draws[c], 95):.3f}]")
        ratio = np.array(draws[CURVES[-1]]) / np.array(draws[CURVES[0]])
        r0 = nominal[CURVES[-1]] / nominal[CURVES[0]]
        verdict = "Stulov FAILS" if r0 >= STULOV_FAIL_RATIO else "Stulov not falsified"
        print(f"  growth {CURVES[0][-6:]} -> {CURVES[-1][-6:]}: x{r0:.2f} [{np.percentile(ratio, 5):.2f}, "
              f"{np.percentile(ratio, 95):.2f}]; registered: Stulov x1.3-1.6, Hunt-Crossley x2.3-3; {verdict} "
              f"(line x{STULOV_FAIL_RATIO}); MC share >= line {np.mean(ratio >= STULOV_FAIL_RATIO) * 100:.1f}%")


if __name__ == "__main__":
    main()
