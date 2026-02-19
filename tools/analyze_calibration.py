#!/usr/bin/env python3
"""Analyze calibration/sensitivity CSV from preamp-bench.

Usage:
    python tools/analyze_calibration.py /tmp/calibrate.csv
    python tools/analyze_calibration.py /tmp/sensitivity.csv
"""

import csv
import sys
from collections import defaultdict


def load_csv(path):
    rows = []
    with open(path) as f:
        reader = csv.DictReader(f)
        for r in reader:
            parsed = {}
            for k, v in r.items():
                k = k.strip()
                try:
                    parsed[k] = float(v)
                except ValueError:
                    parsed[k] = v
            rows.append(parsed)
    return rows


def ds_values(rows):
    return sorted(set(r["ds_at_c4"] for r in rows))


def notes_in(rows):
    return sorted(set(int(r["midi"]) for r in rows))


def velocities_in(rows):
    return sorted(set(int(r["velocity"]) for r in rows))


def midi_name(midi):
    names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
    return f"{names[midi % 12]}{midi // 12 - 1}"


def register_spread(rows, vel, metric="t3_rms_db"):
    """Max - min of a metric across notes at a given velocity."""
    vals = [r[metric] for r in rows if int(r["velocity"]) == vel]
    if not vals:
        return 0.0
    return max(vals) - min(vals)


def print_section(title):
    print(f"\n{'=' * 60}")
    print(f"  {title}")
    print(f"{'=' * 60}")


def analyze_single(rows):
    """Analyze a single calibrate run (one DS value)."""
    notes = notes_in(rows)
    vels = velocities_in(rows)

    # 1. Register spread at each velocity
    print_section("Register Spread (t3_rms_db: after output_scale)")
    print(f"  {'Vel':>4}  {'Spread (dB)':>11}  {'Min':>8}  {'Max':>8}  {'Min Note':>8}  {'Max Note':>8}")
    print(f"  {'-' * 55}")
    for v in vels:
        vrows = [r for r in rows if int(r["velocity"]) == v]
        vals = [(r["t3_rms_db"], int(r["midi"])) for r in vrows]
        if not vals:
            continue
        mn = min(vals, key=lambda x: x[0])
        mx = max(vals, key=lambda x: x[0])
        spread = mx[0] - mn[0]
        print(
            f"  {v:>4}  {spread:>11.1f}  {mn[0]:>8.1f}  {mx[0]:>8.1f}"
            f"  {midi_name(mn[1]):>8}  {midi_name(mx[1]):>8}"
        )

    # 2. Trim values and proxy error
    print_section("Trim & Proxy Error (v=127 or highest velocity)")
    max_vel = max(vels)
    vrows = sorted(
        [r for r in rows if int(r["velocity"]) == max_vel],
        key=lambda r: int(r["midi"]),
    )
    print(f"  {'Note':>6}  {'MIDI':>4}  {'Trim':>6}  {'Proxy':>6}  {'ProxErr':>7}  {'t3_rms':>6}  {'t5_rms':>6}  {'Compress':>8}")
    print(f"  {'-' * 65}")
    for r in vrows:
        print(
            f"  {r.get('note_name', midi_name(int(r['midi']))):>6}"
            f"  {int(r['midi']):>4}"
            f"  {r['trim_db']:>6.1f}"
            f"  {r['proxy_db']:>6.1f}"
            f"  {r['proxy_error_db']:>7.1f}"
            f"  {r['t3_rms_db']:>6.1f}"
            f"  {r['t5_rms_db']:>6.1f}"
            f"  {r['tanh_compression_db']:>8.1f}"
        )

    # 3. Dynamic range per note
    if len(vels) >= 2:
        print_section("Dynamic Range (peak: ff - pp)")
        min_vel = min(vels)
        print(f"  {'Note':>6}  {'MIDI':>4}  {'ff peak':>8}  {'pp peak':>8}  {'DR (dB)':>8}")
        print(f"  {'-' * 42}")
        for n in notes:
            ff = [r for r in rows if int(r["midi"]) == n and int(r["velocity"]) == max_vel]
            pp = [r for r in rows if int(r["midi"]) == n and int(r["velocity"]) == min_vel]
            if ff and pp:
                dr = ff[0]["t5_peak_db"] - pp[0]["t5_peak_db"]
                print(
                    f"  {midi_name(n):>6}  {n:>4}"
                    f"  {ff[0]['t5_peak_db']:>8.1f}"
                    f"  {pp[0]['t5_peak_db']:>8.1f}"
                    f"  {dr:>8.1f}"
                )

    # 4. Tanh ceiling map
    print_section("Tanh Compression > 1 dB")
    compressed = [r for r in rows if r["tanh_compression_db"] > 1.0]
    if compressed:
        for r in sorted(compressed, key=lambda x: -x["tanh_compression_db"]):
            print(
                f"  {r.get('note_name', midi_name(int(r['midi']))):>6}"
                f"  v={int(r['velocity']):>3}"
                f"  compression={r['tanh_compression_db']:.1f} dB"
            )
    else:
        print("  None (all < 1 dB)")


def analyze_sensitivity(rows):
    """Analyze a sensitivity sweep (multiple DS values)."""
    ds_vals = ds_values(rows)
    notes = notes_in(rows)
    vels = velocities_in(rows)
    max_vel = max(vels)

    # 1. Register spread at each DS
    print_section("Register Spread vs DS_AT_C4 (v={}, t3_rms_db)".format(max_vel))
    print(f"  {'DS':>6}  {'Spread':>8}  {'Min Note':>8}  {'Max Note':>8}")
    print(f"  {'-' * 36}")
    best_ds = None
    best_spread = 999.0
    for ds in ds_vals:
        ds_rows = [r for r in rows if abs(r["ds_at_c4"] - ds) < 1e-4 and int(r["velocity"]) == max_vel]
        if not ds_rows:
            continue
        vals = [(r["t3_rms_db"], int(r["midi"])) for r in ds_rows]
        mn = min(vals, key=lambda x: x[0])
        mx = max(vals, key=lambda x: x[0])
        spread = mx[0] - mn[0]
        if spread < best_spread:
            best_spread = spread
            best_ds = ds
        print(
            f"  {ds:>6.2f}  {spread:>8.1f}"
            f"  {midi_name(mn[1]):>8}  {midi_name(mx[1]):>8}"
        )
    if best_ds is not None:
        print(f"\n  >>> Optimal DS_AT_C4 = {best_ds:.2f} (spread = {best_spread:.1f} dB)")

    # 2. New trim anchors at the optimal DS
    if best_ds is not None:
        print_section(f"Suggested Trim Anchors (DS={best_ds:.2f}, v={max_vel})")
        ds_rows = sorted(
            [r for r in rows if abs(r["ds_at_c4"] - best_ds) < 1e-4 and int(r["velocity"]) == max_vel],
            key=lambda r: int(r["midi"]),
        )
        if ds_rows:
            # Target: median of t3_rms values (flatten to center)
            t3_vals = [r["t3_rms_db"] for r in ds_rows]
            target = sorted(t3_vals)[len(t3_vals) // 2]
            print(f"  Target t3_rms: {target:.1f} dB (median)")
            print()
            print(f"  {'Note':>6}  {'MIDI':>4}  {'t3_rms':>7}  {'New Trim':>8}")
            print(f"  {'-' * 30}")
            anchors = []
            for r in ds_rows:
                midi = int(r["midi"])
                new_trim = target - r["t3_rms_db"]
                anchors.append((midi, new_trim))
                print(
                    f"  {midi_name(midi):>6}  {midi:>4}"
                    f"  {r['t3_rms_db']:>7.1f}  {new_trim:>+8.1f}"
                )

            # Code snippet
            print()
            print("  // Ready-to-paste anchor array for tables.rs:")
            print("  const ANCHORS: [(f64, f64); {}] = [".format(len(anchors)))
            for midi, trim in anchors:
                print(f"      ({float(midi):.1f}, {trim:.1f}),  // {midi_name(midi)}")
            print("  ];")

    # 3. Sensitivity coefficients: dt3_rms / d_ds per note
    if len(ds_vals) >= 3:
        print_section(f"Sensitivity: d(t3_rms)/d(DS) per note (v={max_vel})")
        print(f"  {'Note':>6}  {'MIDI':>4}  {'Slope (dB/0.1 DS)':>18}")
        print(f"  {'-' * 32}")
        for n in notes:
            points = []
            for ds in ds_vals:
                match = [
                    r
                    for r in rows
                    if abs(r["ds_at_c4"] - ds) < 1e-4
                    and int(r["midi"]) == n
                    and int(r["velocity"]) == max_vel
                ]
                if match:
                    points.append((ds, match[0]["t3_rms_db"]))
            if len(points) >= 2:
                # Linear regression
                n_pts = len(points)
                sx = sum(p[0] for p in points)
                sy = sum(p[1] for p in points)
                sxx = sum(p[0] ** 2 for p in points)
                sxy = sum(p[0] * p[1] for p in points)
                denom = n_pts * sxx - sx * sx
                if abs(denom) > 1e-12:
                    slope = (n_pts * sxy - sx * sy) / denom
                    # Report as dB per 0.1 DS change
                    print(f"  {midi_name(n):>6}  {n:>4}  {slope * 0.1:>18.2f}")

    # 4. Dynamic range per note at each DS
    if len(vels) >= 2:
        min_vel = min(vels)
        print_section(f"Dynamic Range vs DS (ff={max_vel}, pp={min_vel})")
        header = f"  {'Note':>6}"
        for ds in ds_vals:
            header += f"  {ds:.2f}"
        print(header)
        print(f"  {'-' * (8 + 8 * len(ds_vals))}")
        for n in notes:
            line = f"  {midi_name(n):>6}"
            for ds in ds_vals:
                ff = [
                    r
                    for r in rows
                    if abs(r["ds_at_c4"] - ds) < 1e-4
                    and int(r["midi"]) == n
                    and int(r["velocity"]) == max_vel
                ]
                pp = [
                    r
                    for r in rows
                    if abs(r["ds_at_c4"] - ds) < 1e-4
                    and int(r["midi"]) == n
                    and int(r["velocity"]) == min_vel
                ]
                if ff and pp:
                    dr = ff[0]["t5_peak_db"] - pp[0]["t5_peak_db"]
                    line += f"  {dr:>5.1f}"
                else:
                    line += f"  {'--':>5}"
            print(line)


def main():
    if len(sys.argv) < 2:
        print("Usage: python tools/analyze_calibration.py <csv_file>")
        sys.exit(1)

    path = sys.argv[1]
    rows = load_csv(path)

    if not rows:
        print(f"No data in {path}")
        sys.exit(1)

    print(f"Loaded {len(rows)} rows from {path}")

    ds_vals = ds_values(rows)
    if len(ds_vals) > 1:
        print(f"Sensitivity sweep: {len(ds_vals)} DS values")
        analyze_sensitivity(rows)
    else:
        print(f"Single calibration: DS={ds_vals[0]:.2f}")

    # Always show single-DS analysis for current/default DS
    for ds in ds_vals:
        ds_rows = [r for r in rows if abs(r["ds_at_c4"] - ds) < 1e-4]
        if len(ds_vals) > 1:
            print_section(f"Detail for DS={ds:.2f}")
        analyze_single(ds_rows)


if __name__ == "__main__":
    main()
