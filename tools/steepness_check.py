#!/usr/bin/env python3
"""Steepness consistency check (docs/research/pickup-overlap-law.md §9, as re-registered).

Scale-free test of the law's knee: identify the swing from self-normalised trajectories only, then
predict the change of H3/H1 over the post-attack interval and compare with the recording.

    Windows:  Sensor Array, 600 ms starting 0.7 s and 1.3 s after onset, fundamental pinned at the
              TRUE pitch (assigned MIDI - 12), FFT 16384 below ~100 Hz else 8192. Same settings on
              the recording and on the synthesised model audio.
    Model:    a pure exponentially decaying swing A0 exp(-r t / 8.686) through the section's static
              law and the pickup's one-pole high-pass (tools/swell_check.py `synth`).
    Identify: swelling notes (38, 50) reuse the swell check's (A0, r) solution set (swell size and
              peak time). Non-swelling notes (62, 78) solve dH1 and d(H2/H1) over 0.7 -> 1.3 s (two
              observables, two unknowns); every solution is kept.
    Predict:  d(H3/H1) over 0.7 -> 1.3 s, at every band-end case (A0, r re-identified at each).
    Pass:     recording inside [min, max] of the predictions widened by +-2 dB. A note whose
              predicted H3/H1 falls below -35 dB re H1 at either window (middle/treble) is
              table-limited: reported, not graded. A note without its finger-thickness and side-gap
              band ends is "partial band" and not graded.

Consistency check only: it cannot select the side gap.

Usage: python tools/steepness_check.py --sensor PATH/TO/analyze --cases cases.json
       cases.json: {row: {case: [table.json, key, g0]}} for rows r01_14, r15_20, r21_42, r43_64
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile
import wave

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import swell_check as SW  # noqa: E402
from pickup_field2d import gcv_smooth  # noqa: E402

SR = 44100
T_A, T_B, WIN_MS = 0.7, 1.3, 600
NOTES = {38: "r01_14", 50: "r15_20", 62: "r21_42", 78: "r43_64"}
SWELLING = (38, 50)
TOL = 2.0
TABLE_LIMIT_DB = -35.0
BAND_KEYS = ("tp", "gs")


def f_true(n):
    return 440 * 2 ** ((n - 69) / 12)


def sensor_h123(sensor, path, onset_s, t0, f0):
    fft = "16384" if f0 < 100 else "8192"
    out = subprocess.run([sensor, path, "--skip-ms", f"{(onset_s + t0) * 1000:.1f}", "--window-ms",
                          str(WIN_MS), "--fft", fft, "--fundamental", f"{f0:.4f}", "--json"],
                         check=True, capture_output=True, text=True).stdout
    d = json.loads(out)
    sf = d.get("supplied_fundamental") or {}
    if not sf.get("confirmed"):
        raise SystemExit(f"Sensor did not confirm the fundamental in {path} at {t0} s: {sf}")
    return np.array(d["harmonics_dbfs"][:3])


def observables(sensor, path, onset_s, f0):
    a = sensor_h123(sensor, path, onset_s, T_A, f0)
    b = sensor_h123(sensor, path, onset_s, T_B, f0)
    return {"dH1": b[0] - a[0], "dR2": (b[1] - b[0]) - (a[1] - a[0]),
            "dR3": (b[2] - b[0]) - (a[2] - a[0]), "R3a": a[2] - a[0], "R3b": b[2] - b[0]}


def write_wav(path, y):
    y = y / (np.abs(y).max() * 1.05)
    v = np.round(y * (2 ** 23 - 1)).astype(np.int32)
    b = np.stack([v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF], axis=1).astype(np.uint8)
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(3)
        w.setframerate(SR)
        w.writeframes(b.tobytes())


def identify_decay(cs, f0, g0, dH1, dR2):
    """(A0, r) whose static-law trajectory reproduces dH1 and d(H2/H1) over the two windows."""
    th = np.arange(1024) * 2 * np.pi / 1024
    As = np.exp(np.linspace(np.log(0.02), np.log(3.2), 1500))
    X = np.array([abs(np.fft.rfft(cs(-g0 + A * np.sin(th)))[1:3]) for A in As])
    h1, r2 = 20 * np.log10(X[:, 0]), 20 * np.log10(X[:, 1] / X[:, 0])
    ca, cb = T_A + WIN_MS / 2000, T_B + WIN_MS / 2000
    sols = []
    for A0 in np.exp(np.linspace(np.log(0.05), np.log(3.0), 400)):
        for r in np.arange(0.2, 40.0, 0.1):
            Aa, Ab = A0 * np.exp(-r * ca / 8.686), A0 * np.exp(-r * cb / 8.686)
            if Ab < As[0]:
                continue
            e1 = np.interp(Ab, As, h1) - np.interp(Aa, As, h1)
            e2 = np.interp(Ab, As, r2) - np.interp(Aa, As, r2)
            if abs(e1 - dH1) <= 0.15 and abs(e2 - dR2) <= 0.15:
                sols.append((round(float(A0), 3), round(float(r), 2)))
    return sols


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--sensor", required=True, help="path to the Sensor Array `analyze` binary")
    ap.add_argument("--cases", required=True)
    ap.add_argument("--notes", default="38,50,62,78")
    a = ap.parse_args()
    cases = json.load(open(a.cases))
    tmp = tempfile.mkdtemp(prefix="steepness_")
    hjson = {x["midi_note"] - 12: x for x in json.load(open(os.path.join(HERE, "..", "ml",
                                                                          "harmonics.json")))}
    for n in (int(v) for v in a.notes.split(",")):
        row, f0 = NOTES[n], f_true(n)
        x, sr = SW.read_wav(hjson[n]["source_file"])
        onset = int(np.argmax(np.abs(x) > 0.1 * np.abs(x).max())) / sr
        rec = observables(a.sensor, hjson[n]["source_file"], onset, f0)
        print(f"true {n} ({row}): recording dH1 {rec['dH1']:+.2f} dR2 {rec['dR2']:+.2f} "
              f"dR3 {rec['dR3']:+.2f} (H3/H1 {rec['R3a']:+.1f} -> {rec['R3b']:+.1f})", flush=True)
        if n in SWELLING:
            S, T, _, _ = SW.recording(n)
        preds, limited = [], False
        for name, (path, key, g0) in cases[row].items():
            t = json.load(open(path))[key]
            cs, _ = gcv_smooth(np.array(t["u"]), np.array(t["C_pF_per_m"]))
            sols = SW.identify(cs, f0, g0, S, T) if n in SWELLING else \
                identify_decay(cs, f0, g0, rec["dH1"], rec["dR2"])
            if not sols:
                print(f"   {name:10s}: no (A0, r) solution", flush=True)
                continue
            got = []
            for A0, r in sols[:: max(1, len(sols) // 6)]:
                p = os.path.join(tmp, f"{n}_{name}_{A0}_{r}.wav")
                write_wav(p, SW.synth(cs, f0, g0, A0, r, dur=2.2))
                m = observables(a.sensor, p, 0.0, f0)
                got.append((A0, r, m["dH1"], m["dR2"], m["dR3"], m["R3a"], m["R3b"]))
                os.remove(p)
            g = np.array(got)
            preds += list(g[:, 4])
            lim = n >= 53 and min(g[:, 5].min(), g[:, 6].min()) < TABLE_LIMIT_DB
            limited |= lim
            print(f"   {name:10s}: {len(sols)} sols, A0 {g[:,0].min():.3f}-{g[:,0].max():.3f} mm, "
                  f"r {g[:,1].min():.2f}-{g[:,1].max():.2f} dB/s | verified dH1 {g[:,2].min():+.2f}.."
                  f"{g[:,2].max():+.2f} dR2 {g[:,3].min():+.2f}..{g[:,3].max():+.2f} | pred dR3 "
                  f"{g[:,4].min():+.2f}..{g[:,4].max():+.2f} (H3/H1 at 0.7 s {g[:,5].min():+.1f}.."
                  f"{g[:,5].max():+.1f}, at 1.3 s {g[:,6].min():+.1f}..{g[:,6].max():+.1f})"
                  f"{' TABLE-LIMITED' if lim else ''}", flush=True)
        partial = not all(any(c.startswith(b) for c in cases[row]) for b in BAND_KEYS)
        if not preds:
            print("   VERDICT: NO SOLUTION")
            continue
        lo, hi = min(preds) - TOL, max(preds) + TOL
        inside = lo <= rec["dR3"] <= hi
        tag = "PASS" if inside else ("FAIL model-high" if rec["dR3"] < lo else "FAIL model-low")
        if limited:
            tag = f"TABLE-LIMITED (not graded; would read {tag})"
        elif partial:
            tag = f"PARTIAL BAND (not graded; would read {tag})"
        print(f"   VERDICT: recording {rec['dR3']:+.2f} in [{lo:+.2f}, {hi:+.2f}] -> {tag}", flush=True)


if __name__ == "__main__":
    main()
