#!/usr/bin/env python3
"""Step 3 of the hammer-contact revision (plan §5o): one candidate instrument per scored shield row from the stage-6
posterior, the difference table, and the ear-gate renders.

For each row (and the report-only no-F3 row), in order of distance from the pool's centre in what the checks pin:
  4. p to the prior median, z* re-solved per grade to hold the grade's median ff contact (the pair's own run);
  5. strike points for all 64 notes: the law at zero scatter above MIDI 60; at MIDI <= 60 the voiced offset nearest 0
     that passes at v_max (validity, let-off at the primary point for the reach reeds, tip <= the row's lower edge);
  6. per-note reed: the measured-ratio FE variant where surveyed, else the variant set's middle entry; the bass blank
     level is the particle's own; the grade split is the pair's;
  7. re-check under every check at both corners, with the drift guards; a failing pair is replaced by the next.

Usage:
    python tools/hammer_step3.py SURVEY CAL_A CAL_B --prefix G3_PREFIX --out-dir DIR [--render --bench BENCH]
"""

import argparse
import functools
import json
import os
import subprocess
import sys
import wave

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hammer_contact import (ANCHORS, BASS_BLANK_LEVELS_IN, BASS_REEDS, CROWN_MM, INCH, LOG_FLOOR,  # noqa: E402
                            MIDI_LOWEST, MM_PER_M, PRIORS, STRIKE_FLOOR_BY_ANCHOR, STRIKE_FLOOR_MM,
                            STRIKE_SCATTER_MM, VOICED_MAX_MIDI, VOICED_OFFSETS_MM, grade_of, modal_spec_text,
                            preamp_tilt_db, reed_data, strike_batch_w, strike_w_chunked, variants)
import hammer_contact_smc as H  # noqa: E402
from hammer_contact_score import ALLOW_DB, SCORED  # noqa: E402
from hammer_contact_score3c import (DPEAK_MAX_MM, F4_MIN_SWING_MM, F4_NOTES, LETOFF_GAP_MM, LETOFF_NB,  # noqa: E402
                                    PRIMARY, band_pass, load_refs, score, shield_inputs, shield_need)

ORIGIN = "fillet-floor"               # the stage-5/6 strike geometry (plan §5j)
P_MEDIAN = 0.5 * (PRIORS["p"][1] + PRIORS["p"][2])  # step 4: p at the prior median (uniform prior)
REACH_NOTES = (33, 36, 39, 42)        # step 3: the bass-reach pin's notes (the run's notes among the reach reeds)
ALL_NOTES = tuple(range(MIDI_LOWEST, MIDI_LOWEST + 64))
TABLE_NOTES = tuple(range(33, 51)) + tuple(F4_NOTES)  # step 8: the difference table's notes
RENDER_NOTES = (33, 36, 41, 45, 48, 53, 60, 72, 84, 96)  # step 9: the ear-gate notes
SPEEDS = (1.0, 0.5, 0.25)             # steps 8-9: v_ff, v/2, v/4
NULL_GRID = np.linspace(-STRIKE_SCATTER_MM, STRIKE_SCATTER_MM, 13)  # step 5 null flag: the +-1.5 mm range, 0.25 mm
NULL_PCT = (10, 90)                   # step 5 null flag: the range a zero-scatter note must sit inside
ZSOLVE_GRID = 33                      # step 4: ln z* grid points per pass (two passes: whole prior, then the bracket)
MAX_TRIES = 25                        # pool pairs tried per row before the row is reported as failed
RENDER_VELOCITY = 127                 # fixed: the bench's output scale is then equal for every render, so the
#                                       speed enters only through the modal state
RENDER_NOTE_S = 2.0                   # seconds per rendered note
GRADES = ("bass", "middle", "treble")


@functools.lru_cache(maxsize=None)
def reed(midi, t_tip, t_root):
    return reed_data(midi, t_tip, t_root)


def note_variants(midi):
    """The keyboard3b variant list for a note: the bass blank levels for reeds 1-20, the FE variants otherwise."""
    if midi - (MIDI_LOWEST - 1) <= BASS_REEDS:
        return [(f"bass {t:.3f}", t * INCH, t * INCH) for t in BASS_BLANK_LEVELS_IN]
    return variants(midi)


def measured_pick(midi, vs, measured):
    rds = [reed(midi, a, b) for _, a, b in vs]
    return int(np.argmin([abs(r["om"][1] / r["om"][0] - measured[midi]) for r in rds]))


def law_mm(rd, k):
    d_ref, L_ref = ANCHORS[ORIGIN]
    return d_ref * (rd["L"] * MM_PER_M / L_ref) ** k


def place(rd, k, off):
    floor_mm = STRIKE_FLOOR_BY_ANCHOR.get(ORIGIN, STRIKE_FLOOR_MM)
    L_mm = rd["L"] * MM_PER_M
    return np.clip(law_mm(rd, k) + off, floor_mm, L_mm - CROWN_MM / 2)


def strike(rd, n_or_z, p, m_h, v, d_s, tau_d):
    """One batch of strikes on one reed; scalars broadcast to the batch size of z*."""
    z = np.atleast_1d(np.asarray(n_or_z, float))
    full = lambda a: np.full(len(z), float(a))  # noqa: E731
    d = np.broadcast_to(np.asarray(d_s, float), z.shape).copy()
    r = strike_w_chunked(rd, full(m_h), full(v), z, full(p), d, tau_d=full(tau_d))
    f1 = rd["f"]
    f2 = rd["om"][1] / rd["om"][0] * f1
    ratio = np.maximum(np.abs(r["pick_mm"][:, 1]), LOG_FLOOR) / np.maximum(np.abs(r["pick_mm"][:, 0]), LOG_FLOOR)
    r["swing_mm"] = np.abs(r["pick_mm"][:, 0])
    r["m2_out_db"] = 20 * np.log10(ratio) + preamp_tilt_db(f2) - preamp_tilt_db(f1)
    return r


def replay_run(kb, j, measured):
    """keyboard3b's per-note draws for instrument j of one run: variant index and strike scatter, replayed from the
    run's seed in its note order. Checked against the run's recorded strike points."""
    n = kb["n"]
    inst = {k: np.asarray(v, float) for k, v in kb["instruments"].items()}
    rng = np.random.default_rng(kb["seed"])
    level = np.argmin(np.abs(inst["t_bass_in"][:, None] - np.array(BASS_BLANK_LEVELS_IN)[None, :]), axis=1)
    out = {}
    for m_str in kb["notes"]:
        midi = int(m_str)
        vs = note_variants(midi)
        bass = midi - (MIDI_LOWEST - 1) <= BASS_REEDS
        pick = level.copy() if bass else rng.integers(0, len(vs), n)
        if midi in measured and not bass:
            pick[:] = measured_pick(midi, vs, measured)
        scatter = rng.uniform(-STRIKE_SCATTER_MM, STRIKE_SCATTER_MM, n)
        rd = reed(midi, *vs[pick[j]][1:])
        d_s = float(place(rd, inst["k"][j], scatter[j]))
        rec = kb["notes"][m_str]["d_s_mm"][j]
        if abs(d_s - rec) > 1e-9:
            sys.exit(f"replay mismatch at MIDI {midi}: {d_s} vs recorded {rec} mm")
        out[midi] = dict(variant=vs[pick[j]], d_s=d_s, grade=kb["notes"][m_str]["grade"])
    return out


def note_arrays(kb):
    return {int(m): {k: (np.array(v, dtype=float) if isinstance(v, list) else v) for k, v in d.items()}
            for m, d in kb["notes"].items()}


def pinned(notes, i, tau_d, note_set=None):
    """Step 3's pinned vector for instrument i of a notes dict (keyboard3b layout)."""
    use = [m for m in notes if note_set is None or m in note_set]
    c = [np.median([notes[m]["t_contact_T1"][i] for m in use if notes[m]["grade"] == g]) for g in range(3)]
    ends = []
    for m in REACH_NOTES:
        v = notes[m]
        ok = v["voiced_d_peak_mm"][i] <= DPEAK_MAX_MM
        ends.append(np.max(v["voiced_crown_end_mm"][i][ok]) if ok.any() else -np.inf)
    f4 = min(20 * np.log10(notes[m]["swing_mm"][i] / F4_MIN_SWING_MM) for m in F4_NOTES)
    return np.array(c + [np.median(ends), f4, np.log(tau_d)])


def margins(notes, i, fits, rec):
    """Guard 1: the worst passing F1 note per grade (dB; fails counted), the F4 margin (dB), the largest d_peak
    against the validity limit (mm), and the bass reach against the primary gap (mm, best valid voiced offset)."""
    f1 = {}
    for g in range(3):
        mg = []
        for m in SCORED:
            if m in notes and m in rec and notes[m]["grade"] == g:
                a, b = fits.get(m, (0.0, 0.0))
                mg.append(rec[m] + ALLOW_DB - (notes[m]["m2_out_db"][i] + a * notes[m]["swing_mm"][i] + b))
        mg = np.array(mg)
        f1[GRADES[g]] = (float(mg[mg >= 0].min()) if (mg >= 0).any() else np.nan, int((mg < 0).sum()))
    f4 = min(20 * np.log10(notes[m]["swing_mm"][i] / F4_MIN_SWING_MM) for m in F4_NOTES if m in notes)
    dp = max(max(float(v["d_peak_mm"][i]) for v in notes.values()),
             max(float(np.nanmax(v["voiced_d_peak_mm"][i])) if v["voiced"] else float(v["d_peak_vmax_mm"][i])
                 for v in notes.values()))
    reach = []
    for m, v in notes.items():
        if v["voiced"] and m - (MIDI_LOWEST - 1) <= PRIMARY[0]:
            ok = v["voiced_d_peak_mm"][i] <= DPEAK_MAX_MM
            reach.append(np.max(v["voiced_crown_end_mm"][i][ok]) - PRIMARY[1] if ok.any() else -np.inf)
    return dict(f1_worst_pass_db=f1, f4_margin_db=f4, d_peak_vs_limit_mm=dp - DPEAK_MAX_MM,
                reach_vs_gap_mm=float(min(reach)))


def load_pool(prefix, cals, survey):
    """Every (proposal, split) pair of the generation: row likelihoods per corner, pinned vectors, run locations."""
    meta = json.load(open(f"{prefix}.meta.json"))
    lay = H.layout_of(meta)
    x, log_q = np.asarray(meta["x"]), np.asarray(meta["log_q"])
    n, ns = meta["n"], len(H.SPLIT_NAMES)
    refs = [load_refs(c, survey) for c in cals]
    rows = list(H.SHIELD_ROWS_MM) + [None]
    L = np.zeros((len(rows), len(cals), n, ns))
    pins = np.full((n, ns, 6), np.nan)
    where = {}
    tau = lay.to_inst(x)["tau_d"]
    for ci, idx in enumerate(np.array_split(np.arange(n), meta["chunks"])):
        for s, sp in enumerate(H.SPLIT_NAMES):
            path = f"{prefix}_c{ci}_{sp}.json"
            kb = json.load(open(path))
            for c, (fits, rec) in enumerate(refs):
                ok, needs = shield_inputs(kb, fits, rec)
                for r, band in enumerate(rows):
                    L[r, c, idx, s] = band_pass(ok, needs, band)
            notes = note_arrays(kb)
            for j, i in enumerate(idx):
                pins[i, s] = pinned(notes, j, tau[i])
                where[(int(i), s)] = (path, j)
            print(f"  pool: chunk {ci} split {sp} read", flush=True)
    base = np.exp((-lay.log_vol - log_q) - (-lay.log_vol - log_q).max())
    return dict(meta=meta, lay=lay, x=x, L=L, pins=pins, where=where, base=base, rows=rows, refs=refs)


def ranked(pool, r):
    """Pool pairs for row r, nearest the weighted centre first, with the pool's weighted median distance."""
    w = pool["base"][None, :, None] * pool["L"][r] / len(H.SPLIT_NAMES)  # (corner, n, split)
    wn = w / w.sum(axis=(1, 2), keepdims=True)
    both = (w > 0).all(axis=0)
    pw = wn.mean(axis=0)[both]
    P = pool["pins"][both]
    ii, ss = np.nonzero(both)
    med = np.array([H.wquant(P[:, k], pw, (0.5,))[0] for k in range(P.shape[1])])
    spread = np.array([np.subtract(*H.wquant(P[:, k], pw, (0.9, 0.1))) for k in range(P.shape[1])])
    d2 = np.sum(((P - med) / spread) ** 2, axis=1)
    order = np.argsort(d2, kind="stable")
    return [(int(ii[o]), int(ss[o]), float(d2[o])) for o in order], med, spread, \
        float(H.wquant(d2, pw, (0.5,))[0]), int(both.sum())


def build(pool, i, s, row_lo, measured):
    """Steps 4-6 for pair (i, s). Returns (candidate dict, report dict) or (None, reason)."""
    path, j = pool["where"][(i, s)]
    kb = json.load(open(path))
    run = replay_run(kb, j, measured)
    notes_run = note_arrays(kb)
    inst = {k: np.asarray(v)[0] for k, v in pool["lay"].to_inst(pool["x"][i:i + 1]).items()}
    split = H.SPLIT_NAMES[s]
    rep = dict(pair=[i, split], params_posterior={k: np.asarray(v).tolist() for k, v in inst.items()})
    # Step 4: p to the median, z* re-solved per grade on the run's own notes, strikes and reeds.
    z_new, dchange = [], {}
    lnz_lo, lnz_hi = np.log(PRIORS["z_star"][1]), np.log(PRIORS["z_star"][2])
    for g in range(3):
        gn = [m for m in run if run[m]["grade"] == g]
        target = float(np.median([notes_run[m]["t_contact_T1"][j] for m in gn]))
        lo_, hi_ = lnz_lo, lnz_hi
        for _ in range(2):
            grid = np.linspace(lo_, hi_, ZSOLVE_GRID)
            med = np.median([strike(reed(m, *run[m]["variant"][1:]), np.exp(grid), P_MEDIAN, inst["m_h"][g],
                                    inst["v"], run[m]["d_s"], inst["tau_d"])["t_contact_T1"] for m in gn], axis=0)
            above = np.nonzero(np.diff(np.sign(med - target)))[0]
            if above.size == 0:
                return None, f"step 4: {GRADES[g]} contact {target:.3f} T1 unreachable at p {P_MEDIAN} in the z* prior"
            k0 = above[0]
            lo_, hi_ = grid[k0], grid[k0 + 1]
        zg = float(np.exp(np.interp(target, sorted([med[k0], med[k0 + 1]]),
                                    [lo_, hi_] if med[k0] <= med[k0 + 1] else [hi_, lo_])))
        z_new.append(zg)
        after = [strike(reed(m, *run[m]["variant"][1:]), zg, P_MEDIAN, inst["m_h"][g], inst["v"], run[m]["d_s"],
                        inst["tau_d"])["t_contact_T1"][0] for m in gn]
        before = [notes_run[m]["t_contact_T1"][j] for m in gn]
        ch = np.array(after) / np.array(before) - 1
        dchange[GRADES[g]] = dict(target_T1=target, median_after_T1=float(np.median(after)),
                                  per_note_change_min=float(ch.min()), per_note_change_max=float(ch.max()))
    if not (z_new[0] >= z_new[1] >= z_new[2]):
        return None, f"step 4: re-solved z* {np.round(np.array(z_new) * 1e3, 3)} mm leaves the grade ordering"
    rep["step4"] = dict(p=P_MEDIAN, z_star_mm=[z * 1e3 for z in z_new], contact=dchange)
    cand = dict(inst, z_star=np.array(z_new), p=np.full(3, P_MEDIAN), split=split)
    # Steps 5-6: reeds and strike points for all 64 notes.
    level = int(np.argmin(np.abs(inst["t_bass_in"] - np.array(BASS_BLANK_LEVELS_IN))))
    notes, moves, flags = {}, [], []
    for midi in ALL_NOTES:
        vs = note_variants(midi)
        bass = midi - (MIDI_LOWEST - 1) <= BASS_REEDS
        kv = level if bass else (measured_pick(midi, vs, measured) if midi in measured else len(vs) // 2)
        rd = reed(midi, *vs[kv][1:])
        g = grade_of(midi, split)
        args = (cand["z_star"][g], P_MEDIAN, inst["m_h"][g])
        entry = dict(grade=g, L_mm=float(rd["L"] * MM_PER_M), voiced=bool(midi <= VOICED_MAX_MIDI),
                     variant=vs[kv][0])
        if midi <= VOICED_MAX_MIDI:
            offs = np.asarray(VOICED_OFFSETS_MM)
            rv = strike(rd, np.full(len(offs), args[0]), args[1], args[2], inst["v_max"], place(rd, inst["k"], offs),
                        inst["tau_d"])
            ok = (rv["d_peak_mm"] <= DPEAK_MAX_MM) & (rv["tip_mm"][:, 0] <= row_lo)
            if midi - (MIDI_LOWEST - 1) <= PRIMARY[0]:
                ok &= rv["crown_end_swing_mm"] >= PRIMARY[1]
            if not ok.any():
                return None, f"step 5: MIDI {midi} has no passing voiced offset"
            cand_k = np.nonzero(ok)[0]
            best = min(cand_k, key=lambda q: (abs(offs[q]), rv["tip_mm"][q, 0]))
            off = float(offs[best])
            if off != 0.0:
                moves.append((midi, off))
            entry.update(offset_mm=off, voiced_tip_mm=[[float(rv["tip_mm"][best, 0])]],
                         voiced_crown_end_mm=[[float(rv["crown_end_swing_mm"][best])]],
                         voiced_d_peak_mm=[[float(rv["d_peak_mm"][best])]])
        else:
            off = 0.0
            entry["offset_mm"] = off
            rv = strike(rd, args[0], args[1], args[2], inst["v_max"], place(rd, inst["k"], off), inst["tau_d"])
            entry.update(tip_vmax_mm=[float(rv["tip_mm"][0, 0])], d_peak_vmax_mm=[float(rv["d_peak_mm"][0])])
            grid = strike(rd, np.full(len(NULL_GRID), args[0]), args[1], args[2], inst["v"],
                          place(rd, inst["k"], NULL_GRID), inst["tau_d"])
        d_s = float(place(rd, inst["k"], off))
        r = strike(rd, args[0], args[1], args[2], inst["v"], d_s, inst["tau_d"])
        entry.update(d_s_mm=[d_s], swing_mm=[float(r["swing_mm"][0])], m2_out_db=[float(r["m2_out_db"][0])],
                     tip_mm=[float(r["tip_mm"][0, 0])], crown_end_mm=[float(r["crown_end_swing_mm"][0])],
                     d_peak_mm=[float(r["d_peak_mm"][0])], t_contact_T1=[float(r["t_contact_T1"][0])],
                     t_first_T1=[float(r["t_first_T1"][0])], contacts=[int(r["n_contacts"][0])])
        if midi > VOICED_MAX_MIDI:
            for key in ("swing_mm", "m2_out_db"):
                lo_p, hi_p = np.percentile(grid[key], NULL_PCT)
                if not lo_p <= entry[key][0] <= hi_p:
                    flags.append((midi, key, round(entry[key][0], 3), round(float(lo_p), 3), round(float(hi_p), 3)))
        notes[str(midi)] = entry
    rep["step5"] = dict(offset_moves=moves, null_flags=flags)
    cand["notes"] = notes
    return cand, rep


def recheck(cand, refs, row_lo):
    """Step 7: every check at both corners on the finished candidate (n = 1)."""
    kb = dict(n=1, instruments={k: np.atleast_1d(v).tolist() for k, v in cand.items()
                                if k not in ("notes", "split")} | dict(z_star=[cand["z_star"].tolist()],
                                                                        p=[cand["p"].tolist()],
                                                                        m_h=[np.asarray(cand["m_h"]).tolist()]),
              notes=cand["notes"])
    out = []
    for fits, rec in refs:
        notes, _, f1, _, _, f4, valid = score(kb, fits, rec)
        s_prim = float(shield_need(notes, 1, *PRIMARY)[0])
        grid = np.mean([shield_need(notes, 1, nb, gap)[0] <= row_lo for nb in LETOFF_NB for gap in LETOFF_GAP_MM])
        out.append(dict(f1=bool(f1[0]), f4=bool(f4[0]), valid=bool(valid[0]), s_star_primary_mm=s_prim,
                        f2f3_primary=bool(s_prim <= row_lo), letoff_grid_share=float(grid),
                        margins=margins(notes, 0, fits, rec),
                        passes=bool(f1[0] and f4[0] and valid[0] and s_prim <= row_lo)))
    return out, note_arrays(kb)


def speed_table(cand, notes_sel, speeds):
    rows = {}
    for midi in notes_sel:
        e = cand["notes"][str(midi)]
        vs = note_variants(midi)
        rd = reed(midi, *[v for v in vs if v[0] == e["variant"]][0][1:])
        g = e["grade"]
        res = []
        for f in speeds:
            r = strike(rd, cand["z_star"][g], cand["p"][g], cand["m_h"][g], cand["v"] * f, e["d_s_mm"][0],
                       cand["tau_d"])
            res.append(dict(speed=f, swing_mm=float(r["swing_mm"][0]), m2_out_db=float(r["m2_out_db"][0]),
                            t_contact_T1=float(r["t_contact_T1"][0]), contacts=int(r["n_contacts"][0]), r=r, rd=rd))
        rows[midi] = res
    return rows


def render(cand, name, out_dir, bench):
    """Step 9: full-chain renders, one WAV per speed with the ear-gate notes in sequence."""
    tab = speed_table(cand, RENDER_NOTES, SPEEDS)
    paths = []
    for si, f in enumerate(SPEEDS):
        frames, params = [], None
        for midi in RENDER_NOTES:
            t = tab[midi][si]
            e = cand["notes"][str(midi)]
            g = e["grade"]
            one = lambda a: np.array([float(a)])  # noqa: E731
            # The modal state at separation: strike_batch_w itself (one strike, so the same step rule as the table).
            r = strike_batch_w(t["rd"], one(cand["m_h"][g]), one(cand["v"] * f), one(cand["z_star"][g]),
                               one(cand["p"][g]), one(e["d_s_mm"][0]), tau_d=one(cand["tau_d"]))
            txt, _, _ = modal_spec_text(t["rd"], r, 0, midi)
            spec = os.path.join(out_dir, f"{name}_m{midi}_v{f:g}.spec")
            wav = os.path.join(out_dir, f"{name}_m{midi}_v{f:g}.wav")
            open(spec, "w").write(txt)
            subprocess.run([bench, "render", "--note", str(midi), "--velocity", str(RENDER_VELOCITY),
                            "--duration", str(RENDER_NOTE_S), "--modal-spec", spec, "--output", wav],
                           check=True, capture_output=True)
            with wave.open(wav) as w:
                params = params or w.getparams()
                frames.append(w.readframes(w.getnframes()))
        out = os.path.join(out_dir, f"{name}_speed{f:g}.wav")
        with wave.open(out, "wb") as w:
            w.setparams(params)
            for fr in frames:
                w.writeframes(fr)
        paths.append(out)
    return paths


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("survey"), ap.add_argument("cal_a"), ap.add_argument("cal_b")
    ap.add_argument("--prefix", required=True)
    ap.add_argument("--out-dir", required=True)
    ap.add_argument("--render", action="store_true")
    ap.add_argument("--bench", default="target/release/preamp-bench")
    a = ap.parse_args()
    os.makedirs(a.out_dir, exist_ok=True)
    measured = {r["midi"]: r["onset"]["ratio"] for r in json.load(open(a.survey))
                if r.get("onset", {}).get("status") == "line"}
    pool = load_pool(a.prefix, (a.cal_a, a.cal_b), a.survey)
    report = {}
    for r, band in enumerate(pool["rows"]):
        name = "noF3" if band is None else f"row{band[0]:g}-{band[1]:g}"
        lo = np.inf if band is None else band[0]
        order, med, spread, d2_med, n_pool = ranked(pool, r)
        print(f"== {name}: pool {n_pool} pairs; centre {np.round(med, 3).tolist()}; weighted median distance "
              f"{d2_med:.3f}", flush=True)
        tried = []
        for (i, s, d2) in order[:MAX_TRIES]:
            cand, rep = build(pool, i, s, lo, measured)
            if cand is None:
                tried.append(dict(pair=[i, H.SPLIT_NAMES[s]], distance=d2, rejected=rep))
                print(f"  pair {i}/{H.SPLIT_NAMES[s]} (d {d2:.3f}): {rep}", flush=True)
                continue
            checks, notes_c = recheck(cand, pool["refs"], lo)
            run_notes = set(int(m) for m in json.load(open(pool["where"][(i, s)][0]))["notes"])
            d2_after = float(np.sum(((pinned(notes_c, 0, cand["tau_d"], run_notes) - med) / spread) ** 2))
            ok = all(c["passes"] for c in checks) and d2_after <= d2_med
            path, j = pool["where"][(i, s)]
            before = [margins(note_arrays(json.load(open(path))), j, fits, rec) for fits, rec in pool["refs"]]
            entry = dict(rep, distance_before=d2, distance_after=d2_after, distance_limit=d2_med, checks=checks,
                         margins_before=before)
            tried.append(dict(pair=[i, H.SPLIT_NAMES[s]], distance=d2, accepted=ok))
            print(f"  pair {i}/{H.SPLIT_NAMES[s]} (d {d2:.3f} -> {d2_after:.3f}, limit {d2_med:.3f}): "
                  f"checks {[c['passes'] for c in checks]} -> {'ACCEPTED' if ok else 'next'}", flush=True)
            if ok:
                tab = speed_table(cand, TABLE_NOTES, SPEEDS)
                entry["table"] = {m: [{k: v for k, v in t.items() if k not in ("r", "rd")} for t in rows]
                                  for m, rows in tab.items()}
                entry["candidate"] = {k: (np.asarray(v).tolist() if k != "notes" else v) for k, v in cand.items()}
                if a.render:
                    entry["renders"] = render(cand, name, a.out_dir, a.bench)
                report[name] = dict(entry, tried=tried)
                break
        else:
            report[name] = dict(failed=True, tried=tried)
        json.dump(report, open(os.path.join(a.out_dir, "step3.json"), "w"), indent=1, default=float)
    print(f"wrote {os.path.join(a.out_dir, 'step3.json')}")


if __name__ == "__main__":
    main()
