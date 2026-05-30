#!/usr/bin/env python3
"""
Pump-dynamics fitter for the shadow-preamp replacement model.

Inputs:
  /tmp/pump_sweep_88k2.csv     — Step-1 static-LUT (R, pump_v)
  /tmp/pump_sin/sin_*.csv      — pump-sinusoid captures at various tremolo freqs

Goal: find a model that maps R(t) → pump(t) under slewed-R conditions, so we
can replace the 12-node DK shadow solver (~1.44 µs/sample) with a cheap
analytical predictor (~20 ns/sample).

Strategy: subtract the static LUT contribution, then fit a model to the
residual `xi(t) = pump(t) - LUT(R(t))`. Try a ladder of candidate models in
order of complexity, report RMSE per (model, frequency) pair.

  1. 1-pole linear IIR driven by R (lpf-on-R approach):
        pump_hat[n] = LUT( R_lpf[n] )
        R_lpf[n] = (1-a)*R_lpf[n-1] + a*R[n]
  2. 1-pole IIR driven by dR/dt
  3. 1-pole IIR on residual driven by R
  4. 2-pole IIR (biquad) on residual driven by R
  5. Same with separate up/down coefficients (asymmetric)

Reports RMSE in mV for each model at each test frequency; the best model
becomes the seed for the Rust implementation.
"""

import csv
import glob
import os
import sys

import numpy as np
from scipy.interpolate import interp1d
from scipy.optimize import minimize

LUT_CSV = "/tmp/pump_sweep_88k2.csv"
SIN_DIR = "/tmp/pump_sin"
RESULTS_CSV = "/tmp/pump_fit_results.csv"


# -----------------------------------------------------------------------------
# I/O
# -----------------------------------------------------------------------------


def load_lut(path):
    rows = []
    with open(path) as f:
        for line in f:
            if line.startswith("#") or line.startswith("r_ldr"):
                continue
            parts = line.strip().split(",")
            rows.append((float(parts[0]), float(parts[1])))
    rows.sort()
    r = np.array([x[0] for x in rows])
    v = np.array([x[1] for x in rows])
    return r, v


def load_sinusoid(path):
    samples, r, pump, pump_pm = [], [], [], []
    sr = freq = None
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line.startswith("#"):
                for tok in line.split():
                    if tok.startswith("sr="):
                        sr = float(tok[3:])
                    elif tok.startswith("freq="):
                        freq = float(tok[5:])
                continue
            if line.startswith("sample"):
                continue
            parts = line.split(",")
            samples.append(int(parts[0]))
            r.append(float(parts[1]))
            pump.append(float(parts[2]))
            pump_pm.append(float(parts[3]))
    return {
        "sr": sr,
        "freq": freq,
        "n": np.array(samples),
        "r": np.array(r),
        "pump": np.array(pump),
        "pump_pm": np.array(pump_pm),
    }


def make_lut_interp(r, v):
    """Log-R linear interpolation, returns f(R) -> pump."""
    ln_r = np.log(r)
    return lambda x: np.interp(np.log(np.clip(x, r[0], r[-1])), ln_r, v)


# -----------------------------------------------------------------------------
# Models
# -----------------------------------------------------------------------------


def model_lpf_on_R(R, sr, params, lut_fn):
    """pump_hat[n] = LUT( R_lpf[n] ), where R_lpf is 1-pole LPF on R."""
    (tau_ms,) = params
    if tau_ms <= 0:
        return np.full_like(R, np.nan)
    a = 1.0 - np.exp(-1.0 / (sr * tau_ms * 1e-3))
    r_lpf = np.empty_like(R)
    r_lpf[0] = R[0]
    for n in range(1, len(R)):
        r_lpf[n] = r_lpf[n - 1] + a * (R[n] - r_lpf[n - 1])
    return lut_fn(r_lpf)


def model_lpf_on_lnR(R, sr, params, lut_fn):
    """LPF in log space, then exp -> LUT."""
    (tau_ms,) = params
    if tau_ms <= 0:
        return np.full_like(R, np.nan)
    a = 1.0 - np.exp(-1.0 / (sr * tau_ms * 1e-3))
    ln_r = np.log(R)
    ln_lpf = np.empty_like(ln_r)
    ln_lpf[0] = ln_r[0]
    for n in range(1, len(ln_r)):
        ln_lpf[n] = ln_lpf[n - 1] + a * (ln_r[n] - ln_lpf[n - 1])
    return lut_fn(np.exp(ln_lpf))


def model_iir1_dR(R, sr, params, lut_fn):
    """pump_hat = LUT(R) + xi, xi[n] = a*xi[n-1] + b*(R[n] - R[n-1])."""
    a, b = params
    if not (0 <= a < 1):
        return np.full_like(R, np.nan)
    base = lut_fn(R)
    xi = np.zeros_like(R)
    for n in range(1, len(R)):
        xi[n] = a * xi[n - 1] + b * (R[n] - R[n - 1])
    return base + xi


def model_iir1_dlnR(R, sr, params, lut_fn):
    """pump_hat = LUT(R) + xi, xi[n] = a*xi[n-1] + b*(ln R[n] - ln R[n-1])."""
    a, b = params
    if not (0 <= a < 1):
        return np.full_like(R, np.nan)
    base = lut_fn(R)
    ln_r = np.log(R)
    xi = np.zeros_like(R)
    for n in range(1, len(R)):
        xi[n] = a * xi[n - 1] + b * (ln_r[n] - ln_r[n - 1])
    return base + xi


def model_iir2_dlnR(R, sr, params, lut_fn):
    """2-pole IIR on residual driven by d(ln R)/dt:
    xi[n] = a1*xi[n-1] + a2*xi[n-2] + b0*u[n] + b1*u[n-1]
    where u[n] = ln R[n] - ln R[n-1].
    Stability requires roots of (1 - a1 z^-1 - a2 z^-2) inside unit circle.
    """
    a1, a2, b0, b1 = params
    # Stability check: poles z satisfy z^2 - a1 z - a2 = 0
    disc = a1 * a1 + 4 * a2
    if disc >= 0:
        z1 = 0.5 * (a1 + np.sqrt(disc))
        z2 = 0.5 * (a1 - np.sqrt(disc))
        if abs(z1) >= 1 or abs(z2) >= 1:
            return np.full_like(R, np.nan)
    else:
        mag = np.sqrt(-a2)  # |z| = sqrt(-a2) for complex conj pair
        if mag >= 1:
            return np.full_like(R, np.nan)
    base = lut_fn(R)
    ln_r = np.log(R)
    u = np.zeros_like(R)
    u[1:] = ln_r[1:] - ln_r[:-1]
    xi = np.zeros_like(R)
    for n in range(2, len(R)):
        xi[n] = a1 * xi[n - 1] + a2 * xi[n - 2] + b0 * u[n] + b1 * u[n - 1]
    return base + xi


def model_iir1_asym(R, sr, params, lut_fn):
    """1-pole on residual driven by d(ln R)/dt, separate b for sign."""
    a, b_up, b_dn = params
    if not (0 <= a < 1):
        return np.full_like(R, np.nan)
    base = lut_fn(R)
    ln_r = np.log(R)
    xi = np.zeros_like(R)
    for n in range(1, len(R)):
        du = ln_r[n] - ln_r[n - 1]
        b = b_up if du > 0 else b_dn
        xi[n] = a * xi[n - 1] + b * du
    return base + xi


# -----------------------------------------------------------------------------
# Fitting harness
# -----------------------------------------------------------------------------


def rmse(pred, truth, skip=200):
    """RMSE in mV, skipping initial transient samples to ignore startup."""
    d = pred[skip:] - truth[skip:]
    return 1000.0 * np.sqrt(np.mean(d * d))


def fit_model(model_fn, R, sr, target, lut_fn, x0, bounds=None, method="Nelder-Mead"):
    def loss(p):
        pred = model_fn(R, sr, p, lut_fn)
        if not np.all(np.isfinite(pred)):
            return 1e9
        return rmse(pred, target)

    res = minimize(loss, x0, method=method, options={"xatol": 1e-6, "fatol": 1e-6, "maxiter": 5000})
    return res.x, res.fun


# -----------------------------------------------------------------------------
# Main
# -----------------------------------------------------------------------------


def main():
    if not os.path.exists(LUT_CSV):
        print(f"ERROR: missing {LUT_CSV}", file=sys.stderr)
        sys.exit(1)

    lut_r, lut_v = load_lut(LUT_CSV)
    lut_fn = make_lut_interp(lut_r, lut_v)
    print(f"LUT loaded: {len(lut_r)} points, R ∈ [{lut_r[0]:.0f}, {lut_r[-1]:.0f}] Ω, "
          f"pump ∈ [{lut_v.min():.3f}, {lut_v.max():.3f}] V")

    sin_files = sorted(glob.glob(os.path.join(SIN_DIR, "sin_*.csv")))
    if not sin_files:
        print(f"ERROR: no sin_*.csv in {SIN_DIR}", file=sys.stderr)
        sys.exit(1)

    models = [
        ("lpf_R",        model_lpf_on_R,     [50.0],                None),
        ("lpf_lnR",      model_lpf_on_lnR,   [50.0],                None),
        ("iir1_dR",      model_iir1_dR,      [0.999, 1e-6],         None),
        ("iir1_dlnR",    model_iir1_dlnR,    [0.999, -1.0],         None),
        ("iir1_asym",    model_iir1_asym,    [0.999, -1.0, 1.0],    None),
        ("iir2_dlnR",    model_iir2_dlnR,    [1.99, -0.99, -1.0, 0.5], None),
    ]

    results = []
    print(f"\n{'freq Hz':>8s}  {'baseline':>10s}  ", end="")
    for name, _, _, _ in models:
        print(f"{name:>14s}  ", end="")
    print()

    for path in sin_files:
        data = load_sinusoid(path)
        sr = data["sr"]
        freq = data["freq"]
        R = data["r"]
        # Fit to pair-mean (cancels Nyquist 2-cycle).
        target = data["pump_pm"]

        # Baseline: just LUT, no dynamics. Tells us the cost of doing nothing.
        baseline = lut_fn(R)
        baseline_rmse = rmse(baseline, target)

        print(f"{freq:>7.1f}   {baseline_rmse:>10.1f}  ", end="")
        row = {"freq": freq, "baseline_mv": baseline_rmse}
        for name, fn, x0, bounds in models:
            try:
                p, err = fit_model(fn, R, sr, target, lut_fn, x0, bounds)
                row[name + "_rmse_mv"] = err
                row[name + "_params"] = list(p)
                print(f"{err:>14.1f}  ", end="")
            except Exception as e:
                print(f"{'FAIL':>14s}  ", end="")
                row[name + "_rmse_mv"] = None
        print()
        results.append(row)

    # Detail dump per freq.
    print("\n=== fitted parameters ===")
    for r in results:
        print(f"\nfreq = {r['freq']} Hz   (baseline RMSE = {r['baseline_mv']:.1f} mV)")
        for name, _, _, _ in models:
            key = name + "_rmse_mv"
            pkey = name + "_params"
            if r.get(key) is None:
                print(f"  {name:>12s}  FAIL")
            else:
                ps = "  ".join(f"{p:.6e}" for p in r[pkey])
                print(f"  {name:>12s}  RMSE = {r[key]:7.2f} mV   params = [{ps}]")

    with open(RESULTS_CSV, "w") as f:
        w = csv.writer(f)
        cols = ["freq", "baseline_mv"]
        for name, _, _, _ in models:
            cols.append(name + "_rmse_mv")
        w.writerow(cols)
        for r in results:
            row = [r["freq"], r["baseline_mv"]]
            for name, _, _, _ in models:
                row.append(r.get(name + "_rmse_mv"))
            w.writerow(row)
    print(f"\nWrote results CSV → {RESULTS_CSV}")


if __name__ == "__main__":
    main()
