#!/usr/bin/env python3
"""Pickup atlas (describing-function map) and the known-answer check against it.

docs/research/pickup-overlap-law.md §9, "Measurement taps", stimuli (1) and (3).

atlas:  the static law alone, driven by a clean sinusoidal swing u = -g0 + A sin(2 pi f t) at a fixed
        frequency, amplitude swept; H1..H6 measured with the Sensor Array on the raw source (before
        the pickup's RC network). One map per table case (section x band end).
known:  a pickup-tap render of the engine (preamp, power amp and attack noise off, speaker
        passthrough) against the atlas at the swing the reed tap reports for the same window (Hann-weighted mean mode-1
        amplitude over the analysed window),
        after removing the RC network's per-harmonic tilt (one-pole high-pass at PICKUP_FC).
        0.5 s windows start after mode 2 has died (reed tap: mode2/mode1 below -60 dB). Reported as
        measured differences; the pass threshold is registered separately.

Usage:
    python tools/pickup_atlas.py atlas --sensor ANALYZE --cases cases.json --out atlas.json
    python tools/pickup_atlas.py known --sensor ANALYZE --atlas atlas.json --notes 36,60,84
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile

import numpy as np
from scipy.signal import hilbert

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import pickup_rev_metrics as M  # noqa: E402
from pickup_field2d import gcv_smooth  # noqa: E402
from steepness_check import write_wav  # noqa: E402

SR = 44100
F_ATLAS = 441.0  # 100 cycles per 0.1 s: whole cycles in every Sensor frame length used
PICKUP_FC = 880.5
BENCH = os.path.join(HERE, "..", "target", "release", "preamp-bench")
KNOWN_WIN = 0.5  # s; the analyser needs a window somewhat longer than FFT 16384 (371 ms)
AMPS = np.round(np.exp(np.linspace(np.log(0.02), np.log(3.0), 41)), 4)


def sensor_dbc(sensor, path, f0, skip_ms, win_ms, kmax=6):
    fft = "16384" if f0 < 100 else "8192"
    d = json.loads(subprocess.run([sensor, path, "--skip-ms", str(skip_ms), "--window-ms",
                                   str(win_ms), "--fft", fft, "--fundamental", f"{f0:.4f}",
                                   "--json"], check=True, capture_output=True, text=True).stdout)
    if not (d.get("supplied_fundamental") or {}).get("confirmed"):
        return None
    return d["harmonics_dbc"][1:kmax]


def atlas(a):
    cases = json.load(open(a.cases))
    tmp = tempfile.mkdtemp(prefix="atlas_")
    out = {}
    t = np.arange(int(1.0 * SR)) / SR
    for row, cs_map in cases.items():
        for name, (path, key, g0) in cs_map.items():
            tab = json.load(open(path))[key]
            u = np.array(tab["u"])
            cs, _ = gcv_smooth(u, np.array(tab["C_pF_per_m"]))
            rows = []
            for A in AMPS:
                if -g0 + A > u.max() or -g0 - A < u.min():
                    continue
                p = os.path.join(tmp, "tone.wav")
                s = cs(-g0 + A * np.sin(2 * np.pi * F_ATLAS * t))
                write_wav(p, s - s.mean())
                h = sensor_dbc(a.sensor, p, F_ATLAS, 100, 800)
                rows.append([float(A)] + (h if h is not None else [None] * 5))
            out[f"{row}/{name}"] = {"g0": g0, "t_p": tab["t_p"], "g_s": tab["g_s"],
                                    "cols": ["A_mm", "H2", "H3", "H4", "H5", "H6"], "rows": rows}
            print(f"{row}/{name}: {len(rows)} amplitudes", flush=True)
    json.dump(out, open(a.out, "w"), indent=1)


def hpf_tilt_db(f0, k):
    mag = lambda f: f / np.hypot(f, PICKUP_FC)  # noqa: E731
    return 20 * np.log10(mag(k * f0) / mag(f0))


def section_of(n):
    reed = n - 32
    return "r01_14" if reed <= 14 else "r15_20" if reed <= 20 else "r21_42" if reed <= 42 else "r43_64"


def known(a):
    at = json.load(open(a.atlas))
    tmp = tempfile.mkdtemp(prefix="known_")
    resid = []
    for n in (int(v) for v in a.notes.split(",")):
        sec = section_of(n)
        m = np.array([r for r in at[f"{sec}/central"]["rows"] if r[1] is not None], dtype=float)
        wav = os.path.join(tmp, f"p{n}.wav")
        subprocess.run([BENCH, "render", "--note", str(n), "--velocity", "127", "--duration", "2.5",
                        "--volume", "0.80", "--speaker", "0.0", "--no-mlp", "--no-preamp",
                        "--no-poweramp", "--no-attack-noise", "--output", wav],
                       check=True, capture_output=True)
        tap = os.path.join(tmp, f"t{n}")
        subprocess.run([BENCH, "reed-tap", "--note", str(n), "--velocity", "127", "--duration", "2.5",
                        "--output-prefix", tap], check=True, capture_output=True)
        m1 = M.read_wav(f"{tap}_mode1_mm.wav") * 10.0  # mm (full scale 10 mm)
        m2 = M.read_wav(f"{tap}_mode2_mm.wav") * 10.0
        f0 = M.refine_f0(M.read_wav(wav)[int(0.3 * SR):int(0.7 * SR)], M.midi_hz(n))
        worst = 0.0
        for t0 in (0.3, 0.8, 1.3, 1.8):
            seg = slice(int(t0 * SR), int((t0 + KNOWN_WIN) * SR))
            env = np.abs(hilbert(m1))[seg]  # mode-1 amplitude envelope
            w = np.hanning(len(env))
            A = float(np.sum(env * w) / np.sum(w))  # Hann-weighted mean over the analysed window
            m2db = 20 * np.log10(np.abs(m2[seg]).max() / A + 1e-30)
            h = sensor_dbc(a.sensor, wav, f0, int(t0 * 1000), int(KNOWN_WIN * 1000))
            if h is None:
                print(f"  MIDI {n} t {t0}: Sensor refused")
                continue
            exp = [float(np.interp(np.log(A), np.log(m[:, 0]), m[:, k])) + hpf_tilt_db(f0, k + 1)
                   for k in range(1, 6)]
            d = [None if h[i] is None else h[i] - exp[i] for i in range(5)]
            strong = [abs(d[i]) for i in range(5) if d[i] is not None and max(h[i], exp[i]) > -40]
            worst = max([worst] + strong)
            resid.append([n, t0] + [d[i] if d[i] is not None and max(h[i], exp[i]) > -40 else None
                                    for i in range(5)])
            print(f"  MIDI {n} t {t0:.1f} s: A {A:.3f} mm (mode2 {m2db:+.0f} dB) | render H2..H6 "
                  + " ".join("   --" if v is None else f"{v:+6.1f}" for v in h) + " | atlas+RC "
                  + " ".join(f"{v:+6.1f}" for v in exp) + " | resid "
                  + " ".join("  --" if v is None else f"{v:+5.2f}" for v in d), flush=True)
        print(f"KNOWN MIDI {n} ({sec}): worst |diff| on harmonics above -40 dBc {worst:.2f} dB",
              flush=True)
    # Sign pattern: a shared sign per harmonic across all cells is a bug even inside tolerance.
    for k in range(5):
        v = [r[2 + k] for r in resid if r[2 + k] is not None]
        if v:
            print(f"H{k + 2}: {len(v)} cells, mean {np.mean(v):+.2f} dB, "
                  f"{sum(x > 0 for x in v)} positive / {sum(x < 0 for x in v)} negative")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("atlas")
    p.add_argument("--sensor", required=True)
    p.add_argument("--cases", required=True)
    p.add_argument("--out", required=True)
    p = sub.add_parser("known")
    p.add_argument("--sensor", required=True)
    p.add_argument("--atlas", required=True)
    p.add_argument("--notes", default="36,48,60,72,84")
    a = ap.parse_args()
    atlas(a) if a.cmd == "atlas" else known(a)


if __name__ == "__main__":
    main()
