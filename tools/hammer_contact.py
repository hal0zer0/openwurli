#!/usr/bin/env python3
"""Hammer-reed contact prototype: the offline first pass of docs/research/hammer-contact-revision.md.

A point-mass hammer (m_h, speed v) strikes the reed from below through a felt pad,
F = 10 N * (delta / z*)^p while delta > 0 (one-sided), averaged over a +-0.1 L patch centred on
x_s. The reed is the FE beam of reed_fe.py: frequencies, mass-normalised modes and shapes from one
eigenproblem, scaled to pitch.

Prior samples are integrated in lockstep. Each step is an exact modal update (rotation plus the
particular solution for a force held over the step), so the step size is set by the contact force,
not by the highest mode. A sample stops once the hammer is moving away and sits below the largest
reach of the reed patch, or at 60 T1. (A heavy hammer on a light treble reed can ride it for
several periods before separating.)

The priors and the falsifier are the ones registered in the plan (§4), stated in PRIORS below.

Usage:
    python tools/hammer_contact.py selftest
    python tools/hammer_contact.py run [--n 1000] [--notes 33,36,...] [--damping floor|upper] [--out f.json]
"""

import argparse
import json
import os
import subprocess
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
# Upper damping corner: omega^2 anchored at C4's mode-2 loss, the only anchor (~10 /s at 1466 Hz).
# (Pass 1 ran with a G3 anchor, 23 /s at 1295 Hz, withdrawn once G3's line proved fixed.)
UPPER_ALPHA_REF = 10.0
UPPER_F_REF = 1466.0
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


def strike_batch(rd, m_h, v, z_star, p, x_s, alpha=None, dt=None, t_max_periods=60.0):
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
        dt = min(dt, T1 / 2000.0)  # the free modal flow is exact, so this cap only bounds the contact sampling
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
        mine = strike_batch(rd_a1, np.array([mh]), np.array([2.0]), np.array([F_REF / K]), np.array([1.0]),
                            np.array([0.5]))
        print(f"  p=1 tau {tau_ratio} T1: contacts {len(c7)} vs {mine['n_contacts'][0]}; tip mode1 "
              f"{d20[0]*1e3:.3f} vs {mine['tip_mm'][0,0]:.3f} mm; mode2 {d20[1]*1e3:.4f} vs "
              f"{mine['tip_mm'][0,1]:.4f} mm; rebound {rb:+.3f} vs {mine['rebound'][0]:+.3f}")


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
            m2_pick = 20 * np.log10(np.maximum(np.abs(res["pick_mm"][:, 1]), 1e-30) / np.abs(res["pick_mm"][:, 0]))
            for i in range(len(swing)):
                rows.append(dict(variant=name, m_h=pr["m_h"][i], v=pr["v_ff"][i], z_star=pr["z_star"][i],
                                 p=pr["p"][i], x_s=pr["x_s"][i], contacts=int(res["n_contacts"][i]),
                                 t_contact_T1=float(res["t_contact_T1"][i]), swing_mm=float(swing[i]),
                                 m2_tip_db=float(m2_tip[i]), m2_pick_db=float(m2_pick[i]),
                                 pick_mm=[float(x) for x in res["pick_mm"][i, :7]],
                                 finished=bool(np.isfinite(res["t_end_T1"][i])),
                                 energy_err=float(res["energy_err"][i]),
                                 mode_ratios=list((rd["om"][:7] / rd["om"][0]).round(4)),
                                 coupling=list((rd["ph_pick"][:7] / rd["ph_pick"][0]).round(4))))
            print(f"MIDI {midi} {name}: {len(swing)} strikes, dt {res['dt']*1e6:.2f} us, unfinished "
                  f"{res['unfinished']}, max |energy err| {np.max(np.abs(res['energy_err'])):.1e}", flush=True)
        results[midi] = rows
        summarise(midi, rows)
    if out:
        with open(out, "w") as fh:
            json.dump(dict(priors=PRIORS, damping=damping, seed=seed, edges=EDGE_MM, results=results), fh)


# Keyboard pass: one prior sample = one instrument. Felt is graded (z*_bass >= z*_mid >= z*_treble,
# drawn as three log-uniform values sorted), p per grade, m_h and v_ff shared, x_s and geometry per
# reed. Grade boundaries are a nuisance carried over two candidate splits.
SPLITS = {"damper-felt": (46, 72), "code": (52, 72), "blank-pickup": (52, 74)}


def grade_of(midi, split):
    b, m = SPLITS[split]
    return 0 if midi <= b else (1 if midi <= m else 2)


def draw_instruments(rng, n):
    lo, hi = PRIORS["z_star"][1], PRIORS["z_star"][2]
    z = np.sort(np.exp(rng.uniform(np.log(lo), np.log(hi), (n, 3))), axis=1)[:, ::-1]  # bass >= mid >= treble
    p = rng.uniform(PRIORS["p"][1], PRIORS["p"][2], (n, 3))
    m = np.exp(rng.uniform(np.log(PRIORS["m_h"][1]), np.log(PRIORS["m_h"][2]), n))
    v = np.exp(rng.uniform(np.log(PRIORS["v_ff"][1]), np.log(PRIORS["v_ff"][2]), n))
    return dict(z_star=z, p=p, m_h=m, v=v)


def preamp_tilt_db(f):
    """Model chain tilt: the pickup's 880 Hz one-pole high-pass plus the bench-measured preamp response
    (flat within 1 dB to ~7.6 kHz, -3 dB at 16 kHz; `preamp-bench sweep`)."""
    fc = 880.0
    hp = 20 * np.log10(f / np.sqrt(f ** 2 + fc ** 2))
    fs = np.array([50, 1500.9, 3144.4, 4900.5, 6587.4, 8854.9, 10266.4, 11902.8, 13800.2, 16000.0])
    gs = np.array([7.00, 7.02, 6.93, 6.77, 6.53, 6.12, 5.79, 5.36, 4.76, 3.96])
    return hp + np.interp(f, fs, gs)


def keyboard(notes, n, seed, out, split, survey=""):
    """One run of the keyboard pass. `survey` (mode2_survey.py JSON) conditions the geometry at each
    note with an own mode-2 line: the FE variant nearest the measured ratio is used there."""
    rng = np.random.default_rng(seed)
    inst = draw_instruments(rng, n)
    measured = {}
    if survey:
        for row in json.load(open(survey)):
            o = row.get("onset", {})
            if o.get("status") == "line":
                measured[row["midi"]] = o["ratio"]
    res = {}
    for midi in notes:
        g = grade_of(midi, split)
        vs = variants(midi)
        pick = rng.integers(0, len(vs), n)
        if midi in measured:
            fe = [reed_data(midi, t_tip, t_root) for _, t_tip, t_root in vs]
            ratios = [r["om"][1] / r["om"][0] for r in fe]
            pick[:] = int(np.argmin([abs(r - measured[midi]) for r in ratios]))
        x_s = rng.uniform(PRIORS["x_s"][1], PRIORS["x_s"][2], n)
        swing = np.zeros(n)
        m2_out = np.zeros(n)
        contacts = np.zeros(n, int)
        t_c = np.zeros(n)
        for k, (name, t_tip, t_root) in enumerate(vs):
            sel = np.where(pick == k)[0]
            if sel.size == 0:
                continue
            rd = reed_data(midi, t_tip, t_root)
            r = strike_batch(rd, inst["m_h"][sel], inst["v"][sel], inst["z_star"][sel, g], inst["p"][sel, g], x_s[sel])
            f1 = rd["f"]
            f2 = rd["om"][1] / rd["om"][0] * f1
            swing[sel] = np.abs(r["pick_mm"][:, 0])
            ratio = np.maximum(np.abs(r["pick_mm"][:, 1]), 1e-30) / np.maximum(np.abs(r["pick_mm"][:, 0]), 1e-30)
            m2_out[sel] = 20 * np.log10(ratio) + preamp_tilt_db(f2) - preamp_tilt_db(f1)
            contacts[sel] = r["n_contacts"]
            t_c[sel] = r["t_contact_T1"]
        res[midi] = dict(swing_mm=swing.tolist(), m2_out_db=m2_out.tolist(), contacts=contacts.tolist(),
                         t_contact_T1=t_c.tolist(), grade=g, variant=[vs[k][0] for k in pick])
        print(f"MIDI {midi} (grade {g}) done", flush=True)
    with open(out, "w") as fh:
        json.dump(dict(split=split, seed=seed, n=n, priors=PRIORS, instruments={k: np.asarray(v).tolist() for k, v in inst.items()},
                       notes=res), fh)



def fundamental_decay_db(midi):
    """The shipping fundamental decay law (tables::fundamental_decay_rate), dB/s."""
    f = 440.0 * 2 ** ((midi - 69) / 12)
    return max(0.005 * f ** 1.22, 3.0)


def modal_spec_text(rd, res, i, midi):
    """The reed's modal state at separation as a `preamp-bench render --modal-spec` file, at the
    LOW-damping corner: every mode decays at the fundamental's rate (no extra loss on modes 2-7)."""
    om = rd["om"][:7]
    q, qd = res["q"][i, :7], res["qd"][i, :7]
    R = np.sqrt(q ** 2 + (qd / om) ** 2)
    theta = np.arctan2(q, qd / om)
    disp = rd["ph_pick"][:7] * R  # signed amplitude at the finger region, m
    swing_mm = abs(disp[0]) * 1e3
    amps = disp / abs(disp[0])
    lines = [("ratios", om / om[0]), ("amplitudes", amps), ("phases", theta),
             ("decay_db_s", np.full(7, fundamental_decay_db(midi)))]
    txt = "".join(f"{k} " + " ".join(f"{x:.9g}" for x in v) + "\n" for k, v in lines)
    return txt + f"swing_mm {swing_mm:.9g}\n", swing_mm, 20 * np.log10(max(abs(amps[1]), 1e-30))


def calibrate(notes, n, seed, bench, sensor, workdir, out):
    """Chain-identical calibration of the mode-2 proxy, per note: render prototype strikes through the
    bench to the preamp tap and measure them with mode2_survey's Sensor procedure."""
    from mode2_survey import h1_of, sensor_bands

    rng = np.random.default_rng(seed)
    os.makedirs(workdir, exist_ok=True)
    rows = []
    for midi in notes:
        vs = variants(midi)
        for k, (name, t_tip, t_root) in enumerate(vs):
            rd = reed_data(midi, t_tip, t_root)
            pr = draw(rng, max(1, n // len(vs)))
            res = strike_batch(rd, pr["m_h"], pr["v_ff"], pr["z_star"], pr["p"], pr["x_s"])
            f1 = rd["f"]
            f2 = rd["om"][1] / rd["om"][0] * f1
            tilt = preamp_tilt_db(f2) - preamp_tilt_db(f1)
            for i in range(len(pr["m_h"])):
                txt, swing, m2_pick = modal_spec_text(rd, res, i, midi)
                spec = os.path.join(workdir, f"m{midi}_{k}_{i}.spec")
                wav = os.path.join(workdir, f"m{midi}_{k}_{i}.wav")
                with open(spec, "w") as fh:
                    fh.write(txt)
                subprocess.run([bench, "render", "--note", str(midi), "--velocity", "127", "--duration", "1.0",
                                "--no-poweramp", "--speaker", "0", "--no-mlp", "--modal-spec", spec, "--output", wav],
                               check=True, capture_output=True)
                # Same Sensor procedure as the survey's onset window; the model's line is taken at its
                # KNOWN frequency, within +-0.4 % (the voice detunes each note by up to +-3 cents). A
                # mode 2 within 25 Hz of a harmonic is unresolvable here, as it would be in a recording,
                # and is not scored.
                f1n = 440.0 * 2 ** ((midi - 69) / 12)
                f2n = f1n * rd["om"][1] / rd["om"][0]
                h = np.arange(1, 40) * f1n
                if np.min(np.abs(h - f2n)) < 25.0:
                    o = dict(status="unresolvable")
                else:
                    bands = sensor_bands(sensor, wav, 5, 900, 40, max(20.0, 0.8 * f1n), f2n * 1.1)
                    h1 = h1_of(bands, f1n) if bands else None
                    near = [b for b in (bands or []) if abs(b["centre_hz"] - f2n) < max(4.0, 0.004 * f2n)]
                    if h1 is None or not near:
                        o = dict(status="no line")
                    else:
                        b2 = max(near, key=lambda b: b["candidate_dbfs"])
                        o = dict(status="line", re_h1_db=round(b2["candidate_dbfs"] - h1["candidate_dbfs"], 1))
                rows.append(dict(midi=midi, variant=name, swing_mm=swing, proxy_db=m2_pick + tilt,
                                 chain_db=o.get("re_h1_db"), status=o.get("status"), contacts=int(res["n_contacts"][i])))
        sub = [r for r in rows if r["midi"] == midi and r["chain_db"] is not None]
        if sub:
            d = np.array([r["chain_db"] - r["proxy_db"] for r in sub])
            print(f"MIDI {midi}: {len(sub)} measured; chain - proxy p10/50/90 {np.percentile(d,10):.1f}/"
                  f"{np.median(d):.1f}/{np.percentile(d,90):.1f} dB", flush=True)
        else:
            print(f"MIDI {midi}: no chain measurement resolved a line", flush=True)
    with open(out, "w") as fh:
        json.dump(rows, fh, indent=1)



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
    ap.add_argument("cmd", choices=("selftest", "run", "keyboard", "calibrate"))
    ap.add_argument("--bench", default="target/release/preamp-bench")
    ap.add_argument("--sensor", default="")
    ap.add_argument("--workdir", default="")
    ap.add_argument("--survey", default="")
    ap.add_argument("--split", choices=tuple(SPLITS), default="code")
    ap.add_argument("--n", type=int, default=1000)
    ap.add_argument("--notes", default="33,42,48,52,55,60,66,72,78,84,90,96")
    ap.add_argument("--damping", choices=("floor", "upper"), default="floor")
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--out", default="")
    a = ap.parse_args()
    if a.cmd == "selftest":
        selftest()
    elif a.cmd == "calibrate":
        calibrate([int(x) for x in a.notes.split(",")], a.n, a.seed, a.bench, a.sensor, a.workdir, a.out)
    elif a.cmd == "keyboard":
        keyboard([int(x) for x in a.notes.split(",")], a.n, a.seed, a.out, a.split, a.survey)
    else:
        run([int(x) for x in a.notes.split(",")], a.n, a.damping, a.seed, a.out)
