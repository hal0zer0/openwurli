#!/usr/bin/env python3
"""Multi-velocity reference: per-layer swing identification (IDENTIFICATION half only).

docs/research/pickup-overlap-law.md §9 ("Multi-velocity reference") and its registered procedure:

  Split:     identification notes only (every other white note from true A1). The held-out notes
             are refused here; they are scored once, after the model is frozen.
  Windows:   Sensor Array, fundamental pinned at the true pitch (file labels run one octave below),
             window 0.639 s (3 x 0.213 s, the ~9.4 Hz modulation period;
             0.426 s gives no FFT-8192 frame at 22.05 kHz), hop 0.213 s, from 0.2 s
             after onset to at least 0.3 s before the end of the usable file (a detected fade-out
             ends it early). FFT 16384 below ~100 Hz at 44.1 kHz, else 8192.
  Unknowns:  (A0, r) of a pure exponentially decaying swing through the section's static law.
             r is an EFFECTIVE decay over this file's window span (a fit statistic), not the reed's
             mode-1 constant. A layer SWELLS if its H1 peak is not in the first window and rises
             >= 0.5 dB above it (above the ~0.2-0.4 dB modulation).
               swelling:     identify from H1 swell size + peak time; out of sample: first->last
                             d(H2/H1) and d(H3/H1).
               non-swelling: identify from first->last dH1 and d(H2/H1); out of sample d(H3/H1).
             Search on the static law (fast), verify on synthesised audio with the same Sensor
             settings over THIS file's own span. Accept: every verified solution within +-2 dB on
             every out-of-sample trajectory. Mixed = INDETERMINATE; none = REJECTED.
  Confirm:   each accepted layer through the engine's full chain at the identified (A0, r)
             (`preamp-bench render --swing-mm --decay-db-s`, MLP off, speaker passthrough); it must
             reproduce all four observables within +-2 dB, else CHAIN-REJECTED (reported apart
             from identification failures: that is where a chain stopped being linear).
  Span:      per note, A0 of the hardest accepted layer over the softest (a lower bound on the
             instrument's span); re-identified at the g0 / finger-thickness / side-gap band ends on
             the law-only path; admissible where it moves <= +-15 % across them.

Usage: python tools/kw_identify.py --kw DIR --sensor ANALYZE --cases cases.json --out out.json
       [--notes A1,C2] [--jobs 4]
"""

import argparse
import glob
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import wave
from multiprocessing import Pool

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from pickup_field2d import gcv_smooth  # noqa: E402
from steepness_check import write_wav  # noqa: E402
import swell_check as SW  # noqa: E402

IDENT = ["A1", "C2", "E2", "G2", "B2", "D3", "F3", "A3", "C4", "E4", "G4", "B4", "D5", "F5", "A5",
         "C6", "E6", "G6", "B6"]
HELD_OUT = ["B1", "D2", "F2", "A2", "C3", "E3", "G3", "B3", "D4", "F4", "A4", "C5", "E5", "G5", "B5",
            "D6", "F6", "A6", "C7"]
PERIOD = 0.213
WIN, HOP, START, TAIL = 3 * PERIOD, PERIOD, 0.2, 0.3
TOL = 2.0
SWELL_MIN_DB = 0.5
SEMI = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
BENCH = os.path.join(HERE, "..", "target", "release", "preamp-bench")
SR_SYN = 44100


def midi_of(name):
    return 12 * (int(name[1]) + 1) + SEMI[name[0]]


def section_of(n):
    reed = n - 32
    return "r01_14" if reed <= 14 else "r15_20" if reed <= 20 else "r21_42" if reed <= 42 else "r43_64"


def files_for(kw, true_name):
    if true_name in HELD_OUT:
        raise SystemExit(f"{true_name} is HELD OUT until the model is frozen")
    label = f"{true_name[0].lower()}{int(true_name[1]) - 1}"
    out = {}
    for p in glob.glob(os.path.join(kw, "*.wav")):
        m = re.match(r".*/Wurlitzer 200a [A-Z0-9]+_" + label + r"([0-9a-f]{2})0000\.wav$", p)
        if m:
            out[int(m.group(1), 16)] = p
    return dict(sorted(out.items()))


def read_wav(path):
    with wave.open(path) as w:
        sr, ch, sw = w.getframerate(), w.getnchannels(), w.getsampwidth()
        raw = w.readframes(w.getnframes())
    x = np.frombuffer(raw, np.int16).astype(float) / 32768 if sw == 2 else SW.read_wav(path)[0]
    return (x.reshape(-1, ch).mean(1) if ch > 1 else x), sr


def usable_end(x, sr):
    """End of the usable file: its length, or the start of a fade-out (last 0.6 s falling
    > 10 dB/s faster than the 1.0 s before it)."""
    b = int(0.1 * sr)
    env = np.array([20 * np.log10(np.sqrt(np.mean(x[i:i + b] ** 2)) + 1e-12)
                    for i in range(0, len(x) - b, b)])
    t = np.arange(len(env)) * 0.1
    if len(env) < 20:
        return len(x) / sr, False
    late = np.polyfit(t[-6:], env[-6:], 1)[0]
    early = np.polyfit(t[-16:-6], env[-16:-6], 1)[0]
    if late < early - 10.0:
        return t[-6], True
    return len(x) / sr, False


def fft_for(f0, sr):
    return "16384" if (sr > 30000 and f0 < 100) else "8192"


def sensor(an, path, t0, f0, sr):
    d = json.loads(subprocess.run([an, path, "--skip-ms", f"{t0 * 1000:.1f}", "--window-ms",
                                   f"{WIN * 1000:.0f}", "--fft", fft_for(f0, sr), "--fundamental",
                                   f"{f0:.4f}", "--json"], check=True, capture_output=True,
                                  text=True).stdout)
    sf = d.get("supplied_fundamental") or {}
    h = d["harmonics_dbfs"][:3]
    return np.array(h, dtype=float) if sf.get("confirmed") and None not in h else None


def trajectory(an, path, onset, starts, f0, sr):
    """Rows up to the last window where the fundamental is confirmed and H1..H3 are all reported
    (the analyser withholds what it cannot measure). Returns (rows, starts used) or (None, None)."""
    rows = []
    for s in starts:
        r = sensor(an, path, onset + s, f0, sr)
        if r is None:
            break
        rows.append(r)
    if len(rows) < 3:
        return None, None
    return np.array(rows), starts[:len(rows)]


def summarise(tr, starts):
    h1, r2, r3 = tr[:, 0], tr[:, 1] - tr[:, 0], tr[:, 2] - tr[:, 0]
    i = int(np.argmax(h1))
    return {"S": float(h1[i] - h1[0]), "T": float(starts[i]), "dH1": float(h1[-1] - h1[0]),
            "dR2": float(r2[-1] - r2[0]), "dR3": float(r3[-1] - r3[0])}


def static_map(cs, g0, amax):
    th = np.arange(1024) * 2 * np.pi / 1024
    As = np.exp(np.linspace(np.log(0.02), np.log(amax), 1200))
    X = np.array([abs(np.fft.rfft(cs(-g0 + A * np.sin(th)))[1:3]) for A in As])
    return As, 20 * np.log10(X[:, 0]), 20 * np.log10(X[:, 1] / X[:, 0])


def identify(cs, g0, amax, obs, starts, swelling):
    As, h1, r2 = static_map(cs, g0, amax)
    cen = starts + WIN / 2
    sols = []
    for A0 in np.exp(np.linspace(np.log(0.05), np.log(amax), 300)):
        for r in np.arange(0.1, 40.0, 0.1):
            A = A0 * np.exp(-r * cen / 8.686)
            if A[-1] < As[0]:
                break
            e1 = np.interp(A, As, h1)
            if swelling:
                i = int(np.argmax(e1))
                ok = abs((e1[i] - e1[0]) - obs["S"]) <= 0.15 and abs(starts[i] - obs["T"]) <= HOP / 2
            else:
                e2 = np.interp(A, As, r2)
                ok = abs((e1[-1] - e1[0]) - obs["dH1"]) <= 0.15 and \
                    abs((e2[-1] - e2[0]) - obs["dR2"]) <= 0.15
            if ok:
                sols.append((float(A0), float(r)))
    return sols


def out_of_sample(swelling):
    return ("dR2", "dR3") if swelling else ("dR3",)


def verify(an, cs, g0, sols, obs, starts, f0, swelling, tmp, tag):
    """Synthesise a spread of solutions, measure with Sensor over the file's own span."""
    if not sols:
        return "NO SOLUTION", []
    pick = sorted(sols)
    pick = [pick[0], pick[len(pick) // 2], pick[-1]] if len(pick) > 2 else pick
    res = []
    for A0, r in pick:
        p = os.path.join(tmp, f"{tag}_{A0:.4f}_{r:.2f}.wav")
        write_wav(p, SW.synth(cs, f0, g0, A0, r, dur=starts[-1] + WIN + 0.1))
        tr, st = trajectory(an, p, 0.0, starts, f0, SR_SYN)
        os.remove(p)
        if tr is None or len(st) != len(starts):
            continue
        m = summarise(tr, st)
        m.update(A0=A0, r=r, ok=all(abs(m[k] - obs[k]) <= TOL for k in out_of_sample(swelling)))
        res.append(m)
    if not res:
        return "NO SOLUTION", []
    oks = [m["ok"] for m in res]
    return ("ACCEPTED" if all(oks) else "REJECTED" if not any(oks) else "INDETERMINATE"), res


def mode1_scale(n):
    """Mode-1 amplitude at onset per mm of --swing-mm at v127 (reed tap), to set A0 exactly."""
    tmp = tempfile.mkdtemp(prefix="kwscale_")
    pre = os.path.join(tmp, "t")
    subprocess.run([BENCH, "reed-tap", "--note", str(n), "--velocity", "127", "--duration", "0.3",
                    "--swing-mm", "1.0", "--decay-db-s", "0", "--output-prefix", pre], check=True, capture_output=True)
    m1 = SW.read_wav(f"{pre}_mode1_mm.wav")[0] * 10.0
    return float(np.abs(m1[int(0.02 * SR_SYN):int(0.08 * SR_SYN)]).max())


def confirm(an, n, A0, r, obs, starts, f0, scale, tmp):
    p = os.path.join(tmp, f"full{n}_{A0:.4f}.wav")
    subprocess.run([BENCH, "render", "--note", str(n), "--velocity", "127", "--duration",
                    f"{starts[-1] + WIN + 0.1:.3f}", "--volume", "0.80", "--speaker", "0.0",
                    "--no-mlp", "--swing-mm", f"{A0 / scale:.5f}", "--decay-db-s", f"{r:.3f}",
                    "--output", p], check=True, capture_output=True)
    tr, st = trajectory(an, p, 0.0, starts, f0, SR_SYN)
    os.remove(p)
    if tr is None or len(st) != len(starts):
        return False, None
    m = summarise(tr, st)
    keys = ("S", "dH1", "dR2", "dR3")
    return all(abs(m[k] - obs[k]) <= TOL for k in keys), m


def job(args):
    an, name, layer, src, cases = args
    path = src
    n = midi_of(name)
    f0 = 440 * 2 ** ((n - 69) / 12)
    sec = section_of(n)
    x, sr = read_wav(path)
    # The vendor files carry a non-standard fmt chunk the analyser's reader refuses: analyse a
    # lossless re-wrap (same 16-bit PCM, same rate).
    # The data chunk is copied byte for byte; both chunks are hashed and recorded.
    tmp = tempfile.mkdtemp(prefix="kw_")
    clean = os.path.join(tmp, "clean.wav")
    with wave.open(src) as w:
        params, raw = w.getparams(), w.readframes(w.getnframes())
    with wave.open(clean, "wb") as w:
        w.setparams(params)
        w.writeframes(raw)
    with wave.open(clean) as w:
        raw2 = w.readframes(w.getnframes())
    pcm_hash = [hashlib.sha256(raw).hexdigest(), hashlib.sha256(raw2).hexdigest()]
    if pcm_hash[0] != pcm_hash[1]:
        raise SystemExit(f"re-wrap of {src} is not lossless")
    path = clean
    onset = int(np.argmax(np.abs(x) > 0.1 * np.abs(x).max())) / sr
    end, faded = usable_end(x, sr)
    starts = np.arange(START, end - onset - TAIL - WIN + 1e-9, HOP)
    out = {"note": name, "midi": n, "layer": layer, "file": os.path.basename(src), "sr": sr,
           "pcm_sha256_original_rewrap": pcm_hash,
           "faded": faded, "span_s": [float(starts[0]), float(starts[-1] + WIN)] if len(starts) else None}
    if len(starts) < 3:
        out["status"] = "TOO SHORT"
        return out
    tr, starts = trajectory(an, path, onset, starts, f0, sr)
    if tr is None:
        out["status"] = "SENSOR REFUSED / TOO SHORT"
        return out
    out["span_s"] = [float(starts[0]), float(starts[-1] + WIN)]
    obs = summarise(tr, starts)
    swelling = obs["T"] > starts[0] and obs["S"] >= SWELL_MIN_DB
    out.update(obs=obs, swelling=bool(swelling))
    out["cases"] = {}
    for cname, (tpath, key, g0) in cases[sec].items():
        t = json.load(open(tpath))[key]
        u = np.array(t["u"])
        cs, _ = gcv_smooth(u, np.array(t["C_pF_per_m"]))
        amax = min(u.max() + g0, -u.min() - g0) - 0.02
        sols = identify(cs, g0, amax, obs, starts, swelling)
        st, res = verify(an, cs, g0, sols, obs, starts, f0, swelling, tmp, f"{n}_{layer}_{cname}")
        out["cases"][cname] = {"status": st, "n_sols": len(sols), "amax": amax, "verified": res}
        if cname == "central" and st != "ACCEPTED":
            break
    c = out["cases"].get("central", {})
    out["status"] = c.get("status", "NO CENTRAL")
    if out["status"] == "ACCEPTED":
        A0 = float(np.median([m["A0"] for m in c["verified"]]))
        r = float(np.median([m["r"] for m in c["verified"]]))
        ok, m = confirm(an, n, A0, r, obs, starts, f0, mode1_scale(n), tmp)
        out["confirm"] = {"ok": ok, "A0": A0, "r": r, "full": m}
        if not ok:
            out["status"] = "CHAIN-REJECTED"
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--kw", required=True)
    ap.add_argument("--sensor", required=True)
    ap.add_argument("--cases", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--notes", default=",".join(IDENT))
    ap.add_argument("--jobs", type=int, default=4)
    a = ap.parse_args()
    cases = json.load(open(a.cases))
    work = []
    for name in a.notes.split(","):
        for layer, path in files_for(a.kw, name).items():
            work.append((a.sensor, name, layer, path, cases))
    res = []
    with Pool(a.jobs) as pool:
        for r in pool.imap_unordered(job, work):
            res.append(r)
            o = r.get("obs", {})
            conf = r.get("confirm", {})
            sp = r.get("span_s") or [0, 0]
            print(f"{r['note']} L{r['layer']:3d} span {sp[1] - sp[0]:.2f}s: {r['status']:15s} "
                  f"swell {r.get('swelling')} "
                  f"S {o.get('S', 0):+.2f} T {o.get('T', 0):.2f} dH1 {o.get('dH1', 0):+.2f} "
                  f"dR2 {o.get('dR2', 0):+.2f} dR3 {o.get('dR3', 0):+.2f} | A0 {conf.get('A0', 0):.3f} "
                  f"r {conf.get('r', 0):.2f}{' FADE' if r.get('faded') else ''}", flush=True)
            json.dump(res, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
