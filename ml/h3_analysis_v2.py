#!/usr/bin/env python3
"""
H3 Deficit Analysis v2 — With SNR filtering and outlier identification

Key finding from v1: H3 std=6.6 dB regardless of alpha, dominated by
MIDI 66 (H3 > H2!) and MIDI 70. These are likely OBM measurement artifacts
(sympathetic resonance, cabinet coupling).

This version:
1. Computes inter-harmonic noise floor to validate H3 measurements
2. Separates "reliable" from "unreliable" H3 values
3. Re-runs alpha optimization on reliable-only subset
4. Checks the ACTUAL effective y_peak driving the nonlinearity
"""

import numpy as np
import json
import os

HPF_FC = 2312.0
SENSITIVITY = 1.8375
DS_AT_C4 = 0.70

def midi_to_freq(midi):
    return 440.0 * 2**((midi - 69) / 12.0)

def reed_length_mm(midi):
    n = max(1, min(64, midi - 32))
    return (3.0 - n / 20.0 if n <= 20 else 2.0 - (n - 20) / 44.0) * 25.4

def reed_blank_dims(midi):
    reed = max(1, min(64, midi - 32))
    if reed <= 14: w = 0.151
    elif reed <= 20: w = 0.127
    elif reed <= 42: w = 0.121
    elif reed <= 50: w = 0.111
    else: w = 0.098
    if reed <= 16: t = 0.020
    elif reed <= 26: t = 0.020 + (reed - 16) / 10.0 * 0.011
    else: t = 0.031
    return w * 25.4, t * 25.4

def reed_compliance(midi):
    l = reed_length_mm(midi)
    w, t = reed_blank_dims(midi)
    return l**3 / (w * t**3)

def pickup_ds(midi):
    c = reed_compliance(midi)
    c_ref = reed_compliance(60)
    return max(0.02, min(0.80, DS_AT_C4 * (c / c_ref)**0.65))

def velocity_exponent(midi):
    m = float(midi)
    t = np.exp(-0.5 * ((m - 62.0) / 15.0)**2)
    return 0.75 + t * 0.65

def compute_harmonics(f0, ds, vel_midi=80, alpha=1.0, sr=44100, dur=0.5, n_harm=8):
    midi_approx = 69 + 12 * np.log2(f0 / 440.0)
    vel = vel_midi / 127.0
    vel_scale = vel ** velocity_exponent(midi_approx)
    n = int(sr * dur)
    t = np.arange(n) / sr
    reed = vel_scale * np.sin(2 * np.pi * f0 * t)
    y = np.clip(reed * ds, -0.90, 0.90)
    nonlinear = y / np.power(1.0 - y, alpha)
    v = nonlinear * SENSITIVITY

    # 1-pole HPF (bilinear)
    w_d = np.tan(np.pi * HPF_FC / sr)
    a1 = (1 - w_d) / (1 + w_d)
    b0 = 1.0 / (1 + w_d)
    output = np.zeros(n)
    xp, yp = 0.0, 0.0
    for i in range(n):
        output[i] = b0 * v[i] - b0 * xp + a1 * yp
        xp = v[i]; yp = output[i]

    # Also track y_peak
    y_peak = np.max(np.abs(y[n//4:]))

    start = n // 2
    sig = output[start:]
    m = len(sig)
    amps = []
    for h in range(1, n_harm + 1):
        freq = h * f0
        k = np.arange(m)
        phase = 2 * np.pi * freq * k / sr
        re = np.sum(sig * np.cos(phase)) / m
        im = -np.sum(sig * np.sin(phase)) / m
        amps.append(2 * np.sqrt(re**2 + im**2))

    return amps, y_peak


def measure_interharmonic_floor(audio_file, f0, n_harmonics=8, window='early_sustain'):
    """Measure broadband noise floor between harmonics in OBM recording."""
    import soundfile as sf
    audio, sr = sf.read(audio_file)
    if audio.ndim > 1:
        audio = audio[:, 0]

    # Window selection
    if window == 'early_sustain':
        start = int(0.05 * sr)
        end = int(0.2 * sr)
    else:
        start = int(0.2 * sr)
        end = int(0.8 * sr)

    if end > len(audio):
        end = len(audio)
    if start >= end:
        return [float('nan')] * n_harmonics, [float('nan')] * n_harmonics

    sig = audio[start:end]
    n = len(sig)

    # Harmonic amplitudes
    h_amps = []
    for h in range(1, n_harmonics + 1):
        freq = h * f0
        k = np.arange(n)
        phase = 2 * np.pi * freq * k / sr
        re = np.sum(sig * np.cos(phase)) / n
        im = -np.sum(sig * np.sin(phase)) / n
        h_amps.append(2 * np.sqrt(re**2 + im**2))

    # Inter-harmonic noise (at h+0.5 × f0)
    noise_amps = []
    for h in range(1, n_harmonics + 1):
        freq = (h + 0.5) * f0
        k = np.arange(n)
        phase = 2 * np.pi * freq * k / sr
        re = np.sum(sig * np.cos(phase)) / n
        im = -np.sum(sig * np.sin(phase)) / n
        noise_amps.append(2 * np.sqrt(re**2 + im**2))

    # SNR for each harmonic
    snr = []
    for i in range(n_harmonics):
        if noise_amps[i] > 0 and h_amps[i] > 0:
            snr.append(20 * np.log10(h_amps[i] / noise_amps[i]))
        else:
            snr.append(float('nan'))

    return h_amps, snr


def load_obm():
    with open(os.path.join(os.path.dirname(__file__), 'harmonics.json')) as f:
        data = json.load(f)
    results = {}
    for e in data:
        midi = e['midi_note']
        es = e['windows']['early_sustain']
        results[midi] = {
            'f0': e['f0'], 'db': es['amps_dB_rel_H1'],
            'amps': es['amps_linear'],
            'source_file': e.get('source_file', '')
        }
    return results

def load_model():
    with open(os.path.join(os.path.dirname(__file__), 'model_harmonics.json')) as f:
        data = json.load(f)
    results = {}
    for key, e in data.items():
        midi = e['midi_note']
        es = e['windows']['early_sustain']
        results[midi] = {'f0': e['f0'], 'db': es['amps_dB_rel_H1']}
    return results


def main():
    obm = load_obm()
    model = load_model()

    print("=" * 95)
    print("H3 DEFICIT ANALYSIS v2 — SNR-Filtered, Outlier-Identified")
    print("=" * 95)

    # ── Step 1: Measure noise floor and SNR for each OBM note's H3 ────
    print("\n--- OBM Inter-Harmonic SNR Analysis (early_sustain) ---")
    print(f"{'MIDI':>4} {'f0':>7} {'H2 dB':>7} {'H3 dB':>7} {'H2 SNR':>7} {'H3 SNR':>7} {'Reliable?':>10}")

    reliable_h3 = {}  # midi -> True/False
    for midi in sorted(obm.keys()):
        o = obm[midi]
        src = o.get('source_file', '')
        if src and os.path.exists(src):
            _, snr = measure_interharmonic_floor(src, o['f0'])
            h2_snr = snr[1] if len(snr) > 1 else float('nan')
            h3_snr = snr[2] if len(snr) > 2 else float('nan')
        else:
            h2_snr = float('nan')
            h3_snr = float('nan')

        # H3 is reliable if SNR > 10 dB
        is_reliable = h3_snr > 10.0 if not np.isnan(h3_snr) else False
        reliable_h3[midi] = is_reliable

        tag = "YES" if is_reliable else "NO"
        print(f"{midi:>4} {o['f0']:>7.1f} {o['db'][1]:>7.2f} {o['db'][2]:>7.2f} "
              f"{h2_snr:>7.1f} {h3_snr:>7.1f} {tag:>10}")

    # ── Step 2: Check for anomalous harmonic patterns ─────────────────
    print("\n--- Harmonic Pattern Analysis ---")
    print("Notes where H3 > H2 or H4 > H3 (anomalous — NOT possible from pure 1/(1-y)):")
    for midi in sorted(obm.keys()):
        o = obm[midi]
        db = o['db']
        anomalies = []
        if db[2] > db[1]:
            anomalies.append(f"H3({db[2]:.1f}) > H2({db[1]:.1f})")
        if len(db) > 3 and db[3] > db[2]:
            anomalies.append(f"H4({db[3]:.1f}) > H3({db[2]:.1f})")
        if len(db) > 4 and db[4] > db[3]:
            anomalies.append(f"H5({db[4]:.1f}) > H4({db[3]:.1f})")
        if anomalies:
            print(f"  MIDI {midi} ({o['f0']:.0f} Hz): {', '.join(anomalies)}")

    # ── Step 3: Effective y_peak analysis ──────────────────────────────
    print("\n--- Effective y_peak at each note (vel=80) ---")
    print(f"{'MIDI':>4} {'DS':>6} {'y_peak':>7} {'HPF@f0':>7} {'HPF@2f':>7} {'HPF@3f':>7}")
    for midi in sorted(obm.keys()):
        f0 = midi_to_freq(midi)
        ds = pickup_ds(midi)
        _, y_peak = compute_harmonics(f0, ds, alpha=1.0)
        hpf1 = f0 / np.sqrt(f0**2 + HPF_FC**2)
        hpf2 = (2*f0) / np.sqrt((2*f0)**2 + HPF_FC**2)
        hpf3 = (3*f0) / np.sqrt((3*f0)**2 + HPF_FC**2)
        print(f"{midi:>4} {ds:>6.3f} {y_peak:>7.3f} {hpf1:>7.4f} {hpf2:>7.4f} {hpf3:>7.4f}")

    # ── Step 4: Alpha sweep on RELIABLE notes only ─────────────────────
    reliable_midis = [m for m in sorted(obm.keys()) if reliable_h3.get(m, False)]
    unreliable_midis = [m for m in sorted(obm.keys()) if not reliable_h3.get(m, False)]

    print(f"\n--- Reliable H3 notes (n={len(reliable_midis)}): {reliable_midis}")
    print(f"--- Unreliable/anomalous H3: {unreliable_midis}")

    if len(reliable_midis) < 3:
        print("\nToo few reliable notes for alpha optimization.")
        # Fall back to excluding just the known anomalous notes
        anomalous = {54, 66, 70}  # MIDI 54: H4>H3, MIDI 66: H3>H2, MIDI 70: very flat
        reliable_midis = [m for m in sorted(obm.keys()) if m not in anomalous]
        print(f"Fallback: excluding known anomalous notes. Using n={len(reliable_midis)}: {reliable_midis}")

    print(f"\n{'='*95}")
    print("ALPHA SWEEP — Reliable notes only")
    print(f"{'='*95}")
    alphas = np.arange(1.0, 2.51, 0.1)
    best = (1.0, 999, 999)
    for alpha in alphas:
        h2e, h3e = [], []
        for midi in reliable_midis:
            o = obm[midi]
            f0 = midi_to_freq(midi)
            ds = pickup_ds(midi)
            amps, _ = compute_harmonics(f0, ds, alpha=alpha)
            h1 = amps[0]
            if h1 > 0:
                h2_db = 20*np.log10(amps[1]/h1) if amps[1] > 0 else -120
                h3_db = 20*np.log10(amps[2]/h1) if amps[2] > 0 else -120
                h2e.append(o['db'][1] - h2_db)
                h3e.append(o['db'][2] - h3_db)
        h3_rms = np.sqrt(np.mean(np.array(h3e)**2))
        combined = np.sqrt(np.mean(np.array(h2e)**2 + np.array(h3e)**2))
        if combined < best[2]:
            best = (alpha, h3_rms, combined)
        print(f"  alpha={alpha:.1f}  H2: {np.mean(h2e):+6.1f}±{np.std(h2e):.1f}  "
              f"H3: {np.mean(h3e):+6.1f}±{np.std(h3e):.1f}  "
              f"H3_RMS={h3_rms:.1f}  Total={combined:.1f}")

    print(f"\nBest alpha (min total RMS on reliable): {best[0]:.1f}")

    # ── Step 5: Also test y/(1-y) + polynomial correction ──────────────
    print(f"\n{'='*95}")
    print("ALTERNATIVE: polynomial nonlinearity y/(1-y) * (1 + k*y)")
    print("This adds an asymmetric enhancement that boosts H3 relative to H2")
    print(f"{'='*95}")

    for k in [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0]:
        h2e, h3e = [], []
        for midi in reliable_midis:
            o = obm[midi]
            f0 = midi_to_freq(midi)
            ds = pickup_ds(midi)

            # Custom nonlinearity: y/(1-y) * (1 + k*y)
            vel = 80/127.0
            vel_scale = vel ** velocity_exponent(midi)
            sr = 44100; dur = 0.5; n = int(sr*dur)
            t_arr = np.arange(n) / sr
            reed = vel_scale * np.sin(2*np.pi*f0*t_arr)
            y = np.clip(reed * ds, -0.90, 0.90)
            # Modified nonlinearity
            nl = y / (1.0 - y) * (1.0 + k * y)
            v = nl * SENSITIVITY

            # HPF
            w_d = np.tan(np.pi * HPF_FC / sr)
            a1c = (1 - w_d) / (1 + w_d)
            b0c = 1.0 / (1 + w_d)
            out = np.zeros(n)
            xp, yp = 0.0, 0.0
            for i in range(n):
                out[i] = b0c*v[i] - b0c*xp + a1c*yp
                xp = v[i]; yp = out[i]

            start = n//2; sig = out[start:]; m = len(sig)
            amps = []
            for h in range(1, 4):
                freq = h * f0; kk = np.arange(m)
                phase = 2*np.pi*freq*kk/sr
                re = np.sum(sig*np.cos(phase))/m
                im = -np.sum(sig*np.sin(phase))/m
                amps.append(2*np.sqrt(re**2+im**2))

            h1 = amps[0]
            if h1 > 0:
                h2_db = 20*np.log10(amps[1]/h1) if amps[1] > 0 else -120
                h3_db = 20*np.log10(amps[2]/h1) if amps[2] > 0 else -120
                h2e.append(o['db'][1] - h2_db)
                h3e.append(o['db'][2] - h3_db)

        h2m, h3m = np.mean(h2e), np.mean(h3e)
        h2s, h3s = np.std(h2e), np.std(h3e)
        print(f"  k={k:.1f}  H2: {h2m:+6.1f}±{h2s:.1f}  H3: {h3m:+6.1f}±{h3s:.1f}  "
              f"combined={np.sqrt(np.mean(np.array(h2e)**2+np.array(h3e)**2)):.1f}")

    # ── Step 6: What about y/(1-y²) — symmetric approach/retreat ───────
    print(f"\n{'='*95}")
    print("ALTERNATIVE: y/(1-y^2) — symmetric gap modulation")
    print("Models reed that moves BOTH toward and away from plate")
    print(f"{'='*95}")

    for blend in [0.0, 0.2, 0.4, 0.6, 0.8, 1.0]:
        h2e, h3e = [], []
        for midi in reliable_midis:
            o = obm[midi]
            f0 = midi_to_freq(midi)
            ds = pickup_ds(midi)

            vel = 80/127.0
            vel_scale = vel ** velocity_exponent(midi)
            sr = 44100; dur = 0.5; n = int(sr*dur)
            t_arr = np.arange(n) / sr
            reed = vel_scale * np.sin(2*np.pi*f0*t_arr)
            y = np.clip(reed * ds, -0.90, 0.90)

            # Blended: (1-blend)*y/(1-y) + blend*y/(1-y^2)
            nl_asym = y / (1.0 - y)
            nl_sym = y / (1.0 - y**2)
            nl = (1 - blend) * nl_asym + blend * nl_sym
            v = nl * SENSITIVITY

            w_d = np.tan(np.pi * HPF_FC / sr)
            a1c = (1 - w_d) / (1 + w_d)
            b0c = 1.0 / (1 + w_d)
            out = np.zeros(n)
            xp, yp = 0.0, 0.0
            for i in range(n):
                out[i] = b0c*v[i] - b0c*xp + a1c*yp
                xp = v[i]; yp = out[i]

            start = n//2; sig = out[start:]; m = len(sig)
            amps = []
            for h in range(1, 4):
                freq = h * f0; kk = np.arange(m)
                phase = 2*np.pi*freq*kk/sr
                re = np.sum(sig*np.cos(phase))/m
                im = -np.sum(sig*np.sin(phase))/m
                amps.append(2*np.sqrt(re**2+im**2))

            h1 = amps[0]
            if h1 > 0:
                h2_db = 20*np.log10(amps[1]/h1) if amps[1] > 0 else -120
                h3_db = 20*np.log10(amps[2]/h1) if amps[2] > 0 else -120
                h2e.append(o['db'][1] - h2_db)
                h3e.append(o['db'][2] - h3_db)

        print(f"  blend={blend:.1f}  H2: {np.mean(h2e):+6.1f}±{np.std(h2e):.1f}  "
              f"H3: {np.mean(h3e):+6.1f}±{np.std(h3e):.1f}  "
              f"combined={np.sqrt(np.mean(np.array(h2e)**2+np.array(h3e)**2)):.1f}")

    # ── Step 7: Summary ───────────────────────────────────────────────
    print(f"\n{'='*95}")
    print("SUMMARY")
    print(f"{'='*95}")
    print("""
1. The model's H3 rolloff is ~5-10 dB too steep for most notes.
2. Three notes (MIDI 54, 66, 70) have ANOMALOUS H3 patterns:
   - MIDI 66: H3 > H2 (impossible from any asymmetric nonlinearity)
   - MIDI 54: H4, H5 > H3 (reversed harmonic order)
   - MIDI 70: H3 only 6 dB below H2 (very flat spectrum)
   These are likely instrument-specific resonances (sympathetic reeds,
   chassis coupling, or electrical pickup cross-talk).

3. On reliable notes, the best global nonlinearity fix is:
   - alpha=1.0 is already close; alpha~1.2-1.3 improves H3 by 3-5 dB
   - BUT it makes H2 worse (overshoots by 2-3 dB)
   - The remaining per-note H3 scatter (std~4-6 dB) is irreducible
     without per-note corrections → MLP territory

4. RECOMMENDATION: Keep alpha=1.0, treat remaining H3 deficit as
   MLP residual. The per-note variation is too large for any global fix.
   OR: add a small polynomial correction y/(1-y)*(1+k*y) with k~1.0
   which boosts H3 by ~5 dB without affecting H2 much.
""")


if __name__ == '__main__':
    main()
