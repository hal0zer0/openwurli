#!/usr/bin/env python3
"""Large-swing bracket for one pickup section (plan §5k calibration caveat; overlap-law §6).

Eight law variants, the corners of
    {plane at -12 mm | converged plane} x {neighbours mirrored | neighbours at rest} x {no tilt | tilt-averaged}.

The main corner (plane -12, neighbours mirrored, no tilt) is the section re-solved whole with the fixed solver.
Every other corner adds a smooth correction taken from coarse knots solved both ways (same h): the corner's knots
minus the main corner's, as a natural cubic spline through the differences. Tilt averages the smoothed law over
the finger region with the FE mode-1 shape, about the displacement the prototype reports as pickup swing (the
mean over that region, tools/hammer_contact.py reed_data).

Writes DIR/<corner>/chosen.json and DIR/<corner>/pickup_tables.rs (the shipping file with this section's table
replaced). Only the bracket section changes; the others are emitted from the shipping data unchanged.

Usage:
    python tools/pickup_bracket.py SHIPPING.json MAIN.json HALF_KNOTS.json REST_KNOTS.json \\
        --section r15_20 --midi 48 --blank-in 0.023 --out-dir DIR
"""

import argparse
import json
import os
import sys

import numpy as np
from scipy.interpolate import CubicSpline

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pickup_field2d as F  # noqa: E402

MAIN_PLANE_MM = -12.0         # the solver's default grounded plane (pickup_field2d.FAR_BELOW)
CONVERGED_PLANE_MM = -120.0   # closure study: the plane is converged to ~1 % by -60 mm (overlap-law §6)
REST_U_MM = 0.085             # reed rest just outside the band, u = -0.085 mm (engine PICKUP_REST_OFFSET_MM)


def knots(path, plane):
    d = json.load(open(path))
    pts = sorted((u, c) for fb, u, c in d["points"] if fb == plane)
    return np.array([u for u, _ in pts], float), np.array([c for _, c in pts])


def tilt_ratio(midi, blank_in):
    """Mode-1 displacement across the finger region relative to its mean (the reported pickup swing)."""
    import hammer_contact as H
    r = H.reed(midi, blank_in * H.INCH, blank_in * H.INCH, nm=H.N_MODES)
    xx = np.linspace(r["L"] - H.FINGER_REGION_MM[0] * H.MM, r["L"] - H.FINGER_REGION_MM[1] * H.MM,
                     H.FINGER_REGION_PTS)
    phi = np.interp(xx, r["xe"], r["phi"][:, 0])
    return phi / phi.mean()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("shipping"), ap.add_argument("main"), ap.add_argument("half_knots"), ap.add_argument("rest_knots")
    ap.add_argument("--section", required=True)
    ap.add_argument("--midi", type=int, required=True, help="note whose FE mode shape sets the tilt")
    ap.add_argument("--blank-in", type=float, required=True, help="reed blank thickness for that FE reed, inches")
    ap.add_argument("--out-dir", required=True)
    a = ap.parse_args()
    sec = a.section
    main_t = json.load(open(a.main))[sec]
    u = np.array(main_t["u"])
    C_main = np.array(main_t["C_pF_per_m"])
    uk, c_ref = knots(a.half_knots, MAIN_PLANE_MM)
    variants = {}
    for plane_name, plane in (("plane12", MAIN_PLANE_MM), ("planeconv", CONVERGED_PLANE_MM)):
        for nb_name, path in (("nbmirror", a.half_knots), ("nbrest", a.rest_knots)):
            uk2, c = knots(path, plane)
            assert np.allclose(uk2, uk), (path, plane)
            corr = CubicSpline(uk, c - c_ref, bc_type="natural")
            variants[f"{plane_name}_{nb_name}"] = C_main + corr(u)
    ratio = tilt_ratio(a.midi, a.blank_in)
    print(f"tilt: MIDI {a.midi}, blank {a.blank_in} in: displacement {ratio.min():.3f}..{ratio.max():.3f} x pickup swing")
    shipping = json.load(open(a.shipping))
    for name, C in list(variants.items()):
        sm = F.law_smooth(u, C, sec)
        disp = u + REST_U_MM
        variants[f"{name}_tilton"] = np.array([np.mean(sm(ratio * d - REST_U_MM)) for d in disp])
        variants[f"{name}_tiltoff"] = variants.pop(name)
    for name, C in sorted(variants.items()):
        out = os.path.join(a.out_dir, name)
        os.makedirs(out, exist_ok=True)
        chosen = dict(shipping)
        chosen[sec] = dict(main_t, C_pF_per_m=C.tolist(), bracket_corner=name,
                           bracket_sources=[a.main, a.half_knots, a.rest_knots], tilt_midi=a.midi,
                           tilt_blank_in=a.blank_in)
        json.dump(chosen, open(os.path.join(out, "chosen.json"), "w"))
        F.emit_rust(os.path.join(out, "chosen.json"), os.path.join(out, "pickup_tables.rs"))
        rest = float(F.law_smooth(u, C, sec)(-REST_U_MM))
        print(f"{name}: C at rest {rest:.3f} pF/m, C(-6) {np.interp(-6, u, C):.3f}, C(+6) {np.interp(6, u, C):.3f}")


if __name__ == "__main__":
    main()
