#!/usr/bin/env python3
"""Per-build chain characterisation (measurement only; docs/research/pickup-overlap-law.md §9 taps).

1. Stage ladder: C2 / C4 / C6 at v40 / v80 / v127, at the pickup, preamp and full-chain taps
   (volume 0.80, speaker passthrough, MLP off): H1 (dBFS) and H2..H5 (dBc), Sensor Array with the
   fundamental pinned, window 0.1 s + 0.5 s. What each stage adds is the change between taps.
2. Volume sweep: C2 and C4 at v127, full chain, volume 0.2..1.0: H1, H3 and sample peak (dBFS);
   the power amp clips at high volume (60 mV in for 4.75 V out), so H3 and the peak show where.
3. Bass chord (all reeds share one plate node): C2+G2+C3 at v127 through the full chain against the
   sum of the three single-note renders (same chain, same seeds): the residual (dB re the chord) is
   the nonlinear interaction the chord adds. Reported at the preamp tap and at the full chain.

Usage: python tools/chain_characterise.py --sensor ANALYZE
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

BENCH = os.path.join(HERE, "..", "target", "release", "preamp-bench")
TAPS = {"pickup": ["--no-preamp", "--no-poweramp", "--no-attack-noise"],
        "preamp": ["--no-poweramp"], "full": []}


def render(tmp, name, note, vel, extra, vol="0.80", dur="1.2"):
    p = os.path.join(tmp, name + ".wav")
    subprocess.run(["nice", "-n", "19", BENCH, "render", "--note", str(note), "--velocity", str(vel),
                    "--duration", dur, "--volume", vol, "--speaker", "0.0", "--no-mlp", "--output", p,
                    *extra], check=True, capture_output=True)
    return p


def sensor(an, path, f0):
    fft = "16384" if f0 < 100 else "8192"
    d = json.loads(subprocess.run([an, path, "--skip-ms", "100", "--window-ms", "500", "--fft", fft,
                                   "--fundamental", f"{f0:.4f}", "--json"], check=True,
                                  capture_output=True, text=True).stdout)
    h = d["harmonics_dbfs"][:5]
    if not (d.get("supplied_fundamental") or {}).get("confirmed") or h[0] is None:
        return None
    return [h[0]] + [None if v is None else v - h[0] for v in h[1:]]


def fmt(v, w=6):
    return " " * (w - 2) + "--" if v is None else f"{v:+{w}.1f}"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--sensor", required=True)
    a = ap.parse_args()
    tmp = tempfile.mkdtemp(prefix="chain_")

    print("1. STAGE LADDER   note vel  tap     H1 dBFS | H2    H3    H4    H5 (dBc)")
    for n in (36, 60, 84):
        f0 = M.midi_hz(n)
        for v in (40, 80, 127):
            for tap, extra in TAPS.items():
                r = sensor(a.sensor, render(tmp, f"s{n}_{v}_{tap}", n, v, extra), f0)
                if r is None:
                    print(f"                 {n:4d} {v:3d}  {tap:6s}  refused")
                    continue
                print(f"                 {n:4d} {v:3d}  {tap:6s} {r[0]:+7.1f} |"
                      + "".join(fmt(x) for x in r[1:]))

    print("\n2. VOLUME SWEEP   note vol   H1 dBFS  H3 dBc  peak dBFS")
    for n in (36, 60):
        f0 = M.midi_hz(n)
        for vol in ("0.2", "0.4", "0.6", "0.8", "1.0"):
            p = render(tmp, f"v{n}_{vol}", n, 127, [], vol=vol)
            r = sensor(a.sensor, p, f0)
            pk = 20 * np.log10(np.abs(M.read_wav(p)).max() + 1e-30)
            print(f"                 {n:4d} {vol:4s} {fmt(r and r[0], 8)} {fmt(r and r[2], 7)}"
                  f"  {pk:+7.1f}")

    print("\n3. BASS CHORD C2+G2+C3 v127: nonlinear interaction residual (chord - sum of singles)")
    notes = (36, 43, 48)
    for tap, extra in (("preamp", ["--no-poweramp"]), ("full", [])):
        chord = os.path.join(tmp, f"chord_{tap}.wav")
        subprocess.run(["nice", "-n", "19", BENCH, "render-poly", "--notes",
                        ",".join(map(str, notes)), "--velocities", "127,127,127", "--duration",
                        "1.2", "--volume", "0.80", "--speaker", "0.0", "--single-seeds", "--output", chord,
                        *extra],
                       check=True, capture_output=True)
        c = M.read_wav(chord)
        s = sum(M.read_wav(render(tmp, f"c{n}_{tap}", n, 127, extra)) for n in notes)
        k = min(len(c), len(s))
        c, s = c[:k], s[:k]
        seg = slice(int(0.1 * 44100), k)
        res = 10 * np.log10(np.sum((c[seg] - s[seg]) ** 2) / np.sum(c[seg] ** 2) + 1e-30)
        print(f"   {tap:6s}: residual {res:+.1f} dB re chord; chord peak "
              f"{20 * np.log10(np.abs(c).max() + 1e-30):+.1f} dBFS")


if __name__ == "__main__":
    main()
