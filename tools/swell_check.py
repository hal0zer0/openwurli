#!/usr/bin/env python3
"""No-fit bass swell check (docs/research/pickup-overlap-law.md §9, registered before running).

Hypothesis: the real bass fundamental's slow swell is the pickup operating past its fundamental
peak as the swing decays through it. One extraction function (ENVELOPE) is applied to both the
reference recordings (at their true pitch, assigned MIDI - 12) and to model signals synthesised
from the field-solved law, so both sides come from the same code:

    ENVELOPE: Hann-windowed DFT, 200 ms windows starting 0.05 s after onset, hop 0.2 s; f0 refined
              once (search +-15 cents); each partial's level taken as the maximum of the DTFT within
              +-1/T of k*f0 (partials drift off exact multiples; Hann scalloping at half a bin costs
              1.4 dB); H1, H2 and RMS per window, in dB. Times are window STARTS on both sides.

Model: swing A(t) = A0 exp(-r t / 8.686) (A in mm at the finger region, r in dB/s), source = the
section's static law C(-g0 + A sin), through the pickup network's one-pole high-pass (PICKUP_FC).
Identify (A0, r) from the recorded swell SIZE and PEAK TIME only; every solution is kept. Predict
H2 and RMS change at the 2.05 s window, at the band ends of finger thickness, side gap and rest
offset (A0 and r re-solved at each). A cell passes if the recording lies in [min, max] of the
predictions widened by +-2 dB (H2) / +-1 dB (RMS).

Usage: python tools/swell_check.py [--tables JSON]   (see main() for the table map)
"""

import argparse
import json
import os
import sys
import wave

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from pickup_field2d import law_smooth  # noqa: E402

SR = 44100
FC = 880.5
WIN_S, HOP_S, START_S = 0.2, 0.2, 0.05
REPORT_T = 2.05
NOTES = (38, 42, 46, 50)


def envelope(x, sr, onset, f0_guess, t_max=3.7):
    """The one extraction used on both sides: returns (starts, H1 dB, H2 dB, RMS dB)."""
    def mag(seg, f):
        n = np.arange(len(seg))
        return abs(np.sum(seg * np.hanning(len(seg)) * np.exp(-2j * np.pi * f * n / sr)))
    seg = x[onset + int(0.1 * sr):onset + int(0.5 * sr)]
    cands = f0_guess * 2 ** (np.linspace(-15, 15, 301) / 1200)
    f0 = cands[int(np.argmax([mag(seg, c) for c in cands]))]
    starts = np.arange(START_S, t_max, HOP_S)
    h1, h2, rms = [], [], []
    def partial(w, k):  # max of the DTFT within +-1/T of k*f0
        span = 1.0 / WIN_S
        return max(mag(w, k * f0 + d) for d in np.linspace(-span, span, 21))
    for t in starts:
        w = x[onset + int(t * sr):onset + int((t + WIN_S) * sr)]
        h1.append(20 * np.log10(partial(w, 1) + 1e-30))
        h2.append(20 * np.log10(partial(w, 2) + 1e-30))
        rms.append(20 * np.log10(np.sqrt(np.mean(w ** 2)) + 1e-30))
    return starts, np.array(h1), np.array(h2), np.array(rms)


def read_wav(path):
    with wave.open(path) as w:
        sr, ch, sw = w.getframerate(), w.getnchannels(), w.getsampwidth()
        raw = w.readframes(w.getnframes())
    if sw == 2:
        x = np.frombuffer(raw, np.int16).astype(float) / 32768
    else:
        b = np.frombuffer(raw, np.uint8).reshape(-1, 3).astype(np.int32)
        v = b[:, 0] | (b[:, 1] << 8) | (b[:, 2] << 16)
        x = np.where(v >= 1 << 23, v - (1 << 24), v) / float(1 << 23)
    if ch > 1:
        x = x.reshape(-1, ch).mean(1)
    return x, sr


def summary(starts, h1, h2, rms):
    k = int(np.argmin(abs(starts - REPORT_T)))
    i = int(np.argmax(h1))
    return h1[i] - h1[0], starts[i], h2[k] - h2[0], rms[k] - rms[0]


def recording(midi_true):
    e = {x["midi_note"] - 12: x for x in json.load(open(os.path.join(HERE, "..", "ml",
                                                                        "harmonics.json")))}[midi_true]
    x, sr = read_wav(e["source_file"])
    onset = int(np.argmax(np.abs(x) > 0.1 * np.abs(x).max()))
    return summary(*envelope(x, sr, onset, 440 * 2 ** ((midi_true - 69) / 12)))


def synth(cs, f0, g0, A0, r, dur=3.8):
    t = np.arange(int(dur * SR)) / SR
    A = A0 * np.exp(-r * t / 8.686)
    s = cs(-g0 + A * np.sin(2 * np.pi * f0 * t))
    # One-pole high-pass, bilinear (as the engine's network): a backward-Euler form puts the
    # effective corner near 825 Hz and tilts H6 of C4 by -0.36 dB against the engine.
    w = np.tan(np.pi * FC / SR)
    b0, a1 = 1 / (1 + w), (1 - w) / (1 + w)
    y = np.empty_like(s)
    y[0] = 0.0
    for i in range(1, len(s)):
        y[i] = a1 * y[i - 1] + b0 * (s[i] - s[i - 1])
    return y


def identify(cs, f0, g0, S, T):
    """Fast static-envelope search for (A0, r) matching swell size and peak time."""
    th = np.arange(1024) * 2 * np.pi / 1024
    As = np.linspace(0.3, 3.2, 1200)
    w1 = f0 / np.hypot(f0, FC)
    h1 = np.array([20 * np.log10(abs(np.fft.rfft(cs(-g0 + A * np.sin(th)))[1]) * w1) for A in As])
    starts = np.arange(START_S, 3.7, HOP_S)
    sols = []
    for A0 in np.arange(0.8, 3.0, 0.01):
        for r in np.arange(0.2, 6.0, 0.02):
            e = np.interp(A0 * np.exp(-r * (starts + WIN_S / 2) / 8.686), As, h1)
            if abs((e.max() - e[0]) - S) <= 0.15 and abs(starts[int(np.argmax(e))] - T) <= 0.1:
                sols.append((round(A0, 2), round(r, 2)))
    return sols


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--cases", required=True, help="JSON: {row: {case: [table.json, key, g0]}}")
    a = ap.parse_args()
    cases = json.load(open(a.cases))
    verdict = []
    for n in NOTES:
        S, T, dh2, drms = recording(n)
        f0 = 440 * 2 ** ((n - 69) / 12)
        row = "r01_14" if n - 32 <= 14 else "r15_20"
        print(f"true {n} ({row}): recording swell {S:+.2f} dB @ {T:.2f} s, dH2 {dh2:+.1f}, dRMS {drms:+.1f}")
        preds = []
        for name, (path, key, g0) in cases[row].items():
            t = json.load(open(path))[key]
            cs = law_smooth(np.array(t["u"]), np.array(t["C_pF_per_m"]), row)
            sols = identify(cs, f0, g0, S, T)
            if not sols:
                print(f"   {name:16s}: no (A0, r) reproduces the swell")
                continue
            # verify a spread of the solutions through the SAME extraction code
            pick = sols[:: max(1, len(sols) // 6)]
            got = []
            for A0, r in pick:
                s2 = summary(*envelope(synth(cs, f0, g0, A0, r), SR, 0, f0))
                got.append((A0, r) + s2)
            g = np.array(got)
            preds += [row_[4:] for row_ in got]
            print(f"   {name:16s}: {len(sols)} sols, A0 {g[:,0].min():.2f}-{g[:,0].max():.2f} mm, "
                  f"r {g[:,1].min():.2f}-{g[:,1].max():.2f} dB/s | synth swell {g[:,2].min():+.1f}.."
                  f"{g[:,2].max():+.1f} | dH2 {g[:,4].min():+.1f}..{g[:,4].max():+.1f} | dRMS "
                  f"{g[:,5].min():+.1f}..{g[:,5].max():+.1f}")
        if not preds:
            verdict += [("H2", n, "NO SOLUTION"), ("RMS", n, "NO SOLUTION")]
            continue
        p = np.array(preds)
        for lab, col, rv, tol in (("H2", 0, dh2, 2.0), ("RMS", 1, drms, 1.0)):
            lo, hi = p[:, col].min() - tol, p[:, col].max() + tol
            v = "PASS" if lo <= rv <= hi else ("FAIL model-high" if rv < lo else "FAIL model-low")
            verdict.append((lab, n, v))
            print(f"   CELL {lab}: recording {rv:+.1f} in [{lo:+.1f}, {hi:+.1f}] -> {v}")
    npass = sum(v == "PASS" for _, _, v in verdict)
    print(f"VERDICT: {npass}/8 pass; misses: {[f'{l}@{n}: {v}' for l, n, v in verdict if v != 'PASS']}")


if __name__ == "__main__":
    main()
