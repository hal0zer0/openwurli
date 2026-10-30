#!/usr/bin/env python3
"""Sequential importance sampling over the keyboard pass's instrument parameters (plan §5k): ABC-PMC with the
registered pass/fail rules as an indicator likelihood.

Target: prior(theta) x (1/3) sum over splits of P(pass | theta, split). The felt-grade split is a discrete nuisance
with a uniform prior; every proposal is scored under all three splits (one keyboard3b seed per chunk draws the
per-note nuisances for all three).

Coordinates x, in which the prior is uniform on a box intersected with the grade orderings (bass >= middle >= treble):
    stage 4 (plan §5k): ln z*_b, ln z*_m, ln z*_t | p_b, p_m, p_t | ln m_h | ln v_ff | u_vmax | k | t_bass (in)
    stage 5 (plan §5l): ln z*_b, ln z*_m, ln z*_t | p_b, p_m, p_t | ln m_h_b, ln m_h_m, ln m_h_t | u_vmax | k | t_bass
                        (hammer mass graded and ordered like the felt; v_ff fixed at V_FF_STAGE5)
with u_vmax = ln(v_max / v_ff) / ln(V_MAX_CEIL / v_ff): v_max log-uniform on [v_ff, V_MAX_CEIL] is uniform in u.
Every particle and meta file records its stage.

Proposal of generation g: with probability FRESH_SHARE a prior draw; otherwise a weighted particle of generation
g - 1 plus a Gaussian perturbation (covariance KERNEL_SCALE x the weighted particle covariance, plus a ridge),
redrawn whole (particle and perturbation) until inside the prior support. Its density is
    q(x) = (1 - FRESH_SHARE) K(x) / Z + FRESH_SHARE / |S|,
K the kernel mixture, Z its mass inside the support (Monte Carlo), |S| the support volume. A scored proposal's
weight is (1 / |S|) / q(x) x (splits passed) / 3.

Commands:
    seed    CAL SURVEY KB... --out PARTICLES.json [--stage 5 --vff-window LO HI]
                                                      generation 0 from earlier keyboard runs (seeds the proposal only:
                                                      they were scored under the previous stage's rules); for stage 5,
                                                      only instruments with v_ff in the window, mass copied to all grades
    propose PARTICLES.json --n N --chunks C --seed S --prefix P
                                                      writes P.meta.json and P_c{i}.inst.json (keyboard3b --instruments)
    weigh   CAL SURVEY --prefix P --out PARTICLES.json reads P_c{i}_{split}.json, weights, reports
    mix     PARTICLES.json ... --out PARTICLES.json     one generation weighed under several calibrations (the bracket's
                                                      verdict corners): mean of the normalised weights, so the next
                                                      proposal covers every corner's posterior
"""

import argparse
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hammer_contact import BASS_BLANKS_IN, MIDI_LOWEST, PRIORS, SPLITS, STRIKE_K_RANGE, V_MAX_CEIL
from hammer_contact_pool import K_EDGE_BAND, MARGINAL_F4_DB
from hammer_contact_score3c import F4_MIN_SWING_MM, F4_NOTES, load_refs, survivors

# --- Stage priors (plan §5k) ------------------------------------------------------------------------------------
# Strike-law exponent: widened from [0, 2] because pass 3d's survivors piled at 2 (physics review). The upper end is
# the pickup-comb ceiling at A1: the crown's tip-ward end must stay short of where the reed enters the comb, 0-5 mm
# before its tip (photo read, one reed; bass comb unread); with the +1.5 mm scatter that is reached at k 2.52-2.64,
# so inside [0, 2.5] the ceiling never binds. k > 2 is checked against the technician's "about halfway" and the
# patent's mid-reed on the posterior, not bounded in the prior.
SMC_K_RANGE = (0.0, 2.5)
# Stage 5 (plan §5l): the ff hammer speed is a convention, fixed once. Plan §4: action ratio ~2.9 (manual: blow
# 27.4 mm, key dip 9.5 mm) x peak piano key speed 1 m/s; Askenfelt & Jansson's forte 5 m/s over a 47 mm blow, scaled
# to the 200A's 27.4 mm blow, gives the same ~2.9 m/s.
V_FF_STAGE5 = 2.9

# --- Sampler controls (numerical; checked by the ESS and by generation-to-generation agreement) -------------------
FRESH_SHARE = 0.2      # share of fresh prior draws per generation, so a few seeds cannot lock onto one mode
KERNEL_SCALE = 2.0     # kernel covariance = 2 x weighted particle covariance (Beaumont et al. 2009, Biometrika 96)
RIDGE_FRAC = 0.01      # ridge added to the kernel covariance, as a fraction of each coordinate's prior variance
Z_MC = 200_000         # Monte Carlo draws for the kernel mixture's in-support mass Z
DRAW_BATCH = 4096      # kernel draws per rejection round
LOGQ_BATCH = 512       # proposals per block when evaluating the kernel mixture density
ESS_TARGET = 300       # stop when a generation's effective sample size reaches this
# A generation whose weights rest on a few particles would collapse the next kernel onto them. Below this ESS the
# kernel is built from tempered weights W^alpha, alpha the largest in (0, 1] giving this ESS (plan §5l amendment).
# The proposal changes, the target does not: the importance weights stay valid for any proposal with fresh draws.
KERNEL_MIN_ESS = 50
TEMPER_BISECTIONS = 60  # bisection steps for alpha (numerical; resolves alpha far below any effect)
MAX_GEN = 4            # generations before reporting without reaching ESS_TARGET
A1_HALFWAY_FRAC = 0.6  # report: posterior share striking A1 beyond this fraction of L ("about halfway", mid-reed)
M_H_EDGE_FRAC = 0.1    # report (stage 5, plan §5l): bass hammer mass within this fraction of the prior's ceiling

LNZ = (np.log(PRIORS["z_star"][1]), np.log(PRIORS["z_star"][2]))
P_RANGE = (PRIORS["p"][1], PRIORS["p"][2])
LNM = (np.log(PRIORS["m_h"][1]), np.log(PRIORS["m_h"][2]))
LNV = (np.log(PRIORS["v_ff"][1]), np.log(PRIORS["v_ff"][2]))
UNIT = (0.0, 1.0)
ORDERED_GROUP_FRACTION = 1.0 / 6.0  # an ordered triple fills 1/3! of its cube
SPLIT_NAMES = tuple(sorted(SPLITS))


class Layout:
    """One stage's coordinates: the prior box, the ordered triples, and the maps to and from instrument parameters."""

    def __init__(self, stage):
        self.stage = stage
        if stage == 4:
            self.box = np.array([LNZ, LNZ, LNZ, P_RANGE, P_RANGE, P_RANGE, LNM, LNV, UNIT, SMC_K_RANGE, BASS_BLANKS_IN])
            self.names = ("ln z*_b", "ln z*_m", "ln z*_t", "p_b", "p_m", "p_t", "ln m_h", "ln v_ff", "u_vmax", "k",
                          "t_bass")
            self.ordered = ((0, 1, 2),)
        elif stage == 5:
            self.box = np.array([LNZ, LNZ, LNZ, P_RANGE, P_RANGE, P_RANGE, LNM, LNM, LNM, UNIT, SMC_K_RANGE,
                                 BASS_BLANKS_IN])
            self.names = ("ln z*_b", "ln z*_m", "ln z*_t", "p_b", "p_m", "p_t", "ln m_h_b", "ln m_h_m", "ln m_h_t",
                          "u_vmax", "k", "t_bass")
            self.ordered = ((0, 1, 2), (6, 7, 8))
        else:
            sys.exit(f"unknown stage {stage}")
        self.log_vol = float(np.sum(np.log(self.box[:, 1] - self.box[:, 0]))
                             + len(self.ordered) * np.log(ORDERED_GROUP_FRACTION))

    def in_support(self, x):
        ok = np.all((x >= self.box[:, 0]) & (x <= self.box[:, 1]), axis=1)
        for a, b, c in self.ordered:
            ok &= (x[:, a] >= x[:, b]) & (x[:, b] >= x[:, c])
        return ok

    def prior_draw(self, rng, n):
        x = rng.uniform(self.box[:, 0], self.box[:, 1], (n, len(self.box)))
        for g in self.ordered:
            x[:, list(g)] = np.sort(x[:, list(g)], axis=1)[:, ::-1]  # uniform on the ordered region
        return x

    def to_x(self, inst):
        z, p = np.log(np.asarray(inst["z_star"])), np.asarray(inst["p"])
        if self.stage == 4:
            v = np.asarray(inst["v"])
            u = np.log(np.asarray(inst["v_max"]) / v) / np.log(V_MAX_CEIL / v)
            return np.column_stack([z, p, np.log(inst["m_h"]), np.log(v), u, inst["k"], inst["t_bass_in"]])
        m = np.log(np.asarray(inst["m_h"]))
        m = m if m.ndim == 2 else np.repeat(m[:, None], 3, axis=1)  # a shared mass seeds every grade
        u = np.clip(np.log(np.asarray(inst["v_max"]) / V_FF_STAGE5) / np.log(V_MAX_CEIL / V_FF_STAGE5), 0.0, 1.0)
        return np.column_stack([z, p, m, u, inst["k"], inst["t_bass_in"]])

    def to_inst(self, x):
        if self.stage == 4:
            v = np.exp(x[:, 7])
            return dict(z_star=np.exp(x[:, 0:3]), p=x[:, 3:6], m_h=np.exp(x[:, 6]), v=v,
                        v_max=v * np.exp(x[:, 8] * np.log(V_MAX_CEIL / v)), k=x[:, 9], t_bass_in=x[:, 10])
        v = np.full(len(x), V_FF_STAGE5)
        return dict(z_star=np.exp(x[:, 0:3]), p=x[:, 3:6], m_h=np.exp(x[:, 6:9]), v=v,
                    v_max=v * np.exp(x[:, 9] * np.log(V_MAX_CEIL / v)), k=x[:, 10], t_bass_in=x[:, 11])


LAYOUTS = {s: Layout(s) for s in (4, 5)}
# Stage-4 names kept at module level (earlier checks import them).
BOX, NAMES, LOG_VOL = LAYOUTS[4].box, LAYOUTS[4].names, LAYOUTS[4].log_vol
in_support, prior_draw, to_x, to_inst = (LAYOUTS[4].in_support, LAYOUTS[4].prior_draw, LAYOUTS[4].to_x,
                                         LAYOUTS[4].to_inst)


def layout_of(d):
    return LAYOUTS[d.get("stage", 4)]


def kernel(particles):
    x, W = np.asarray(particles["x"]), np.asarray(particles["W"])
    box = layout_of(particles).box
    if len(x) < 2:
        sys.exit(f"generation {particles['gen']}: {len(x)} particle(s); a kernel needs at least 2")
    mu = W @ x
    cov = (x - mu).T @ ((x - mu) * W[:, None]) / (1.0 - np.sum(W ** 2))
    prior_var = (box[:, 1] - box[:, 0]) ** 2 / 12.0
    sig = KERNEL_SCALE * cov + RIDGE_FRAC * np.diag(prior_var)
    return x, W, np.linalg.cholesky(sig)


def ess(W):
    W = np.asarray(W) / np.sum(W)
    return 1.0 / np.sum(W ** 2)


def tempered(W):
    """Kernel mixture weights: W itself, or W^alpha when ess(W) < KERNEL_MIN_ESS. Returns (weights, alpha)."""
    W = np.asarray(W, float)
    if ess(W) >= KERNEL_MIN_ESS:
        return W, 1.0
    if len(W) <= KERNEL_MIN_ESS:
        return np.full(len(W), 1.0 / len(W)), 0.0
    lo, hi = 0.0, 1.0
    for _ in range(TEMPER_BISECTIONS):
        mid = 0.5 * (lo + hi)
        lo, hi = (mid, hi) if ess(W ** mid) >= KERNEL_MIN_ESS else (lo, mid)
    Wt = W ** lo
    return Wt / Wt.sum(), lo


def kernel_draw(rng, x, W, L, n):
    j = rng.choice(len(x), size=n, p=W)
    return x[j] + rng.standard_normal((n, x.shape[1])) @ L.T


def log_kernel_density(y, x, W, L):
    d = x.shape[1]
    log_det = 2.0 * np.sum(np.log(np.diag(L)))
    out = np.empty(len(y))
    for a in range(0, len(y), LOGQ_BATCH):
        diff = y[a:a + LOGQ_BATCH, None, :] - x[None, :, :]
        sol = np.linalg.solve(L, diff.reshape(-1, d).T).T.reshape(diff.shape)
        lp = -0.5 * np.sum(sol ** 2, axis=2) - 0.5 * (d * np.log(2 * np.pi) + log_det) + np.log(W)[None, :]
        m = lp.max(axis=1)
        out[a:a + LOGQ_BATCH] = m + np.log(np.exp(lp - m[:, None]).sum(axis=1))
    return out


def wquant(a, w, qs=(0.1, 0.5, 0.9)):
    o = np.argsort(a)
    c = np.cumsum(w[o]) / np.sum(w)
    return [a[o][np.searchsorted(c, qq)] for qq in qs]


def cmd_seed(args):
    lay = LAYOUTS[args.stage]
    fits, rec = load_refs(args.cal, args.survey)
    by_seed = {}
    for path in args.kb:
        kb = json.load(open(path))
        by_seed.setdefault(kb["seed"], {})[kb["split"]] = kb
    xs, ws = [], []
    for s, runs in sorted(by_seed.items()):
        if set(runs) != set(SPLIT_NAMES):
            sys.exit(f"seed {s}: splits {sorted(runs)}, need all of {SPLIT_NAMES}")
        passes = np.column_stack([survivors(runs[sp], fits, rec) for sp in SPLIT_NAMES])
        keep = passes.any(axis=1)
        if args.vff_window:
            v = np.asarray(runs[SPLIT_NAMES[0]]["instruments"]["v"])
            keep &= (v >= args.vff_window[0]) & (v < args.vff_window[1])
        inst = {k: np.asarray(v)[keep] for k, v in runs[SPLIT_NAMES[0]]["instruments"].items()}
        xs.append(lay.to_x(inst))
        ws.append(passes[keep].sum(axis=1) / len(SPLIT_NAMES))
    x, w = np.vstack(xs), np.concatenate(ws)
    json.dump(dict(gen=0, stage=lay.stage, x=x.tolist(), W=(w / w.sum()).tolist(),
                   note="seed only: previous stage's rules"), open(args.out, "w"))
    print(f"generation 0: {len(x)} particles from seeds {sorted(by_seed)}")


def cmd_propose(args):
    particles = json.load(open(args.particles))
    lay = layout_of(particles)
    rng = np.random.default_rng(args.seed)
    w_kernel, alpha = tempered(particles["W"])
    x, W, L = kernel(dict(particles, W=w_kernel.tolist()))
    z = np.mean(lay.in_support(kernel_draw(rng, x, W, L, Z_MC)))
    n_fresh = rng.binomial(args.n, FRESH_SHARE)
    got, tried = [], 0
    while sum(len(g) for g in got) < args.n - n_fresh:
        y = kernel_draw(rng, x, W, L, DRAW_BATCH)
        tried += DRAW_BATCH
        got.append(y[lay.in_support(y)])
    prop = np.vstack([lay.prior_draw(rng, n_fresh)] + got)[:args.n]
    prop = prop[rng.permutation(len(prop))]
    log_q = np.logaddexp(np.log1p(-FRESH_SHARE) + log_kernel_density(prop, x, W, L) - np.log(z),
                         np.log(FRESH_SHARE) - lay.log_vol)
    chunks = np.array_split(np.arange(args.n), args.chunks)
    for i, idx in enumerate(chunks):
        inst = {k: v.tolist() for k, v in lay.to_inst(prop[idx]).items()}
        json.dump(dict(instruments=inst), open(f"{args.prefix}_c{i}.inst.json", "w"))
    json.dump(dict(gen=particles["gen"] + 1, stage=lay.stage, n=args.n, chunks=args.chunks, seed=args.seed,
                   x=prop.tolist(),
                   log_q=log_q.tolist(), z=float(z), z_mc_accept=float(len(np.vstack(got))) / tried,
                   fresh=int(n_fresh), kernel_alpha=alpha), open(f"{args.prefix}.meta.json", "w"))
    print(f"generation {particles['gen'] + 1}: {args.n} proposals ({n_fresh} fresh) in {args.chunks} chunks; "
          f"kernel in-support mass Z {z:.3f}; kernel weights alpha {alpha:.3f} (ESS {ess(particles['W']):.0f} -> "
          f"{ess(w_kernel):.0f})")


def cmd_weigh(args):
    fits, rec = load_refs(args.cal, args.survey)
    meta = json.load(open(f"{args.prefix}.meta.json"))
    lay = layout_of(meta)
    x, log_q = np.asarray(meta["x"]), np.asarray(meta["log_q"])
    chunks = np.array_split(np.arange(meta["n"]), meta["chunks"])
    passes = np.zeros((meta["n"], len(SPLIT_NAMES)), bool)
    a1_frac = np.full(meta["n"], np.nan)
    f4_db = np.full(meta["n"], np.nan)
    for i, idx in enumerate(chunks):
        seeds = set()
        for s, sp in enumerate(SPLIT_NAMES):
            kb = json.load(open(f"{args.prefix}_c{i}_{sp}.json"))
            if kb["n"] != len(idx) or kb["split"] != sp:
                sys.exit(f"chunk {i} {sp}: n {kb['n']} split {kb['split']}, expected {len(idx)} {sp}")
            if not np.allclose(kb["instruments"]["m_h"], lay.to_inst(x[idx])["m_h"]):
                sys.exit(f"chunk {i} {sp}: instruments differ from the proposal")
            seeds.add(kb["seed"])
            passes[idx, s] = survivors(kb, fits, rec)
            if s == 0:
                a1 = kb["notes"][str(MIDI_LOWEST)]
                a1_frac[idx] = np.asarray(a1["d_s_mm"]) / a1["L_mm"]
                f4_db[idx] = np.min([20 * np.log10(np.asarray(kb["notes"][str(m)]["swing_mm"]) / F4_MIN_SWING_MM)
                                     for m in F4_NOTES], axis=0)
        if len(seeds) != 1:
            sys.exit(f"chunk {i}: splits run with seeds {sorted(seeds)}; one seed must draw all three")
    log_base = -lay.log_vol - log_q  # prior / proposal density; normalised away, so shifted by its max for range
    w_split = np.exp(log_base - log_base.max())[:, None] * passes / len(SPLIT_NAMES)
    w = w_split.sum(axis=1)
    keep = w > 0
    print(f"generation {meta['gen']}: {meta['n']} proposals ({meta['fresh']} fresh); survivors by split "
          + ", ".join(f"{sp} {passes[:, s].sum()}" for s, sp in enumerate(SPLIT_NAMES))
          + f"; distinct {keep.sum()}")
    if not keep.any():
        return
    W = w[keep] / w[keep].sum()
    ess = 1.0 / np.sum(W ** 2)
    print(f"  ESS {ess:.0f} (target {ESS_TARGET}); split posterior "
          + ", ".join(f"{sp} {w_split[:, s].sum() / w.sum():.2f}" for s, sp in enumerate(SPLIT_NAMES)))
    inst = lay.to_inst(x[keep])
    rows = [(f"z*_{g} mm", inst["z_star"][:, gi] * 1e3) for gi, g in enumerate(("bass", "middle", "treble"))]
    rows += [(f"p_{g}", inst["p"][:, gi]) for gi, g in enumerate(("bass", "middle", "treble"))]
    if inst["m_h"].ndim == 2:
        rows += [(f"m_h_{g} g", inst["m_h"][:, gi] * 1e3) for gi, g in enumerate(("bass", "middle", "treble"))]
        rows += [("m_h bass/treble", inst["m_h"][:, 0] / inst["m_h"][:, 2])]
    else:
        rows += [("m_h g", inst["m_h"] * 1e3), ("v_ff m/s", inst["v"])]
    rows += [("v_max m/s", inst["v_max"]), ("k", inst["k"]),
             ("bass blank mil", inst["t_bass_in"] * 1e3), ("A1 strike d_s/L", a1_frac[keep])]
    for name, a in rows:
        print(f"  {name:18s} p10/50/90 " + "/".join(f"{v:.2f}" for v in wquant(a, W)))
    k_lo, k_hi = SMC_K_RANGE
    k_prev = STRIKE_K_RANGE[1]
    print(f"  k above the previous edge {k_prev}: {np.sum(W[inst['k'] > k_prev]):.2f}; within {K_EDGE_BAND} of "
          f"{k_hi}: {np.sum(W[inst['k'] >= k_hi - K_EDGE_BAND]):.2f} (prior {K_EDGE_BAND / (k_hi - k_lo):.2f})")
    print(f"  A1 struck beyond {A1_HALFWAY_FRAC} L: {np.sum(W[a1_frac[keep] > A1_HALFWAY_FRAC]):.2f}; "
          f"F4 margin < {MARGINAL_F4_DB} dB: {np.sum(W[f4_db[keep] < MARGINAL_F4_DB]):.2f}")
    if inst["m_h"].ndim == 2:
        ceil = PRIORS["m_h"][2]
        print(f"  m_h_bass within {M_H_EDGE_FRAC:.0%} of its {ceil * 1e3:.0f} g ceiling: "
              f"{np.sum(W[inst['m_h'][:, 0] >= ceil * (1 - M_H_EDGE_FRAC)]):.2f} "
              f"(prior {1 - (1 - np.log(1 / (1 - M_H_EDGE_FRAC)) / np.log(ceil / PRIORS['m_h'][1])) ** 3:.2f})")
    json.dump(dict(gen=meta["gen"], stage=lay.stage, x=x[keep].tolist(), W=W.tolist(), ess=float(ess),
                   passes=passes[keep].tolist()), open(args.out, "w"))


def cmd_mix(args):
    sets = [json.load(open(p)) for p in args.particles]
    if len({s["gen"] for s in sets}) != 1 or len({s.get("stage", 4) for s in sets}) != 1:
        sys.exit("particle files come from different generations or stages")
    W = {}
    for s in sets:
        for x, w in zip(s["x"], s["W"]):
            W[tuple(x)] = W.get(tuple(x), 0.0) + w / len(sets)
    x = np.array(list(W))
    w = np.array(list(W.values()))
    json.dump(dict(gen=sets[0]["gen"], stage=sets[0].get("stage", 4), x=x.tolist(), W=(w / w.sum()).tolist(),
                   ess=float(1.0 / np.sum((w / w.sum()) ** 2)),
                   mixed_from=args.particles), open(args.out, "w"))
    print(f"generation {sets[0]['gen']}: {len(x)} particles mixed from {len(sets)} weightings; ESS {1.0 / np.sum((w / w.sum()) ** 2):.0f}")


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("seed")
    s.add_argument("cal"), s.add_argument("survey"), s.add_argument("kb", nargs="+")
    s.add_argument("--out", required=True)
    s.add_argument("--stage", type=int, default=4, choices=tuple(LAYOUTS))
    s.add_argument("--vff-window", type=float, nargs=2, default=None, metavar=("LO", "HI"),
                   help="stage 5 seeding: keep instruments with v_ff in [LO, HI) m/s")
    p = sub.add_parser("propose")
    p.add_argument("particles"), p.add_argument("--n", type=int, required=True)
    p.add_argument("--chunks", type=int, required=True), p.add_argument("--seed", type=int, required=True)
    p.add_argument("--prefix", required=True)
    w = sub.add_parser("weigh")
    w.add_argument("cal"), w.add_argument("survey"), w.add_argument("--prefix", required=True)
    w.add_argument("--out", required=True)
    x = sub.add_parser("mix")
    x.add_argument("particles", nargs="+"), x.add_argument("--out", required=True)
    a = ap.parse_args()
    {"seed": cmd_seed, "propose": cmd_propose, "weigh": cmd_weigh, "mix": cmd_mix}[a.cmd](a)


if __name__ == "__main__":
    main()
