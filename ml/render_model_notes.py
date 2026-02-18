"""Render model notes to match real observations for residual computation.

Collects unique (midi_note, velocity_bucket) pairs from the real note
observations, then renders each one individually via the Rust preamp-bench
CLI. Extracts identical harmonic features from model output.

The render path matches DI recordings: reed -> pickup -> preamp, bypassing
power amp and speaker. This matches the OBM recording path (DI from preamp
output jack).

Usage:
    python render_model_notes.py --input harmonics.json --output model_harmonics.json
"""

import argparse
import json
import os
import subprocess
import sys
import numpy as np

from goertzel_utils import load_audio, extract_harmonics_fft, amps_to_dB, midi_to_freq
from extract_harmonics import WINDOWS, DECAY_TIMES, N_HARMONICS, rms

# Velocity buckets: map continuous velocity to 8 discrete levels
VELOCITY_BUCKETS = [20, 35, 50, 65, 80, 95, 110, 127]

# Render duration per note
NOTE_DURATION_S = 2.0

PROJECT_DIR = os.path.join(os.path.dirname(__file__), "..")


def bucket_velocity(velocity_midi):
    """Map a MIDI velocity to the nearest bucket value."""
    return min(VELOCITY_BUCKETS, key=lambda b: abs(b - velocity_midi))


def collect_unique_pairs(features):
    """Collect unique (midi_note, velocity_bucket) pairs from real observations."""
    pairs = set()
    for f in features:
        midi = f["midi_note"]
        vel = bucket_velocity(f.get("velocity_midi", 80))
        pairs.add((midi, vel))
    return sorted(pairs)


def render_note(midi, velocity, output_path):
    """Render a single note via preamp-bench CLI.

    Renders reed -> pickup -> preamp (DI path), bypassing power amp and speaker
    to match OBM recording conditions.

    Returns True on success.
    """
    cmd = [
        "cargo", "run", "-p", "preamp-bench", "--release", "--",
        "render",
        "--note", str(midi),
        "--velocity", str(velocity),
        "--duration", str(NOTE_DURATION_S),
        "--volume", "1.0",
        "--no-poweramp",
        "--speaker", "0.0",
        "--output", output_path,
    ]
    result = subprocess.run(
        cmd,
        capture_output=True, text=True,
        cwd=PROJECT_DIR
    )
    if result.returncode != 0:
        print(f"  RENDER FAILED for MIDI {midi} vel {velocity}:")
        print(f"    {result.stderr[:500]}")
        return False
    return True


def render_all_notes(pairs, output_dir):
    """Render all (midi, vel) pairs, returning dict of WAV paths."""
    os.makedirs(output_dir, exist_ok=True)
    wav_paths = {}

    # Build once in release mode before rendering
    print("  Building preamp-bench (release)...")
    build_result = subprocess.run(
        ["cargo", "build", "-p", "preamp-bench", "--release"],
        capture_output=True, text=True,
        cwd=PROJECT_DIR
    )
    if build_result.returncode != 0:
        print(f"  BUILD FAILED:\n{build_result.stderr[:1000]}")
        return {}
    print("  Build successful")

    for i, (midi, vel) in enumerate(pairs):
        wav_path = os.path.join(output_dir, f"model_{midi}_{vel}.wav")
        print(f"  [{i+1}/{len(pairs)}] Rendering MIDI {midi:>3} vel {vel:>3}...", end="", flush=True)

        if render_note(midi, vel, wav_path):
            wav_paths[(midi, vel)] = wav_path
            print(" OK")
        else:
            print(" FAILED")

    return wav_paths


def extract_model_features(wav_paths, pairs):
    """Extract harmonic features from individually rendered model WAVs.

    Each WAV contains a single note starting at t=0 with known timing.

    Returns dict mapping (midi, vel) -> feature dict.
    """
    features = {}

    for midi, vel in pairs:
        key = (midi, vel)
        if key not in wav_paths:
            continue

        wav_path = wav_paths[key]
        try:
            audio, sr = load_audio(wav_path)
        except Exception as e:
            print(f"  ERROR loading {wav_path}: {e}")
            continue

        f0 = midi_to_freq(midi)
        note_duration = len(audio) / sr

        feat = {
            "midi_note": midi,
            "velocity_midi": vel,
            "f0": f0,
            "duration_s": round(note_duration, 4),
            "windows": {},
        }

        # Extract harmonics at each window
        for win_name, (win_start, win_end, min_dur) in WINDOWS.items():
            actual_end = min(win_end, note_duration)
            if win_start >= actual_end:
                feat["windows"][win_name] = None
                continue

            start_idx = int(win_start * sr)
            end_idx = int(actual_end * sr)
            segment = audio[start_idx:end_idx]

            if len(segment) < 128:
                feat["windows"][win_name] = None
                continue

            amps, freqs = extract_harmonics_fft(segment, sr, f0, N_HARMONICS)
            dB = amps_to_dB(amps)

            feat["windows"][win_name] = {
                "amps_linear": [round(float(a), 8) for a in amps],
                "amps_dB_rel_H1": [round(float(d), 2) for d in dB],
                "freqs_hz": [round(float(f), 2) for f in freqs],
            }

        # Decay rate
        decay_amps = []
        for t in DECAY_TIMES:
            if t >= note_duration - 0.05:
                decay_amps.append(None)
                continue
            start_idx = int(t * sr)
            end_idx = min(int((t + 0.100) * sr), len(audio))
            if end_idx - start_idx < 64:
                decay_amps.append(None)
                continue
            segment = audio[start_idx:end_idx]
            h1_amp = extract_harmonics_fft(segment, sr, f0, 1)[0][0]
            decay_amps.append(round(float(h1_amp), 8))

        valid_points = [(t, a) for t, a in zip(DECAY_TIMES, decay_amps)
                        if a is not None and a > 1e-15]
        decay_rate = None
        if len(valid_points) >= 3:
            times = np.array([p[0] for p in valid_points])
            log_amps = np.log10(np.array([p[1] for p in valid_points]))
            if np.std(times) > 0:
                slope, _ = np.polyfit(times, log_amps, 1)
                decay_rate = round(float(-20.0 * slope), 2)

        feat["decay"] = {
            "times_s": DECAY_TIMES,
            "h1_amps": decay_amps,
            "decay_rate_dB_s": decay_rate,
        }

        # Overshoot
        overshoot_dB = None
        peak_end = min(int(0.010 * sr), len(audio))
        sustain_start = int(0.100 * sr)
        sustain_end = min(int(0.200 * sr), len(audio))
        if peak_end > 0 and sustain_end > sustain_start:
            peak_rms = rms(audio[0:peak_end])
            sustain_rms = rms(audio[sustain_start:sustain_end])
            overshoot_dB = round(float(20.0 * np.log10(peak_rms / sustain_rms)), 2)
        feat["overshoot_dB"] = overshoot_dB

        # Spectral centroid
        for win_name in ["attack", "sustain"]:
            win_data = feat["windows"].get(win_name)
            if win_data is None:
                feat[f"centroid_{win_name}"] = None
                continue
            amps_list = win_data["amps_linear"]
            freqs_list = win_data["freqs_hz"]
            amps_arr = np.array(amps_list)
            freqs_arr = np.array(freqs_list)
            valid = amps_arr > 1e-15
            if np.any(valid):
                centroid = float(np.sum(freqs_arr[valid] * amps_arr[valid]) /
                               np.sum(amps_arr[valid]))
                feat[f"centroid_{win_name}"] = round(centroid, 1)
            else:
                feat[f"centroid_{win_name}"] = None

        features[key] = feat

    return features


def main():
    parser = argparse.ArgumentParser(description="Render model notes and extract features")
    parser.add_argument("--input", default="harmonics.json",
                        help="Input harmonics JSON (to determine which notes to render)")
    parser.add_argument("--output", default="model_harmonics.json",
                        help="Output JSON with model harmonic features")
    parser.add_argument("--render-dir", default=None,
                        help="Directory for rendered WAVs (default: ml_data/renders)")
    args = parser.parse_args()

    input_path = os.path.join(os.path.dirname(__file__), args.input)
    with open(input_path) as f:
        real_features = json.load(f)

    # Collect unique (midi, velocity_bucket) pairs
    pairs = collect_unique_pairs(real_features)
    print(f"Unique (midi, velocity) pairs: {len(pairs)}")
    for midi, vel in pairs:
        print(f"  MIDI {midi:>3} vel {vel:>3}")

    # Render directory
    render_dir = args.render_dir or os.path.join(os.path.dirname(__file__), "ml_data", "renders")

    # Render all notes
    print(f"\nRendering {len(pairs)} notes...")
    wav_paths = render_all_notes(pairs, render_dir)
    print(f"  Successfully rendered: {len(wav_paths)}/{len(pairs)}")

    if not wav_paths:
        print("ERROR: No notes rendered successfully")
        sys.exit(1)

    # Extract features
    print("\nExtracting model harmonic features...")
    model_features = extract_model_features(wav_paths, pairs)
    print(f"  Extracted features for {len(model_features)} note/velocity combos")

    # Convert to serializable format (tuple keys -> string keys)
    output_data = {}
    for (midi, vel), feat in model_features.items():
        key = f"{midi}_{vel}"
        output_data[key] = feat

    output_path = os.path.join(os.path.dirname(__file__), args.output)
    with open(output_path, 'w') as f:
        json.dump(output_data, f, indent=2)
    print(f"\nSaved to {output_path}")


if __name__ == "__main__":
    main()
