"""Note detection from Wurlitzer recordings.

Two modes:
1. basic-pitch for polyphonic recordings -> note events with onset/offset/pitch/amplitude
2. Direct injection for OBM isolated notes -> gold-tier observations with known parameters

Usage:
    python extract_notes.py --input-dir input --output notes.json
    python extract_notes.py --obm-only --output notes.json
"""

import argparse
import json
import os
import sys
import numpy as np

from goertzel_utils import load_audio, extract_harmonics_fft, midi_to_freq


# OBM file mapping: filename -> (MIDI note, actual frequency Hz)
# From compare_ltas.py -- these are isolated single notes, gold-tier quality
OBM_DIR = os.path.join(os.path.dirname(__file__), "input",
                        "5726__oldbassman__wurlitzer-200a")

OBM_FILES = {
    "87994__oldbassman__d3.wav":  (50, 147.7),
    "87998__oldbassman__f3.wav":  (54, 186.2),   # labeled F3, actually F#3
    "87988__oldbassman__a3.wav":  (58, 233.8),   # labeled A3, actually Bb3
    "87995__oldbassman__d4.wav":  (62, 293.8),
    "87999__oldbassman__f4.wav":  (66, 370.8),   # labeled F4, actually F#4
    "87989__oldbassman__a4.wav":  (70, 466.2),   # labeled A4, actually Bb4
    "87992__oldbassman__d5.wav":  (74, 587.7),
    "87993__oldbassman__f5.wav":  (78, 738.5),   # labeled F5, actually F#5
    "87990__oldbassman__a5.wav":  (82, 932.3),   # labeled A5, actually Bb5
    "87996__oldbassman__d6.wav":  (86, 1175.4),
    "88000__oldbassman__f6.wav":  (90, 1481.5),  # labeled F6, actually F#6
    "87991__oldbassman__a6.wav":  (94, 1863.1),  # labeled A6, actually Bb6
    "87997__oldbassman__d7.wav":  (98, 2363.1),
}


def detect_obm_onset(data, sr, threshold_frac=0.10):
    """Find onset in OBM isolated note by threshold detection."""
    peak = np.max(np.abs(data))
    if peak < 1e-10:
        return 0.0
    threshold = threshold_frac * peak
    for i in range(len(data)):
        if abs(data[i]) > threshold:
            return i / sr
    return 0.0


def detect_obm_offset(data, sr, onset_s, threshold_frac=0.01):
    """Find offset: last sample above 1% of peak, searching backwards."""
    peak = np.max(np.abs(data))
    if peak < 1e-10:
        return len(data) / sr
    threshold = threshold_frac * peak
    for i in range(len(data) - 1, 0, -1):
        if abs(data[i]) > threshold:
            return i / sr
    return len(data) / sr


def extract_obm_notes():
    """Extract gold-tier note observations from OBM isolated recordings."""
    notes = []
    for fname, (midi, f0) in sorted(OBM_FILES.items(), key=lambda x: x[1][0]):
        path = os.path.join(OBM_DIR, fname)
        if not os.path.exists(path):
            print(f"  WARNING: missing {path}")
            continue
        data, sr = load_audio(path)
        onset_s = detect_obm_onset(data, sr)
        offset_s = detect_obm_offset(data, sr, onset_s)
        notes.append({
            "id": f"obm_{midi}",
            "onset_s": round(onset_s, 4),
            "offset_s": round(offset_s, 4),
            "midi_note": midi,
            "measured_f0": f0,
            "amplitude": 0.63,  # mf = MIDI 80 / 127
            "velocity_midi": 80,
            "source_file": os.path.abspath(path),
            "source_type": "obm_isolated",
            "isolation_tier": "gold",
        })
        print(f"  OBM {midi:>3} ({fname}): onset={onset_s:.3f}s  dur={offset_s - onset_s:.2f}s")
    return notes


def correct_octave_errors(notes, audio, sr):
    """Auto-correct octave errors and filter weak detections.

    For each note, checks spectral energy at f0, f0/2, and 2*f0 in the
    early sustain window (50-200ms after onset). If an octave neighbor is
    >2x stronger, shifts the note. If no clear peak exists, marks for removal.

    Modifies notes in-place. Returns (corrected_count, removed_count).
    """
    corrected = 0
    to_remove = []

    for i, note in enumerate(notes):
        midi = note["midi_note"]
        f0 = midi_to_freq(midi)
        onset_sample = int(note["onset_s"] * sr)

        # Early sustain window: 50-200ms after onset
        start = onset_sample + int(0.050 * sr)
        end = min(onset_sample + int(0.200 * sr), len(audio))
        if end - start < 128:
            # Fall back to 0-150ms
            start = onset_sample
            end = min(onset_sample + int(0.150 * sr), len(audio))
            if end - start < 128:
                to_remove.append(i)
                continue

        segment = audio[start:end]

        # Measure energy at f0
        amp_f0 = extract_harmonics_fft(segment, sr, f0, 1)[0][0]

        # Noise floor: energy between harmonics (f0 * 1.37)
        f_noise = f0 * 1.37
        amp_noise = 1e-20
        if f_noise < sr / 2 - 200:
            amp_noise = max(extract_harmonics_fft(segment, sr, f_noise, 1)[0][0], 1e-20)

        # Check octave above (2*f0)
        amp_up = 0.0
        if f0 * 2 < sr / 2 - 200:
            amp_up = extract_harmonics_fft(segment, sr, f0 * 2, 1)[0][0]

        # Check octave below (f0/2)
        amp_down = 0.0
        if f0 / 2 > 30:
            amp_down = extract_harmonics_fft(segment, sr, f0 / 2, 1)[0][0]

        # Decision logic
        best_amp = amp_f0
        best_midi = midi

        if amp_up > amp_f0 * 2.0 and amp_up > amp_noise * 5:
            best_amp = amp_up
            best_midi = midi + 12
        elif amp_down > amp_f0 * 2.0 and amp_down > amp_noise * 5:
            best_amp = amp_down
            best_midi = midi - 12

        # Filter: reject if best peak is weak relative to noise
        peak_to_noise = best_amp / max(amp_noise, 1e-20)
        if peak_to_noise < 3.0:
            to_remove.append(i)
            continue

        # Apply correction
        if best_midi != midi:
            note["midi_note"] = best_midi
            note["measured_f0"] = round(midi_to_freq(best_midi), 2)
            note["octave_corrected"] = midi  # store original for traceability
            corrected += 1

    # Remove weak notes (iterate in reverse to preserve indices)
    for i in reversed(to_remove):
        notes.pop(i)

    return corrected, len(to_remove)


def extract_polyphonic_notes(audio_path, onset_threshold=0.5, frame_threshold=0.3):
    """Run basic-pitch on a polyphonic recording.

    Includes octave auto-correction and weak peak filtering.
    Returns list of note dicts.
    """
    from basic_pitch.inference import predict

    print(f"  Running basic-pitch on {os.path.basename(audio_path)}...")
    model_output, midi_data, note_events = predict(
        audio_path,
        onset_threshold=onset_threshold,
        frame_threshold=frame_threshold,
        minimum_note_length=58.0,   # Wurlitzer staccato can be short (ms)
        minimum_frequency=55.0,     # A1
        maximum_frequency=2100.0,   # ~C7
    )

    notes = []
    basename = os.path.splitext(os.path.basename(audio_path))[0]
    for i, (start_s, end_s, pitch_midi, amplitude, pitch_bends) in enumerate(note_events):
        pitch_midi = int(round(pitch_midi))
        # Map amplitude to velocity (basic-pitch amplitude is 0-1 float)
        velocity_midi = max(1, min(127, int(round(amplitude * 127))))
        notes.append({
            "id": f"{basename}_{i:04d}",
            "onset_s": round(float(start_s), 4),
            "offset_s": round(float(end_s), 4),
            "midi_note": pitch_midi,
            "measured_f0": round(440.0 * 2.0 ** ((pitch_midi - 69) / 12.0), 2),
            "amplitude": round(float(amplitude), 4),
            "velocity_midi": velocity_midi,
            "source_file": os.path.abspath(audio_path),
            "source_type": "basic_pitch",
            "isolation_tier": "pending",  # scored later
        })

    raw_count = len(notes)
    print(f"    -> {raw_count} notes detected, verifying pitches...")

    # Load audio for octave correction
    audio, sr = load_audio(audio_path)
    corrected, removed = correct_octave_errors(notes, audio, sr)
    print(f"    -> {corrected} octave-corrected, {removed} weak-removed, "
          f"{len(notes)} kept")

    return notes


def find_recordings(input_dir):
    """Find all WAV/FLAC recordings in input_dir (excluding OBM isolated notes)."""
    recordings = []
    obm_subdir = os.path.basename(os.path.normpath(OBM_DIR))
    for entry in sorted(os.listdir(input_dir)):
        path = os.path.join(input_dir, entry)
        if os.path.isdir(path):
            # Skip OBM directory -- handled separately
            if obm_subdir in entry:
                continue
            # Recurse into subdirectories
            for sub in sorted(os.listdir(path)):
                subpath = os.path.join(path, sub)
                if sub.lower().endswith(('.wav', '.flac')) and os.path.isfile(subpath):
                    recordings.append(subpath)
        elif entry.lower().endswith(('.wav', '.flac')) and os.path.isfile(path):
            # Skip model renders and test outputs
            if any(skip in entry.lower() for skip in ['vurli', 'model', 'output', 'test']):
                continue
            recordings.append(path)
    return recordings


def main():
    parser = argparse.ArgumentParser(description="Extract note events from Wurlitzer recordings")
    parser.add_argument("--input-dir", default=os.path.join(os.path.dirname(__file__), "input"),
                        help="Directory containing recordings")
    parser.add_argument("--output", default="notes.json",
                        help="Output JSON file")
    parser.add_argument("--obm-only", action="store_true",
                        help="Only extract OBM isolated notes")
    parser.add_argument("--onset-threshold", type=float, default=0.5,
                        help="basic-pitch onset threshold")
    parser.add_argument("--frame-threshold", type=float, default=0.3,
                        help="basic-pitch frame threshold")
    args = parser.parse_args()

    all_notes = []

    # Always include OBM isolated notes
    print("Extracting OBM isolated notes...")
    obm_notes = extract_obm_notes()
    all_notes.extend(obm_notes)
    print(f"  {len(obm_notes)} OBM notes extracted")

    if not args.obm_only:
        # Find and process polyphonic recordings
        recordings = find_recordings(args.input_dir)
        print(f"\nFound {len(recordings)} polyphonic recordings:")
        for r in recordings:
            print(f"  {os.path.basename(r)}")

        for rec_path in recordings:
            try:
                notes = extract_polyphonic_notes(
                    rec_path,
                    onset_threshold=args.onset_threshold,
                    frame_threshold=args.frame_threshold)
                all_notes.extend(notes)
            except Exception as e:
                print(f"  ERROR processing {rec_path}: {e}")

    # Summary
    print(f"\nTotal: {len(all_notes)} notes")
    by_type = {}
    for n in all_notes:
        t = n["source_type"]
        by_type[t] = by_type.get(t, 0) + 1
    for t, c in sorted(by_type.items()):
        print(f"  {t}: {c}")

    # MIDI range coverage
    midis = [n["midi_note"] for n in all_notes]
    print(f"  MIDI range: {min(midis)}-{max(midis)}")

    # Save
    output_path = os.path.join(os.path.dirname(__file__), args.output)
    with open(output_path, 'w') as f:
        json.dump(all_notes, f, indent=2)
    print(f"\nSaved to {output_path}")


if __name__ == "__main__":
    main()
