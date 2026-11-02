#!/usr/bin/env python3
"""Sequential importance sampling over the keyboard pass's instrument parameters (plan §5k): ABC-PMC with the
registered pass/fail rules as an indicator likelihood.

Target: prior(theta) x (1/3) sum over splits of P(pass | theta, split). The felt-grade split is a discrete nuisance
with a uniform prior; every proposal is scored under all three splits (one keyboard3b seed per chunk draws the
per-note nuisances for all three). Stage 6 also averages P(pass) over the let-off grid, and reports every
shield-dependent result per clearance row (no clearance can be read): its sampling target is the equal mixture of the
strictest and loosest rows, and each row is an exact reweighting of it (hammer_contact_score3c.band_pass).

Coordinates x, in which the prior is uniform on a box intersected with the grade orderings (bass >= middle >= treble):
    stage 4 (plan §5k): ln z*_b, ln z*_m, ln z*_t | p_b, p_m, p_t | ln m_h | ln v_ff | u_vmax | k | t_bass (in)
    stage 5 (plan §5l): ln z*_b, ln z*_m, ln z*_t | p_b, p_m, p_t | ln m_h_b, ln m_h_m, ln m_h_t | u_vmax | k | t_bass
                        (hammer mass graded and ordered like the felt; v_ff fixed at V_FF_STAGE5)
    stage 6 (plan §5n): stage 5's | ln tau_d (felt loss, log-uniform on TAU_D_PRIOR_S)
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
    weigh   CAL SURVEY --prefix P --out PARTICLES.json
                                                      reads P_c{i}_{split}.json, weights, reports (stage 6: per row)
    lift    PARTICLES.json --seed S --out PARTICLES.json  stage 6 generation 0: a stage-5 posterior with tau_d prior draws
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
from hammer_contact_score3c import (F4_MIN_SWING_MM, F4_NOTES, LETOFF_GAP_MM, LETOFF_NB, band_pass, load_refs,
                                    shield_inputs, survivors)

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
# Stage 6 (plan §5n): felt loss in Stulov's hereditary form, tau_d log-uniform on this range (plan §5m: Giordano &
# Winans' branch gaps map to 0.02-0.09 ms, Stulov's fits to Yanagisawa & Nakamura's loops to 0.10-0.29 ms).
TAU_D_PRIOR_S = (0.01e-3, 0.3e-3)
# Stage 6's registered prediction (plan §5m): the C4 ff contact median is >= 10 % shorter for tau_d >= 0.1 ms than
# for tau_d <= 0.02 ms; it fails at < 5 % shorter, or longer.
PREDICT_MIDI = 60
TAU_D_LOW_MAX_S = 0.02e-3
TAU_D_HIGH_MIN_S = 0.1e-3
PREDICT_SHORTER = 0.10
PREDICT_FAIL_SHORTER = 0.05
# Stage 6 shield rows (plan §5n amendment, 2026-11-02). No clearance can be read (unsourced in every held document), so
# every shield-dependent result is reported per row, each row a clearance uniform on its band (mm), and the A1 strike is
# stated as conditional on an unsourced clearance. Rows, expectations and structural rules are the physics review's,
# registered before any weighing. Rows are ordered strictest first: P(pass) rises down the rows.
SHIELD_ROWS_MM = ((11.0, 13.0), (12.0, 14.0), (13.0, 16.0), (14.0, 18.0))
A1_EXPECT = ((0.20, 0.53), (0.15, 0.45), (0.09, 0.40), (0.05, 0.37))  # registered P(A1 beyond A1_HALFWAY_FRAC), per row
A1_TAU_ROWS_SCORED = 2  # structural rule 2 (A1 share lower at high tau_d) is scored on the first two rows

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
MAX_GEN = {4: 4, 5: 4, 6: 8}  # per stage: generations before reporting without reaching ESS_TARGET (6: plan §5n)
A1_HALFWAY_FRAC = 0.6  # report: posterior share striking A1 beyond this fraction of L ("about halfway", mid-reed)
M_H_EDGE_FRAC = 0.1    # report (stage 5, plan §5l): bass hammer mass within this fraction of the prior's ceiling
BOOTSTRAP_DRAWS = 1000  # stage 6: weighted bootstrap of the registered quantities (90 % interval)
BOOTSTRAP_SEED = 0
BOOTSTRAP_PCT = (5, 95)  # the 90 % interval's percentiles
BOOTSTRAP_MIN_SIZE = 2   # smallest resample (a median of a bin needs points on both sides)
LIFT_COPIES = 4        # stage 6 seeding: tau_d prior draws per stage-5 particle (seeds the proposal only)

LNZ = (np.log(PRIORS["z_star"][1]), np.log(PRIORS["z_star"][2]))
P_RANGE = (PRIORS["p"][1], PRIORS["p"][2])
LNM = (np.log(PRIORS["m_h"][1]), np.log(PRIORS["m_h"][2]))
LNV = (np.log(PRIORS["v_ff"][1]), np.log(PRIORS["v_ff"][2]))
LNT = (np.log(TAU_D_PRIOR_S[0]), np.log(TAU_D_PRIOR_S[1]))
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
        elif stage in (5, 6):
            self.box = np.array([LNZ, LNZ, LNZ, P_RANGE, P_RANGE, P_RANGE, LNM, LNM, LNM, UNIT, SMC_K_RANGE,
                                 BASS_BLANKS_IN] + ([LNT] if stage == 6 else []))
            self.names = ("ln z*_b", "ln z*_m", "ln z*_t", "p_b", "p_m", "p_t", "ln m_h_b", "ln m_h_m", "ln m_h_t",
                          "u_vmax", "k", "t_bass") + (("ln tau_d",) if stage == 6 else ())
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
        cols = [z, p, m, u, inst["k"], inst["t_bass_in"]]
        if self.stage == 6:
            cols.append(np.log(inst["tau_d"]))
        return np.column_stack(cols)

    def to_inst(self, x):
        if self.stage == 4:
            v = np.exp(x[:, 7])
            return dict(z_star=np.exp(x[:, 0:3]), p=x[:, 3:6], m_h=np.exp(x[:, 6]), v=v,
                        v_max=v * np.exp(x[:, 8] * np.log(V_MAX_CEIL / v)), k=x[:, 9], t_bass_in=x[:, 10])
        v = np.full(len(x), V_FF_STAGE5)
        inst = dict(z_star=np.exp(x[:, 0:3]), p=x[:, 3:6], m_h=np.exp(x[:, 6:9]), v=v,
                    v_max=v * np.exp(x[:, 9] * np.log(V_MAX_CEIL / v)), k=x[:, 10], t_bass_in=x[:, 11])
        if self.stage == 6:
            inst["tau_d"] = np.exp(x[:, 12])
        return inst


LAYOUTS = {s: Layout(s) for s in (4, 5, 6)}
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


def cmd_lift(args):
    """Stage 6 generation 0: a stage-5 posterior with ln tau_d drawn from its prior, LIFT_COPIES per particle. Seeds
    the proposal only (the stage-5 weights were found without felt loss)."""
    particles = json.load(open(args.particles))
    if particles.get("stage", 4) != 5:
        sys.exit(f"{args.particles}: stage {particles.get('stage', 4)}; lift takes a stage-5 posterior")
    rng = np.random.default_rng(args.seed)
    x5, W5 = np.asarray(particles["x"]), np.asarray(particles["W"])
    x = np.repeat(x5, LIFT_COPIES, axis=0)
    x = np.column_stack([x, rng.uniform(*LNT, len(x))])
    W = np.repeat(W5, LIFT_COPIES) / LIFT_COPIES
    lay = LAYOUTS[6]
    if not lay.in_support(x).all():
        sys.exit("lifted particles fall outside the stage-6 support")
    json.dump(dict(gen=0, stage=6, x=x.tolist(), W=(W / W.sum()).tolist(), lifted_from=args.particles,
                   note="seed only: stage-5 posterior, tau_d from its prior"), open(args.out, "w"))
    print(f"generation 0 (stage 6): {len(x)} particles, {LIFT_COPIES} tau_d draws per stage-5 particle; "
          f"ESS {ess(W):.0f}")


def wboot(stat, W, rng):
    """90 % interval of stat(idx) over BOOTSTRAP_DRAWS resamples by weight, each the size of the ESS (so the interval
    carries the importance sampler's Monte Carlo error, not only the posterior's spread)."""
    m = max(int(round(ess(W))), BOOTSTRAP_MIN_SIZE)
    vals = np.array([stat(rng.choice(len(W), size=m, p=W)) for _ in range(BOOTSTRAP_DRAWS)])
    return np.percentile(vals[np.isfinite(vals)], BOOTSTRAP_PCT) if np.isfinite(vals).any() else (np.nan, np.nan)


def shorter_stat(t, lo_b, hi_b, W, rng):
    """The registered C4 statistic on contact times t: 1 - wmedian(tau_d >= high) / wmedian(tau_d <= low), with its
    bootstrap interval."""
    def stat(idx):
        a, b = t[idx][lo_b[idx]], t[idx][hi_b[idx]]
        return 1.0 - np.median(b) / np.median(a) if a.size and b.size else np.nan

    if not (lo_b.any() and hi_b.any()) or not np.isfinite(t).all():
        return np.nan, (np.nan, np.nan)
    s = 1.0 - wquant(t[hi_b], W[hi_b], (0.5,))[0] / wquant(t[lo_b], W[lo_b], (0.5,))[0]
    return s, wboot(stat, W, rng)


def report_row(name, W, inst, a1_frac, c4_tc, c4_first, w_split, expect, scored_tau, rng):
    """One shield row's registered quantities (plan §5n). W: this row's normalised weights. Returns the A1 share, its
    bootstrap interval, and the tau_d rule's difference interval."""
    td = inst["tau_d"]
    lo_b, hi_b = td <= TAU_D_LOW_MAX_S, td >= TAU_D_HIGH_MIN_S
    beyond = (a1_frac > A1_HALFWAY_FRAC).astype(float)
    share = float(np.sum(W * beyond))
    sh_lo, sh_hi = wboot(lambda idx: np.mean(beyond[idx]), W, rng)
    print(f"  ROW {name}: ESS {ess(W):.0f}; split "
          + ", ".join(f"{sp} {w_split[:, k].sum() / w_split.sum():.2f}" for k, sp in enumerate(SPLIT_NAMES)))
    if expect is not None:
        inside = expect[0] <= share <= expect[1]
        print(f"    A1 struck beyond {A1_HALFWAY_FRAC} L: {share:.2f} [{sh_lo:.2f}, {sh_hi:.2f}] -> "
              f"{'inside' if inside else 'OUTSIDE'} the registered {expect[0]:.2f}-{expect[1]:.2f}")
    else:
        print(f"    A1 struck beyond {A1_HALFWAY_FRAC} L: {share:.2f} [{sh_lo:.2f}, {sh_hi:.2f}] (report only)")
    s, (s_lo, s_hi) = shorter_stat(c4_tc, lo_b, hi_b, W, rng)
    verdict = ("no verdict (a tau_d bin holds no mass)" if not np.isfinite(s) else "HOLDS" if s >= PREDICT_SHORTER
               else "FAILS" if s < PREDICT_FAIL_SHORTER else "neither (between the clauses)")
    print(f"    C4 (MIDI {PREDICT_MIDI}) ff total contact: shorter by {s * 100:.1f} % [{s_lo * 100:.1f}, "
          f"{s_hi * 100:.1f}] at tau_d >= {TAU_D_HIGH_MIN_S * 1e3:g} ms than <= {TAU_D_LOW_MAX_S * 1e3:g} ms "
          f"(mass {W[hi_b].sum():.2f} / {W[lo_b].sum():.2f}) -> {verdict}")
    f, (f_lo, f_hi) = shorter_stat(c4_first, lo_b, hi_b, W, rng)
    print(f"    C4 first-pulse width (unscored): " + (f"shorter by {f * 100:.1f} % [{f_lo * 100:.1f}, {f_hi * 100:.1f}]"
                                                      if np.isfinite(f) else "not recorded in this generation"))
    diff = float(np.sum(W[hi_b] * beyond[hi_b]) / W[hi_b].sum() - np.sum(W[lo_b] * beyond[lo_b]) / W[lo_b].sum())

    def dstat(idx):
        a, b = beyond[idx][lo_b[idx]], beyond[idx][hi_b[idx]]
        return np.mean(b) - np.mean(a) if a.size and b.size else np.nan

    d_lo, d_hi = wboot(dstat, W, rng)
    rule = ("FAILS" if d_lo > 0 else "holds") if scored_tau else "report only"
    bins = []
    edges = (TAU_D_PRIOR_S[0], TAU_D_LOW_MAX_S, TAU_D_HIGH_MIN_S, TAU_D_PRIOR_S[1])
    for a, b in zip(edges[:-1], edges[1:]):
        sel = (td >= a) & (td <= b)
        bins.append(f"{np.sum(W[sel] * beyond[sel]) / W[sel].sum():.2f}" if W[sel].sum() > 0 else "-")
    print(f"    A1 share by tau_d bin (<= {TAU_D_LOW_MAX_S * 1e3:g} / middle / >= {TAU_D_HIGH_MIN_S * 1e3:g} ms): "
          f"{' / '.join(bins)}; high minus low {diff:+.2f} [{d_lo:+.2f}, {d_hi:+.2f}] -> rule 2 {rule}")
    return share, (sh_lo, sh_hi)


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
    # Stage 6 (plan §5n) marginalises the let-off grid and reports per shield row, so a proposal passes with a
    # probability; earlier stages score the 12 mm shield at the primary let-off point.
    stage6 = lay.stage >= 6
    chunks = np.array_split(np.arange(meta["n"]), meta["chunks"])
    passes = np.zeros((meta["n"], len(SPLIT_NAMES)))
    n_grid = len(LETOFF_NB) * len(LETOFF_GAP_MM)
    ok_all = np.zeros((meta["n"], len(SPLIT_NAMES)), bool)
    needs_all = np.full((meta["n"], len(SPLIT_NAMES), n_grid), np.inf)
    c4_first = np.full(meta["n"], np.nan)
    a1_frac = np.full(meta["n"], np.nan)
    f4_db = np.full(meta["n"], np.nan)
    c4_tc = np.full(meta["n"], np.nan)
    for i, idx in enumerate(chunks):
        seeds = set()
        want = lay.to_inst(x[idx])
        for s, sp in enumerate(SPLIT_NAMES):
            kb = json.load(open(f"{args.prefix}_c{i}_{sp}.json"))
            if kb["n"] != len(idx) or kb["split"] != sp:
                sys.exit(f"chunk {i} {sp}: n {kb['n']} split {kb['split']}, expected {len(idx)} {sp}")
            for key in ("m_h", "tau_d"):
                if (key in want) != (key in kb["instruments"]) or \
                        (key in want and not np.allclose(kb["instruments"][key], want[key])):
                    sys.exit(f"chunk {i} {sp}: instruments differ from the proposal ({key})")
            seeds.add(kb["seed"])
            if stage6:
                ok_all[idx, s], needs = shield_inputs(kb, fits, rec)
                needs_all[idx, s] = needs.T
            else:
                passes[idx, s] = survivors(kb, fits, rec)
            if s == 0:
                a1 = kb["notes"][str(MIDI_LOWEST)]
                a1_frac[idx] = np.asarray(a1["d_s_mm"]) / a1["L_mm"]
                f4_db[idx] = np.min([20 * np.log10(np.asarray(kb["notes"][str(m)]["swing_mm"]) / F4_MIN_SWING_MM)
                                     for m in F4_NOTES], axis=0)
                if str(PREDICT_MIDI) in kb["notes"]:
                    c4 = kb["notes"][str(PREDICT_MIDI)]
                    c4_tc[idx] = c4["t_contact_T1"]
                    if "t_first_T1" in c4:  # recorded from stage-6 generation 2 on
                        c4_first[idx] = c4["t_first_T1"]
        if len(seeds) != 1:
            sys.exit(f"chunk {i}: splits run with seeds {sorted(seeds)}; one seed must draw all three")
    if stage6:
        def row_lik(band):
            return np.column_stack([band_pass(ok_all[:, k], needs_all[:, k].T, band) for k in range(len(SPLIT_NAMES))])
        row_L = [row_lik(b) for b in SHIELD_ROWS_MM]
        passes = 0.5 * (row_L[0] + row_L[-1])  # sampling target: the strictest and loosest rows, equally
    log_base = -lay.log_vol - log_q  # prior / proposal density; normalised away, so shifted by its max for range
    w_split = np.exp(log_base - log_base.max())[:, None] * passes / len(SPLIT_NAMES)
    w = w_split.sum(axis=1)
    keep = w > 0
    print(f"generation {meta['gen']}: {meta['n']} proposals ({meta['fresh']} fresh); "
          f"{'summed pass probability' if lay.stage >= 6 else 'survivors'} by split "
          + ", ".join(f"{sp} {passes[:, s].sum():g}" for s, sp in enumerate(SPLIT_NAMES))
          + f"; distinct {keep.sum()}" + ("; let-off grid marginalised; sampling target = rows "
                                          f"{SHIELD_ROWS_MM[0]} and {SHIELD_ROWS_MM[-1]} mm equally (not itself a result)"
                                          if stage6 else ""))
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
    if "tau_d" in inst:
        rows += [("tau_d ms", inst["tau_d"] * 1e3)]
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
    rows_out = []
    if stage6:
        rng = np.random.default_rng(BOOTSTRAP_SEED)
        inst_all = lay.to_inst(x)
        base = np.exp(log_base - log_base.max())
        shares = []
        named = [(f"{lo:g}-{hi:g} mm", L, A1_EXPECT[r], r < A1_TAU_ROWS_SCORED)
                 for r, ((lo, hi), L) in enumerate(zip(SHIELD_ROWS_MM, row_L))]
        named.append(("no F3 (report only)", row_lik(None), None, False))
        for r, (name, L, expect, scored_tau) in enumerate(named):
            ws = base[:, None] * L / len(SPLIT_NAMES)
            wr = ws.sum(axis=1)
            sel = wr > 0
            Wr = wr[sel] / wr[sel].sum()
            shares.append(report_row(name, Wr, {k: v[sel] for k, v in inst_all.items()}, a1_frac[sel], c4_tc[sel],
                                     c4_first[sel], ws[sel], expect, scored_tau, rng))
            if expect is not None:
                rows_out.append(dict(band=list(SHIELD_ROWS_MM[r]), W=(wr[keep] / wr[keep].sum()).tolist(),
                                     ess=float(1.0 / np.sum(Wr ** 2))))
        row_ess = ", ".join(f"{d['ess']:.0f}" for d in rows_out)
        print(f"  ESS per registered row: {row_ess} (target {ESS_TARGET} each)")
        for r in range(len(SHIELD_ROWS_MM) - 1):
            (a, (a_lo, a_hi)), (b, (b_lo, b_hi)) = shares[r], shares[r + 1]
            half = max((a_hi - a_lo) / 2, (b_hi - b_lo) / 2)
            print(f"  rule 1, {SHIELD_ROWS_MM[r]} -> {SHIELD_ROWS_MM[r + 1]} mm: A1 share {a:.2f} -> {b:.2f} "
                  f"(rise {b - a:+.2f}, allowed {half:.2f}) -> {'FAILS' if b - a > half else 'holds'}")
    json.dump(dict(gen=meta["gen"], stage=lay.stage, x=x[keep].tolist(), W=W.tolist(), ess=float(ess),
                   passes=(passes[keep] if stage6 else passes[keep].astype(bool)).tolist(),
                   **({"rows": rows_out} if stage6 else {})), open(args.out, "w"))


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
    s.add_argument("--stage", type=int, default=4, choices=(4, 5))
    s.add_argument("--vff-window", type=float, nargs=2, default=None, metavar=("LO", "HI"),
                   help="stage 5 seeding: keep instruments with v_ff in [LO, HI) m/s")
    p = sub.add_parser("propose")
    p.add_argument("particles"), p.add_argument("--n", type=int, required=True)
    p.add_argument("--chunks", type=int, required=True), p.add_argument("--seed", type=int, required=True)
    p.add_argument("--prefix", required=True)
    w = sub.add_parser("weigh")
    w.add_argument("cal"), w.add_argument("survey"), w.add_argument("--prefix", required=True)
    w.add_argument("--out", required=True)
    li = sub.add_parser("lift")
    li.add_argument("particles"), li.add_argument("--seed", type=int, required=True)
    li.add_argument("--out", required=True)
    x = sub.add_parser("mix")
    x.add_argument("particles", nargs="+"), x.add_argument("--out", required=True)
    a = ap.parse_args()
    {"seed": cmd_seed, "lift": cmd_lift, "propose": cmd_propose, "weigh": cmd_weigh, "mix": cmd_mix}[a.cmd](a)


if __name__ == "__main__":
    main()
