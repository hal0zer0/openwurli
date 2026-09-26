#!/usr/bin/env python3
"""Multi-velocity set: bark trend with velocity, REPORT-ONLY (a picture, not a score).

docs/research/pickup-overlap-law.md §9, operator ruling: the set is one hot-voiced, peak-normalised
instrument and not a calibration target. Per note and key-map zone: Sensor on one fixed early window
(0.2 s after onset, 0.639 s = 3 x the set's 0.213 s modulation period), fundamental pinned at the
true pitch; H2/H1 and H3/H1 against mapped zone order. Beside it, the model's same-window numbers
at the same notes (full chain, MLP off, speaker passthrough) at each zone's mid velocity. No
thresholds, no verdicts, no parameters move. Harmonics under the analyser's floor + 6 dB are blanked.

Usage: python tools/kw_trend.py --kw-dir DIR --zonemap zonemap.csv --sensor ANALYZE --out out.json
"""

import argparse
import csv
import json
import os
import subprocess
import tempfile
import wave
from multiprocessing import Pool

HERE = os.path.dirname(os.path.abspath(__file__))
BENCH = os.path.join(HERE, "..", "target", "release", "preamp-bench")
SEMI = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
T0, WIN = 0.2, 0.639


def midi_of(name):
    return 12 * (int(name[1:]) + 1) + SEMI[name[0]]


def sensor(an, path, skip_s, f0):
    fft = "16384" if f0 < 100 and not path.endswith("_22k.wav") else "8192"
    d = json.loads(subprocess.run([an, path, "--skip-ms", f"{skip_s * 1000:.1f}", "--window-ms",
                                   f"{WIN * 1000:.0f}", "--fft", fft, "--fundamental", f"{f0:.4f}",
                                   "--json"], check=True, capture_output=True, text=True).stdout)
    if not (d.get("supplied_fundamental") or {}).get("confirmed"):
        return None
    floor = d["tone"]["noise_floor_dbfs"]
    h = d["harmonics_dbfs"][:3]
    if h[0] is None:
        return None
    ok = [v is not None and (floor is None or v >= floor + 6) for v in h]
    return [None if not ok[k] else h[k] - h[0] for k in (1, 2)]


def rewrap(src, dst):
    """Byte-identical PCM in a standard header (the vendor fmt chunk is non-standard)."""
    with wave.open(src) as w:
        p, raw = w.getparams(), w.readframes(w.getnframes())
    with wave.open(dst, "wb") as w:
        w.setparams(p)
        w.writeframes(raw)
    return p.framerate, raw


def job(a):
    an, kw, row = a
    n = midi_of(row["true_pitch"])
    f0 = 440 * 2 ** ((n - 69) / 12)
    tmp = tempfile.mkdtemp(prefix="kwt_")
    rec = os.path.join(tmp, "rec_22k.wav" if row["sample_rate"] == "22050" else "rec.wav")
    sr, raw = rewrap(os.path.join(kw, row["file"]), rec)
    import numpy as np
    x = np.frombuffer(raw, np.int16).astype(float)
    onset = int(np.argmax(np.abs(x) > 0.1 * np.abs(x).max())) / sr
    kw_r = sensor(an, rec, onset + T0, f0)
    vel = (int(row["vel_lo"]) + int(row["vel_hi"])) // 2
    vel = max(vel, 1)
    mod = os.path.join(tmp, "model.wav")
    subprocess.run(["nice", "-n", "19", BENCH, "render", "--note", str(n), "--velocity", str(vel),
                    "--duration", f"{T0 + WIN + 0.2:.2f}", "--volume", "0.80", "--speaker", "0.0",
                    "--no-mlp", "--output", mod], check=True, capture_output=True)
    mod_r = sensor(an, mod, T0, f0)
    return {"note": row["true_pitch"], "midi": n, "zone": [int(row["vel_lo"]), int(row["vel_hi"])],
            "file": row["file"], "model_velocity": vel, "kw": kw_r, "model": mod_r}


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--kw-dir", required=True)
    ap.add_argument("--zonemap", required=True)
    ap.add_argument("--sensor", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--jobs", type=int, default=4)
    a = ap.parse_args()
    rows = [r for r in csv.DictReader(open(a.zonemap)) if r["is_release"] == "0"]
    with Pool(a.jobs) as pool:
        res = pool.map(job, [(a.sensor, a.kw_dir, r) for r in rows])
    res.sort(key=lambda r: (r["midi"], r["zone"][0]))
    json.dump(res, open(a.out, "w"), indent=1)
    fmt = lambda v: "   --" if v is None else f"{v:+5.1f}"  # noqa: E731
    last = None
    for r in res:
        if r["note"] != last:
            print(f"\n{r['note']} (MIDI {r['midi']})   zone     KW H2/H1 H3/H1 | model H2/H1 H3/H1")
            last = r["note"]
        k = r["kw"] or [None, None]
        m = r["model"] or [None, None]
        print(f"               {r['zone'][0]:3d}-{r['zone'][1]:3d}   {fmt(k[0])} {fmt(k[1])} | "
              f"      {fmt(m[0])} {fmt(m[1])}")


if __name__ == "__main__":
    main()
