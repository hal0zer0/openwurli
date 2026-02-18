"""Compute residuals (real - model) and assemble training dataset.

22 MLP output targets per observation:
- amp_offsets[7]:   real_H_dB - model_H_dB per harmonic H2-H8 (dB). H1 excluded (it's the reference).
- freq_offsets[7]:  1200 * log2(real_freq / model_freq) per harmonic (cents)
- decay_offsets[7]: real_decay / model_decay ratio per harmonic window (H1 at 7 time windows -> simplified to 1 overall)
- d0_correction:    derived from H2 discrepancy

SNR-based masking filters out noise-contaminated harmonics:
- Inter-harmonic noise measured at (h+0.5)*f0
- Harmonics with SNR < threshold are masked (NaN)
- Anomalous patterns (H_{n+1} > H_n) are flagged and masked
- H4-H8 amplitude targets always masked (below OBM noise floor)

Output: ml_data/training_data.npz containing:
- inputs (N, 2): [midi_note_normalized, velocity_normalized]
- targets (N, 22): correction values
- mask (N, 22): True where target is valid
- weights (N,): isolation_score (gold=1.0, silver=0.6, bronze=0.3)

Usage:
    python compute_residuals.py --real harmonics.json --model model_harmonics.json
"""

import argparse
import json
import math
import os
import sys
import numpy as np

from render_model_notes import bucket_velocity
from goertzel_utils import load_audio, extract_harmonics_fft

N_HARMONICS = 8
# Target vector layout (22 values):
#   [0:7]   amp_offsets  for H2-H8 (H1 is reference, always 0)
#   [7:14]  freq_offsets for H2-H8 in cents
#   [14:21] decay proxy offsets for H2-H8 (from sustain vs early_sustain ratio)
#   [21]    d0_correction derived from H2

N_TARGETS = 22
TIER_WEIGHTS = {"gold": 1.0, "silver": 0.6, "bronze": 0.3}

# SNR threshold for masking (dB) — harmonics below this are unreliable
SNR_THRESHOLD_DB = 10.0

# Maximum reliable harmonic index for amplitude targets (0-indexed into H2-H8)
# H2 (index 0) and H3 (index 1) are potentially reliable; H4-H8 (indices 2-6) always masked
MAX_RELIABLE_AMP_HARMONIC = 2  # Only H2 and H3 amplitude targets


def measure_interharmonic_snr(audio, sr, f0, onset_s, n_harmonics=8,
                               window_start=0.05, window_end=0.20):
    """Measure per-harmonic SNR using inter-harmonic noise floor.

    For each harmonic H_n, measures the noise floor at (n+0.5)*f0.
    SNR = 20*log10(H_n / noise_floor).

    Args:
        audio: full audio array
        sr: sample rate
        f0: fundamental frequency
        onset_s: onset time in seconds
        n_harmonics: number of harmonics to measure
        window_start: start of measurement window (relative to onset)
        window_end: end of measurement window (relative to onset)

    Returns:
        snr_db: array of SNR values in dB (n_harmonics,)
    """
    start_idx = int((onset_s + window_start) * sr)
    end_idx = int((onset_s + window_end) * sr)

    if start_idx < 0:
        start_idx = 0
    if end_idx > len(audio):
        end_idx = len(audio)
    if end_idx - start_idx < 128:
        return np.full(n_harmonics, np.nan)

    segment = audio[start_idx:end_idx]

    # Measure harmonic amplitudes
    h_amps, _ = extract_harmonics_fft(segment, sr, f0, n_harmonics)

    # Measure noise floor at inter-harmonic frequencies (h+0.5)*f0
    noise_amps = np.zeros(n_harmonics)
    N = len(segment)
    nfft = N * 4
    window = np.hanning(N)
    windowed = segment * window
    spectrum = np.abs(np.fft.rfft(windowed, n=nfft))
    spectrum = spectrum * 2.0 / N / 0.5
    freqs_axis = np.fft.rfftfreq(nfft, d=1.0 / sr)

    for h in range(n_harmonics):
        noise_freq = (h + 1.5) * f0  # between H_{h+1} and H_{h+2}
        if noise_freq >= sr / 2 - 100:
            noise_amps[h] = 1e-20
            continue
        # Search +/-1% around noise frequency
        f_lo = noise_freq * 0.99
        f_hi = noise_freq * 1.01
        mask = (freqs_axis >= f_lo) & (freqs_axis <= f_hi)
        if not np.any(mask):
            noise_amps[h] = 1e-20
            continue
        idx = np.where(mask)[0]
        # Use MEDIAN of the region (more robust than peak)
        noise_amps[h] = max(np.median(spectrum[idx]), 1e-20)

    # SNR in dB
    snr_db = np.zeros(n_harmonics)
    for h in range(n_harmonics):
        if h_amps[h] > 1e-20 and noise_amps[h] > 1e-20:
            snr_db[h] = 20.0 * np.log10(h_amps[h] / noise_amps[h])
        else:
            snr_db[h] = np.nan

    return snr_db


def detect_anomalous_harmonics(real_dB):
    """Detect physically impossible harmonic patterns.

    A monotonic nonlinearity like 1/(1-y) always produces H2 > H3 > H4...
    If a higher harmonic is stronger than a lower one, it indicates
    instrument-specific resonance, sympathetic coupling, or noise.

    Returns set of harmonic indices (0-based, in H2-H8 space) that are anomalous.
    """
    anomalous = set()
    for h in range(1, min(len(real_dB) - 1, 7)):
        h_idx = h + 1  # h_idx in the full H1-H8 array
        prev_idx = h    # previous harmonic in full array
        if (real_dB[h_idx] is not None and real_dB[prev_idx] is not None
                and real_dB[h_idx] > real_dB[prev_idx]):
            # H_{n+1} stronger than H_n — anomalous (mark the higher harmonic)
            anomalous.add(h)  # index in H2-H8 target space (0 = H2, 1 = H3, ...)
    return anomalous


def compute_note_residual(real_feat, model_feat, snr_db=None):
    """Compute residual vector for one note observation.

    Returns (targets_22, mask_22) where targets has NaN for invalid entries.
    """
    targets = np.full(N_TARGETS, np.nan)
    mask = np.zeros(N_TARGETS, dtype=bool)

    # Use early_sustain window as primary (more stable than sustain for short notes)
    real_win = real_feat["windows"].get("early_sustain")
    model_win = model_feat["windows"].get("early_sustain")
    if real_win is None or model_win is None:
        real_win = real_feat["windows"].get("sustain")
        model_win = model_feat["windows"].get("sustain")
    if real_win is None or model_win is None:
        return targets, mask

    real_dB = real_win["amps_dB_rel_H1"]
    model_dB = model_win["amps_dB_rel_H1"]
    real_freqs = real_win["freqs_hz"]
    model_freqs = model_win["freqs_hz"]

    # Detect anomalous harmonic patterns in the recording
    anomalous = detect_anomalous_harmonics(real_dB)

    # Amplitude offsets: H2-H8 (indices 1-7 in harmonic arrays)
    for h in range(7):  # H2 through H8
        h_idx = h + 1  # index into harmonic arrays

        if real_dB[h_idx] is None or model_dB[h_idx] is None:
            continue

        # Filter 1: Always mask H4-H8 amplitude targets (below OBM noise floor)
        if h >= MAX_RELIABLE_AMP_HARMONIC:
            continue

        # Filter 2: SNR-based masking
        if snr_db is not None and h_idx < len(snr_db):
            if np.isnan(snr_db[h_idx]) or snr_db[h_idx] < SNR_THRESHOLD_DB:
                continue

        # Filter 3: Anomalous pattern detection
        if h in anomalous:
            continue

        targets[h] = real_dB[h_idx] - model_dB[h_idx]
        mask[h] = True

    # Frequency offsets: H2-H8 in cents (less sensitive to noise — keep more)
    for h in range(7):
        h_idx = h + 1

        if (real_freqs[h_idx] is None or model_freqs[h_idx] is None
                or real_freqs[h_idx] <= 0 or model_freqs[h_idx] <= 0):
            continue

        # Only mask freq targets for H4+ (same noise floor concern)
        if h >= MAX_RELIABLE_AMP_HARMONIC:
            continue

        # Also mask if amplitude is anomalous (freq measurement unreliable if amp is noise)
        if snr_db is not None and h_idx < len(snr_db):
            if np.isnan(snr_db[h_idx]) or snr_db[h_idx] < SNR_THRESHOLD_DB:
                continue

        targets[7 + h] = 1200.0 * math.log2(real_freqs[h_idx] / model_freqs[h_idx])
        mask[7 + h] = True

    # Decay proxy: ratio of sustain/early_sustain amplitude for H2 and H3 only
    real_early = real_feat["windows"].get("early_sustain")
    real_sus = real_feat["windows"].get("sustain")
    model_early = model_feat["windows"].get("early_sustain")
    model_sus = model_feat["windows"].get("sustain")

    if (real_early is not None and real_sus is not None
            and model_early is not None and model_sus is not None):
        for h in range(min(MAX_RELIABLE_AMP_HARMONIC, 7)):
            h_idx = h + 1
            re = real_early["amps_linear"][h_idx]
            rs = real_sus["amps_linear"][h_idx]
            me = model_early["amps_linear"][h_idx]
            ms = model_sus["amps_linear"][h_idx]
            if (re is not None and rs is not None and me is not None and ms is not None
                    and re > 1e-12 and rs > 1e-12 and me > 1e-12 and ms > 1e-12):

                # Check SNR for decay target too
                if snr_db is not None and h_idx < len(snr_db):
                    if np.isnan(snr_db[h_idx]) or snr_db[h_idx] < SNR_THRESHOLD_DB:
                        continue

                if h in anomalous:
                    continue

                real_ratio = rs / re    # <1 means decaying
                model_ratio = ms / me
                targets[14 + h] = real_ratio / model_ratio  # >1 means model decays too fast
                mask[14 + h] = True

    # d0_correction: derived from H2 amplitude discrepancy
    # delta_H2 = real_H2_dB - model_H2_dB (positive = model too quiet)
    # Correction: 2^(-delta_H2 / 6) -- empirically scaled
    if mask[0]:  # H2 amp offset is valid
        delta_H2 = targets[0]
        targets[21] = 2.0 ** (-delta_H2 / 6.0)
        mask[21] = True

    return targets, mask


def load_audio_for_snr(features):
    """Load audio files and compute per-note SNR arrays.

    Returns dict: note_id -> snr_db array
    """
    snr_cache = {}
    audio_cache = {}

    for feat in features:
        note_id = feat.get("id", "")
        source_file = feat.get("source_file", "")
        if not source_file or not os.path.exists(source_file):
            continue

        # Load audio (cache by file)
        if source_file not in audio_cache:
            try:
                audio, sr = load_audio(source_file)
                audio_cache[source_file] = (audio, sr)
            except Exception as e:
                print(f"  WARNING: Could not load {source_file}: {e}")
                continue

        audio, sr = audio_cache[source_file]
        f0 = feat["f0"]

        # For OBM isolated notes, onset is at ~0 (start of file)
        # For polyphonic, would need onset_s from scored_notes
        onset_s = 0.0  # OBM files start at the note

        snr = measure_interharmonic_snr(audio, sr, f0, onset_s,
                                         n_harmonics=N_HARMONICS)
        snr_cache[note_id] = snr

    return snr_cache


def assemble_dataset(real_features, model_features, snr_cache=None):
    """Assemble training dataset from real and model features.

    Returns (inputs, targets, mask, weights) arrays.
    """
    inputs_list = []
    targets_list = []
    mask_list = []
    weights_list = []
    note_ids = []

    filter_stats = {
        'total_notes': 0,
        'matched': 0,
        'h2_valid': 0,
        'h3_valid': 0,
        'h3_snr_filtered': 0,
        'h3_anomaly_filtered': 0,
        'h4plus_masked': 0,
    }

    for real_feat in real_features:
        midi = real_feat["midi_note"]
        vel = real_feat.get("velocity_midi", 80)
        vel_bucket = bucket_velocity(vel)
        model_key = f"{midi}_{vel_bucket}"

        filter_stats['total_notes'] += 1

        if model_key not in model_features:
            continue

        filter_stats['matched'] += 1
        model_feat = model_features[model_key]

        # Get SNR data for this note
        note_id = real_feat.get("id", "")
        snr_db = snr_cache.get(note_id) if snr_cache else None

        targets, mask_vec = compute_note_residual(real_feat, model_feat, snr_db)

        # Track filtering stats
        if mask_vec[0]:
            filter_stats['h2_valid'] += 1
        if mask_vec[1]:
            filter_stats['h3_valid'] += 1

        # Skip if no valid targets at all
        if not np.any(mask_vec):
            continue

        # Normalize inputs to [0, 1]
        midi_norm = (midi - 21) / (108 - 21)  # piano range
        vel_norm = vel / 127.0

        tier = real_feat.get("isolation_tier", "bronze")
        weight = TIER_WEIGHTS.get(tier, 0.3)

        inputs_list.append([midi_norm, vel_norm])
        targets_list.append(targets)
        mask_list.append(mask_vec)
        weights_list.append(weight)
        note_ids.append(note_id)

    inputs = np.array(inputs_list, dtype=np.float32)
    targets = np.array(targets_list, dtype=np.float32)
    mask_arr = np.array(mask_list, dtype=bool)
    weights = np.array(weights_list, dtype=np.float32)

    # Replace NaN with 0 in targets (masked out anyway)
    targets = np.nan_to_num(targets, nan=0.0)

    return inputs, targets, mask_arr, weights, note_ids, filter_stats


def print_dataset_summary(inputs, targets, mask, weights, note_ids, filter_stats):
    """Print dataset statistics and sanity checks."""
    n = len(inputs)
    print(f"\nDataset: {n} observations")

    # Filter stats
    print(f"\n  SNR Filtering Summary:")
    print(f"    Total notes examined:  {filter_stats['total_notes']}")
    print(f"    Model-matched:         {filter_stats['matched']}")
    print(f"    H2 targets valid:      {filter_stats['h2_valid']}/{filter_stats['matched']}")
    print(f"    H3 targets valid:      {filter_stats['h3_valid']}/{filter_stats['matched']}")
    print(f"    H4-H8 amp targets:     ALL MASKED (below noise floor)")

    # MIDI range
    midi_vals = inputs[:, 0] * (108 - 21) + 21
    print(f"\n  MIDI range: {midi_vals.min():.0f} - {midi_vals.max():.0f}")
    vel_vals = inputs[:, 1] * 127
    print(f"  Velocity range: {vel_vals.min():.0f} - {vel_vals.max():.0f}")

    # Tier distribution
    tier_counts = {}
    for w in weights:
        for tier, tw in TIER_WEIGHTS.items():
            if abs(w - tw) < 0.01:
                tier_counts[tier] = tier_counts.get(tier, 0) + 1
                break
    for tier in ["gold", "silver", "bronze"]:
        print(f"  {tier}: {tier_counts.get(tier, 0)}")

    # Target coverage per dimension
    target_names = (
        [f"amp_H{h+2}" for h in range(7)] +
        [f"freq_H{h+2}" for h in range(7)] +
        [f"decay_H{h+2}" for h in range(7)] +
        ["d0_corr"]
    )
    print("\n  Target coverage:")
    for i, name in enumerate(target_names):
        valid = mask[:, i].sum()
        if valid > 0:
            vals = targets[mask[:, i], i]
            print(f"    {name:>12}: {valid:>5}/{n} valid  "
                  f"mean={vals.mean():+7.2f}  std={vals.std():6.2f}  "
                  f"range=[{vals.min():+7.2f}, {vals.max():+7.2f}]")
        else:
            print(f"    {name:>12}:     0/{n} valid  (fully masked)")

    # Per-note breakdown for OBM
    obm_mask = np.array(["obm_" in nid for nid in note_ids])
    if obm_mask.any():
        print("\n  Per-note OBM residuals (valid targets only):")
        for i, nid in enumerate(note_ids):
            if "obm_" not in nid:
                continue
            midi = int(inputs[i, 0] * (108 - 21) + 21)
            h2_str = f"{targets[i,0]:+.1f} dB" if mask[i,0] else "MASKED"
            h3_str = f"{targets[i,1]:+.1f} dB" if mask[i,1] else "MASKED"
            print(f"    {nid:>12} (MIDI {midi:>2}): H2={h2_str:>12}  H3={h3_str:>12}")


def main():
    parser = argparse.ArgumentParser(description="Compute residuals and assemble training data")
    parser.add_argument("--real", default="harmonics.json",
                        help="Real harmonic features JSON")
    parser.add_argument("--model", default="model_harmonics.json",
                        help="Model harmonic features JSON")
    parser.add_argument("--output-dir", default="ml_data",
                        help="Output directory for training_data.npz")
    parser.add_argument("--no-snr-filter", action="store_true",
                        help="Disable SNR-based filtering (not recommended)")
    parser.add_argument("--snr-threshold", type=float, default=SNR_THRESHOLD_DB,
                        help=f"SNR threshold in dB (default: {SNR_THRESHOLD_DB})")
    args = parser.parse_args()

    snr_threshold = args.snr_threshold

    base_dir = os.path.dirname(__file__)

    with open(os.path.join(base_dir, args.real)) as f:
        real_features = json.load(f)
    with open(os.path.join(base_dir, args.model)) as f:
        model_features = json.load(f)

    print(f"Real observations: {len(real_features)}")
    print(f"Model note/vel combos: {len(model_features)}")

    # Load audio and compute SNR for each note
    snr_cache = None
    if not args.no_snr_filter:
        print(f"\nComputing inter-harmonic SNR (threshold: {snr_threshold:.0f} dB)...")
        snr_cache = load_audio_for_snr(real_features)
        print(f"  SNR computed for {len(snr_cache)} notes")

        # Print per-note SNR summary
        print(f"\n  Per-harmonic SNR (dB):")
        print(f"  {'Note':>12} {'H1':>7} {'H2':>7} {'H3':>7} {'H4':>7}")
        for feat in real_features:
            nid = feat.get("id", "")
            snr = snr_cache.get(nid)
            if snr is not None:
                vals = [f"{s:>7.1f}" if not np.isnan(s) else "    nan" for s in snr[:4]]
                print(f"  {nid:>12} {vals[0]} {vals[1]} {vals[2]} {vals[3]}")
    else:
        print("\nSNR filtering DISABLED (--no-snr-filter)")

    inputs, targets, mask, weights, note_ids, filter_stats = assemble_dataset(
        real_features, model_features, snr_cache)

    print_dataset_summary(inputs, targets, mask, weights, note_ids, filter_stats)

    # Save
    output_dir = os.path.join(base_dir, args.output_dir)
    os.makedirs(output_dir, exist_ok=True)
    output_path = os.path.join(output_dir, "training_data.npz")
    np.savez(output_path,
             inputs=inputs,
             targets=targets,
             mask=mask,
             weights=weights)
    print(f"\nSaved to {output_path}")
    print(f"  inputs:  {inputs.shape}")
    print(f"  targets: {targets.shape}")
    print(f"  mask:    {mask.shape}")
    print(f"  weights: {weights.shape}")


if __name__ == "__main__":
    main()
