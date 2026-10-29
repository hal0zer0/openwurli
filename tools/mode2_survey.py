#!/usr/bin/env python3
"""Survey the reed's vertical mode-2 line across a multi-sample 200A set, with the Sensor Array.

For each note, the FE beam (reed_fe.py via hammer_contact_eval.py) gives the mode-2 ratio band
over the tip/blank variants, widened by --widen for per-reed scatter (length, taper and solder
vary note to note). The Sensor's decay mode lists spectral candidates. A candidate counts as the
mode-2 line if all of these hold:
- it lies inside the widened band;
- it clears every harmonic of the measured H1 by --clear Hz;
- it clears the sympathetic lines (the undamped top reeds, MIDI 92-96, and their 2nd harmonics);
- its prominence is >= --prom dB.
The strongest such candidate is reported.

Measurements (both with the Sensor's decay mode):
- onset: skip 5 ms, window 900 ms, band +-40 Hz (the fit starts ~85 ms); level re H1 there;
- late: skip 300 ms, window 2500 ms, band +-20 Hz; level re H1, or absence.

Sample naming (Keyboard Waves EXS set): "<prefix>_<note><octave><vv>0000.wav". The labels sit ONE
OCTAVE BELOW scientific pitch (label a0 = A1, MIDI 33).

Usage:
    python tools/mode2_survey.py --samples DIR --sensor ANALYZE [--layer 7e] [--out survey.json]
"""

import argparse
import glob
import json
import os
import subprocess
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hammer_contact import variants  # noqa: E402
from hammer_contact_eval import reed  # noqa: E402

PC = dict(c=0, d=2, e=4, f=5, g=7, a=9, b=11)


def label_to_midi(note, octave):
    return 12 * (octave + 2) + PC[note]


def fe_band(midi):
    rs = []
    for _, t_tip, t_root in variants(midi):
        r = reed(midi, t_tip, t_root, nm=3)
        rs.append(r["om"][1] / r["om"][0])
    return min(rs), max(rs)


def sensor_bands(sensor, wav, skip, window, band_hz, lo, hi):
    cmd = [sensor, wav, "--mode", "decay", "--skip-ms", str(skip), "--window-ms", str(window),
           "--decay-range", f"{lo:.0f},{hi:.0f}", "--decay-band-hz", str(band_hz), "--decay-max-bands", "120",
           "--json"]
    p = subprocess.run(cmd, capture_output=True, text=True)
    try:
        return json.loads(p.stdout)["decay"]["bands"]
    except (json.JSONDecodeError, KeyError, TypeError):
        return None


def h1_of(bands, f_nom):
    near = [b for b in bands if abs(b["centre_hz"] / f_nom - 1) < 0.04]
    return max(near, key=lambda b: b["candidate_dbfs"]) if near else None


def survey_note(sensor, wav, midi, a):
    f_nom = 439.0 * 2 ** ((midi - 69) / 12)  # the set is tuned near A = 439 Hz
    r_lo, r_hi = fe_band(midi)
    lo, hi = r_lo * f_nom * (1 - a.widen), r_hi * f_nom * (1 + a.widen)
    symp = [439.0 * 2 ** ((m - 69) / 12) * k for m in range(92, 97) for k in (1, 2)]
    out = dict(midi=midi, file=os.path.basename(wav), fe_ratio_band=[round(r_lo, 3), round(r_hi, 3)],
               search_hz=[round(lo, 1), round(hi, 1)])
    for tag, skip, window, bw in (("onset", 5, 900, 40), ("late", 300, 2500, 20)):
        bands = sensor_bands(sensor, wav, skip, window, bw, max(20.0, 0.8 * f_nom), hi * 1.05)
        if bands is None:
            out[tag] = dict(status="sensor refused")
            continue
        h1 = h1_of(bands, f_nom)
        if h1 is None:
            out[tag] = dict(status="no H1")
            continue
        f1 = h1["centre_hz"]
        harm = f1 * np.arange(1, int(hi / f1) + 2)
        cands = []
        for b in bands:
            f = b["centre_hz"]
            if not (lo <= f <= hi) or b["candidate_prominence_db"] < a.prom:
                continue
            if np.min(np.abs(harm - f)) < a.clear or min(abs(s - f) for s in symp) < a.clear:
                continue
            cands.append(b)
        if not cands:
            out[tag] = dict(status="no line", h1_hz=round(f1, 2), h1_dbfs=round(h1["candidate_dbfs"], 1))
            continue
        best = max(cands, key=lambda b: b["candidate_dbfs"])
        out[tag] = dict(status="line", f_hz=round(best["centre_hz"], 1), ratio=round(best["centre_hz"] / f1, 3),
                        re_h1_db=round(best["candidate_dbfs"] - h1["candidate_dbfs"], 1),
                        prom_db=round(best["candidate_prominence_db"], 1), h1_hz=round(f1, 2),
                        n_candidates=len(cands))
    # Clearance of the band itself: is any harmonic or sympathetic line inside the search window?
    f1 = out.get("onset", {}).get("h1_hz") or f_nom
    inside = [k for k in range(1, 40) if lo <= k * f1 <= hi]
    out["harmonics_inside_band"] = inside
    out["sympathetic_inside_band"] = [round(s, 0) for s in symp if lo <= s <= hi]
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--samples", required=True)
    ap.add_argument("--sensor", required=True, help="path to the Sensor Array 'analyze' binary")
    ap.add_argument("--layer", default="7e", help="velocity layer (hex), falls back to the loudest present")
    ap.add_argument("--widen", type=float, default=0.05, help="fractional widening of the FE band")
    ap.add_argument("--clear", type=float, default=12.0, help="Hz clearance from harmonics/sympathetics")
    ap.add_argument("--prom", type=float, default=12.0, help="minimum candidate prominence, dB")
    ap.add_argument("--out", default="")
    a = ap.parse_args()
    rows = []
    for note_oct in sorted({os.path.basename(f).split("_")[1][:2]
                            for f in glob.glob(os.path.join(a.samples, "*_????0000.wav"))
                            if not os.path.basename(f).split("_")[0].endswith("re")}):
        note, octave = note_oct[0], int(note_oct[1])
        midi = label_to_midi(note, octave)
        files = sorted(glob.glob(os.path.join(a.samples, f"*_{note_oct}??0000.wav")))
        files = [f for f in files if not f.endswith("7f0000.wav")]
        pick = [f for f in files if f.endswith(f"{note_oct}{a.layer}0000.wav")] or files[-1:]
        if not pick:
            continue
        row = survey_note(a.sensor, pick[0], midi, a)
        rows.append(row)
        o, l = row.get("onset", {}), row.get("late", {})
        print(f"MIDI {midi:3d} {note_oct} [{os.path.basename(pick[0])[-12:-8]}] FE {row['fe_ratio_band']} "
              f"harm-in-band {row['harmonics_inside_band']} symp {row['sympathetic_inside_band']} | onset: "
              + (f"{o['f_hz']} Hz ({o['ratio']}x) {o['re_h1_db']} dB re H1, prom {o['prom_db']}" if o.get("status") == "line"
                 else o.get("status", "-"))
              + " | late: " + (f"{l['re_h1_db']} dB, prom {l['prom_db']}" if l.get("status") == "line"
                               else l.get("status", "-")), flush=True)
    if a.out:
        with open(a.out, "w") as fh:
            json.dump(rows, fh, indent=1)


if __name__ == "__main__":
    main()
