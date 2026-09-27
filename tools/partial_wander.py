#!/usr/bin/env python3
"""Partial-frequency wander of a sustained note (report-only; evidence for the reed-jitter ruling).

For partials k = 1..3 of a known fundamental: band-limit around k*f0, take the analytic signal,
and fit the unwrapped phase's slope on short segments to get a frequency time series over the
sustain. A quadratic (the deterministic glide as the swing decays, <= ~0.02 % by the backbone
estimate) is removed from each partial. Reported per file:

  wander_rms_pct[k]   rms of (f_k - glide) / (k f0), percent
  tau_s[k]            autocorrelation time of that residual (first 1/e crossing)
  common_pct          sqrt of the covariance of the fractional residuals of H1 and the strongest
                      other partial (`pair`): a reed's pitch wander moves every partial together,
                      while measurement noise in two bands is independent, so the covariance keeps
                      the wander and drops the noise
  rho                 their correlation
  A partial more than 30 dB below H1 is flagged unreliable (neighbour leakage dominates its phase).

The estimator UNDER-READS slow wander (the glide removal takes part of it) and biases tau short:
a synthetic OU process with sigma 0.04 %, tau 0.32 s reads ~0.020 %, tau ~0.06 s; with tau 0.02 s
it reads ~0.036 %. Read recordings only against model renders through the same estimator.

Every number from a recording is an UPPER BOUND on reed wander: the recording chain, the medium
and the instrument's other vibrations (sympathetic reeds, reed-bar modes) add to it. The
estimator is calibrated on model renders with known jitter (sigma 0.04 %, tau 0.32 s) and with
jitter off (its own floor) before any recording is read.

No Sensor Array mode measures a partial's frequency over time (decay mode fits one frequency per
partial over a multi-second window), hence this estimator.

Usage: python tools/partial_wander.py --wav FILE --f0 HZ [--skip 0.3] [--dur 1.5] [--json]
"""

import argparse
import json
import wave

import numpy as np
from scipy.signal import butter, hilbert, sosfiltfilt

SEG_S = 0.040


def read_wav(path):
    """Mono float, via the wave module (handles the vendor files' non-standard fmt chunk)."""
    with wave.open(path) as w:
        sr, n, ch, sw = w.getframerate(), w.getnframes(), w.getnchannels(), w.getsampwidth()
        raw = w.readframes(n)
    if sw == 2:
        x = np.frombuffer(raw, "<i2").astype(np.float64) / 32768.0
    elif sw == 3:
        b = np.frombuffer(raw, np.uint8).reshape(-1, 3)
        x = (b[:, 0].astype(np.int32) | (b[:, 1].astype(np.int32) << 8)
             | (b[:, 2].astype(np.int32) << 16))
        x = np.where(x >= 1 << 23, x - (1 << 24), x).astype(np.float64) / (1 << 23)
    elif sw == 4:
        x = np.frombuffer(raw, "<i4").astype(np.float64) / 2 ** 31
    else:
        raise ValueError(f"{path}: {sw}-byte samples")
    return sr, x.reshape(-1, ch).mean(axis=1)


def partial_track(x, sr, fk, bw, t0, t1):
    """Frequency (Hz) of the component near fk on SEG_S segments of [t0, t1)."""
    lo, hi = max(fk - bw, 1.0), min(fk + bw, 0.45 * sr)
    y = sosfiltfilt(butter(8, [lo, hi], btype="band", fs=sr, output="sos"), x)
    ph = np.unwrap(np.angle(hilbert(y)))
    amp = np.abs(hilbert(y))
    seg = int(SEG_S * sr)
    ts, fs, amps = [], [], []
    for a in range(int(t0 * sr), int(t1 * sr) - seg, seg):
        t = np.arange(seg) / sr
        fs.append(np.polyfit(t, ph[a:a + seg], 1)[0] / (2 * np.pi))
        ts.append((a + seg / 2) / sr)
        amps.append(amp[a:a + seg].mean())
    return np.array(ts), np.array(fs), np.array(amps)


def acf_time(r, dt):
    r = r - r.mean()
    if r.std() == 0:
        return None
    c = np.correlate(r, r, "full")[len(r) - 1:] / (r.var() * len(r))
    below = np.nonzero(c < np.exp(-1))[0]
    if len(below) == 0:
        return None
    i = below[0]
    # linear interpolation between lag i-1 and i
    return dt * ((i - 1) + (c[i - 1] - np.exp(-1)) / (c[i - 1] - c[i]))


def analyse(path, f0, skip, dur, kmax=3):
    sr, x = read_wav(path)
    onset = int(np.argmax(np.abs(x) > 0.1 * np.abs(x).max())) / sr
    t0 = onset + skip
    t1 = min(t0 + dur, len(x) / sr - 0.1)
    if t1 - t0 < 0.6:
        return {"file": path, "refused": f"sustain {t1 - t0:.2f} s < 0.6 s"}
    bw = min(0.4 * f0, 40.0)
    out = {"file": path, "f0": f0, "sr": sr, "window_s": [round(t0, 3), round(t1, 3)],
           "band_hz": bw, "seg_s": SEG_S, "partials": {}}
    res = {}
    for k in range(1, kmax + 1):
        if k * f0 + bw > 0.45 * sr:
            break
        ts, fs, amps = partial_track(x, sr, k * f0, bw, t0, t1)
        glide = np.polyval(np.polyfit(ts - ts[0], fs, 2), ts - ts[0])
        r = (fs - glide) / (k * f0)
        res[k] = r
        out["partials"][k] = {
            "mean_hz": float(fs.mean()),
            "glide_pct": float(100 * (glide[-1] - glide[0]) / (k * f0)),
            "wander_rms_pct": float(100 * r.std()),
            "tau_s": acf_time(r, SEG_S),
            "level_db_re_h1": None,
            "amp": float(amps.mean()),
        }
    if 1 in out["partials"]:
        a1 = out["partials"][1]["amp"]
        for k, p in out["partials"].items():
            p["level_db_re_h1"] = float(20 * np.log10(p["amp"] / a1))
            p["reliable"] = p["level_db_re_h1"] > -30.0
    others = [k for k in res if k != 1 and out["partials"][k]["reliable"]]
    if 1 in res and others:
        k2 = max(others, key=lambda k: out["partials"][k]["level_db_re_h1"])
        c = float(np.mean((res[1] - res[1].mean()) * (res[k2] - res[k2].mean())))
        out["pair"] = k2
        out["common_pct"] = float(100 * np.sqrt(c)) if c > 0 else 0.0
        out["rho"] = float(np.corrcoef(res[1], res[k2])[0, 1])
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--wav", required=True)
    ap.add_argument("--f0", type=float, required=True, help="TRUE fundamental, Hz")
    ap.add_argument("--skip", type=float, default=0.3, help="seconds after onset")
    ap.add_argument("--dur", type=float, default=1.5)
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()
    r = analyse(a.wav, a.f0, a.skip, a.dur)
    if a.json:
        print(json.dumps(r))
        return
    if "refused" in r:
        print(f"{a.wav}: REFUSED ({r['refused']})")
        return
    print(f"{a.wav}  f0 {a.f0:.2f}  window {r['window_s']}  band +-{r['band_hz']:.0f} Hz")
    for k, p in r["partials"].items():
        tau = "--" if p["tau_s"] is None else f"{p['tau_s']:.3f}"
        flag = "" if p["reliable"] else "  (unreliable: < -30 dB)"
        print(f"  H{k}: {p['level_db_re_h1']:+6.1f} dB  wander {p['wander_rms_pct']:.4f} %  "
              f"tau {tau} s  glide {p['glide_pct']:+.4f} %{flag}")
    if "common_pct" in r:
        print(f"  common (H1xH{r['pair']}): {r['common_pct']:.4f} %   rho {r['rho']:+.2f}")


if __name__ == "__main__":
    main()
