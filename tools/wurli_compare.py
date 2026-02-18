#!/usr/bin/env python3
"""
wurli_compare.py — A/B comparison between extracted real Wurlitzer notes
and OpenWurli synthesized versions.

Designed for Dr Dawgg's calibration workflow:
  1. Selects best extracted notes per pitch (highest isolation)
  2. Renders matching OpenWurli notes via preamp-bench
  3. Computes per-note harmonic, decay, and spectral comparisons
  4. Generates structured JSON report + side-by-side WAVs

Usage:
    # Compare best notes from Improv extraction against OpenWurli
    python tools/wurli_compare.py /tmp/wurli_extracted/Improv-Wurli200/ \
        -o /tmp/wurli_comparison/ --top-per-pitch 3

    # Compare specific notes
    python tools/wurli_compare.py /tmp/wurli_extracted/Improv-Wurli200/ \
        -o /tmp/wurli_comparison/ --notes B4,G5,C3

    # Merge extractions from multiple recordings
    python tools/wurli_compare.py /tmp/wurli_extracted/Improv-Wurli200/ \
        /tmp/wurli_extracted/SoWhat/ /tmp/wurli_extracted/ComeTogether/ \
        -o /tmp/wurli_comparison/ --top-per-pitch 2

    # Quick summary (no renders, just stats)
    python tools/wurli_compare.py /tmp/wurli_extracted/Improv-Wurli200/ --summary-only
"""

import argparse
import json
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np
import soundfile as sf


# ---------------------------------------------------------------------------
# Audio analysis helpers (no librosa — avoid numba entirely in this tool)
# ---------------------------------------------------------------------------

def compute_stft(y, sr, n_fft=8192, hop_length=512):
    """Manual STFT returning magnitude spectrogram and frequency axis."""
    window = np.hanning(n_fft).astype(np.float32)
    n_frames = max(1, (len(y) - n_fft) // hop_length + 1)
    S = np.zeros((n_fft // 2 + 1, n_frames), dtype=np.float32)
    for i in range(n_frames):
        start = i * hop_length
        frame = y[start:start + n_fft]
        if len(frame) < n_fft:
            frame = np.pad(frame, (0, n_fft - len(frame)))
        S[:, i] = np.abs(np.fft.rfft(frame * window))
    freqs = np.fft.rfftfreq(n_fft, 1.0 / sr)
    return S, freqs


def harmonic_profile(y, sr, f0_hz, n_harmonics=10, n_fft=8192):
    """
    Extract harmonic amplitudes (dB relative to fundamental).
    Returns dict with h1..h10 in dB, plus raw linear ratios.
    """
    S, freqs = compute_stft(y, sr, n_fft=n_fft)
    S_avg = S.mean(axis=1)

    amplitudes = []
    for h in range(1, n_harmonics + 1):
        fh = f0_hz * h
        if fh > sr / 2:
            break
        idx = np.argmin(np.abs(freqs - fh))
        lo = max(0, idx - 4)
        hi = min(len(S_avg), idx + 5)
        peak = float(S_avg[lo:hi].max())
        amplitudes.append(peak)

    if not amplitudes or amplitudes[0] < 1e-10:
        return None

    h1 = amplitudes[0]
    ratios = [a / h1 for a in amplitudes]
    db = [20 * np.log10(r + 1e-10) for r in ratios]

    return {
        "n_harmonics": len(amplitudes),
        "ratios": [round(r, 4) for r in ratios],
        "db": [round(d, 1) for d in db],
        "f0_hz": f0_hz,
    }


def measure_decay(y, sr, skip_attack_s=0.08, window_s=0.5):
    """Measure decay rate in dB/s from RMS envelope after attack."""
    hop = 512
    frame_len = 2048
    n_frames = max(1, (len(y) - frame_len) // hop + 1)

    rms = np.zeros(n_frames)
    for i in range(n_frames):
        start = i * hop
        frame = y[start:start + frame_len]
        if len(frame) < frame_len:
            frame = np.pad(frame, (0, frame_len - len(frame)))
        rms[i] = np.sqrt(np.mean(frame ** 2))

    start_frame = int(skip_attack_s * sr / hop)
    end_frame = min(len(rms), int((skip_attack_s + window_s) * sr / hop))
    if end_frame - start_frame < 5:
        return None

    rms_slice = rms[start_frame:end_frame]
    rms_db = 20 * np.log10(rms_slice + 1e-10)
    t = np.arange(len(rms_db)) * hop / sr

    coeffs = np.polyfit(t, rms_db, 1)
    return round(float(coeffs[0]), 1)


def measure_spectral_centroid(y, sr, n_fft=4096):
    """Compute mean spectral centroid in Hz."""
    S, freqs = compute_stft(y, sr, n_fft=n_fft)
    power = S ** 2
    centroid_per_frame = np.sum(freqs[:, None] * power, axis=0) / (np.sum(power, axis=0) + 1e-10)
    return round(float(np.mean(centroid_per_frame)), 1)


def measure_rms(y):
    """RMS in dB."""
    rms = np.sqrt(np.mean(y ** 2))
    return round(20 * np.log10(rms + 1e-10), 1)


def measure_attack_time(y, sr, threshold_db=-10):
    """Time from onset to peak RMS (ms)."""
    hop = 128
    frame_len = 1024
    n_frames = max(1, (len(y) - frame_len) // hop + 1)
    rms = np.zeros(n_frames)
    for i in range(n_frames):
        start = i * hop
        frame = y[start:start + frame_len]
        if len(frame) < frame_len:
            frame = np.pad(frame, (0, frame_len - len(frame)))
        rms[i] = np.sqrt(np.mean(frame ** 2))

    if len(rms) == 0 or rms.max() < 1e-10:
        return None

    peak_idx = np.argmax(rms)
    peak_time_ms = peak_idx * hop / sr * 1000
    return round(float(peak_time_ms), 1)


# ---------------------------------------------------------------------------
# Note selection
# ---------------------------------------------------------------------------

def load_extractions(dirs):
    """Load extraction metadata from one or more directories. Merge and deduplicate."""
    all_notes = []
    for d in dirs:
        d = Path(d)
        meta_path = d / "extraction_metadata.json"
        if not meta_path.exists():
            print(f"  WARNING: No metadata in {d}", file=sys.stderr)
            continue
        with open(meta_path) as f:
            data = json.load(f)
        for note in data["notes"]:
            note["_source_dir"] = str(d)
            note["_wav_path"] = str(d / note["filename"])
            all_notes.append(note)
    return all_notes


def select_best_notes(all_notes, top_per_pitch=3, specific_notes=None):
    """
    Select best notes per MIDI pitch (highest isolation score).
    Returns dict: midi_note -> list of note dicts.
    """
    by_pitch = defaultdict(list)
    for note in all_notes:
        by_pitch[note["midi_note"]].append(note)

    # Sort each pitch group by isolation (best first)
    for midi in by_pitch:
        by_pitch[midi].sort(key=lambda n: -n["isolation_score"])

    selected = {}
    for midi, notes in sorted(by_pitch.items()):
        note_name = notes[0]["note_name"]
        if specific_notes and note_name not in specific_notes:
            continue
        selected[midi] = notes[:top_per_pitch]

    return selected


# ---------------------------------------------------------------------------
# OpenWurli rendering
# ---------------------------------------------------------------------------

def render_synth_note(midi_note, velocity_midi, duration, output_path, preamp_bench="preamp-bench"):
    """Render a note via preamp-bench and return the path."""
    cmd = [
        "cargo", "run", "-p", preamp_bench, "--release", "--",
        "render",
        "--note", str(midi_note),
        "--velocity", str(velocity_midi),
        "--duration", str(duration),
        "--output", str(output_path),
    ]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
    if result.returncode != 0:
        print(f"  WARN: render failed for MIDI {midi_note}: {result.stderr[:200]}",
              file=sys.stderr)
        return None
    return output_path


# ---------------------------------------------------------------------------
# Per-note comparison
# ---------------------------------------------------------------------------

def compare_note(real_path, synth_path, f0_hz, sr=44100):
    """
    Compare a real extracted note against a synth render.
    Returns a dict of comparison metrics.
    """
    y_real, sr_r = sf.read(str(real_path), dtype="float32")
    if y_real.ndim > 1:
        y_real = y_real.mean(axis=1)

    y_synth, sr_s = sf.read(str(synth_path), dtype="float32")
    if y_synth.ndim > 1:
        y_synth = y_synth.mean(axis=1)

    # Use sustain portion for harmonic analysis (skip first 80ms)
    skip = int(0.08 * sr)
    dur = min(len(y_real), len(y_synth), int(1.0 * sr))  # Compare up to 1s

    real_sustain = y_real[skip:skip + dur] if len(y_real) > skip else y_real
    synth_sustain = y_synth[skip:skip + dur] if len(y_synth) > skip else y_synth

    # Harmonic profiles
    hp_real = harmonic_profile(real_sustain, sr, f0_hz)
    hp_synth = harmonic_profile(synth_sustain, sr, f0_hz)

    # Compute harmonic distance (RMS of dB differences for H2-H8)
    harmonic_distance = None
    harmonic_diffs = None
    if hp_real and hp_synth:
        n = min(len(hp_real["db"]), len(hp_synth["db"]))
        diffs = [hp_synth["db"][i] - hp_real["db"][i] for i in range(1, n)]  # skip H1 (=0)
        harmonic_distance = round(float(np.sqrt(np.mean(np.array(diffs) ** 2))), 1)
        harmonic_diffs = [round(d, 1) for d in diffs]

    # Decay rates
    decay_real = measure_decay(y_real, sr)
    decay_synth = measure_decay(y_synth, sr)

    # Spectral centroids
    sc_real = measure_spectral_centroid(real_sustain, sr)
    sc_synth = measure_spectral_centroid(synth_sustain, sr)

    # Attack time
    attack_real = measure_attack_time(y_real, sr)
    attack_synth = measure_attack_time(y_synth, sr)

    # RMS levels
    rms_real = measure_rms(y_real)
    rms_synth = measure_rms(y_synth)

    return {
        "harmonics_real": hp_real,
        "harmonics_synth": hp_synth,
        "harmonic_distance_db": harmonic_distance,
        "harmonic_diffs_db": harmonic_diffs,
        "decay_real_db_s": decay_real,
        "decay_synth_db_s": decay_synth,
        "decay_diff_db_s": round(decay_synth - decay_real, 1) if (decay_real and decay_synth) else None,
        "centroid_real_hz": sc_real,
        "centroid_synth_hz": sc_synth,
        "centroid_diff_hz": round(sc_synth - sc_real, 1) if (sc_real and sc_synth) else None,
        "attack_real_ms": attack_real,
        "attack_synth_ms": attack_synth,
        "rms_real_db": rms_real,
        "rms_synth_db": rms_synth,
    }


# ---------------------------------------------------------------------------
# Report generation
# ---------------------------------------------------------------------------

def print_comparison_report(comparisons, output_dir=None):
    """Print human-readable comparison report (Dr Dawgg format)."""
    print(f"\n{'=' * 70}")
    print(f"  WURLI A/B COMPARISON REPORT — {len(comparisons)} notes")
    print(f"{'=' * 70}")

    # Aggregate metrics
    all_harm_dist = [c["comparison"]["harmonic_distance_db"]
                     for c in comparisons if c["comparison"]["harmonic_distance_db"] is not None]
    all_decay_diff = [c["comparison"]["decay_diff_db_s"]
                      for c in comparisons if c["comparison"]["decay_diff_db_s"] is not None]
    all_cent_diff = [c["comparison"]["centroid_diff_hz"]
                     for c in comparisons if c["comparison"]["centroid_diff_hz"] is not None]

    if all_harm_dist:
        print(f"\n  HARMONIC DISTANCE (RMS dB, H2-H8 vs real)")
        print(f"    Mean:   {np.mean(all_harm_dist):>6.1f} dB")
        print(f"    Median: {np.median(all_harm_dist):>6.1f} dB")
        print(f"    Worst:  {max(all_harm_dist):>6.1f} dB")
        print(f"    Best:   {min(all_harm_dist):>6.1f} dB")

    if all_decay_diff:
        print(f"\n  DECAY RATE DIFFERENCE (synth - real, dB/s)")
        print(f"    Mean:   {np.mean(all_decay_diff):>+6.1f} dB/s")
        print(f"    {'(positive = synth decays faster, negative = synth sustains longer)'}")

    if all_cent_diff:
        print(f"\n  SPECTRAL CENTROID DIFFERENCE (synth - real, Hz)")
        print(f"    Mean:   {np.mean(all_cent_diff):>+6.0f} Hz")
        print(f"    {'(positive = synth brighter, negative = synth darker)'}")

    # Per-octave breakdown
    by_octave = defaultdict(list)
    for c in comparisons:
        octave = c["midi_note"] // 12 - 1
        by_octave[octave].append(c)

    print(f"\n  PER-OCTAVE BREAKDOWN")
    print(f"  {'Oct':>4s}  {'n':>3s}  {'HarmDist':>8s}  {'DecayΔ':>8s}  {'CentΔ':>8s}  Notes")
    print(f"  {'-'*4}  {'-'*3}  {'-'*8}  {'-'*8}  {'-'*8}  {'-'*20}")
    for octave in sorted(by_octave.keys()):
        notes = by_octave[octave]
        hd = [c["comparison"]["harmonic_distance_db"] for c in notes
              if c["comparison"]["harmonic_distance_db"] is not None]
        dd = [c["comparison"]["decay_diff_db_s"] for c in notes
              if c["comparison"]["decay_diff_db_s"] is not None]
        cd = [c["comparison"]["centroid_diff_hz"] for c in notes
              if c["comparison"]["centroid_diff_hz"] is not None]

        note_names = sorted(set(c["note_name"] for c in notes))
        hd_str = f"{np.mean(hd):>6.1f}dB" if hd else "   n/a  "
        dd_str = f"{np.mean(dd):>+6.1f}  " if dd else "   n/a  "
        cd_str = f"{np.mean(cd):>+6.0f}Hz " if cd else "   n/a  "
        print(f"  {octave:>4d}  {len(notes):>3d}  {hd_str}  {dd_str}  {cd_str}  {', '.join(note_names)}")

    # Worst offenders
    if all_harm_dist:
        print(f"\n  WORST HARMONIC MATCHES (biggest timbral gap):")
        ranked = sorted(comparisons,
                        key=lambda c: c["comparison"]["harmonic_distance_db"] or 0,
                        reverse=True)
        for c in ranked[:5]:
            hd = c["comparison"]["harmonic_distance_db"]
            if hd is None:
                continue
            diffs = c["comparison"]["harmonic_diffs_db"]
            diff_str = " ".join(f"H{i+2}:{d:+.0f}" for i, d in enumerate(diffs[:6])) if diffs else ""
            print(f"    {c['note_name']:>5s} iso={c['isolation']:.2f}  "
                  f"dist={hd:.1f}dB  {diff_str}")

    # Best matches
    if all_harm_dist:
        print(f"\n  BEST HARMONIC MATCHES (closest to real):")
        ranked = sorted(comparisons,
                        key=lambda c: c["comparison"]["harmonic_distance_db"] or 999)
        for c in ranked[:5]:
            hd = c["comparison"]["harmonic_distance_db"]
            if hd is None:
                continue
            print(f"    {c['note_name']:>5s} iso={c['isolation']:.2f}  "
                  f"dist={hd:.1f}dB")

    if output_dir:
        print(f"\n  Output directory: {output_dir}")
        print(f"  WAV pairs: <note>_real.wav / <note>_synth.wav")
        print(f"  Full report: comparison_report.json")


def print_summary(all_notes):
    """Quick summary of available extractions (no rendering)."""
    by_pitch = defaultdict(list)
    for note in all_notes:
        by_pitch[note["midi_note"]].append(note)

    print(f"\n{'=' * 60}")
    print(f"  EXTRACTION SUMMARY — {len(all_notes)} notes across "
          f"{len(by_pitch)} pitches")
    print(f"{'=' * 60}")

    print(f"\n  {'Note':>5s}  {'MIDI':>4s}  {'Count':>5s}  {'Best Iso':>8s}  "
          f"{'Vel Range':>12s}  {'Sources'}")
    print(f"  {'-'*5}  {'-'*4}  {'-'*5}  {'-'*8}  {'-'*12}  {'-'*20}")

    for midi in sorted(by_pitch.keys()):
        notes = by_pitch[midi]
        notes.sort(key=lambda n: -n["isolation_score"])
        best_iso = notes[0]["isolation_score"]
        vels = [n["velocity_norm"] for n in notes]
        sources = set(Path(n["_source_dir"]).name for n in notes)
        print(f"  {notes[0]['note_name']:>5s}  {midi:>4d}  {len(notes):>5d}  "
              f"{best_iso:>8.3f}  {min(vels):.2f}-{max(vels):.2f}   "
              f"{', '.join(sorted(sources))}")

    # Coverage assessment
    wurli_range = set(range(41, 97))  # F2 to C7
    covered = set(by_pitch.keys()) & wurli_range
    missing = wurli_range - covered
    print(f"\n  Coverage: {len(covered)}/{len(wurli_range)} Wurlitzer pitches "
          f"({len(covered)/len(wurli_range)*100:.0f}%)")
    if missing:
        from itertools import groupby
        from operator import itemgetter
        missing_sorted = sorted(missing)
        # Show gaps as ranges
        gaps = []
        for _, g in groupby(enumerate(missing_sorted), lambda ix: ix[0] - ix[1]):
            group = [x[1] for x in g]
            if len(group) > 2:
                gaps.append(f"MIDI {group[0]}-{group[-1]}")
            else:
                gaps.append(", ".join(f"MIDI {m}" for m in group))
        print(f"  Missing: {'; '.join(gaps)}")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="A/B comparison: real Wurlitzer vs OpenWurli synthesis"
    )
    parser.add_argument("extraction_dirs", nargs="+",
                        help="Directories with extraction_metadata.json from recording_analyzer")
    parser.add_argument("-o", "--output", default="/tmp/wurli_comparison",
                        help="Output directory for comparison results")
    parser.add_argument("--top-per-pitch", type=int, default=1,
                        help="Number of best notes to compare per pitch (default: 1)")
    parser.add_argument("--notes", type=str, default=None,
                        help="Comma-separated note names to compare (e.g. B4,G5,C3)")
    parser.add_argument("--summary-only", action="store_true",
                        help="Just print extraction summary, no rendering")
    parser.add_argument("--velocity", type=int, default=None,
                        help="Override synth velocity (default: estimate from extraction)")

    args = parser.parse_args()

    # Load all extractions
    print("Loading extractions...")
    all_notes = load_extractions(args.extraction_dirs)
    print(f"  {len(all_notes)} total notes from {len(args.extraction_dirs)} source(s)")

    if args.summary_only:
        print_summary(all_notes)
        return

    specific = args.notes.split(",") if args.notes else None
    selected = select_best_notes(all_notes, args.top_per_pitch, specific)

    total_notes = sum(len(v) for v in selected.values())
    print(f"  Selected {total_notes} notes across {len(selected)} pitches")

    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    # Build the release binary once
    print("\nBuilding preamp-bench (release)...")
    build = subprocess.run(
        ["cargo", "build", "-p", "preamp-bench", "--release"],
        capture_output=True, text=True, timeout=120
    )
    if build.returncode != 0:
        print(f"Build failed: {build.stderr[:500]}", file=sys.stderr)
        sys.exit(1)

    comparisons = []
    total = sum(len(notes) for notes in selected.values())
    done = 0

    for midi, notes in sorted(selected.items()):
        for note_info in notes:
            done += 1
            note_name = note_info["note_name"]
            iso = note_info["isolation_score"]
            print(f"  [{done}/{total}] {note_name} (MIDI {midi}, iso={iso:.2f})...",
                  end="", flush=True)

            # Copy real note to output
            real_src = Path(note_info["_wav_path"])
            if not real_src.exists():
                print(" SKIP (wav missing)")
                continue

            safe_name = note_name.replace("#", "s").replace("♯", "s").replace("♭", "b")
            real_dst = output_dir / f"{safe_name}_real.wav"
            synth_dst = output_dir / f"{safe_name}_synth.wav"

            # If multiple notes at same pitch, add index
            if args.top_per_pitch > 1:
                idx = notes.index(note_info)
                real_dst = output_dir / f"{safe_name}_{idx}_real.wav"
                synth_dst = output_dir / f"{safe_name}_{idx}_synth.wav"

            # Copy real
            y_real, sr_real = sf.read(str(real_src), dtype="float32")
            if y_real.ndim > 1:
                y_real = y_real.mean(axis=1)
            sf.write(str(real_dst), y_real, sr_real)

            # Estimate velocity for synth render
            if args.velocity:
                vel_midi = args.velocity
            else:
                # Map velocity_norm (0-1) to MIDI (1-127)
                vel_midi = max(1, min(127, int(note_info["velocity_norm"] * 127)))

            # Render synth
            duration = max(0.5, note_info["duration"])
            synth_path = render_synth_note(midi, vel_midi, duration, synth_dst)
            if not synth_path:
                print(" RENDER FAILED")
                continue

            # Compare
            comp = compare_note(real_dst, synth_dst, note_info["f0_hz"])

            result = {
                "note_name": note_name,
                "midi_note": midi,
                "f0_hz": note_info["f0_hz"],
                "isolation": iso,
                "velocity_norm": note_info["velocity_norm"],
                "velocity_midi": vel_midi,
                "duration": note_info["duration"],
                "source": Path(note_info["_source_dir"]).name,
                "real_wav": str(real_dst),
                "synth_wav": str(synth_dst),
                "comparison": comp,
            }
            comparisons.append(result)

            hd = comp["harmonic_distance_db"]
            hd_str = f"dist={hd:.1f}dB" if hd else "n/a"
            print(f" {hd_str}")

    # Save full report
    report = {
        "n_comparisons": len(comparisons),
        "sources": args.extraction_dirs,
        "comparisons": comparisons,
    }
    report_path = output_dir / "comparison_report.json"
    with open(report_path, "w") as f:
        json.dump(report, f, indent=2, default=str)

    print_comparison_report(comparisons, output_dir)


if __name__ == "__main__":
    main()
