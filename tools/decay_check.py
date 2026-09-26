#!/usr/bin/env python3
"""Criteria 1 and 7 at the reference recordings' TRUE pitch (docs/research/pickup-overlap-law.md §9).

Criterion 1 (decay, graded): the fundamental in 100 ms Hann windows starting 0.1/0.3/0.5/0.8/1.0/1.5 s
    after onset, at the true fundamental (assigned MIDI - 12, refined +-8 cents), log-linear fit in
    dB/s; the same code on the recording and on the model (full chain, v80, MLP off, speaker
    passthrough). Pass per note: within +-30 % of the recording's rate or +-1.5 dB/s, whichever is
    larger. A note is dropped (reported) if its recorded fundamental falls below the floor + 6 dB
    before 1.5 s (floor = median DTFT level at half-integer multiples of f0 in the same window). The
    Sensor Array's decay mode is run beside it on both sides and reported, not graded.
Criterion 7 (report-only): at true 38 and 42, v80, Sensor with the fundamental pinned, FFT 16384,
    600 ms from 50 ms after onset; model H2/H1 and H3/H1 against the recording's (a floor 6 dB below
    the recording's is the registered reading; chain EQ unbounded, so no verdict).

Usage: python tools/decay_check.py --sensor ANALYZE [--out out.json]
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
import swell_check as SW  # noqa: E402

BENCH = os.path.join(HERE, "..", "target", "release", "preamp-bench")
TIMES = (0.1, 0.3, 0.5, 0.8, 1.0, 1.5)
NOTES = tuple(range(38, 87, 4))


def dtft(seg, f, sr):
    n = np.arange(len(seg))
    return abs(np.sum(seg * np.hanning(len(seg)) * np.exp(-2j * np.pi * f * n / sr)))


def refine(seg, f, sr):
    fs = f * 2 ** (np.linspace(-8, 8, 161) / 1200)
    return fs[int(np.argmax([dtft(seg, c, sr) for c in fs]))]


def decay_rate(x, sr, onset, f_nom):
    f0 = refine(x[onset + int(0.1 * sr):onset + int(0.5 * sr)], f_nom, sr)
    lv, dropped = [], False
    for t in TIMES:
        w = x[onset + int(t * sr):onset + int((t + 0.1) * sr)]
        h1 = 20 * np.log10(dtft(w, f0, sr) + 1e-30)
        fl = 20 * np.log10(np.median([dtft(w, (k + 0.5) * f0, sr) for k in range(1, 8)]) + 1e-30)
        dropped |= h1 < fl + 6
        lv.append(h1)
    return float(np.polyfit(TIMES, lv, 1)[0]), dropped


def sensor_decay(an, path, onset_s, f0):
    out = subprocess.run([an, path, "--mode", "decay", "--skip-ms", f"{(onset_s + 0.1) * 1000:.0f}",
                          "--json"], capture_output=True, text=True)
    try:
        d = json.loads(out.stdout)
    except json.JSONDecodeError:
        return None
    parts = d.get("partials") or d.get("decay", {}).get("partials") or []
    best = None
    for p in parts:
        f, t60 = p.get("frequency_hz") or p.get("hz"), p.get("t60_s")
        if f and t60 and abs(f / f0 - 1) < 0.03:
            best = -60.0 / t60
    return best


def sensor_ratios(an, path, onset_s, f0):
    d = json.loads(subprocess.run([an, path, "--skip-ms", f"{(onset_s + 0.05) * 1000:.0f}",
                                   "--window-ms", "600", "--fft", "16384", "--fundamental",
                                   f"{f0:.4f}", "--json"], check=True, capture_output=True,
                                  text=True).stdout)
    h = d["harmonics_dbc"]
    return [h[1], h[2]]


def model_wav(n, tmp):
    p = os.path.join(tmp, f"m{n}.wav")
    subprocess.run(["nice", "-n", "19", BENCH, "render", "--note", str(n), "--velocity", "80",
                    "--duration", "2.0", "--volume", "0.80", "--speaker", "0.0", "--no-mlp",
                    "--output", p], check=True, capture_output=True)
    return p


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--sensor", required=True)
    ap.add_argument("--out", default=None)
    a = ap.parse_args()
    ref = {x["midi_note"] - 12: x for x in json.load(open(os.path.join(HERE, "..", "ml",
                                                                         "harmonics.json")))}
    tmp = tempfile.mkdtemp(prefix="decay_")
    res, npass, ngraded = {}, 0, 0
    print("true  rec dB/s  model dB/s  band        verdict   | Sensor decay rec / model")
    for n in NOTES:
        f0 = 440 * 2 ** ((n - 69) / 12)
        x, sr = SW.read_wav(ref[n]["source_file"])
        onset = int(np.argmax(np.abs(x) > 0.1 * np.abs(x).max()))
        rr, dropped = decay_rate(x, sr, onset, f0)
        mp = model_wav(n, tmp)
        y, msr = SW.read_wav(mp)
        mr, _ = decay_rate(y, msr, 0, f0)
        tol = max(0.3 * abs(rr), 1.5)
        v = "DROPPED" if dropped else ("PASS" if abs(mr - rr) <= tol else "FAIL")
        if not dropped:
            ngraded += 1
            npass += v == "PASS"
        sd = (sensor_decay(a.sensor, ref[n]["source_file"], onset / sr, f0),
              sensor_decay(a.sensor, mp, 0.0, f0))
        fmt = lambda z: "  --" if z is None else f"{z:+5.1f}"  # noqa: E731
        print(f"{n:4d}  {rr:+7.2f}   {mr:+7.2f}    [{rr - tol:+.1f},{rr + tol:+.1f}]  {v:8s}  | "
              f"{fmt(sd[0])} / {fmt(sd[1])}")
        res[n] = {"rec": rr, "model": mr, "tol": tol, "verdict": v, "sensor_decay": sd}
    print(f"CRITERION 1: {npass}/{ngraded} pass")
    print("\nCriterion 7 (report-only): true  rec H2/H1 H3/H1 | model H2/H1 H3/H1")
    for n in (38, 42):
        f0 = 440 * 2 ** ((n - 69) / 12)
        x, sr = SW.read_wav(ref[n]["source_file"])
        onset = int(np.argmax(np.abs(x) > 0.1 * np.abs(x).max())) / sr
        r = sensor_ratios(a.sensor, ref[n]["source_file"], onset, f0)
        m = sensor_ratios(a.sensor, model_wav(n, tmp), 0.0, f0)
        print(f"                            {n}   {r[0]:+6.1f} {r[1]:+6.1f} | {m[0]:+6.1f} {m[1]:+6.1f}")
        res[f"c7_{n}"] = {"rec": r, "model": m}
    if a.out:
        json.dump(res, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
