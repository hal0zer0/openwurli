"""Harmonic feature extraction from scored note observations.

For each note scoring >= bronze, extracts:
- H1-H8 amplitudes at 3 time windows (attack, early_sustain, sustain)
- Decay rate from H1 amplitude at 6 time points
- Overshoot: peak RMS vs sustain RMS
- Spectral centroid at attack and sustain

Contaminated harmonics (per harmonic_mask from scoring) stored as NaN.

Usage:
    python extract_harmonics.py --input scored_notes.json --output harmonics.json
"""

import argparse
import json
import os
import sys
import numpy as np

from goertzel_utils import (
    load_audio, extract_harmonics_fft, extract_harmonics_goertzel,
    amps_to_dB, midi_to_freq
)

# Time windows relative to note onset (seconds)
WINDOWS = {
    "attack":        (0.000, 0.050, 0.100),  # start, end, min_note_duration
    "early_sustain": (0.050, 0.200, 0.250),
    "sustain":       (0.200, 0.800, 0.500),
}

# Decay measurement time points (seconds after onset)
DECAY_TIMES = [0.1, 0.3, 0.5, 0.8, 1.0, 1.5]

N_HARMONICS = 8


def rms(signal):
    """Root mean square of a signal."""
    if len(signal) == 0:
        return 1e-20
    return max(np.sqrt(np.mean(signal ** 2)), 1e-20)


def extract_note_features(audio, sr, note, use_goertzel=False):
    """Extract harmonic features for a single note.

    Args:
        audio: full audio array for the source file
        sr: sample rate
        note: note dict with onset_s, offset_s, midi_note, harmonic_mask, etc.
        use_goertzel: if True, use precise Goertzel (slower); else FFT

    Returns:
        features dict or None if extraction fails
    """
    onset_s = note["onset_s"]
    offset_s = note["offset_s"]
    duration_s = offset_s - onset_s
    midi = note["midi_note"]
    f0 = note.get("measured_f0", midi_to_freq(midi))
    harmonic_mask = note.get("harmonic_mask", [True] * N_HARMONICS)

    onset_sample = int(onset_s * sr)
    offset_sample = min(int(offset_s * sr), len(audio))

    if onset_sample >= len(audio) or onset_sample >= offset_sample:
        return None

    note_audio = audio[onset_sample:offset_sample]

    features = {
        "id": note["id"],
        "midi_note": midi,
        "f0": f0,
        "duration_s": round(duration_s, 4),
        "windows": {},
    }

    extract_fn = extract_harmonics_goertzel if use_goertzel else extract_harmonics_fft

    # Extract harmonics at each time window
    for win_name, (win_start, win_end, min_dur) in WINDOWS.items():
        if duration_s < min_dur:
            features["windows"][win_name] = None
            continue

        # Clip window to note duration
        actual_end = min(win_end, duration_s)
        if win_start >= actual_end:
            features["windows"][win_name] = None
            continue

        start_idx = int(win_start * sr)
        end_idx = int(actual_end * sr)
        segment = note_audio[start_idx:end_idx]

        if len(segment) < 128:  # too short for meaningful FFT
            features["windows"][win_name] = None
            continue

        if use_goertzel:
            amps = extract_fn(segment, sr, f0, N_HARMONICS)
            freqs = np.array([f0 * (h + 1) for h in range(N_HARMONICS)])
        else:
            amps, freqs = extract_fn(segment, sr, f0, N_HARMONICS)

        # Convert to dB relative to H1
        dB = amps_to_dB(amps)

        # Apply harmonic mask: NaN for contaminated harmonics
        amps_masked = np.array(amps, dtype=float)
        dB_masked = np.array(dB, dtype=float)
        freqs_masked = np.array(freqs, dtype=float)
        for h in range(N_HARMONICS):
            if not harmonic_mask[h]:
                amps_masked[h] = float('nan')
                dB_masked[h] = float('nan')
                freqs_masked[h] = float('nan')

        features["windows"][win_name] = {
            "amps_linear": [round(float(a), 8) if not np.isnan(a) else None
                           for a in amps_masked],
            "amps_dB_rel_H1": [round(float(d), 2) if not np.isnan(d) else None
                               for d in dB_masked],
            "freqs_hz": [round(float(f), 2) if not np.isnan(f) else None
                        for f in freqs_masked],
        }

    # Decay rate: H1 amplitude at multiple time points
    decay_amps = []
    for t in DECAY_TIMES:
        if t >= duration_s - 0.05:
            decay_amps.append(None)
            continue
        start_idx = int(t * sr)
        end_idx = min(int((t + 0.100) * sr), len(note_audio))
        if end_idx - start_idx < 64:
            decay_amps.append(None)
            continue
        segment = note_audio[start_idx:end_idx]
        if use_goertzel:
            h1_amp = extract_fn(segment, sr, f0, 1)[0]
        else:
            h1_amp = extract_fn(segment, sr, f0, 1)[0][0]
        decay_amps.append(round(float(h1_amp), 8))

    # Fit log-linear decay if we have enough points
    valid_points = [(t, a) for t, a in zip(DECAY_TIMES, decay_amps)
                    if a is not None and a > 1e-15]
    decay_rate_dB_s = None
    if len(valid_points) >= 3:
        times = np.array([p[0] for p in valid_points])
        log_amps = np.log10(np.array([p[1] for p in valid_points]))
        # Linear regression: log10(amp) = slope * t + intercept
        if np.std(times) > 0:
            slope, intercept = np.polyfit(times, log_amps, 1)
            decay_rate_dB_s = round(float(-20.0 * slope), 2)  # positive = decaying

    features["decay"] = {
        "times_s": DECAY_TIMES,
        "h1_amps": decay_amps,
        "decay_rate_dB_s": decay_rate_dB_s,
    }

    # Overshoot: peak RMS (0-10ms) / sustain RMS (100-200ms)
    overshoot_dB = None
    if duration_s >= 0.250:
        peak_start = 0
        peak_end = min(int(0.010 * sr), len(note_audio))
        sustain_start = int(0.100 * sr)
        sustain_end = min(int(0.200 * sr), len(note_audio))

        if peak_end > peak_start and sustain_end > sustain_start:
            peak_rms = rms(note_audio[peak_start:peak_end])
            sustain_rms = rms(note_audio[sustain_start:sustain_end])
            overshoot_dB = round(float(20.0 * np.log10(peak_rms / sustain_rms)), 2)

    features["overshoot_dB"] = overshoot_dB

    # Spectral centroid at attack and sustain windows
    for win_name in ["attack", "sustain"]:
        win_data = features["windows"].get(win_name)
        if win_data is None:
            features[f"centroid_{win_name}"] = None
            continue
        amps_list = win_data["amps_linear"]
        freqs_list = win_data["freqs_hz"]
        # Use only valid (non-NaN) harmonics
        valid = [(f, a) for f, a in zip(freqs_list, amps_list)
                 if f is not None and a is not None and a > 1e-15]
        if valid:
            freqs_v = np.array([v[0] for v in valid])
            amps_v = np.array([v[1] for v in valid])
            centroid = float(np.sum(freqs_v * amps_v) / np.sum(amps_v))
            features[f"centroid_{win_name}"] = round(centroid, 1)
        else:
            features[f"centroid_{win_name}"] = None

    return features


def extract_all_harmonics(notes, min_tier="bronze", use_goertzel_for_gold=True):
    """Extract harmonic features for all notes at or above min_tier.

    Args:
        notes: list of scored note dicts
        min_tier: minimum isolation tier to process
        use_goertzel_for_gold: if True, use precise Goertzel for gold-tier notes

    Returns:
        list of feature dicts
    """
    tier_order = {"gold": 3, "silver": 2, "bronze": 1, "reject": 0, "pending": -1}
    min_tier_val = tier_order.get(min_tier, 1)

    eligible = [n for n in notes
                if tier_order.get(n.get("isolation_tier", "reject"), 0) >= min_tier_val]

    print(f"Extracting harmonics for {len(eligible)} notes (>= {min_tier})...")

    # Group by source file to avoid reloading audio
    by_file = {}
    for note in eligible:
        sf = note["source_file"]
        if sf not in by_file:
            by_file[sf] = []
        by_file[sf].append(note)

    all_features = []
    for source_file, file_notes in sorted(by_file.items()):
        print(f"  Loading {os.path.basename(source_file)}...")
        try:
            audio, sr = load_audio(source_file)
        except Exception as e:
            print(f"    ERROR loading: {e}")
            continue

        for note in file_notes:
            is_gold = note.get("isolation_tier") == "gold"
            use_goertzel = use_goertzel_for_gold and is_gold

            features = extract_note_features(audio, sr, note, use_goertzel=use_goertzel)
            if features is not None:
                features["isolation_tier"] = note.get("isolation_tier", "unknown")
                features["isolation_score"] = note.get("isolation_score", 0.0)
                features["velocity_midi"] = note.get("velocity_midi", 80)
                features["source_file"] = note["source_file"]
                all_features.append(features)

        print(f"    Extracted {sum(1 for n in file_notes if True)} notes")

    return all_features


def print_summary(features):
    """Print extraction summary."""
    n = len(features)
    print(f"\nExtracted features for {n} notes")

    # Count by tier
    tiers = {}
    for f in features:
        t = f.get("isolation_tier", "unknown")
        tiers[t] = tiers.get(t, 0) + 1
    for t in ["gold", "silver", "bronze"]:
        print(f"  {t}: {tiers.get(t, 0)}")

    # Window coverage
    for win_name in WINDOWS:
        count = sum(1 for f in features if f["windows"].get(win_name) is not None)
        print(f"  {win_name} window: {count}/{n} notes")

    # Decay rate coverage
    decay_count = sum(1 for f in features if f["decay"]["decay_rate_dB_s"] is not None)
    print(f"  Decay rate: {decay_count}/{n} notes")

    # Overshoot coverage
    os_count = sum(1 for f in features if f["overshoot_dB"] is not None)
    print(f"  Overshoot: {os_count}/{n} notes")


def main():
    parser = argparse.ArgumentParser(description="Extract harmonic features from scored notes")
    parser.add_argument("--input", default="scored_notes.json",
                        help="Input JSON from score_isolation.py")
    parser.add_argument("--output", default="harmonics.json",
                        help="Output JSON with harmonic features")
    parser.add_argument("--min-tier", default="bronze",
                        choices=["gold", "silver", "bronze"],
                        help="Minimum isolation tier to extract")
    parser.add_argument("--goertzel-all", action="store_true",
                        help="Use Goertzel for all notes (slow but precise)")
    args = parser.parse_args()

    input_path = os.path.join(os.path.dirname(__file__), args.input)
    with open(input_path) as f:
        notes = json.load(f)

    features = extract_all_harmonics(
        notes,
        min_tier=args.min_tier,
        use_goertzel_for_gold=not args.goertzel_all)

    print_summary(features)

    output_path = os.path.join(os.path.dirname(__file__), args.output)
    with open(output_path, 'w') as f:
        json.dump(features, f, indent=2)
    print(f"\nSaved to {output_path}")


if __name__ == "__main__":
    main()
