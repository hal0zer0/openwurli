#!/usr/bin/env python3
"""Pickup-revision metric set: render a fixed matrix with a given preamp-bench binary and
report the numbers the pickup-law revision is gated on, as JSON.

Usage:
    python tools/pickup_rev_metrics.py --bench PATH/TO/preamp-bench --out metrics.json

Taps (all MLP off, speaker character 0 = passthrough, fixed volume, so every tap is a constant
gain away from the node it names — comparisons between builds are valid, absolute levels are not):
    pickup : --no-preamp --no-poweramp  (voice output = pickup x output_scale)
    preamp : --no-poweramp
    full   : whole chain

Metrics:
    trajectory   H2/H1, H3/H1 (dB) vs velocity at C2/C4/C6, pickup and preamp taps
    ladder       H2..H8 re H1 (dB) at C2 and C4 ff, pickup and preamp taps
    typical_rms  C4 v64/80/100/110 RMS (dBFS) at each tap
    register     full-chain RMS (dBFS) at v100 for C2..C6 in half-octave steps
    hf_ratio     C4 ff 2.5-6 kHz harmonic vs inharmonic energy (dB), preamp tap
    alias        preamp-bench alias-audit --json for C3/C4/C5/C6/G6 at v120

Harmonics are measured over 100-400 ms after note-on, Hann-windowed, at integer multiples of a
fundamental refined around the nominal pitch (per-note detune is +-3 cents).
"""

import argparse
import json
import os
import subprocess
import tempfile
import wave

import numpy as np

SR = 44100.0
VOLUME = "0.80"
TAPS = {
    "pickup": ["--no-preamp", "--no-poweramp"],
    "preamp": ["--no-poweramp"],
    "full": [],
}


def midi_hz(n):
    return 440.0 * 2.0 ** ((n - 69) / 12.0)


def read_wav(path):
    with wave.open(path, "rb") as w:
        assert w.getsampwidth() == 3, f"{path}: expected 24-bit"
        raw = w.readframes(w.getnframes())
    b = np.frombuffer(raw, dtype=np.uint8).reshape(-1, 3).astype(np.int32)
    v = b[:, 0] | (b[:, 1] << 8) | (b[:, 2] << 16)
    v = np.where(v >= 1 << 23, v - (1 << 24), v)
    return v.astype(np.float64) / float(1 << 23)


def render(bench, tmp, note, vel, tap, extra=(), duration=0.6):
    out = os.path.join(tmp, f"n{note}_v{vel}_{tap}_{len(extra)}.wav")
    cmd = [bench, "render", "--note", str(note), "--velocity", str(vel),
           "--duration", str(duration), "--volume", VOLUME, "--speaker", "0.0",
           "--no-mlp", "--output", out, *TAPS[tap], *extra]
    subprocess.run(cmd, check=True, capture_output=True)
    x = read_wav(out)
    clipped = int(np.sum(np.abs(x) >= 1.0 - 2.0 ** -22))
    if clipped:
        raise SystemExit(f"clipped WAV ({clipped} samples): {' '.join(cmd)}")
    return x


def window(x, t0=0.1, t1=0.4):
    return x[int(t0 * SR):int(t1 * SR)]


def tone_mag(seg, f):
    n = np.arange(len(seg))
    w = np.hanning(len(seg))
    return abs(np.sum(seg * w * np.exp(-2j * np.pi * f * n / SR)))


def refine_f0(seg, f_nom):
    cents = np.linspace(-8, 8, 161)
    fs = f_nom * 2.0 ** (cents / 1200.0)
    return fs[int(np.argmax([tone_mag(seg, f) for f in fs]))]


def harmonics_db(x, note, kmax=8):
    seg = window(x)
    f0 = refine_f0(seg, midi_hz(note))
    h1 = tone_mag(seg, f0)
    return {k: 20 * np.log10(max(tone_mag(seg, k * f0), 1e-300) / h1)
            for k in range(2, kmax + 1) if k * f0 < SR / 2}


def rms_db(x, t0=0.1, t1=0.4):
    return 20 * np.log10(np.sqrt(np.mean(window(x, t0, t1) ** 2)) + 1e-300)


def hf_ratio_db(x, note, lo=2500.0, hi=6000.0, half_bw=15.0):
    seg = window(x)
    f0 = refine_f0(seg, midi_hz(note))
    spec = np.abs(np.fft.rfft(seg * np.hanning(len(seg)))) ** 2
    freqs = np.fft.rfftfreq(len(seg), 1 / SR)
    band = (freqs >= lo) & (freqs <= hi)
    harm = np.zeros_like(band)
    for k in range(1, int(hi / f0) + 2):
        harm |= np.abs(freqs - k * f0) <= half_bw
    e_h = spec[band & harm].sum()
    e_i = spec[band & ~harm].sum()
    return 10 * np.log10(e_h / e_i)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--bench", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    m = {"bench": os.path.abspath(a.bench)}
    with tempfile.TemporaryDirectory() as tmp:
        r = lambda n, v, t, e=(): render(a.bench, tmp, n, v, t, e)

        m["trajectory"] = {}
        for tap in ("pickup", "preamp"):
            for n in (36, 60, 84):
                for v in (20, 40, 64, 80, 100, 127):
                    h = harmonics_db(r(n, v, tap), n, kmax=3)
                    m["trajectory"][f"{tap}/{n}/{v}"] = {"h2": h[2], "h3": h[3]}

        m["ladder"] = {f"{tap}/{n}": harmonics_db(r(n, 127, tap), n)
                       for tap in ("pickup", "preamp") for n in (36, 60)}

        m["typical_rms"] = {f"{tap}/{v}": rms_db(r(60, v, tap))
                            for tap in TAPS for v in (64, 80, 100, 110)}

        m["register"] = {str(n): rms_db(r(n, 100, "full")) for n in range(36, 85, 6)}

        m["hf_ratio"] = hf_ratio_db(r(60, 127, "preamp"), 60)


    m["alias"] = {}
    for n in (48, 60, 72, 84, 91):
        p = subprocess.run([a.bench, "alias-audit", "--note", str(n), "--velocity", "120", "--json"],
                           check=True, capture_output=True, text=True)
        m["alias"][str(n)] = json.loads(p.stdout)

    with open(a.out, "w") as f:
        json.dump(m, f, indent=1, sort_keys=True)
    print(f"wrote {a.out}")


if __name__ == "__main__":
    main()
