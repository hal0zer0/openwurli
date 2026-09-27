#!/usr/bin/env python3
"""Dynamic admitted edge per knot: the largest ff swing passing criteria 2 and 6.

docs/research/pickup-overlap-law.md §8-§9, as re-registered 2026-09-26:

  Criterion 2 (ff falloff). Verdict at the FULL CHAIN: RMS in 100 ms windows starting 0.1..2.0 s;
      pass if no window rises more than 0.5 dB above the one before it AND the total fall (first
      window to last) is at least half the reed's own mode-1 decay over the same windows. The
      threshold is MEASURED at the reed tap (`preamp-bench reed-tap`, mode-1 displacement, RMS in
      the same two windows); the computed value (linear decay law + air drag) is printed beside it.
  Criterion 6 (Andersen, bass swell). Verdict at the PICKUP tap (the raw law source before its RC
      network): the fundamental, in 100 ms windows from onset to 2.0 s, may rise no more than 6 dB
      above its first window. No timing clause. The full-chain value is printed beside it.

Render settings are those of the edges they replace (v127, 2.5 s, volume 0.80, MLP off, bench
default speaker). Both taps use the bench's per-note seed. The edge is bisected from [0.4, cap],
nine halvings, assuming pass-below / fail-above; every probe is printed so a non-monotone case
shows.

With --no-crit6 only criterion 2 decides (the operator's variant A, criterion 6 report-only);
criterion 6's rise is still printed for every probe.

Usage: python tools/swing_edges.py --notes 33,36 --cap 3.5 [--work DIR] [--out edges.json] [--no-crit6]
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

SR = 44100
BENCH = os.path.join(HERE, "..", "target", "release", "preamp-bench")
SWELL_MAX_DB = 6.0


def run(args):
    subprocess.run(["nice", "-n", "19", BENCH, *args], check=True, capture_output=True)


def windows_db(x, starts, fn):
    return np.array([fn(x[int(a * SR):int((a + 0.1) * SR)]) for a in starts])


def rms(seg):
    return 20 * np.log10(np.sqrt(np.mean(seg ** 2)) + 1e-300)


def computed_reed_decay_db(n, A):
    """Mode-1 decay over 0.1 -> 2.0 s from the formula in force (linear law + air drag)."""
    beta = BETA[n]
    f = M.midi_hz(n)
    a_ = max(0.005 * f ** 1.22, 3.0) / 8.686
    b_ = beta / 8.686
    r, amp = A, {}
    for i in range(2101):
        if i in (150, 2050):  # window centres
            amp[i] = r
        r *= np.exp(-(a_ + b_ * r) * 0.001)
    return 20 * np.log10(amp[150] / amp[2050])


# Air-drag coefficient per knot (dB/s per mm), as used by the edges this tool replaces.
BETA = {33: 0.259, 36: 0.356, 39: 0.485, 42: 0.651, 45: 0.863, 48: 1.125, 51: 1.417, 52: 1.519,
        53: 0.664, 54: 0.756, 57: 1.114, 60: 1.633, 63: 2.381, 66: 3.446, 69: 4.863, 72: 5.921,
        75: 7.116, 78: 8.562, 81: 10.32, 84: 12.45, 87: 15.05, 90: 18.24, 93: 22.16, 96: 27.02}


def checks(n, A, work):
    full = os.path.join(work, f"n{n}_{A:.4f}.wav")
    run(["render", "--note", str(n), "--velocity", "127", "--duration", "2.5", "--volume", "0.80",
         "--no-mlp", "--swing-mm", f"{A}", "--output", full])
    tap = os.path.join(work, f"tap{n}_{A:.4f}")
    run(["reed-tap", "--note", str(n), "--velocity", "127", "--duration", "2.5",
         "--swing-mm", f"{A}", "--output-prefix", tap])
    x = M.read_wav(full)
    m1 = M.read_wav(f"{tap}_mode1_mm.wav")
    src = M.read_wav(f"{tap}_pickup_source.wav")

    rs = np.arange(0.1, 2.05, 0.1)
    w = windows_db(x, rs, rms)
    reed = windows_db(m1, rs, rms)
    need = 0.5 * (reed[0] - reed[-1])
    fall, jump = w[0] - w[-1], float(np.max(np.diff(w)))
    c2 = fall >= need and jump <= 0.5

    hs = np.arange(0.0, 2.05, 0.1)
    f0 = M.refine_f0(src[int(0.1 * SR):int(0.4 * SR)], M.midi_hz(n))
    h_pick = windows_db(src, hs, lambda s: 20 * np.log10(M.tone_mag(s, f0) + 1e-300))
    f0x = M.refine_f0(x[int(0.1 * SR):int(0.4 * SR)], M.midi_hz(n))
    h_full = windows_db(x, hs, lambda s: 20 * np.log10(M.tone_mag(s, f0x) + 1e-300))
    rise = float(h_pick.max() - h_pick[0])
    c6 = rise <= SWELL_MAX_DB
    for p in (full, *(f"{tap}_{k}.wav" for k in ("total_mm", "mode1_mm", "mode2_mm",
                                                 "modes3to7_mm", "pickup_source"))):
        os.remove(p)
    return {"A": A, "c2": bool(c2), "c6": bool(c6), "fall": fall, "need_measured": need,
            "need_computed": 0.5 * computed_reed_decay_db(n, A), "max_jump": jump,
            "rise_pickup": rise, "t_peak_pickup": float(hs[int(np.argmax(h_pick))]),
            "rise_full": float(h_full.max() - h_full[0])}


def edge(n, lo, hi, work, use_c6=True):
    probes = []
    for _ in range(9):
        mid = 0.5 * (lo + hi)
        r = checks(n, mid, work)
        probes.append(r)
        print(f"  MIDI {n} A {mid:.3f}: crit2 {r['c2']} (fall {r['fall']:.2f} need "
              f"{r['need_measured']:.2f} [computed {r['need_computed']:.2f}], jump "
              f"{r['max_jump']:+.2f}) crit6 {r['c6']} (pickup {r['rise_pickup']:+.2f} dB @ "
              f"{r['t_peak_pickup']:.1f} s, full {r['rise_full']:+.2f})", flush=True)
        lo, hi = (mid, hi) if (r["c2"] and (r["c6"] or not use_c6)) else (lo, mid)
    return lo, probes


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--notes", required=True)
    ap.add_argument("--cap", type=float, required=True, help="upper end of the search (mm)")
    ap.add_argument("--work", default=None)
    ap.add_argument("--out", default=None)
    ap.add_argument("--no-crit6", action="store_true",
                    help="criterion 2 only (variant A: criterion 6 report-only)")
    a = ap.parse_args()
    work = a.work or tempfile.mkdtemp(prefix="swing_edges_")
    os.makedirs(work, exist_ok=True)
    res = {}
    for n in (int(v) for v in a.notes.split(",")):
        e, probes = edge(n, 0.4, a.cap, work, use_c6=not a.no_crit6)
        cap_hit = e > a.cap - 0.01
        print(f"EDGE MIDI {n}: {e:.3f} mm{' (>= search cap)' if cap_hit else ''}", flush=True)
        res[n] = {"edge": e, "cap": a.cap, "cap_hit": cap_hit, "probes": probes}
    if a.out:
        json.dump(res, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
