#!/usr/bin/env python3
"""Hammer-reed contact prototype: the offline first pass of docs/research/hammer-contact-revision.md.

A point-mass hammer (m_h, speed v) strikes the reed from below through a felt pad,
F = 10 N * (delta / z*)^p while delta > 0 (one-sided), averaged over a +-0.1 L patch centred on
x_s. The reed is the FE beam of reed_fe.py: frequencies, mass-normalised modes and shapes from one
eigenproblem, scaled to pitch.

Prior samples are integrated in lockstep. Each step is an exact modal update (rotation plus the
particular solution for a force held over the step), so the step size is set by the contact force,
not by the highest mode. A sample stops once the hammer is moving away and sits below the largest
reach of the reed patch, or at 4 T1.

The priors and the falsifier are the ones registered in the plan (§4), stated in PRIORS below.

Usage:
    python tools/hammer_contact.py selftest
    python tools/hammer_contact.py run [--n 1000] [--notes 33,36,...] [--damping floor|upper] [--out f.json]
"""

import argparse
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hammer_contact_eval import patch, reed  # noqa: E402

# Registered first-pass priors (plan §4, 2026-10-29).
PRIORS = dict(
    m_h=("log-uniform", 1e-3, 5e-3),  # kg
    z_star=("log-uniform", 0.1e-3, 5e-3),  # m, felt compression at 10 N
    p=("uniform", 2.2, 3.5),
    x_s=("uniform", 0.25, 0.60),  # fraction of L, patch +-0.1 L
    v_ff=("log-uniform", 1.4, 3.5),  # m/s
)
F_REF = 10.0  # N
N_MODES = 20
# Upper damping corner: omega^2 anchored at the faster observed mode-2 loss (G3, ~23 /s at 1295 Hz).
UPPER_ALPHA_REF = 23.0
UPPER_F_REF = 1295.0
# Offline falsifier: the swing prior's dynamic admitted edges (mm at the finger region, ff).
EDGE_MM = {"bass": 4.29, "middle": 2.90, "treble": 2.89}


def section(midi):
    return "bass" if midi <= 52 else ("middle" if midi <= 72 else "treble")


def variants(midi):
    """FE geometry variants (equal weight): bass blanks, or tapered tips."""
    if midi - 32 <= 20:
        return [("bass .020", 0.020 * 25.4e-3, 0.020 * 25.4e-3), ("bass .026", 0.026 * 25.4e-3, 0.026 * 25.4e-3)]
    return [(f"tip {t:.3f}", t * 1e-3, 0.034 * 25.4e-3) for t in (0.51, 0.585, 0.66)]


def draw(rng, n):
    out = {}
    for k, (kind, lo, hi) in PRIORS.items():
        out[k] = np.exp(rng.uniform(np.log(lo), np.log(hi), n)) if kind == "log-uniform" else rng.uniform(lo, hi, n)
    return out


def reed_data(midi, t_tip, t_root):
    r = reed(midi, t_tip, t_root, nm=N_MODES)
    om = r["om"][:N_MODES] * (r["f"] / r["fe1"])  # scaled to pitch, as in the model evaluation
    L = r["L"]
    xs_grid = np.round(np.arange(0.25, 0.6001, 0.01), 2)
    ph_grid = np.array([patch(r, xs)[:N_MODES] for xs in xs_grid])
    xx = np.linspace(L - 8e-3, L - 2e-3, 61)  # finger region, as in reed_fe_inputs.py
    ph_pick = np.array([np.mean(np.interp(xx, r["xe"], r["phi"][:, n])) for n in range(N_MODES)])
    ph_tip = r["phi"][-1, :N_MODES]
    return dict(f=r["f"], om=om, mu=r["mu"], fe1=r["fe1"], xs_grid=xs_grid, ph_grid=ph_grid,
                ph_pick=ph_pick, ph_tip=ph_tip)


def strike_batch(rd, m_h, v, z_star, p, x_s, alpha=None, dt=None, t_max_periods=4.0, early_stop=False):
    """Integrate S strikes in lockstep. Returns per-sample results (arrays of length S)."""
    S = len(m_h)
    om = rd["om"]
    T1 = 1.0 / rd["f"]
    ph = np.stack([np.interp(x_s, rd["xs_grid"], rd["ph_grid"][:, n]) for n in range(N_MODES)], axis=1)
    K = F_REF / z_star ** p
    if dt is None:
        # Resolve the stiffest contact in the batch: rigid-target compression time ~ delta_max / v.
        dmax = ((p + 1) * 0.5 * m_h * v ** 2 / K) ** (1 / (p + 1))
        dt = float(np.min(dmax / v)) / 200.0
        dt = min(dt, T1 / 20000.0)
    # Modes damped past critical don't ring; cap them just under it so the update stays defined.
    alpha = np.zeros(N_MODES) if alpha is None else np.minimum(alpha, 0.9 * om)
    # Kick-drift-kick (impulse method): half kicks from the contact force, and between them the
    # EXACT free (damped) modal flow plus the hammer's free drift. Second order and symplectic when
    # undamped; the free flow is exact at any mode frequency.
    wd = om * np.sqrt(1 - (alpha / om) ** 2)
    e = np.exp(-alpha * dt)
    c, s_ = np.cos(wd * dt), np.sin(wd * dt)

    def force(yy, qq, pp, KK, phh):
        d = yy - np.einsum("sn,sn->s", phh, qq)
        return np.where(d > 0, KK * np.power(np.maximum(d, 0.0), pp), 0.0)

    q = np.zeros((S, N_MODES))
    qd = np.zeros((S, N_MODES))
    y = np.zeros(S)
    vy = v.copy()
    active = np.ones(S, bool)
    in_contact = np.zeros(S, bool)
    n_contacts = np.zeros(S, int)
    t_contact = np.zeros(S)
    t_first_end = np.full(S, np.nan)
    t_end = np.full(S, np.nan)
    F_now = np.zeros(S)
    e0 = 0.5 * m_h * v ** 2
    t = 0.0
    idx = np.arange(S)
    n_steps = int(t_max_periods * T1 / dt) + 1
    for _ in range(n_steps):
        a = idx[active]
        if a.size == 0:
            break
        pha, Ka, pa, ma = ph[a], K[a], p[a], m_h[a]
        F0 = F_now[a]
        # half kick
        vya = vy[a] - F0 / ma * (dt / 2)
        qda = qd[a] + pha * F0[:, None] * (dt / 2)
        qa = q[a]
        # drift: exact free modal flow, hammer free flight
        q_new = e * (qa * c + (qda + alpha * qa) / wd * s_)
        qd_new = e * (qda * c - (wd * qa + alpha * (qda + alpha * qa) / wd) * s_)
        ya = y[a] + vya * dt
        # half kick with the force at the new state
        F1 = force(ya, q_new, pa, Ka, pha)
        vya = vya - F1 / ma * (dt / 2)
        qd_new = qd_new + pha * F1[:, None] * (dt / 2)
        q[a], qd[a], y[a], vy[a], F_now[a] = q_new, qd_new, ya, vya, F1
        now = F1 > 0
        started = now & ~in_contact[a]
        n_contacts[a[started]] += 1
        ended = ~now & in_contact[a]
        first_end = ended & np.isnan(t_first_end[a])
        t += dt
        t_first_end[a[first_end]] = t
        t_contact[a[now]] += dt
        in_contact[a] = now
        # Done: hammer moving away and below the patch's largest possible reach.
        # (early_stop reproduces the model evaluation's rule, 1 mm below the patch's CURRENT position,
        # which can miss a later recontact; kept only for the self-test.)
        if early_stop:
            reach = 1e-3 - np.einsum("sn,sn->s", pha, q_new)
        else:
            reach = np.einsum("sn,sn->s", np.abs(pha), np.sqrt(q_new ** 2 + (qd_new / om) ** 2))
        done = (~now) & (vya < 0) & (n_contacts[a] > 0) & (ya < -reach)
        t_end[a[done]] = t
        active[a[done]] = False
    amp = np.sqrt(q ** 2 + (qd / om) ** 2)  # mass-normalised modal amplitude
    e_reed = 0.5 * np.sum(qd ** 2 + (om * q) ** 2, axis=1)
    e_ham = 0.5 * m_h * vy ** 2
    return dict(
        dt=dt, unfinished=int(active.sum()), n_contacts=n_contacts, t_contact_T1=t_contact / T1,
        t_first_T1=t_first_end / T1, t_end_T1=t_end / T1, amp=amp, q=q, qd=qd, rebound=vy / v,
        energy_err=(e_reed + e_ham - e0) / e0,
        tip_mm=amp * np.abs(rd["ph_tip"])[None, :] * 1e3,
        pick_mm=amp * rd["ph_pick"][None, :] * 1e3,  # signed coupling at the finger region
    )


def selftest():
    """Energy balance (undamped), dt convergence, and agreement with the model evaluation's strike()."""
    from hammer_contact_eval import strike as eval_strike
    rd_a1 = reed_data(33, 0.020 * 25.4e-3, 0.020 * 25.4e-3)
    rng = np.random.default_rng(1)
    pr = draw(rng, 200)
    res = strike_batch(rd_a1, pr["m_h"], pr["v_ff"], pr["z_star"], pr["p"], pr["x_s"])
    print(f"A1 batch of 200: dt {res['dt']*1e6:.3f} us, unfinished {res['unfinished']}, "
          f"max |energy error| {np.max(np.abs(res['energy_err'])):.2e}")
    # dt convergence on a stiff and a soft case
    for zs, label in ((0.2e-3, "stiff"), (3e-3, "soft")):
        one = dict(m_h=np.array([3e-3]), v=np.array([2.0]), z_star=np.array([zs]), p=np.array([3.0]),
                   x_s=np.array([0.5]))
        r1 = strike_batch(rd_a1, one["m_h"], one["v"], one["z_star"], one["p"], one["x_s"])
        r2 = strike_batch(rd_a1, one["m_h"], one["v"], one["z_star"], one["p"], one["x_s"], dt=r1["dt"] / 4)
        print(f"  {label}: contacts {r1['n_contacts'][0]} vs {r2['n_contacts'][0]} (dt/4); tip mode1 "
              f"{r1['tip_mm'][0,0]:.4f} vs {r2['tip_mm'][0,0]:.4f} mm; mode2 re mode1 "
              f"{20*np.log10(r1['tip_mm'][0,1]/r1['tip_mm'][0,0]):.2f} vs "
              f"{20*np.log10(r2['tip_mm'][0,1]/r2['tip_mm'][0,0]):.2f} dB")
    # Linear spring (p = 1) against the model evaluation (its p = 1 path is closed form), 20 modes.
    r = reed(33, 0.020 * 25.4e-3, 0.020 * 25.4e-3, nm=N_MODES)
    for tau_ratio in (0.3, 0.75):
        mh = 3e-3
        T1 = 1 / r["f"]
        K = mh * (np.pi / (tau_ratio * T1)) ** 2
        c7, d20, rb = eval_strike(r, 0.5, mh, 2.0, tau_ratio, nm=N_MODES)
        args = (rd_a1, np.array([mh]), np.array([2.0]), np.array([F_REF / K]), np.array([1.0]), np.array([0.5]))
        for early in (True, False):
            mine = strike_batch(*args, early_stop=early)
            print(f"  p=1 tau {tau_ratio} T1, {'its stop rule' if early else 'full-reach stop'}: contacts "
                  f"{len(c7)} vs {mine['n_contacts'][0]}; tip mode1 {d20[0]*1e3:.3f} vs {mine['tip_mm'][0,0]:.3f} mm; "
                  f"mode2 {d20[1]*1e3:.4f} vs {mine['tip_mm'][0,1]:.4f} mm; rebound {rb:+.3f} vs "
                  f"{mine['rebound'][0]:+.3f}; end {mine['t_end_T1'][0]:.2f} T1")


def run(notes, n, damping, seed, out):
    rng = np.random.default_rng(seed)
    results = {}
    for midi in notes:
        vs = variants(midi)
        rows = []
        for name, t_tip, t_root in vs:
            rd = reed_data(midi, t_tip, t_root)
            alpha = None
            if damping == "upper":
                alpha = UPPER_ALPHA_REF * (rd["om"] / (2 * np.pi * UPPER_F_REF)) ** 2
            pr = draw(rng, n // len(vs))
            res = strike_batch(rd, pr["m_h"], pr["v_ff"], pr["z_star"], pr["p"], pr["x_s"], alpha=alpha)
            swing = np.abs(res["pick_mm"][:, 0])
            m2_tip = 20 * np.log10(np.maximum(res["tip_mm"][:, 1], 1e-30) / res["tip_mm"][:, 0])
            for i in range(len(swing)):
                rows.append(dict(variant=name, m_h=pr["m_h"][i], v=pr["v_ff"][i], z_star=pr["z_star"][i],
                                 p=pr["p"][i], x_s=pr["x_s"][i], contacts=int(res["n_contacts"][i]),
                                 t_contact_T1=float(res["t_contact_T1"][i]), swing_mm=float(swing[i]),
                                 m2_tip_db=float(m2_tip[i]), energy_err=float(res["energy_err"][i]),
                                 mode_ratios=list((rd["om"][:7] / rd["om"][0]).round(4)),
                                 coupling=list((rd["ph_pick"][:7] / rd["ph_pick"][0]).round(4))))
            print(f"MIDI {midi} {name}: {len(swing)} strikes, dt {res['dt']*1e6:.2f} us, unfinished "
                  f"{res['unfinished']}, max |energy err| {np.max(np.abs(res['energy_err'])):.1e}", flush=True)
        results[midi] = rows
        summarise(midi, rows)
    if out:
        with open(out, "w") as fh:
            json.dump(dict(priors=PRIORS, damping=damping, seed=seed, edges=EDGE_MM, results=results), fh)


def summarise(midi, rows):
    edge = EDGE_MM[section(midi)]
    sw = np.array([r["swing_mm"] for r in rows])
    ok = sw <= edge
    def pct(a, q):
        return np.percentile(a, q) if a.size else float("nan")
    for label, sel in (("all", np.ones_like(ok)), ("survivors", ok)):
        sub = [r for r, k in zip(rows, sel) if k]
        if not sub:
            print(f"  {label}: none")
            continue
        c = np.array([r["contacts"] for r in sub])
        tc = np.array([r["t_contact_T1"] for r in sub])
        m2 = np.array([r["m2_tip_db"] for r in sub])
        s2 = np.array([r["swing_mm"] for r in sub])
        print(f"  {label} ({len(sub)}/{len(rows)}, edge {edge} mm): swing p10/50/90 {pct(s2,10):.2f}/"
              f"{pct(s2,50):.2f}/{pct(s2,90):.2f} mm | single-contact {np.mean(c==1)*100:.0f}% | contact "
              f"p50 {pct(tc,50):.2f} T1 | tip mode2 re mode1 p10/50/90 {pct(m2,10):.1f}/{pct(m2,50):.1f}/"
              f"{pct(m2,90):.1f} dB")


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=("selftest", "run"))
    ap.add_argument("--n", type=int, default=1000)
    ap.add_argument("--notes", default="33,42,48,52,55,60,66,72,78,84,90,96")
    ap.add_argument("--damping", choices=("floor", "upper"), default="floor")
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--out", default="")
    a = ap.parse_args()
    if a.cmd == "selftest":
        selftest()
    else:
        run([int(x) for x in a.notes.split(",")], a.n, a.damping, a.seed, a.out)
