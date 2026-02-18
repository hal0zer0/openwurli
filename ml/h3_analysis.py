#!/usr/bin/env python3
"""
H3 Deficit Analysis — Pickup Nonlinearity Harmonic Decomposition

Computes exact harmonic spectrum of y/(1-y)^alpha through a 1-pole HPF
for various alpha values, and compares against OBM recordings.

The key question: is 1/(1-y) the right nonlinearity, or does the real
Wurlitzer pickup have a stronger nonlinearity (alpha > 1)?
"""

import numpy as np
import json
import os

# ──────────────────────────────────────────────────────────────────────
# Physical parameters (from tables.rs)
# ──────────────────────────────────────────────────────────────────────
HPF_FC = 2312.0  # Pickup HPF corner frequency
SENSITIVITY = 1.8375  # V_hv * C_0 / (C_0 + C_p)
DS_AT_C4 = 0.70

def midi_to_freq(midi):
    return 440.0 * 2**((midi - 69) / 12.0)

def reed_length_mm(midi):
    n = max(1, min(64, midi - 32))
    if n <= 20:
        inches = 3.0 - n / 20.0
    else:
        inches = 2.0 - (n - 20) / 44.0
    return inches * 25.4

def reed_blank_dims(midi):
    reed = max(1, min(64, midi - 32))
    if reed <= 14: w = 0.151
    elif reed <= 20: w = 0.127
    elif reed <= 42: w = 0.121
    elif reed <= 50: w = 0.111
    else: w = 0.098
    if reed <= 16: t = 0.020
    elif reed <= 26:
        frac = (reed - 16) / 10.0
        t = 0.020 + frac * 0.011
    else: t = 0.031
    return w * 25.4, t * 25.4

def reed_compliance(midi):
    l = reed_length_mm(midi)
    w, t = reed_blank_dims(midi)
    return l**3 / (w * t**3)

def pickup_ds(midi):
    c = reed_compliance(midi)
    c_ref = reed_compliance(60)
    ds = DS_AT_C4 * (c / c_ref)**0.65
    return max(0.02, min(0.80, ds))

def velocity_exponent(midi):
    m = float(midi)
    center, sigma = 62.0, 15.0
    t = np.exp(-0.5 * ((m - center) / sigma)**2)
    return 0.75 + t * (1.4 - 0.75)

# ──────────────────────────────────────────────────────────────────────
# Numerical harmonic analysis
# ──────────────────────────────────────────────────────────────────────
def compute_harmonics_of_nonlinearity(f0, ds, vel_midi=80, alpha=1.0,
                                       sr=44100, duration=0.5, n_harmonics=8):
    """
    Generate a pure sine at f0, apply y/(1-y)^alpha nonlinearity,
    then 1-pole HPF at HPF_FC. Measure harmonic amplitudes via DFT.
    """
    midi_approx = 69 + 12 * np.log2(f0 / 440.0)
    vel = vel_midi / 127.0
    vel_exp = velocity_exponent(midi_approx)
    vel_scale = vel ** vel_exp

    n_samples = int(sr * duration)
    t = np.arange(n_samples) / sr
    # Pure sine (fundamental mode only, amplitude 1.0 * vel_scale)
    reed = vel_scale * np.sin(2 * np.pi * f0 * t)

    # Scale to physical displacement fraction
    y = np.clip(reed * ds, -0.90, 0.90)

    # Nonlinear pickup: y / (1 - y)^alpha
    nonlinear = y / np.power(1.0 - y, alpha)
    v = nonlinear * SENSITIVITY

    # 1-pole HPF at HPF_FC
    omega = 2 * np.pi * HPF_FC / sr
    coeff = 1.0 / (1.0 + omega)  # simplified 1-pole HPF coefficient
    # More accurate: bilinear transform
    w_d = np.tan(np.pi * HPF_FC / sr)
    a1 = (1 - w_d) / (1 + w_d)
    b0 = 1.0 / (1 + w_d)

    output = np.zeros(n_samples)
    x_prev = 0.0
    y_prev = 0.0
    for i in range(n_samples):
        output[i] = b0 * v[i] - b0 * x_prev + a1 * y_prev
        x_prev = v[i]
        y_prev = output[i]

    # Steady-state: skip first half
    start = n_samples // 2
    signal = output[start:]
    n = len(signal)

    # DFT at each harmonic
    amps = []
    for h in range(1, n_harmonics + 1):
        freq = h * f0
        k = np.arange(n)
        phase = 2 * np.pi * freq * k / sr
        re = np.sum(signal * np.cos(phase)) / n
        im = -np.sum(signal * np.sin(phase)) / n
        mag = 2 * np.sqrt(re**2 + im**2)
        amps.append(mag)

    return amps


def compute_all_notes(alpha=1.0):
    """Compute harmonic spectrum for all 13 OBM notes."""
    obm_midis = [50, 54, 58, 62, 66, 70, 74, 78, 82, 86, 90, 94, 98]
    results = {}
    for midi in obm_midis:
        f0 = midi_to_freq(midi)
        ds = pickup_ds(midi)
        amps = compute_harmonics_of_nonlinearity(f0, ds, alpha=alpha)
        h1 = amps[0]
        db_rel = [0.0] + [20*np.log10(a/h1) if a > 0 else -120 for a in amps[1:]]
        results[midi] = {
            'f0': f0, 'ds': ds, 'amps': amps,
            'db_rel_h1': db_rel,
            'h2_db': db_rel[1], 'h3_db': db_rel[2],
            'h3_h2_db': db_rel[2] - db_rel[1] if len(db_rel) > 2 else None
        }
    return results


def load_obm_harmonics():
    """Load OBM recording harmonic data."""
    path = os.path.join(os.path.dirname(__file__), 'harmonics.json')
    with open(path) as f:
        data = json.load(f)
    results = {}
    for entry in data:
        midi = entry['midi_note']
        es = entry['windows']['early_sustain']
        db = es['amps_dB_rel_H1']
        results[midi] = {
            'f0': entry['f0'],
            'h2_db': db[1], 'h3_db': db[2],
            'h3_h2_db': db[2] - db[1],
            'all_db': db
        }
    return results


def load_model_harmonics():
    """Load model-rendered harmonic data."""
    path = os.path.join(os.path.dirname(__file__), 'model_harmonics.json')
    with open(path) as f:
        data = json.load(f)
    results = {}
    for key, entry in data.items():
        midi = entry['midi_note']
        es = entry['windows']['early_sustain']
        db = es['amps_dB_rel_H1']
        results[midi] = {
            'f0': entry['f0'],
            'h2_db': db[1], 'h3_db': db[2],
            'h3_h2_db': db[2] - db[1],
            'all_db': db
        }
    return results


def main():
    obm = load_obm_harmonics()
    model = load_model_harmonics()

    print("=" * 90)
    print("H3 DEFICIT ANALYSIS — OBM vs Model vs Theoretical Nonlinearity")
    print("=" * 90)

    # Print DS values for reference
    print("\n--- Displacement Scale Values ---")
    print(f"{'MIDI':>4} {'Note':>4} {'f0':>8} {'DS':>6}")
    note_names = ['C','C#','D','D#','E','F','F#','G','G#','A','A#','B']
    for midi in sorted(obm.keys()):
        f0 = midi_to_freq(midi)
        ds = pickup_ds(midi)
        name = note_names[midi % 12] + str(midi // 12 - 1)
        print(f"{midi:>4} {name:>4} {f0:>8.1f} {ds:>6.3f}")

    # Compute theoretical for alpha=1.0 (current model)
    theory_1_0 = compute_all_notes(alpha=1.0)

    # Print comparison table
    print("\n--- H2 Comparison (early_sustain, dB re H1) ---")
    print(f"{'MIDI':>4} {'f0':>7} {'OBM':>8} {'Model':>8} {'Theory':>8} {'OBM-Mod':>8} {'OBM-Thy':>8}")
    for midi in sorted(obm.keys()):
        o = obm[midi]
        m = model.get(midi, {})
        t = theory_1_0.get(midi, {})
        m_h2 = m.get('h2_db', float('nan'))
        t_h2 = t.get('h2_db', float('nan'))
        print(f"{midi:>4} {o['f0']:>7.1f} {o['h2_db']:>8.2f} {m_h2:>8.2f} {t_h2:>8.2f} "
              f"{o['h2_db']-m_h2:>+8.2f} {o['h2_db']-t_h2:>+8.2f}")

    print("\n--- H3 Comparison (early_sustain, dB re H1) ---")
    print(f"{'MIDI':>4} {'f0':>7} {'OBM':>8} {'Model':>8} {'Theory':>8} {'OBM-Mod':>8} {'OBM-Thy':>8}")
    for midi in sorted(obm.keys()):
        o = obm[midi]
        m = model.get(midi, {})
        t = theory_1_0.get(midi, {})
        m_h3 = m.get('h3_db', float('nan'))
        t_h3 = t.get('h3_db', float('nan'))
        print(f"{midi:>4} {o['f0']:>7.1f} {o['h3_db']:>8.2f} {m_h3:>8.2f} {t_h3:>8.2f} "
              f"{o['h3_db']-m_h3:>+8.2f} {o['h3_db']-t_h3:>+8.2f}")

    print("\n--- H3/H2 Step (dB) — Key Diagnostic ---")
    print(f"{'MIDI':>4} {'f0':>7} {'OBM':>8} {'Model':>8} {'Theory':>8}")
    for midi in sorted(obm.keys()):
        o = obm[midi]
        m = model.get(midi, {})
        t = theory_1_0.get(midi, {})
        print(f"{midi:>4} {o['f0']:>7.1f} {o['h3_h2_db']:>8.2f} "
              f"{m.get('h3_h2_db', float('nan')):>8.2f} "
              f"{t.get('h3_h2_db', float('nan')):>8.2f}")

    # Now sweep alpha values
    print("\n" + "=" * 90)
    print("ALPHA SWEEP — Finding best-fit nonlinearity exponent")
    print("=" * 90)
    print("Nonlinearity: y / (1-y)^alpha")
    print("alpha=1.0: standard parallel-plate capacitor")
    print("alpha>1.0: enhanced fringing fields / concentrated field effect")
    print()

    alphas = [1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0]
    results_by_alpha = {}
    for alpha in alphas:
        theory = compute_all_notes(alpha=alpha)
        h3_errors = []
        h2_errors = []
        for midi in sorted(obm.keys()):
            o = obm[midi]
            t = theory.get(midi, {})
            if t:
                h2_errors.append(o['h2_db'] - t['h2_db'])
                h3_errors.append(o['h3_db'] - t['h3_db'])
        results_by_alpha[alpha] = {
            'h2_mean': np.mean(h2_errors), 'h2_std': np.std(h2_errors),
            'h3_mean': np.mean(h3_errors), 'h3_std': np.std(h3_errors),
            'h3_rms': np.sqrt(np.mean(np.array(h3_errors)**2)),
            'combined_rms': np.sqrt(np.mean(np.array(h2_errors)**2 + np.array(h3_errors)**2))
        }

    print(f"{'Alpha':>6} {'H2 mean':>8} {'H2 std':>7} {'H3 mean':>8} {'H3 std':>7} {'H3 RMS':>7} {'Total':>7}")
    for alpha in alphas:
        r = results_by_alpha[alpha]
        print(f"{alpha:>6.2f} {r['h2_mean']:>+8.2f} {r['h2_std']:>7.2f} "
              f"{r['h3_mean']:>+8.2f} {r['h3_std']:>7.2f} "
              f"{r['h3_rms']:>7.2f} {r['combined_rms']:>7.2f}")

    # Find optimal alpha by minimizing H3 RMS error
    fine_alphas = np.arange(1.0, 3.01, 0.1)
    best_alpha = 1.0
    best_rms = 999
    for alpha in fine_alphas:
        theory = compute_all_notes(alpha=alpha)
        h3_errors = []
        for midi in sorted(obm.keys()):
            o = obm[midi]
            t = theory.get(midi, {})
            if t:
                h3_errors.append(o['h3_db'] - t['h3_db'])
        rms = np.sqrt(np.mean(np.array(h3_errors)**2))
        if rms < best_rms:
            best_rms = rms
            best_alpha = alpha

    print(f"\nOptimal alpha (min H3 RMS): {best_alpha:.1f} (RMS={best_rms:.2f} dB)")

    # Show detailed comparison at optimal alpha
    print(f"\n--- Detailed comparison at alpha={best_alpha:.1f} ---")
    theory_opt = compute_all_notes(alpha=best_alpha)
    print(f"{'MIDI':>4} {'f0':>7} {'OBM H3':>8} {'Thy H3':>8} {'Error':>8} "
          f"{'OBM H2':>8} {'Thy H2':>8} {'Error':>8}")
    for midi in sorted(obm.keys()):
        o = obm[midi]
        t = theory_opt.get(midi, {})
        print(f"{midi:>4} {o['f0']:>7.1f} {o['h3_db']:>8.2f} {t['h3_db']:>8.2f} "
              f"{o['h3_db']-t['h3_db']:>+8.2f} "
              f"{o['h2_db']:>8.2f} {t['h2_db']:>8.2f} "
              f"{o['h2_db']-t['h2_db']:>+8.2f}")

    # Check if H2 accuracy degrades at optimal alpha
    h2_errors_opt = []
    h3_errors_opt = []
    for midi in sorted(obm.keys()):
        o = obm[midi]
        t = theory_opt.get(midi, {})
        h2_errors_opt.append(o['h2_db'] - t['h2_db'])
        h3_errors_opt.append(o['h3_db'] - t['h3_db'])

    print(f"\nAt alpha={best_alpha:.1f}:")
    print(f"  H2 mean error: {np.mean(h2_errors_opt):+.2f} dB (std {np.std(h2_errors_opt):.2f})")
    print(f"  H3 mean error: {np.mean(h3_errors_opt):+.2f} dB (std {np.std(h3_errors_opt):.2f})")

    # Also test combined alpha + DS correction
    print("\n" + "=" * 90)
    print("ALPHA + DS JOINT OPTIMIZATION")
    print("=" * 90)
    print("Testing whether adjusting both alpha and DS_AT_C4 together gives better fit...")

    best_combo = (1.0, 0.70, 999)
    for alpha in np.arange(1.0, 3.01, 0.1):
        for ds_c4 in np.arange(0.30, 0.90, 0.05):
            errors_sq = []
            for midi in sorted(obm.keys()):
                o = obm[midi]
                f0 = midi_to_freq(midi)
                c = reed_compliance(midi)
                c_ref = reed_compliance(60)
                ds = max(0.02, min(0.80, ds_c4 * (c / c_ref)**0.65))
                amps = compute_harmonics_of_nonlinearity(f0, ds, alpha=alpha)
                h1 = amps[0]
                if h1 > 0:
                    h2_db = 20*np.log10(amps[1]/h1) if amps[1] > 0 else -120
                    h3_db = 20*np.log10(amps[2]/h1) if amps[2] > 0 else -120
                    errors_sq.append((o['h2_db'] - h2_db)**2)
                    errors_sq.append((o['h3_db'] - h3_db)**2)
            rms = np.sqrt(np.mean(errors_sq))
            if rms < best_combo[2]:
                best_combo = (alpha, ds_c4, rms)

    print(f"Best: alpha={best_combo[0]:.1f}, DS_AT_C4={best_combo[1]:.2f}, RMS={best_combo[2]:.2f} dB")

    # Show detailed at best combo
    alpha_best, ds_best, _ = best_combo
    print(f"\n--- Detailed at alpha={alpha_best:.1f}, DS_AT_C4={ds_best:.2f} ---")
    print(f"{'MIDI':>4} {'f0':>7} {'DS':>6} {'OBM H2':>8} {'Thy H2':>8} {'err':>6} "
          f"{'OBM H3':>8} {'Thy H3':>8} {'err':>6}")
    h2e, h3e = [], []
    for midi in sorted(obm.keys()):
        o = obm[midi]
        f0 = midi_to_freq(midi)
        c = reed_compliance(midi)
        c_ref = reed_compliance(60)
        ds = max(0.02, min(0.80, ds_best * (c / c_ref)**0.65))
        amps = compute_harmonics_of_nonlinearity(f0, ds, alpha=alpha_best)
        h1 = amps[0]
        h2_db = 20*np.log10(amps[1]/h1) if amps[1] > 0 else -120
        h3_db = 20*np.log10(amps[2]/h1) if amps[2] > 0 else -120
        e2 = o['h2_db'] - h2_db
        e3 = o['h3_db'] - h3_db
        h2e.append(e2); h3e.append(e3)
        print(f"{midi:>4} {o['f0']:>7.1f} {ds:>6.3f} {o['h2_db']:>8.2f} {h2_db:>8.2f} {e2:>+6.1f} "
              f"{o['h3_db']:>8.2f} {h3_db:>8.2f} {e3:>+6.1f}")
    print(f"\n  H2: mean={np.mean(h2e):+.1f} std={np.std(h2e):.1f}")
    print(f"  H3: mean={np.mean(h3e):+.1f} std={np.std(h3e):.1f}")


if __name__ == '__main__':
    main()
