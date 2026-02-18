"""Isolation scoring for extracted note observations.

Four sub-scores [0-1] combined into a composite:
1. Temporal isolation: concurrent notes with significant energy
2. Harmonic collision: per-harmonic overlap detection (+/-50 cents)
3. Energy dominance: target vs concurrent spectral energy
4. Duration: note length adequacy for analysis

Usage:
    python score_isolation.py --input notes.json --output scored_notes.json
"""

import argparse
import json
import math
import os
import sys
import numpy as np

from goertzel_utils import midi_to_freq


def decay_remaining_amplitude(midi_note, time_since_offset_s):
    """Estimate remaining amplitude fraction of a released note.

    Uses calibration formula: decay_dB_s = 0.26 * exp(0.049 * midi).
    Returns fraction of original amplitude remaining.
    """
    if time_since_offset_s <= 0:
        return 1.0
    decay_rate = 0.26 * math.exp(0.049 * midi_note)  # dB/s
    decay_dB = decay_rate * time_since_offset_s
    return 10.0 ** (-decay_dB / 20.0)


def score_temporal(target, all_notes, window_start_s, window_end_s):
    """Score temporal isolation: how many concurrent notes have significant energy.

    Returns score in [0, 1]. 1.0 = completely isolated.
    """
    score = 1.0
    for other in all_notes:
        if other["id"] == target["id"]:
            continue
        if other["source_file"] != target["source_file"]:
            continue

        # Check if other note overlaps with analysis window
        other_onset = other["onset_s"]
        other_offset = other["offset_s"]

        # Note is held during window
        if other_onset < window_end_s and other_offset > window_start_s:
            # Still sounding -- penalty scaled by relative amplitude
            rel_amp = other["amplitude"] / max(target["amplitude"], 1e-6)
            if rel_amp > 0.1:
                score -= 0.10 * min(rel_amp, 1.0)
        # Note released before window but may still be decaying
        elif other_offset < window_start_s:
            time_since_release = window_start_s - other_offset
            remaining = decay_remaining_amplitude(other["midi_note"], time_since_release)
            rel_energy = remaining * other["amplitude"] / max(target["amplitude"], 1e-6)
            if rel_energy > 0.1:
                score -= 0.10 * min(rel_energy, 1.0)

    return max(0.05, score)  # floor at 0.05 -- never veto on temporal alone


def harmonic_collision_check(target_midi, concurrent_midis, n_harmonics=8):
    """Check which harmonics of target collide with harmonics of concurrent notes.

    Two frequencies collide if within +/-50 cents (~2.93%).

    Returns:
        overall_score: weighted fraction of clean harmonics [0-1]
        harmonic_mask: boolean array, True = clean, False = collided
    """
    target_f0 = midi_to_freq(target_midi)
    collision_threshold = 2.0 ** (50.0 / 1200.0)  # ~1.0293

    harmonic_mask = np.ones(n_harmonics, dtype=bool)  # True = clean

    for h_target in range(n_harmonics):
        fh = target_f0 * (h_target + 1)
        for other_midi in concurrent_midis:
            other_f0 = midi_to_freq(other_midi)
            for h_other in range(n_harmonics):
                fh_other = other_f0 * (h_other + 1)
                ratio = max(fh, fh_other) / max(min(fh, fh_other), 1e-6)
                if ratio < collision_threshold:
                    harmonic_mask[h_target] = False
                    break
            if not harmonic_mask[h_target]:
                break

    # Weighted score: H1-H4 weighted 2x relative to H5-H8
    weights = np.array([2.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0])
    clean_weight = np.sum(weights[harmonic_mask])
    total_weight = np.sum(weights)
    overall_score = clean_weight / total_weight

    return overall_score, harmonic_mask.tolist()


def score_harmonic_collision(target, all_notes, window_start_s, window_end_s, n_harmonics=8):
    """Score harmonic collision for target note against concurrent notes.

    Returns (overall_score, harmonic_mask).
    """
    concurrent_midis = []
    for other in all_notes:
        if other["id"] == target["id"]:
            continue
        if other["source_file"] != target["source_file"]:
            continue

        other_onset = other["onset_s"]
        other_offset = other["offset_s"]

        # Check if still has significant energy during window
        has_energy = False
        if other_onset < window_end_s and other_offset > window_start_s:
            has_energy = True
        elif other_offset < window_start_s:
            time_since = window_start_s - other_offset
            remaining = decay_remaining_amplitude(other["midi_note"], time_since)
            if remaining * other["amplitude"] > 0.05:
                has_energy = True

        if has_energy:
            concurrent_midis.append(other["midi_note"])

    if not concurrent_midis:
        return 1.0, [True] * n_harmonics

    return harmonic_collision_check(target["midi_note"], concurrent_midis, n_harmonics)


def score_energy_dominance(target, all_notes, window_start_s, window_end_s):
    """Score energy dominance: fraction of energy from target vs all concurrent.

    Uses amplitude * decay as energy proxy.
    """
    target_energy = target["amplitude"]

    total_energy = target_energy
    for other in all_notes:
        if other["id"] == target["id"]:
            continue
        if other["source_file"] != target["source_file"]:
            continue

        other_onset = other["onset_s"]
        other_offset = other["offset_s"]
        window_mid = (window_start_s + window_end_s) / 2.0

        if other_onset < window_end_s and other_offset > window_start_s:
            # Held note
            total_energy += other["amplitude"]
        elif other_offset < window_start_s:
            time_since = window_mid - other_offset
            remaining = decay_remaining_amplitude(other["midi_note"], time_since)
            total_energy += remaining * other["amplitude"]

    if total_energy < 1e-10:
        return 1.0
    return target_energy / total_energy


def score_duration(duration_s):
    """Score note duration adequacy for harmonic analysis.

    <150ms: 0.0 (too short)
    150-300ms: 0.3 (attack only)
    300-600ms: 0.7 (short sustain)
    600ms+: 1.0 (full analysis)
    """
    if duration_s < 0.150:
        return 0.0
    elif duration_s < 0.300:
        return 0.3
    elif duration_s < 0.600:
        return 0.7
    else:
        return 1.0


def compute_composite_score(temporal, collision, dominance, duration):
    """Weighted geometric mean of sub-scores.

    Collision=0 vetoes (all harmonics contaminated). Duration=0 vetoes (too short).
    Temporal and dominance can be low without vetoing -- the per-harmonic mask
    handles actual contamination.

    Weights: collision 0.35, temporal 0.20, dominance 0.20, duration 0.25
    """
    if collision <= 0.0 or duration <= 0.0:
        return 0.0

    # Floor temporal and dominance to avoid veto from polyphonic density
    temporal = max(temporal, 0.05)
    dominance = max(dominance, 0.05)

    # Weighted geometric mean
    log_score = (0.35 * math.log(collision) +
                 0.20 * math.log(temporal) +
                 0.20 * math.log(dominance) +
                 0.25 * math.log(duration))
    return math.exp(log_score)


def tier_from_score(score):
    """Map composite score to quality tier."""
    if score >= 0.85:
        return "gold"
    elif score >= 0.55:
        return "silver"
    elif score >= 0.15:
        return "bronze"
    else:
        return "reject"


def score_notes(notes):
    """Score isolation for all notes. Modifies notes in-place.

    OBM isolated notes automatically get gold tier.
    """
    # Group notes by source file for efficient concurrent note lookup
    by_file = {}
    for note in notes:
        sf = note["source_file"]
        if sf not in by_file:
            by_file[sf] = []
        by_file[sf].append(note)

    for note in notes:
        # OBM isolated notes are automatically gold
        if note.get("source_type") == "obm_isolated":
            note["isolation_score"] = 1.0
            note["isolation_tier"] = "gold"
            note["sub_scores"] = {
                "temporal": 1.0,
                "collision": 1.0,
                "dominance": 1.0,
                "duration": 1.0,
            }
            note["harmonic_mask"] = [True] * 8
            continue

        duration = note["offset_s"] - note["onset_s"]
        dur_score = score_duration(duration)

        if dur_score == 0.0:
            note["isolation_score"] = 0.0
            note["isolation_tier"] = "reject"
            note["sub_scores"] = {
                "temporal": 0.0,
                "collision": 0.0,
                "dominance": 0.0,
                "duration": 0.0,
            }
            note["harmonic_mask"] = [False] * 8
            continue

        # Analysis window: sustain region (50ms to min(800ms, note end) after onset)
        window_start = note["onset_s"] + 0.050
        window_end = min(note["onset_s"] + 0.800, note["offset_s"])
        if window_start >= window_end:
            window_start = note["onset_s"]
            window_end = note["offset_s"]

        file_notes = by_file[note["source_file"]]

        temp_score = score_temporal(note, file_notes, window_start, window_end)
        collision_score, harmonic_mask = score_harmonic_collision(
            note, file_notes, window_start, window_end)
        dom_score = score_energy_dominance(note, file_notes, window_start, window_end)

        composite = compute_composite_score(temp_score, collision_score, dom_score, dur_score)
        tier = tier_from_score(composite)

        note["isolation_score"] = round(composite, 4)
        note["isolation_tier"] = tier
        note["sub_scores"] = {
            "temporal": round(temp_score, 4),
            "collision": round(collision_score, 4),
            "dominance": round(dom_score, 4),
            "duration": round(dur_score, 4),
        }
        note["harmonic_mask"] = harmonic_mask


def print_summary(notes):
    """Print tier distribution and coverage summary."""
    tiers = {"gold": 0, "silver": 0, "bronze": 0, "reject": 0}
    for note in notes:
        tier = note.get("isolation_tier", "reject")
        tiers[tier] = tiers.get(tier, 0) + 1

    total = len(notes)
    print(f"\nIsolation scoring summary ({total} notes):")
    for tier in ["gold", "silver", "bronze", "reject"]:
        count = tiers[tier]
        pct = 100.0 * count / max(total, 1)
        print(f"  {tier:>7}: {count:>5} ({pct:>5.1f}%)")

    usable = tiers["gold"] + tiers["silver"] + tiers["bronze"]
    print(f"  usable: {usable:>5} (bronze+)")

    # MIDI range of usable notes
    usable_midis = [n["midi_note"] for n in notes if n.get("isolation_tier") != "reject"]
    if usable_midis:
        print(f"  MIDI range (usable): {min(usable_midis)}-{max(usable_midis)}")

    # Per-source breakdown
    print("\nPer-source breakdown:")
    by_source = {}
    for n in notes:
        src = os.path.basename(n["source_file"])
        if src not in by_source:
            by_source[src] = {"gold": 0, "silver": 0, "bronze": 0, "reject": 0}
        by_source[src][n.get("isolation_tier", "reject")] += 1

    for src in sorted(by_source.keys()):
        counts = by_source[src]
        total_src = sum(counts.values())
        usable_src = counts["gold"] + counts["silver"] + counts["bronze"]
        print(f"  {src}: {total_src} total, {usable_src} usable "
              f"(G:{counts['gold']} S:{counts['silver']} B:{counts['bronze']})")


def main():
    parser = argparse.ArgumentParser(description="Score isolation quality for extracted notes")
    parser.add_argument("--input", default="notes.json",
                        help="Input JSON from extract_notes.py")
    parser.add_argument("--output", default="scored_notes.json",
                        help="Output JSON with isolation scores")
    args = parser.parse_args()

    input_path = os.path.join(os.path.dirname(__file__), args.input)
    with open(input_path) as f:
        notes = json.load(f)

    print(f"Scoring isolation for {len(notes)} notes...")
    score_notes(notes)
    print_summary(notes)

    output_path = os.path.join(os.path.dirname(__file__), args.output)
    with open(output_path, 'w') as f:
        json.dump(notes, f, indent=2)
    print(f"\nSaved to {output_path}")


if __name__ == "__main__":
    main()
