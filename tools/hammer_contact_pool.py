#!/usr/bin/env python3
"""Pool keyboard3b runs drawn with different seeds and score them with the pass-3c rules (plan §5i, §5j).

One seed draws the instruments for every split of a run, so a survivor is identified by (seed, index); the
same instrument can survive in several splits. Reports, per split and pooled across splits: survivor counts as
distinct instruments; the pooled k marginal against the prior's upper edge; and one line per distinct survivor
with its F4 margin and the splits it survives in.

Usage:
    python tools/hammer_contact_pool.py CALIBRATION.json SURVEY.json KB3B.json [KB3B.json ...]
"""

import json
import os
import sys
from collections import defaultdict

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hammer_contact import STRIKE_K_RANGE
from hammer_contact_score import q
from hammer_contact_score3c import F4_MIN_SWING_MM, F4_NOTES, load_refs, survivors

# F4 margin below which a survivor is reported as marginal: near the threshold the top-note swing sits in the
# near-null region, where the contact form alone (Winkler crown vs rigid) moves it 3-12 dB (physics review);
# the low end of that spread.
MARGINAL_F4_DB = 3.0
K_BIN = 0.25         # report-only histogram width for the pooled k marginal
K_EDGE_BAND = 0.1    # report-only: share of survivors within this distance of the prior's upper k edge


def main():
    fits, rec = load_refs(sys.argv[1], sys.argv[2])
    n_by_split = defaultdict(int)
    hits = defaultdict(list)    # (seed, index) -> splits it survives in
    params = {}                 # (seed, index) -> instrument parameters
    f4 = {}                     # (seed, index) -> F4 margin, dB (split-independent: F4 notes are top grade in all)
    seeds = defaultdict(set)
    for path in sys.argv[3:]:
        kb = json.load(open(path))
        sp, seed = kb["split"], kb["seed"]
        if seed in seeds[sp]:
            sys.exit(f"{path}: split {sp} seed {seed} given twice")
        seeds[sp].add(seed)
        n_by_split[sp] += kb["n"]
        ok = survivors(kb, fits, rec)
        inst = {k: np.array(v) for k, v in kb["instruments"].items()}
        for i in np.flatnonzero(ok):
            key = (seed, int(i))
            hits[key].append(sp)
            params[key] = {k: v[i] for k, v in inst.items()}
            f4[key] = min(20 * np.log10(kb["notes"][str(m)]["swing_mm"][i] / F4_MIN_SWING_MM)
                          for m in F4_NOTES if str(m) in kb["notes"])
    print("pooled pass-3c rules | " + " | ".join(
        f"{sp}: {sum(sp in s for s in hits.values())} of {n_by_split[sp]} (seeds {sorted(seeds[sp])})"
        for sp in sorted(n_by_split)))
    print(f"  distinct survivors across splits: {len(hits)}")
    if not hits:
        return
    keys = sorted(hits)
    z = np.array([params[k]["z_star"] for k in keys]) * 1e3
    for gi, gname in enumerate(("bass", "middle", "treble")):
        print(f"  z*_{gname} p10/50/90 {q(z[:, gi])} mm")
    for name, scale, unit in (("m_h", 1e3, "g"), ("v", 1.0, "m/s"), ("v_max", 1.0, "m/s"), ("k", 1.0, "")):
        print(f"  {name} p10/50/90 {q(np.array([params[k][name] for k in keys]) * scale)} {unit}")
    k_all = np.array([params[k]["k"] for k in keys])
    k_lo, k_hi = STRIKE_K_RANGE
    edges = np.arange(k_lo, k_hi + K_BIN / 2, K_BIN)
    counts, _ = np.histogram(k_all, edges)
    print(f"  k marginal, bins of {K_BIN}: " + " ".join(f"[{a:.2f},{b:.2f}):{c}" for a, b, c in zip(edges[:-1], edges[1:], counts)))
    print(f"  k within {K_EDGE_BAND} of the prior edge {k_hi}: {np.mean(k_all >= k_hi - K_EDGE_BAND)*100:.0f}% "
          f"(prior {K_EDGE_BAND / (k_hi - k_lo)*100:.0f}%)")
    marg = sum(f4[k] < MARGINAL_F4_DB for k in keys)
    print(f"  F4 margin < {MARGINAL_F4_DB} dB (marginal): {marg} of {len(keys)}")
    print(f"  seed  index | z* b/m/t mm | p b/m/t | m_h g | v_ff | v_max | k | bass blank mil | F4 margin dB | splits")
    for k in keys:
        pr = params[k]
        print(f"  {k[0]:4d} {k[1]:6d} | {'/'.join(f'{x*1e3:.2f}' for x in pr['z_star'])} | "
              f"{'/'.join(f'{x:.2f}' for x in pr['p'])} | {pr['m_h']*1e3:.2f} | {pr['v']:.2f} | {pr['v_max']:.2f} | "
              f"{pr['k']:.2f} | {pr['t_bass_in']*1e3:.1f} | {f4[k]:5.1f}{' marginal' if f4[k] < MARGINAL_F4_DB else ''} | "
              f"{','.join(hits[k])}")


if __name__ == "__main__":
    main()
