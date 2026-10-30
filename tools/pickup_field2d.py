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
    # The uniform fine region always holds the finger (0..t_p) as well as the reed: a per-point
    # solve with u past t_r + 0.6 used to leave the finger's bottom face in the graded cells, whose
    # positions move with u (jitter that halving h does not remove).
    ys = axis(min(u_lo - t_r - 0.6, -0.6), max(t_p, u_hi) + 0.6, h, FAR_BELOW, t_p + RF_ABOVE)
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


# Evidence bands per keyboard section (mm). Every absolute is a graded guess; see
# docs/research/pickup-overlap-law.md for sources. Tables are generated at both ends of each band.
SECTIONS = {
    # reeds 1-20: unground .020" reed; fingers ~2x the middle section (photo ratio 1.8-2.4x)
    "bass": {"W": 3.84, "t_r": (0.51,), "t_p": (0.85, 1.7)},
    # reeds 21-42: thin, bendable fingers; tongue .020" (200) .. .026" (200A, supplier figure)
    "middle": {"W": 3.07, "t_r": (0.51, 0.66), "t_p": (0.4, 0.8)},
    # reeds 43-64: "a lot thicker" than the middle, uniform to the top
    "treble": {"W": 2.64, "t_r": (0.51, 0.66), "t_p": (0.85, 2.0)},
}
SIDE_GAPS = (0.051, 0.064, 0.076)  # .002 / .0025 / .003" per side (patent spacer set)
U_STEP = 0.025


def table_cases():
    for sec, s in SECTIONS.items():
        for t_r in s["t_r"]:
            for t_p in s["t_p"]:
                for g_s in SIDE_GAPS:
                    yield sec, s["W"], t_r, t_p, g_s


def _solve_table(args):
    sec, W, t_r, t_p, g_s, h, step = args
    us = np.round(np.arange(-2.0, t_r + t_p + 0.5 + 1e-9, step), 5)
    C = solve_case(W, t_r, t_p, g_s, us, h=h)
    key = f"{sec}/tr={t_r}/tp={t_p}/gs={g_s}"
    return key, {"section": sec, "W": W, "t_r": t_r, "t_p": t_p, "g_s": g_s, "h": h,
                 "u": us.tolist(), "C_pF_per_m": (C * 1e12).tolist()}


def run_tables(h, step, out, jobs):
    from multiprocessing import Pool
    work = [(*c, h, step) for c in table_cases()]
    with Pool(jobs) as pool:
        res = dict(pool.imap_unordered(_solve_table, work))
    with open(out, "w") as f:
        json.dump(res, f)
    print(f"wrote {len(res)} tables (h={h}, u step={step}) to {out}")


# The configuration the plugin ships: one table per reed range. Reed width W and side clearance g_s
# come from the same measured row: reed and slot widths of ONE 140B comb (Electric Piano Forum
# topic 8418, 2014; method and serial unknown; the poster expects 200A reeds to differ). Clearance
# per side = (slot - reed) / 2. The 1951 patent's 0.064 mm design gap is the named outlier. Finger
# thickness t_p and reed thickness t_r follow the finger sections (thick 1-20 | thin 21-42 | thick
# 43-64); t_p is a guessed band, load-bearing in the thin middle section.
# u: the penetration range each table covers (wider in the bass, whose swings are larger).
CHOSEN = {
    "r01_14": {"W": 3.84, "t_r": 0.51, "t_p": 1.275, "g_s": 0.267, "u": (-4.5, 4.5)},
    "r15_20": {"W": 3.23, "t_r": 0.51, "t_p": 1.275, "g_s": 0.229, "u": (-4.5, 4.5)},
    "r21_42": {"W": 3.07, "t_r": 0.66, "t_p": 0.6, "g_s": 0.229, "u": (-3.0, 3.0)},
    "r43_64": {"W": 2.51, "t_r": 0.66, "t_p": 1.425, "g_s": 0.19, "u": (-3.0, 3.0)},
}
SECTIONS = list(CHOSEN)
NORM_SECTION = "r21_42"  # all tables share this section's full swing as their unit
CHOSEN_GS = 0.064  # superseded reference gap (patent), kept for the band-case study (--tables)


def run_chosen(h, step, out, jobs):
    from multiprocessing import Pool
    work = []
    for sec, s in CHOSEN.items():
        work.append((sec, s["W"], s["t_r"], s["t_p"], s["g_s"], h, step))
    with Pool(jobs) as pool:
        res = dict(pool.imap_unordered(_solve_chosen, work))
    with open(out, "w") as f:
        json.dump(res, f)
    print(f"wrote {len(res)} shipping tables (h={h}) to {out}")


def _solve_chosen(args):
    sec, W, t_r, t_p, g_s, h, step = args
    us = np.round(np.arange(-3.0, t_r + t_p + 1.0 + 1e-9, step), 5)
    C = solve_case(W, t_r, t_p, g_s, us, h=h)
    return sec, {"section": sec, "W": W, "t_r": t_r, "t_p": t_p, "g_s": g_s, "h": h,
                 "u": us.tolist(), "C_pF_per_m": (C * 1e12).tolist()}


def gcv_smooth(u, C):
    """Smoothing spline with lambda chosen by generalized cross-validation; returns (spline, lambda).

    scipy does not expose the chosen lambda, so the (private) GCV routine is wrapped to record it —
    the provenance must let a reader separate smoothing from grid differences.
    """
    import scipy.interpolate._bsplines as bs
    from scipy.interpolate import make_smoothing_spline
    seen = {}
    orig = bs._compute_optimal_gcv_parameter

    def capture(*args):
        seen["lam"] = orig(*args)
        return seen["lam"]

    bs._compute_optimal_gcv_parameter = capture
    try:
        sm = make_smoothing_spline(u, C)
    finally:
        bs._compute_optimal_gcv_parameter = orig
    return sm, float(seen["lam"])


# Smoothing strength per section, chosen by h-convergence (docs/research/pickup-overlap-law.md §9):
# the smallest decade at which C'' agrees between the h 0.0025 table and an h 0.00125 re-solve of
# the section's reach window within 10 % (tools/pickup_field2d.py --check-window). Not chosen by
# the data (GCV); the two now agree to <= 0.34 dB on harmonics above -30 dB re H1.
SMOOTH_LAM = {"r01_14": 1e-4, "r15_20": 1e-4, "r21_42": 1e-4, "r43_64": 1e-4}


def law_smooth(u, C, section):
    """The smoothing spline every tool uses for a section's law: lambda from SMOOTH_LAM."""
    from scipy.interpolate import make_smoothing_spline
    return make_smoothing_spline(u, C, lam=SMOOTH_LAM[section])


def normalised_laws(path_in):
    """Per-section law as the plugin evaluates it, from a --chosen JSON.

    Each section's C(u) is first denoised with a smoothing spline whose strength is chosen by
    generalized cross-validation (scipy make_smoothing_spline): the solver's discretization noise is
    tiny in C (~1e-4 of range) but double differentiation amplifies it into C'' — and harmonics come
    from C'' — so the raw table would inject spurious harmonics. The fitted residual is reported and
    must stay within the solver's own error. The smoothed values then become a natural cubic spline
    (values + second derivatives on a uniform grid). All sections share one normalisation — the
    NORM_SECTION's full swing, C(max) minus its far-field value — so the physical coupling
    differences between sections survive.

    Returns (T, {section: (u, y, spline, provenance)}).
    """
    from scipy.interpolate import CubicSpline
    T = json.load(open(path_in))
    smooth, prov, knots = {}, {}, {}
    for sec in SECTIONS:
        u = np.array(T[sec]["u"])
        C = np.array(T[sec]["C_pF_per_m"])
        lam = SMOOTH_LAM[sec]
        sm = law_smooth(u, C, sec)
        res = sm(u) - C
        rms, mx = np.sqrt(np.mean(res ** 2)) / np.ptp(C), np.abs(res).max() / np.ptp(C)
        prov[sec] = (f"lambda {lam:.0e} (h-convergence), residual rms {rms:.2e} / max {mx:.2e} "
                     f"of range")
        # An extended section (--merge-extension) mixes the shipping grid with coarser far knots;
        # the plugin's table is uniform, so its smoothed law is sampled on the shipping grid.
        if not np.allclose(np.diff(u), np.diff(u).mean()):
            u = u[0] + U_STEP * np.arange(int(round((u[-1] - u[0]) / U_STEP)) + 1)
            prov[sec] += f", sampled every {U_STEP} mm"
        print(f"{sec}: {prov[sec]}")
        smooth[sec], knots[sec] = sm(u), u
    ref = smooth[NORM_SECTION]
    dc_ref = ref.max() - ref[0]
    laws = {}
    for sec in SECTIONS:
        u = knots[sec]
        y = (smooth[sec] - ref[0]) / dc_ref
        laws[sec] = (u, y, CubicSpline(u, y, bc_type="natural"), prov[sec])
    return T, laws


def emit_rust(path_in, path_out):
    """Write crates/openwurli-dsp/src/pickup_tables.rs from a --chosen JSON (see normalised_laws)."""
    T, laws = normalised_laws(path_in)
    lines = [
        "//! Pickup capacitance-vs-position tables, one per keyboard section.",
        "//!",
        "//! GENERATED by `tools/pickup_field2d.py --emit-rust` from a 2D field solve of the pickup",
        "//! cross-section (see docs/research/pickup-overlap-law.md). Do not edit by hand.",
        "//!",
        "//! `u` is the reed's penetration past the near finger face (mm). Values are the reed-to-pickup",
        "//! capacitance normalised by the reeds-21-42 section's full swing; `m` holds the natural cubic",
        "//! spline's second derivatives at the same knots.",
        "",
        "#![allow(clippy::unreadable_literal, clippy::excessive_precision)]",
        "",
        "use crate::pickup::PickupLawTable;",
        "",
    ]
    for sec in SECTIONS:
        t = T[sec]
        u, y, cs, prov_sec = laws[sec]
        du = float(np.diff(u).mean())
        assert np.allclose(np.diff(u), du), "non-uniform grid"
        m = cs(u, 2)
        # off-knot reference values for the Rust spline test
        probe = u[:-1][:: max(1, len(u) // 16)] + 0.37 * du
        check = [(float(p), float(cs(p))) for p in probe]
        fmt = lambda arr: ",\n".join("    " + ", ".join(f"{v:.12e}" for v in arr[i:i + 4])
                                     for i in range(0, len(arr), 4))
        name = sec.upper()
        lines += [
            f"/// {sec}: W {t['W']} mm, t_r {t['t_r']} mm, t_p {t['t_p']} mm, g_s {t['g_s']} mm,"
            f" grid h {t['h']} mm.",
            f"/// Smoothing: {prov_sec}.",
            f"pub static {name}: PickupLawTable = PickupLawTable {{",
            f"    u0: {u[0]:.6},",
            f"    du: {du:.6},",
            f"    t_r: {t['t_r']},",
            f"    t_p: {t['t_p']},",
            f"    y: &{name}_Y,",
            f"    m: &{name}_M,",
            "};",
            "",
            f"static {name}_Y: [f64; {len(y)}] = [",
            fmt(y),
            "];",
            "",
            f"static {name}_M: [f64; {len(m)}] = [",
            fmt(m),
            "];",
            "",
            "/// Off-knot (u, value) pairs from the generator's own spline, for the evaluation test.",
            "#[cfg(test)]",
            f"pub static {name}_CHECK: [(f64, f64); {len(check)}] = [",
            *[f"    ({p:.12e}, {v:.12e})," for p, v in check],
            "];",
            "",
        ]
    with open(path_out, "w") as f:
        f.write("\n".join(lines))
    print(f"wrote {path_out}")


SPICE_PWL_STEP = 0.01  # mm; PWL kinks stay below -64 dB re H1 on H2..H8 (0.025 mm: -44 dB)


def emit_spice(path_in, path_out):
    """Write spice/subcircuits/pickup_law.inc: the same per-section laws as the plugin, as ngspice
    .func pwl() tables. ngspice has no spline function, so the plugin's spline is sampled at
    SPICE_PWL_STEP; at that density linear interpolation adds no harmonic error that the
    convergence gate could see."""
    T, laws = normalised_laws(path_in)
    lines = [
        "* Pickup capacitance-vs-position laws, one per keyboard section.",
        "*",
        "* GENERATED by tools/pickup_field2d.py --emit-spice from a 2D field solve of the pickup",
        "* cross-section (see docs/research/pickup-overlap-law.md). Do not edit by hand.",
        "* Same laws as crates/openwurli-dsp/src/pickup_tables.rs: the plugin's natural cubic spline,",
        f"* sampled every {SPICE_PWL_STEP} mm. u = penetration past the near finger face (mm); values",
        "* are the reed-to-pickup capacitance normalised by the reeds-21-42 section's full swing.",
        "",
    ]
    for sec in SECTIONS:
        t = T[sec]
        u, _, cs, prov = laws[sec]
        n = int(np.floor((u[-1] - u[0]) / SPICE_PWL_STEP + 1e-9))
        us = np.round(u[0] + SPICE_PWL_STEP * np.arange(n + 1), 6)
        pairs = [f"{p:.3f},{float(cs(p)):.9e}" for p in us]
        lines += [
            f"* {sec}: W {t['W']} mm, t_r {t['t_r']} mm, t_p {t['t_p']} mm, g_s {t['g_s']} mm,"
            f" grid h {t['h']} mm. Smoothing: {prov}.",
            f".func pickup_law_{sec}(u) = pwl(u,",
            *[f"+ {', '.join(pairs[i:i + 6])}{',' if i + 6 < len(pairs) else ')'}"
              for i in range(0, len(pairs), 6)],
            "",
        ]
    with open(path_out, "w") as f:
        f.write("\n".join(lines))
    print(f"wrote {path_out}")


def _solve_point(args):
    W, t_r, t_p, g_s, u, h = args
    return float(solve_case(W, t_r, t_p, g_s, np.array([u]), h=h)[0])


def run_refine(section, u_lo, u_hi, h, step, out, jobs, t_p=None, W=None, g_s=None):
    """Re-solve one shipping section on a sub-range of u at a finer grid, one point per job —
    for a targeted convergence check where a table is resolution-limited."""
    from multiprocessing import Pool
    s = dict(CHOSEN[section])
    for k, v in (("t_p", t_p), ("W", W), ("g_s", g_s)):  # band-end overrides
        if v is not None:
            s[k] = v
    us = np.round(np.arange(u_lo, u_hi + 1e-9, step), 5)
    with Pool(jobs) as pool:
        C = pool.map(_solve_point, [(s["W"], s["t_r"], s["t_p"], s["g_s"], u, h) for u in us])
    res = {section: {"section": section, "W": s["W"], "t_r": s["t_r"], "t_p": s["t_p"],
                     "g_s": s["g_s"], "h": h, "u": us.tolist(),
                     "C_pF_per_m": (np.array(C) * 1e12).tolist()}}
    with open(out, "w") as f:
        json.dump(res, f)
    print(f"wrote refined {section} u [{u_lo}, {u_hi}] (h={h}) to {out}")


def merge_refine(base_path, part_path, out_path):
    """Replace a table's points with a re-solve of the same u knots (same h, same geometry).

    Used after the fine-region fix: for u <= t_r the fixed mesh is identical to the old one, so
    re-solving only u > t_r and merging keeps ONE solve path per table (not a splice)."""
    base, part = json.load(open(base_path)), json.load(open(part_path))
    for key, t in part.items():
        b = base[key]
        for f in ("W", "t_r", "t_p", "g_s", "h"):
            assert abs(b[f] - t[f]) < 1e-12, (key, f, b[f], t[f])
        pos = {round(u, 5): i for i, u in enumerate(b["u"])}
        for u, c in zip(t["u"], t["C_pF_per_m"]):
            b["C_pF_per_m"][pos[round(u, 5)]] = c  # KeyError = knot mismatch: refuse
        b["resolved_above_u"] = min(t["u"])
    with open(out_path, "w") as f:
        json.dump(base, f)
    print(f"merged {part_path} into {base_path} -> {out_path}")


def merge_extension(base_path, out_path, ext_paths):
    """Add --refine solves outside a section's range (same geometry and h) to a --chosen JSON.

    The far knots may be coarser than the shipping grid; normalised_laws smooths the section's data
    as one set and samples the law on the shipping grid."""
    base = json.load(open(base_path))
    for path in ext_paths:
        for key, t in json.load(open(path)).items():
            b = base[key]
            for f in ("W", "t_r", "t_p", "g_s", "h"):
                assert abs(b[f] - t[f]) < 1e-12, (path, key, f, b[f], t[f])
            lo, hi = min(b["u"]), max(b["u"])
            new = [(u, c) for u, c in zip(t["u"], t["C_pF_per_m"])]
            assert all(u < lo or u > hi for u, _ in new), f"{path}: knots inside {key}'s range"
            pts = sorted(list(zip(b["u"], b["C_pF_per_m"])) + new)
            b["u"], b["C_pF_per_m"] = [u for u, _ in pts], [c for _, c in pts]
            b.setdefault("extended_by", []).append(path)
    with open(out_path, "w") as f:
        json.dump(base, f)
    print(f"merged {ext_paths} into {base_path} -> {out_path}")


def static_harmonics(u, C, g0, A, kmax=8, n=1024):
    """Harmonics (dB re H1) of the static law C(-g0 + A sin), through a GCV smoothing spline.

    Used only by check_convergence (the original h/2 gate, superseded by --check-window). The
    plugin's tables use law_smooth (lambda from SMOOTH_LAM), not GCV."""
    cs, _ = gcv_smooth(u, C)
    th = np.arange(n) * 2 * np.pi / n
    X = np.abs(np.fft.rfft(cs(-g0 + A * np.sin(th))))[1:kmax + 1]
    return 20 * np.log10(np.maximum(X[1:], 1e-300) / X[0])


def check_window(path_ship, path_half, rel_above_db=-30.0, rel_tol_db=0.5, abs_tol_db=-45.0,
                 c2_tol=0.10):
    """h/2 gate on a --refine window, like for like (registered 2026-09-26).

    The shipping-grid table is CROPPED to the window and both are smoothed with the SAME lambda,
    the smallest of 1e-7..1e-3 (decades) at which C'' agrees between the grids within c2_tol of its
    peak: the smoothing is chosen by h-convergence, not by the data, so the gate measures the grid.
    Swings 0.02 mm up to the window's edge, g0 0.07-0.10 mm; thresholds as in check_convergence.
    Also reported (not the gate): the SHIPPING table's own smoothing error, i.e. the full-range
    spline the plugin compiles (law_smooth for a section in SMOOTH_LAM, else GCV) against the
    converged window spline of the h/2 data.
    """
    from scipy.interpolate import make_smoothing_spline
    a, b = json.load(open(path_ship)), json.load(open(path_half))
    th = np.arange(1024) * 2 * np.pi / 1024

    def harm(sp, g0, A):
        X = np.abs(np.fft.rfft(sp(-g0 + A * np.sin(th))))[1:9]
        return 20 * np.log10(np.maximum(X[1:], 1e-300) / X[0])

    def compare(spa, spb, lo, hi):
        wl, wr = -np.inf, 0.0
        for g0 in (0.07, 0.085, 0.10):
            for A in np.arange(0.02, min(-g0 - lo, hi + g0) + 1e-9, 0.01):
                ha, hb = harm(spa, g0, A), harm(spb, g0, A)
                big = (ha > rel_above_db) | (hb > rel_above_db)
                if (~big).any():
                    lin = np.abs(10 ** (ha / 20) - 10 ** (hb / 20))[~big]
                    wl = max(wl, 20 * np.log10(max(float(lin.max()), 1e-300)))
                if big.any():
                    wr = max(wr, float(np.max(np.abs(ha - hb)[big])))
        return wl, wr

    ok = True
    for key in sorted(set(a) & set(b)):
        ua, Ca = np.array(a[key]["u"]), np.array(a[key]["C_pF_per_m"])
        ub, Cb = np.array(b[key]["u"]), np.array(b[key]["C_pF_per_m"])
        m = (ua >= ub[0] - 1e-9) & (ua <= ub[-1] + 1e-9)
        uc, Cc = ua[m], Ca[m]
        uu = np.linspace(ub[0], ub[-1], 4001)
        lam = None
        for lv in (1e-7, 1e-6, 1e-5, 1e-4, 1e-3):
            d2a = make_smoothing_spline(uc, Cc, lam=lv)(uu, 2)
            d2b = make_smoothing_spline(ub, Cb, lam=lv)(uu, 2)
            if np.abs(d2a - d2b).max() <= c2_tol * np.abs(d2b).max():
                lam = lv
                break
        if lam is None:
            print(f"{key}: C'' never converges between the grids up to lam 1e-3 -> FAIL")
            ok = False
            continue
        spa = make_smoothing_spline(uc, Cc, lam=lam)
        spb = make_smoothing_spline(ub, Cb, lam=lam)
        wl, wr = compare(spa, spb, ub[0], ub[-1])
        passed = wl <= abs_tol_db and wr <= rel_tol_db
        ok &= passed
        ship = law_smooth(ua, Ca, key) if key in SMOOTH_LAM else gcv_smooth(ua, Ca)[0]
        sl, sr = compare(ship, spb, ub[0], ub[-1])
        print(f"{key} window u [{ub[0]}, {ub[-1]}], lam {lam:.0e}: GRID linear {wl:6.1f} dB | "
              f"above {rel_above_db:.0f} dB: {wr:.2f} dB | {'PASS' if passed else 'FAIL'} || "
              f"shipping smoothing vs converged: linear {sl:6.1f} dB, rel {sr:.2f} dB (reported)")
    print("GATE (window, like for like):", "PASS" if ok else "FAIL")
    return ok


def check_convergence(path_a, path_b, rel_above_db=-30.0, rel_tol_db=0.5, abs_tol_db=-45.0,
                      a_max=None):
    """Harmonic-convergence gate between an h and an h/2 table set (static law, H2..H8).

    Superseded by --check-window: each table here is GCV-smoothed over its own u range, so the gate
    partly compares smoothers rather than grids (docs/research/pickup-overlap-law.md §9).

    Over swings 0.02 mm .. min(t_p, 1 mm) (extend once the real swing is known), g0 0.07-0.10 mm, and only swings that stay
    inside both tables' u range — so a --refine window can be gated against its partner): every
    harmonic above rel_above_db re H1 agrees to rel_tol_db, and every harmonic at or below it
    differs by <= abs_tol_db re H1 (linear). Each bound governs where it is well-posed: a dB gap is
    meaningless far below H1, and a linear bound re H1 is over-strict on a harmonic louder than H1
    (fold-through).
    """
    a, b = json.load(open(path_a)), json.load(open(path_b))
    ok = True
    for key in sorted(set(a) & set(b)):
        ta, tb = a[key], b[key]
        ua, Ca = np.array(ta["u"]), np.array(ta["C_pF_per_m"])
        ub, Cb = np.array(tb["u"]), np.array(tb["C_pF_per_m"])
        u_lo, u_hi = max(ua[0], ub[0]), min(ua[-1], ub[-1])
        worst_lin, worst_rel = -np.inf, 0.0
        for g0 in (0.07, 0.085, 0.10):
            reach = min(ta["t_p"], 1.0) if a_max is None else a_max
            a_top = min(reach, -g0 - u_lo, u_hi + g0)
            for A in np.arange(0.02, a_top + 1e-9, 0.01):
                ha, hb = static_harmonics(ua, Ca, g0, A), static_harmonics(ub, Cb, g0, A)
                big = (ha > rel_above_db) | (hb > rel_above_db)
                if (~big).any():
                    lin = np.abs(10 ** (ha / 20) - 10 ** (hb / 20))[~big]
                    worst_lin = max(worst_lin, 20 * np.log10(max(float(lin.max()), 1e-300)))
                if big.any():
                    worst_rel = max(worst_rel, float(np.max(np.abs(ha - hb)[big])))
        passed = worst_lin <= abs_tol_db and worst_rel <= rel_tol_db
        ok &= passed
        print(f"{key:40s} linear {worst_lin:6.1f} dB re H1 | above {rel_above_db:.0f} dB: "
              f"{worst_rel:.2f} dB | {'PASS' if passed else 'FAIL'}")
    print("GATE:", "PASS" if ok else "FAIL")
    return ok


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", nargs=2, metavar=("A.json", "B.json"),
                    help="harmonic-convergence gate between two table sets")
    ap.add_argument("--g-s", type=float, default=None,
                    help="--refine: override the section's side clearance (band-end cases)")
    ap.add_argument("--t-p", type=float, default=None,
                    help="--refine: override the section's finger thickness (band-end cases)")
    ap.add_argument("--a-max", type=float, default=None,
                    help="--check: largest swing (mm) to gate; default min(t_p, 1 mm)")
    ap.add_argument("--out", default=None)
    ap.add_argument("--h", type=float, default=0.005)  # converged: 0.005 vs 0.0025 agree to 0.1%
    ap.add_argument("--tables", action="store_true",
                    help="solve every evidence-band case (SECTIONS x SIDE_GAPS) instead of the fit survey")
    ap.add_argument("--u-step", type=float, default=U_STEP)
    ap.add_argument("--jobs", type=int, default=4,
                    help="worker processes; each holds one solve (~1 GB at --h 0.0025, ~4 GB at 0.00125)")
    ap.add_argument("--chosen", action="store_true",
                    help="solve only the shipping configuration (CHOSEN), one table per section")
    ap.add_argument("--emit-spice", nargs=2, metavar=("CHOSEN.json", "OUT.inc"),
                    help="write the per-section laws as ngspice .func pwl() tables")
    ap.add_argument("--emit-rust", nargs=2, metavar=("CHOSEN.json", "OUT.rs"),
                    help="generate the plugin's pickup_tables.rs from a --chosen solve")
    ap.add_argument("--check-window", nargs=2, metavar=("SHIPPING.json", "HALF_H_WINDOW.json"),
                    help="h/2 gate on a --refine window, like for like (same lambda, by h-convergence)")
    ap.add_argument("--merge-refine", nargs=3, metavar=("BASE.json", "PART.json", "OUT.json"))
    ap.add_argument("--merge-extension", nargs="+", metavar="JSON",
                    help="BASE.json OUT.json EXT.json [EXT.json ...]: add solves outside a section's range")
    ap.add_argument("--refine", nargs=3, metavar=("SECTION", "U_LO", "U_HI"),
                    help="re-solve one shipping section on a u sub-range (finer --h)")
    a = ap.parse_args()
    if a.refine:
        run_refine(a.refine[0], float(a.refine[1]), float(a.refine[2]), a.h, a.u_step, a.out,
                   a.jobs, a.t_p, g_s=a.g_s)
        return
    if a.emit_rust:
        emit_rust(*a.emit_rust)
        return
    if a.emit_spice:
        emit_spice(*a.emit_spice)
        return
    if a.chosen:
        run_chosen(a.h, a.u_step, a.out, a.jobs)
        return
    if a.check:
        check_convergence(*a.check, a_max=a.a_max)
        return
    if a.check_window:
        check_window(*a.check_window)
        return
    if a.merge_refine:
        merge_refine(*a.merge_refine)
        return
    if a.merge_extension:
        merge_extension(a.merge_extension[0], a.merge_extension[1], a.merge_extension[2:])
        return
    if a.tables:
        run_tables(a.h, a.u_step, a.out, a.jobs)
        return
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
