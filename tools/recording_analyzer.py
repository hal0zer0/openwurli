#!/usr/bin/env python3
"""
recording_analyzer.py — Extract individual notes and aggregate statistics
from full Wurlitzer 200A recordings.

Tier 1: Onset detection + pitch tracking + isolation scoring → chop clean notes
Tier 2: Aggregate stats (tremolo, spectral centroid, decay rates, harmonic ratios)

Usage:
    # Extract clean notes from a recording
    python tools/recording_analyzer.py extract Improv-Wurli200.wav -o /tmp/extracted/

    # Aggregate statistics across recordings
    python tools/recording_analyzer.py stats Improv-Wurli200.wav ComeTogether.wav ...

    # Both
    python tools/recording_analyzer.py full Improv-Wurli200.wav -o /tmp/extracted/
"""

import argparse
import json
import sys
from pathlib import Path

import librosa
import numpy as np
import soundfile as sf


# Wurlitzer 200A range: F1 (41.2 Hz) to C7 (2093 Hz), but most recordings
# stay within A2-C7. We use MIDI note range 41-96.
WURLI_FMIN = librosa.midi_to_hz(41)  # F2 ~ 87 Hz (lowest practical)
WURLI_FMAX = librosa.midi_to_hz(96)  # C7 ~ 2093 Hz


def load_audio(path, sr=44100):
    """Load audio, convert to mono, resample to target sr."""
    y, orig_sr = sf.read(path, dtype="float32", always_2d=True)
    # Mix to mono
    if y.shape[1] > 1:
        y = y.mean(axis=1)
    else:
        y = y[:, 0]
    # Resample if needed
    if orig_sr != sr:
        y = librosa.resample(y, orig_sr=orig_sr, target_sr=sr)
    return y, sr


def detect_onsets(y, sr, hop_length=512):
    """
    Detect note onsets using spectral flux (pure numpy/scipy, no numba).
    Avoids librosa.onset.onset_strength which uses numba and crashes when
    combined with pyin in the same process.
    """
    from scipy.signal import medfilt

    n_fft = 2048
    # Compute STFT magnitude manually
    n_frames = 1 + (len(y) - n_fft) // hop_length
    S = np.zeros((n_fft // 2 + 1, n_frames), dtype=np.float32)
    window = np.hanning(n_fft).astype(np.float32)

    for i in range(n_frames):
        start = i * hop_length
        frame = y[start:start + n_fft] * window
        S[:, i] = np.abs(np.fft.rfft(frame))

    # Spectral flux: sum of positive differences
    diff = np.diff(S, axis=1)
    diff = np.maximum(diff, 0)
    onset_env = diff.sum(axis=0)

    # Adaptive threshold: median filter + offset
    if len(onset_env) < 3:
        return np.array([], dtype=np.int64), onset_env
    kernel = min(31, len(onset_env) // 2 * 2 + 1)  # Must be odd
    if kernel < 3:
        kernel = 3
    threshold = medfilt(onset_env, kernel_size=kernel) + np.mean(onset_env) * 0.5

    # Peak picking
    peaks = []
    min_gap_frames = int(0.05 * sr / hop_length)  # 50ms minimum gap
    for i in range(1, len(onset_env) - 1):
        if (onset_env[i] > threshold[i] and
                onset_env[i] > onset_env[i - 1] and
                onset_env[i] >= onset_env[i + 1]):
            if not peaks or (i - peaks[-1]) >= min_gap_frames:
                peaks.append(i)

    onsets = np.array(peaks, dtype=np.int64) * hop_length
    return onsets, onset_env


def hz_to_note_name(hz):
    """Convert frequency to note name like 'A4'."""
    if hz is None or np.isnan(hz) or hz <= 0:
        return "?"
    midi = librosa.hz_to_midi(hz)
    return librosa.midi_to_note(int(round(midi)))


def compute_isolation_score(y_segment, sr, f0_hz, n_harmonics=8):
    """
    Score how "isolated" a note is — ratio of energy in the detected pitch's
    harmonics vs total spectral energy. 1.0 = pure tone, 0.0 = all noise.
    """
    if f0_hz is None or np.isnan(f0_hz) or f0_hz <= 0:
        return 0.0

    n_fft = 4096
    S = np.abs(librosa.stft(y_segment, n_fft=n_fft))
    freqs = librosa.fft_frequencies(sr=sr, n_fft=n_fft)

    total_energy = np.sum(S ** 2)
    if total_energy < 1e-12:
        return 0.0

    harmonic_energy = 0.0
    bw_hz = max(f0_hz * 0.03, 5.0)  # 3% bandwidth or 5 Hz min

    for h in range(1, n_harmonics + 1):
        fh = f0_hz * h
        if fh > sr / 2:
            break
        mask = np.abs(freqs - fh) < bw_hz
        harmonic_energy += np.sum(S[mask, :] ** 2)

    return float(harmonic_energy / total_energy)


def estimate_velocity(y_segment, sr):
    """
    Estimate relative velocity from RMS of the attack portion (first 50ms).
    Returns a 0-1 normalized value (needs calibration against the full file).
    """
    n_attack = int(0.05 * sr)
    attack = y_segment[:min(n_attack, len(y_segment))]
    return float(np.sqrt(np.mean(attack ** 2)))


def pitch_track_segment(segment, sr, fmin=WURLI_FMIN, fmax=WURLI_FMAX):
    """
    Pitch-track a short segment (< 5s). Returns median f0, std, and voiced ratio.
    Much cheaper than tracking the whole file.
    """
    # Use the sustain portion (skip first 80ms transient)
    skip = int(0.08 * sr)
    analysis_dur = min(len(segment) - skip, int(1.0 * sr))  # Analyze up to 1s
    if analysis_dur < int(0.1 * sr):
        return None, None, 0.0

    chunk = segment[skip:skip + analysis_dur]
    f0, voiced, _ = librosa.pyin(
        chunk, fmin=fmin, fmax=fmax, sr=sr,
        frame_length=2048, hop_length=512
    )

    valid = f0[voiced] if voiced is not None else np.array([])
    if len(valid) < 3:
        return None, None, 0.0

    return float(np.median(valid)), float(np.std(valid)), len(valid) / len(f0)


def extract_notes(y, sr, output_dir, min_duration=0.3, max_duration=3.0,
                  min_isolation=0.4, progress_cb=None):
    """
    Tier 1: Detect onsets, pitch-track per-segment, score isolation, extract.
    Returns list of dicts with metadata for each extracted note.
    """
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    print(f"  Detecting onsets...")
    onsets, _ = detect_onsets(y, sr)
    print(f"  Found {len(onsets)} onsets")

    # Compute file-wide RMS for velocity normalization
    global_rms = np.sqrt(np.mean(y ** 2))

    results = []
    n_good = 0

    for i, onset_sample in enumerate(onsets):
        if progress_cb and i % 50 == 0:
            progress_cb(i, len(onsets))

        # Determine segment end: next onset or max_duration
        if i + 1 < len(onsets):
            next_onset = onsets[i + 1]
            end_sample = min(next_onset, onset_sample + int(max_duration * sr))
        else:
            end_sample = min(len(y), onset_sample + int(max_duration * sr))

        duration = (end_sample - onset_sample) / sr
        if duration < min_duration:
            continue

        segment = y[onset_sample:end_sample]

        # Pitch-track this segment only (not the whole file!)
        median_f0, pitch_std, voiced_ratio = pitch_track_segment(segment, sr)
        if median_f0 is None:
            continue

        # Skip if pitch is unstable (vibrato/trem is fine, polyphony is not)
        if pitch_std > median_f0 * 0.05:
            continue

        # Isolation score — use the sustain portion (skip first 100ms)
        sustain_start = int(0.1 * sr)
        sustain_end = min(len(segment), int(0.8 * sr))
        if sustain_end - sustain_start < int(0.1 * sr):
            sustain_portion = segment[sustain_start:]
        else:
            sustain_portion = segment[sustain_start:sustain_end]

        isolation = compute_isolation_score(sustain_portion, sr, median_f0)

        if isolation < min_isolation:
            continue

        # Velocity estimate
        vel_rms = estimate_velocity(segment, sr)
        velocity_norm = min(1.0, vel_rms / (global_rms * 3.0)) if global_rms > 0 else 0.5

        midi_note = int(round(librosa.hz_to_midi(median_f0)))
        note_name = hz_to_note_name(median_f0)

        note_info = {
            "index": n_good,
            "onset_time": float(onset_sample / sr),
            "duration": float(duration),
            "f0_hz": round(median_f0, 2),
            "midi_note": midi_note,
            "note_name": note_name,
            "pitch_std_cents": round(1200 * np.log2(1 + pitch_std / median_f0), 1),
            "isolation_score": round(isolation, 3),
            "velocity_norm": round(velocity_norm, 3),
            "rms_db": round(20 * np.log10(vel_rms + 1e-10), 1),
        }
        results.append(note_info)

        # Save the extracted segment
        fname = f"{n_good:04d}_{note_name}_{isolation:.2f}.wav"
        sf.write(str(output_dir / fname), segment, sr)
        note_info["filename"] = fname

        n_good += 1

    # Sort by isolation score (best first)
    results.sort(key=lambda x: -x["isolation_score"])

    # Re-index after sorting
    for i, r in enumerate(results):
        r["rank"] = i

    # Save metadata
    class NumpyEncoder(json.JSONEncoder):
        def default(self, obj):
            if isinstance(obj, (np.floating, np.integer)):
                return float(obj) if isinstance(obj, np.floating) else int(obj)
            return super().default(obj)

    meta_path = output_dir / "extraction_metadata.json"
    with open(meta_path, "w") as f:
        json.dump({
            "total_onsets": int(len(onsets)),
            "extracted_notes": len(results),
            "min_isolation_threshold": min_isolation,
            "notes": results,
        }, f, indent=2, cls=NumpyEncoder)

    return results


def compute_harmonic_profile(y_segment, sr, f0_hz, n_harmonics=10):
    """Compute harmonic amplitudes relative to fundamental."""
    n_fft = 8192
    S = np.abs(librosa.stft(y_segment, n_fft=n_fft))
    freqs = librosa.fft_frequencies(sr=sr, n_fft=n_fft)
    # Average across time frames
    S_avg = S.mean(axis=1)

    harmonics = []
    for h in range(1, n_harmonics + 1):
        fh = f0_hz * h
        if fh > sr / 2:
            break
        idx = np.argmin(np.abs(freqs - fh))
        # Peak search in neighborhood
        lo = max(0, idx - 3)
        hi = min(len(S_avg), idx + 4)
        peak = float(S_avg[lo:hi].max())
        harmonics.append(peak)

    if not harmonics or harmonics[0] < 1e-10:
        return []

    # Normalize to fundamental
    return [float(h / harmonics[0]) for h in harmonics]


def estimate_decay_rate(y_segment, sr, hop_length=512):
    """
    Estimate decay rate in dB/s from RMS envelope.
    Uses linear regression on log-RMS after the attack.
    """
    rms = librosa.feature.rms(y=y_segment, frame_length=2048, hop_length=hop_length)[0]
    if len(rms) < 10:
        return 0.0

    # Skip first 100ms (attack), use next 500ms
    start_frame = int(0.1 * sr / hop_length)
    end_frame = min(len(rms), int(0.6 * sr / hop_length))
    if end_frame - start_frame < 5:
        return 0.0

    rms_slice = rms[start_frame:end_frame]
    rms_db = 20 * np.log10(rms_slice + 1e-10)

    t = np.arange(len(rms_db)) * hop_length / sr
    # Linear fit: dB = slope * t + intercept
    if len(t) < 2:
        return 0.0
    coeffs = np.polyfit(t, rms_db, 1)
    return float(coeffs[0])  # dB/s


def detect_tremolo(y, sr, min_rate=3.0, max_rate=9.0):
    """
    Detect tremolo rate and depth from amplitude modulation.
    Looks for periodicity in the RMS envelope in the tremolo frequency range.
    """
    hop = 256
    rms = librosa.feature.rms(y=y, frame_length=2048, hop_length=hop)[0]
    rms_sr = sr / hop

    if len(rms) < int(rms_sr * 2):
        return None

    # Bandpass the RMS envelope to tremolo range
    from scipy.signal import butter, filtfilt

    nyq = rms_sr / 2
    if max_rate >= nyq:
        max_rate = nyq * 0.9
    b, a = butter(2, [min_rate / nyq, max_rate / nyq], btype="band")
    rms_filt = filtfilt(b, a, rms)

    # Autocorrelation to find tremolo period
    rms_centered = rms_filt - rms_filt.mean()
    corr = np.correlate(rms_centered, rms_centered, mode="full")
    corr = corr[len(corr) // 2:]
    corr /= corr[0] + 1e-10

    # Find first peak after min_rate period
    min_lag = int(rms_sr / max_rate)
    max_lag = int(rms_sr / min_rate)
    if max_lag >= len(corr):
        max_lag = len(corr) - 1

    search = corr[min_lag:max_lag]
    if len(search) < 3:
        return None

    peak_idx = np.argmax(search) + min_lag
    trem_rate = rms_sr / peak_idx
    peak_corr = corr[peak_idx]

    if peak_corr < 0.15:
        return None  # No clear tremolo

    # Depth: ratio of modulation amplitude to mean
    depth_linear = np.std(rms_filt) * 2 * np.sqrt(2) / (np.mean(rms) + 1e-10)
    depth_db = 20 * np.log10(1 + depth_linear + 1e-10)

    return {
        "rate_hz": round(float(trem_rate), 2),
        "depth_db": round(float(depth_db), 1),
        "confidence": round(float(peak_corr), 3),
    }


def aggregate_stats(y, sr, extracted_notes=None):
    """
    Tier 2: Compute aggregate statistics from a recording.
    If extracted_notes is provided, uses those for per-note stats.
    Otherwise runs its own pitch tracking on windowed segments.
    """
    print("  Computing aggregate statistics...")
    stats = {}

    # Tremolo detection on the full recording
    print("    Tremolo detection...")
    trem = detect_tremolo(y, sr)
    stats["tremolo"] = trem
    if trem:
        print(f"    Tremolo: {trem['rate_hz']} Hz, {trem['depth_db']} dB "
              f"(confidence: {trem['confidence']})")
    else:
        print("    No clear tremolo detected")

    # If we have extracted notes, compute per-note statistics
    if extracted_notes:
        print(f"    Analyzing {len(extracted_notes)} extracted notes...")
        decay_rates = {}  # by octave
        h2_h1_ratios = {}  # by octave
        spectral_centroids = {}  # by octave
        velocities = {}  # by note name

        for note in extracted_notes:
            onset = int(note["onset_time"] * sr)
            dur = int(note["duration"] * sr)
            segment = y[onset:onset + dur]
            f0 = note["f0_hz"]
            midi = note["midi_note"]
            octave = midi // 12 - 1  # MIDI octave

            # Decay rate
            dr = estimate_decay_rate(segment, sr)
            decay_rates.setdefault(octave, []).append(dr)

            # Harmonic profile
            sustain_start = int(0.1 * sr)
            sustain_end = min(len(segment), int(0.6 * sr))
            if sustain_end > sustain_start + int(0.05 * sr):
                harmonics = compute_harmonic_profile(
                    segment[sustain_start:sustain_end], sr, f0
                )
                if len(harmonics) >= 2:
                    h2_h1_ratios.setdefault(octave, []).append(harmonics[1])

            # Spectral centroid
            sc = librosa.feature.spectral_centroid(y=segment, sr=sr)[0]
            sc_mean = float(np.mean(sc))
            spectral_centroids.setdefault(octave, []).append(sc_mean)

            # Velocity
            velocities.setdefault(note["note_name"], []).append(note["velocity_norm"])

        # Summarize by octave
        stats["decay_rates_db_per_s"] = {
            f"octave_{k}": {
                "mean": round(float(np.mean(v)), 1),
                "std": round(float(np.std(v)), 1),
                "n": len(v),
            }
            for k, v in sorted(decay_rates.items())
        }

        stats["h2_h1_ratio"] = {
            f"octave_{k}": {
                "mean": round(float(np.mean(v)), 3),
                "std": round(float(np.std(v)), 3),
                "n": len(v),
            }
            for k, v in sorted(h2_h1_ratios.items())
        }

        stats["spectral_centroid_hz"] = {
            f"octave_{k}": {
                "mean": round(float(np.mean(v)), 0),
                "std": round(float(np.std(v)), 0),
                "n": len(v),
            }
            for k, v in sorted(spectral_centroids.items())
        }

        stats["velocity_distribution"] = {
            k: {
                "mean": round(float(np.mean(v)), 3),
                "range": [round(float(min(v)), 3), round(float(max(v)), 3)],
                "n": len(v),
            }
            for k, v in sorted(velocities.items())
        }
    else:
        # Run windowed analysis without extracted notes
        print("    Running windowed pitch-tracked analysis...")
        window_sec = 0.5
        hop_sec = 0.25
        window_samples = int(window_sec * sr)
        hop_samples = int(hop_sec * sr)

        decay_rates_by_octave = {}
        h2_h1_by_octave = {}

        n_windows = (len(y) - window_samples) // hop_samples
        for wi in range(n_windows):
            if wi % 100 == 0:
                print(f"      Window {wi}/{n_windows}...")
            start = wi * hop_samples
            segment = y[start:start + window_samples]

            # Quick pitch estimate
            f0, voiced, _ = librosa.pyin(
                segment, fmin=WURLI_FMIN, fmax=WURLI_FMAX, sr=sr,
                frame_length=2048, hop_length=512
            )
            valid = f0[voiced] if voiced is not None else np.array([])
            if len(valid) < 3:
                continue

            median_f0 = float(np.median(valid))
            if np.std(valid) > median_f0 * 0.05:
                continue  # Unstable pitch — skip

            midi = int(round(librosa.hz_to_midi(median_f0)))
            octave = midi // 12 - 1

            harmonics = compute_harmonic_profile(segment, sr, median_f0)
            if len(harmonics) >= 2:
                h2_h1_by_octave.setdefault(octave, []).append(harmonics[1])

        stats["h2_h1_ratio"] = {
            f"octave_{k}": {
                "mean": round(float(np.mean(v)), 3),
                "std": round(float(np.std(v)), 3),
                "n": len(v),
            }
            for k, v in sorted(h2_h1_by_octave.items())
        }

    return stats


def print_report(results, stats, source_file):
    """Print a human-readable summary."""
    print(f"\n{'=' * 60}")
    print(f"  RECORDING ANALYSIS: {Path(source_file).name}")
    print(f"{'=' * 60}")

    if results:
        print(f"\n  TIER 1: Note Extraction")
        print(f"  {'-' * 40}")
        print(f"  Total onsets detected: (see metadata)")
        print(f"  Clean notes extracted: {len(results)}")

        if results:
            # Distribution by note name
            from collections import Counter
            note_counts = Counter(r["note_name"] for r in results)
            print(f"\n  Notes found (by pitch):")
            for note, count in sorted(note_counts.items(),
                                       key=lambda x: librosa.note_to_midi(x[0])
                                       if x[0] != "?" else 0):
                print(f"    {note:>4s}: {'#' * count} ({count})")

            # Top 10 by isolation
            print(f"\n  Top 10 cleanest extractions:")
            print(f"  {'Rank':>4s}  {'Note':>5s}  {'Iso':>5s}  {'Vel':>4s}  "
                  f"{'Time':>7s}  {'Dur':>5s}  {'File'}")
            for r in results[:10]:
                print(f"  {r['rank']:4d}  {r['note_name']:>5s}  "
                      f"{r['isolation_score']:.3f}  {r['velocity_norm']:.2f}  "
                      f"{r['onset_time']:7.2f}s  {r['duration']:5.2f}s  "
                      f"{r.get('filename', '?')}")

    if stats:
        print(f"\n  TIER 2: Aggregate Statistics")
        print(f"  {'-' * 40}")

        if stats.get("tremolo"):
            t = stats["tremolo"]
            print(f"  Tremolo: {t['rate_hz']} Hz, depth ~{t['depth_db']} dB "
                  f"(confidence {t['confidence']})")
        else:
            print(f"  Tremolo: not detected")

        if stats.get("decay_rates_db_per_s"):
            print(f"\n  Decay rates (dB/s) by octave:")
            for k, v in stats["decay_rates_db_per_s"].items():
                print(f"    {k}: {v['mean']:>6.1f} +/- {v['std']:.1f} (n={v['n']})")

        if stats.get("h2_h1_ratio"):
            print(f"\n  H2/H1 ratio by octave (bark indicator):")
            for k, v in stats["h2_h1_ratio"].items():
                pct = v['mean'] * 100
                print(f"    {k}: {pct:>5.1f}% +/- {v['std']*100:.1f}% (n={v['n']})")

        if stats.get("spectral_centroid_hz"):
            print(f"\n  Spectral centroid (Hz) by octave:")
            for k, v in stats["spectral_centroid_hz"].items():
                print(f"    {k}: {v['mean']:>6.0f} +/- {v['std']:.0f} (n={v['n']})")


def main():
    parser = argparse.ArgumentParser(
        description="Extract notes and statistics from Wurlitzer recordings"
    )
    parser.add_argument("mode", choices=["extract", "stats", "full"],
                        help="extract=Tier 1, stats=Tier 2, full=both")
    parser.add_argument("files", nargs="+", help="Audio files to analyze")
    parser.add_argument("-o", "--output", default="/tmp/wurli_extracted",
                        help="Output directory for extracted notes")
    parser.add_argument("--min-isolation", type=float, default=0.4,
                        help="Minimum isolation score (0-1, default 0.4)")
    parser.add_argument("--min-duration", type=float, default=0.3,
                        help="Minimum note duration in seconds")
    parser.add_argument("--max-duration", type=float, default=3.0,
                        help="Maximum note duration in seconds")

    args = parser.parse_args()

    for filepath in args.files:
        path = Path(filepath)
        if not path.exists():
            print(f"File not found: {filepath}", file=sys.stderr)
            continue

        print(f"\nLoading {path.name}...")
        y, sr = load_audio(str(path))
        duration = len(y) / sr
        print(f"  {duration:.1f}s, {sr} Hz, {len(y)} samples")

        results = None
        stats = None

        file_output = Path(args.output) / path.stem

        if args.mode in ("extract", "full"):
            print(f"\n  --- TIER 1: Note Extraction ---")
            results = extract_notes(
                y, sr, file_output,
                min_duration=args.min_duration,
                max_duration=args.max_duration,
                min_isolation=args.min_isolation,
                progress_cb=lambda i, n: print(f"    Processing onset {i}/{n}..."),
            )
            print(f"  Extracted {len(results)} clean notes → {file_output}/")

        if args.mode in ("stats", "full"):
            print(f"\n  --- TIER 2: Aggregate Statistics ---")
            stats = aggregate_stats(y, sr, extracted_notes=results)

            # Save stats
            file_output.mkdir(parents=True, exist_ok=True)
            stats_path = file_output / "aggregate_stats.json"
            with open(stats_path, "w") as f:
                json.dump(stats, f, indent=2)
            print(f"  Stats saved → {stats_path}")

        print_report(results, stats, filepath)


if __name__ == "__main__":
    main()
