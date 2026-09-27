#!/usr/bin/env python3
"""Option C: bass fingers bent up at an angle (report-only; docs/research/pickup-overlap-law.md §9).

The later factory bass pickups (and common technician practice) bend the bass fingers up by
~20-30 deg. Modelled without new field solves: the finger's face rises by s*tan(theta) at distance
s from the bend root, so the law along the finger is the section's static table evaluated at a
per-slice penetration and averaged over the finger length:

    C_theta(x) = mean over s in [0, L_f] of C(sign * x * phi(s) - g0 - s * tan(theta))

with phi(s) the reed's relative mode-1 amplitude along the finger (1 = the finger-region reference
the swing is quoted at; --no-shape sets phi = 1). Everything downstream is the engine's: the reed
tap's own displacement trace at each velocity (swing set to the unconditioned prior pick) through
C_theta and the engine's pickup network (node modulation included).

Reported per angle, nothing picked: criterion 6's quantities at C2/C3 v127 (fundamental rise above
its first 100 ms window, peak time; pickup tap); the mf swell (v80, same measure); H2/H1 and H3/H1
over velocity (Sensor, 0.1 s + 0.5 s window).

Usage: python tools/angled_fingers.py --sensor ANALYZE --tables T.json --finger-mm L --shape s0,s1
       [--angles 0,10,20,30] [--swing 36=3.02,48=2.43]
       --shape: phi at the finger's root and free end (linear between), e.g. 1.0,1.0 for none.
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import pickup_rev_metrics as M  # noqa: E402
from pickup_atlas import BENCH, C_REED_RATIO, PICKUP_FC, G0, sensor_dbc  # noqa: E402
from pickup_field2d import normalised_laws  # noqa: E402

SR = 44100
SECTION = {36: "r01_14", 48: "r15_20"}
VELS = (40, 64, 80, 100, 127)


def network(src):
    beta = np.pi * PICKUP_FC / SR
    y = np.empty_like(src)
    w, s_prev, m_prev = 0.0, 0.0, 1.0
    for i in range(len(src)):
        mn = 1.0 + C_REED_RATIO * src[i]
        m_avg = 0.5 * (mn + m_prev)
        w = (w * (m_avg - beta) - (src[i] - s_prev)) / (m_avg + beta)
        s_prev, m_prev = src[i], mn
        y[i] = w
    return y


def angled_source(law, x, theta_deg, L, phi0, phi1, n_slices=41):
    s = np.linspace(0.0, L, n_slices)
    phi = phi0 + (phi1 - phi0) * s / L
    rise = s * np.tan(np.radians(theta_deg))
    # bass: band above the reed (sign +1)
    c = np.mean([law(x * p - G0 - r) for p, r in zip(phi, rise)], axis=0)
    c0 = np.mean([law(np.zeros(1) - G0 - r)[0] for r in rise])
    return c - c0


def h1_course(sig, f0):
    return np.array([20 * np.log10(M.tone_mag(sig[int(t * SR):int((t + 0.1) * SR)], f0) + 1e-30)
                     for t in np.arange(0.0, 2.05, 0.1)])


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--sensor", required=True)
    ap.add_argument("--tables", required=True)
    ap.add_argument("--finger-mm", type=float, required=True)
    ap.add_argument("--shape", default="1.0,1.0")
    ap.add_argument("--angles", default="0,10,20,30")
    ap.add_argument("--swing", default="36=3.02,48=2.43")
    a = ap.parse_args()
    _, laws = normalised_laws(a.tables)
    phi0, phi1 = (float(v) for v in a.shape.split(","))
    swings = {int(k): float(v) for k, v in (p.split("=") for p in a.swing.split(","))}
    angles = [float(v) for v in a.angles.split(",")]
    tmp = tempfile.mkdtemp(prefix="angled_")
    out = {}
    for n, A in swings.items():
        u_tab, _, spline, _ = laws[SECTION[n]]
        lo, hi = u_tab[0], u_tab[-1]
        law = lambda u, sp=spline, lo=lo, hi=hi: sp(np.clip(u, lo, hi))  # noqa: E731
        edge = abs(spline(lo + 0.5) - spline(lo)) / abs(spline(u_tab).max() - spline(lo))
        f0 = M.midi_hz(n)
        traces = {}
        for v in VELS:
            pre = os.path.join(tmp, f"t{n}_{v}")
            subprocess.run([BENCH, "reed-tap", "--note", str(n), "--velocity", str(v), "--duration",
                            "2.2", "--swing-mm", f"{A}", "--output-prefix", pre],
                           check=True, capture_output=True)
            traces[v] = M.read_wav(f"{pre}_total_mm.wav") * 10.0
        print(f"\nMIDI {n} ({SECTION[n]}), ff swing {A} mm, finger {a.finger_mm} mm, "
              f"shape {phi0}->{phi1}; law clamped outside u [{lo}, {hi}] (C changes "
              f"{edge:.1e} of range over the last 0.5 mm below)")
        print("  angle | v127 rise/peak  | v80 rise/peak   | H2/H1 at v40 64 80 100 127        "
              "| H3/H1 at v40 64 80 100 127")
        for th in angles:
            row = {}
            for v in VELS:
                src = angled_source(law, traces[v], th, a.finger_mm, phi0, phi1)
                h = h1_course(src, f0)
                p = os.path.join(tmp, "y.wav")
                from steepness_check import write_wav
                write_wav(p, network(src))
                r = sensor_dbc(a.sensor, p, f0, 100, 500)
                row[v] = {"rise": float(h.max() - h[0]), "t_peak": float(0.1 * np.argmax(h)),
                          "H2": None if r is None else r[0], "H3": None if r is None else r[1]}
            f = lambda z: "  --" if z is None else f"{z:+5.1f}"  # noqa: E731
            print(f"  {th:4.0f}  | {row[127]['rise']:+5.2f} dB {row[127]['t_peak']:.1f} s | "
                  f"{row[80]['rise']:+5.2f} dB {row[80]['t_peak']:.1f} s | "
                  + " ".join(f(row[v]["H2"]) for v in VELS) + " | "
                  + " ".join(f(row[v]["H3"]) for v in VELS), flush=True)
            out[f"{n}/{th}"] = row
    json.dump(out, open(os.path.join(tmp, "angled.json"), "w"), indent=1)
    print(f"\n(json: {tmp}/angled.json)")


if __name__ == "__main__":
    main()
