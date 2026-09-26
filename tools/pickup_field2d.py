#!/usr/bin/env python3
"""2D electrostatic field solve of the 200A pickup cross-section: reed capacitance vs vertical
position, and the fringe width of the overlap law fitted to it.

Offline tooling for the pickup overlap law (docs/research/pickup-overlap-law.md). The plugin's
pickup model uses a closed-form law; this script supplies the one number that law cannot take
from the evidence: the fringe width w of the smoothed band overlap, relative to the lateral
side gap g_s.

Geometry (cross-section perpendicular to the reed; mm; half-period by mirror symmetry):
    x = 0        centre of the reed (mirror plane)
    x = P/2      centre of the finger between two slots (mirror plane)
    reed         x in [0, W/2],          y in [u - t_r, u]      grounded
    finger       x in [W/2 + g_s, P/2],  y in [0, t_p]          held at 1 V
    RF plate     y = t_p + 12.7                                 grounded
    far field    y = -12                                        grounded
u is the reed's penetration past the finger's near face: u < 0 means the reed sits outside the
finger band (the rest position is u = -g0), u in [0, min(t_r, t_p)] is the rising ramp.
The finger is long along the reed compared with g_s, so a 2D solve is a fair model.

Output: per case, the mutual capacitance per unit length C(u) and fits of
    C(u) = c0 + a1 * o_w1(u)                         (one kernel)
    C(u) = c0 + a1 * o_w1(u) + a2 * o_w2(u)          (two kernels)
where o_w is the band overlap smoothed by a Lorentzian edge of width w — the law the plugin uses.

Usage:  python tools/pickup_field2d.py [--out results.json]    (needs numpy, scipy)
"""

import argparse
import json

import numpy as np
import scipy.sparse as sp
import scipy.sparse.linalg as spla
from scipy.optimize import least_squares

EPS0 = 8.854e-12  # F/m
P = 12.0          # reed pitch along the bar, mm (approximate; far compared with the gaps)
RF_ABOVE = 12.7   # RF plate height above the finger top, mm
FAR_BELOW = -12.0


def axis(fine_lo, fine_hi, h_fine, lo, hi, growth=1.15, h_max=0.5):
    """Grid lines: uniform h_fine on [fine_lo, fine_hi], geometric growth out to [lo, hi]."""
    core = np.arange(fine_lo, fine_hi + h_fine / 2, h_fine)
    left, h, x = [], h_fine, fine_lo
    while x - h > lo:
        h = min(h * growth, h_max)
        x -= h
        left.append(x)
    right, h, x = [], h_fine, fine_hi
    while x + h < hi:
        h = min(h * growth, h_max)
        x += h
        right.append(x)
    return np.unique(np.concatenate([[lo], left[::-1], core, right, [hi]]))


def solve_case(W, t_r, t_p, g_s, us, h=0.005):
    """Mutual capacitance (F/m, full reed = 2 x half-domain) at each penetration u."""
    xe = W / 2
    xs = axis(max(0.0, xe - 0.6), xe + g_s + 0.6, h, 0.0, P / 2)
    xs = np.unique(np.concatenate([xs, [xe, xe + g_s]]))
    u_lo, u_hi = min(us), max(us)
    ys = axis(u_lo - t_r - 0.6, max(t_p, u_hi) + 0.6, h, FAR_BELOW, t_p + RF_ABOVE)
    nx, ny = len(xs), len(ys)
    X, Y = np.meshgrid(xs, ys, indexing="ij")
    idx = np.arange(nx * ny).reshape(nx, ny)
    # dual-cell widths (for face lengths)
    dxc = np.gradient(xs) if nx > 1 else np.ones(1)
    dyc = np.gradient(ys)
    dxc[0] /= 2
    dxc[-1] /= 2
    dyc[0] /= 2
    dyc[-1] /= 2

    tol = 1e-9
    finger = (X >= xe + g_s - tol) & (Y >= -tol) & (Y <= t_p + tol)
    boundary0 = (np.isclose(Y, ys[0])) | (np.isclose(Y, ys[-1]))

    # Assemble the 5-point finite-volume operator once (geometry-independent part).
    rows, cols, vals = [], [], []
    for (a, b, ax) in (((slice(None, -1), slice(None)), (slice(1, None), slice(None)), 0),
                       ((slice(None), slice(None, -1)), (slice(None), slice(1, None)), 1)):
        ia, ib = idx[a].ravel(), idx[b].ravel()
        if ax == 0:
            d = np.diff(xs)[:, None] * np.ones((1, ny))
            face = np.ones((nx - 1, 1)) * dyc[None, :]
        else:
            d = np.ones((nx, 1)) * np.diff(ys)[None, :]
            face = dxc[:, None] * np.ones((1, ny - 1))
        g = (face / d).ravel()
        rows += [ia, ib, ia, ib]
        cols += [ib, ia, ia, ib]
        vals += [g, g, -g, -g]
    L = sp.csr_matrix((np.concatenate(vals), (np.concatenate(rows), np.concatenate(cols))),
                      shape=(nx * ny, nx * ny))

    out = []
    for u in us:
        reed = (X <= xe + tol) & (Y >= u - t_r - tol) & (Y <= u + tol)
        fixed = (finger | reed | boundary0).ravel()
        phi_fixed = np.where(finger.ravel(), 1.0, 0.0)
        free = ~fixed
        A = L[free][:, free]
        b = -L[free][:, fixed] @ phi_fixed[fixed]
        phi = phi_fixed.copy()
        phi[free] = spla.spsolve(A.tocsc(), b)
        # induced charge on the reed = -eps0 * (L phi) summed over reed nodes (per mm -> per m)
        q = -(L @ phi)[reed.ravel()].sum()
        out.append(abs(2 * EPS0 * q))  # both halves; F/m (lengths cancel in face/d)
    return np.array(out)


def lorentz_F(v):
    return v / 2 + (v * np.arctan(v) - 0.5 * np.log1p(v * v)) / np.pi


def overlap(u, t_r, t_p, w):
    R = lambda z: w * lorentz_F(z / w)
    return R(u) - R(u - t_r) - R(u - t_p) + R(u - t_r - t_p)


def fit(us, C, t_r, t_p, kernels):
    def model(p):
        c = p[0] + p[1] * overlap(us, t_r, t_p, abs(p[2]))
        if kernels == 2:
            c = c + p[3] * overlap(us, t_r, t_p, abs(p[4]))
        return c
    p0 = [C.min(), (C.max() - C.min()) / min(t_r, t_p), 0.1]
    if kernels == 2:
        p0 += [0.1 * p0[1], 1.0]
    r = least_squares(lambda p: (model(p) - C) / C.max(), p0)
    res = r.fun * C.max()
    p = list(r.x)
    p[2] = abs(p[2])
    if kernels == 2:
        p[4] = abs(p[4])
    return p, float(np.max(np.abs(res)) / (C.max() - C.min()))


CASES = {
    # register: W (mm), t_r (mm), t_p (mm, relative evidence only)
    "bass": (3.84, 0.51, 0.65),
    "treble": (3.07, 0.66, 2.0),
}
GAPS = (0.064, 0.13, 0.25)  # side gap g_s (mm): patent spec .0025" .. field-measured ~.010"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=None)
    ap.add_argument("--h", type=float, default=0.005)  # converged: 0.005 vs 0.0025 agree to 0.1%
    a = ap.parse_args()
    results = {}
    for name, (W, t_r, t_p) in CASES.items():
        us = np.round(np.arange(-1.5, t_r + t_p + 0.5 + 1e-9, 0.05), 4)
        for g_s in GAPS:
            C = solve_case(W, t_r, t_p, g_s, us, h=a.h)
            p1, e1 = fit(us, C, t_r, t_p, 1)
            p2, e2 = fit(us, C, t_r, t_p, 2)
            key = f"{name}/gs={g_s}"
            results[key] = {"W": W, "t_r": t_r, "t_p": t_p, "g_s": g_s,
                            "u": us.tolist(), "C_pF_per_m": (C * 1e12).tolist(),
                            "one_kernel": {"w": p1[2], "w_over_gs": p1[2] / g_s, "max_err": e1},
                            "two_kernel": {"w1": p2[2], "w2": p2[4], "a2_over_a1": p2[3] / p2[1],
                                           "max_err": e2}}
            print(f"{key:18s} 1-kernel w={p1[2]:.3f} mm (w/g_s={p1[2]/g_s:.2f}, err {e1:.1%})"
                  f" | 2-kernel w1={p2[2]:.3f} w2={p2[4]:.3f} a2/a1={p2[3]/p2[1]:.2f}"
                  f" (err {e2:.1%})")
    if a.out:
        with open(a.out, "w") as f:
            json.dump(results, f, indent=1)


if __name__ == "__main__":
    main()
