#!/usr/bin/env python3
"""Pool keyboard3b runs drawn with different seeds and score them with the pass-3c rules (plan §5i, §5j).

One seed draws the instruments for every split of a run, so a survivor is identified by (seed, index); the
same instrument can survive in several splits. Reports, per split and pooled across splits: survivor counts as
distinct instruments, and one line per distinct survivor with the splits it survives in.

Usage:
    python tools/hammer_contact_pool.py CALIBRATION.json SURVEY.json KB3B.json [KB3B.json ...]
"""

import json
import os
import sys
from collections import defaultdict

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hammer_contact_score import q
from hammer_contact_score3c import F4_NOTES, load_refs, survivors


def main():
    fits, rec = load_refs(sys.argv[1], sys.argv[2])
    n_by_split = defaultdict(int)
    hits = defaultdict(list)    # (seed, index) -> splits it survives in
    params = {}                 # (seed, index) -> instrument parameters
    f4 = {}                     # (seed, index) -> top-note C7 swing (split-independent: always the treble grade)
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
        top = str(max(F4_NOTES))
        for i in np.flatnonzero(ok):
            key = (seed, int(i))
            hits[key].append(sp)
            params[key] = {k: v[i] for k, v in inst.items()}
            f4[key] = kb["notes"][top]["swing_mm"][i] if top in kb["notes"] else float("nan")
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
    print(f"  seed  index | z* b/m/t mm | p b/m/t | m_h g | v_ff | v_max | k | bass blank mil | MIDI {max(F4_NOTES)} swing mm | splits")
    for k in keys:
        pr = params[k]
        print(f"  {k[0]:4d} {k[1]:6d} | {'/'.join(f'{x*1e3:.2f}' for x in pr['z_star'])} | "
              f"{'/'.join(f'{x:.2f}' for x in pr['p'])} | {pr['m_h']*1e3:.2f} | {pr['v']:.2f} | {pr['v_max']:.2f} | "
              f"{pr['k']:.2f} | {pr['t_bass_in']*1e3:.1f} | {f4[k]:.3f} | {','.join(hits[k])}")


if __name__ == "__main__":
    main()
