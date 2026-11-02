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
# --- Units -----------------------------------------------------------------------------------
MM = 1e-3          # m per mm
INCH = 25.4e-3     # m per inch
A4_HZ = 440.0      # equal-temperament reference: f = A4_HZ * 2^((midi - A4_MIDI) / 12)
A4_MIDI = 69

# --- Felt law ----------------------------------------------------------------------------------
F_REF = 10.0       # N: z* is the felt compression at this force (Giordano & Winans 2000 measure z* at 10 N)

# --- Reed model --------------------------------------------------------------------------------
N_MODES = 20       # modes integrated during contact (model evaluation: >= 14 needed for stiff chatter)
N_RENDER_MODES = 7  # modes the engine renders (tables::NUM_MODES)
BASS_REEDS = 20    # reeds 1-20 are the unground bass blanks (MIDI 33-52)
MIDI_LOWEST = 33   # reed 1 = A1
BASS_BLANKS_IN = (0.020, 0.026)  # disputed bass blank thickness, in: .020" (Vintage Vibe) / .026" (earlier code)
TIP_THICKNESS_MM = (0.51, 0.585, 0.66)  # reeds 21-64 tip thickness band (tables / pickup spec), mm
ROOT_THICKNESS_IN = 0.034  # reeds 21-64 blank root thickness, in (FE reed model)
FINGER_REGION_MM = (8.0, 2.0)  # pickup finger region: from L-8 mm to L-2 mm (reed_fe_inputs.py)
FINGER_REGION_PTS = 61
XS_GRID_FRAC = (0.25, 0.60, 0.01)  # strike-point grid for the pass-1/2 patch tables, fraction of L

# --- Integrator controls (numerics; the self-test checks convergence at a quarter of the step) ---
STEPS_PER_CONTACT = 200.0  # time steps per rigid-target compression time d_max / v
STEPS_PER_T1_MIN = 2000.0  # step cap T1 / 2000 (only bounds contact sampling; the modal flow is exact)
T_MAX_PERIODS = 60.0       # window cap: a heavy hammer can ride a light treble reed for several periods
OVERDAMP_CAP = 0.9         # modal damping capped at 0.9 of critical so the damped update stays defined
STIFFNESS_CHUNK = 64       # samples per chunk of similar contact stiffness (each chunk takes its own step)
MM_PER_M = 1e3             # output conversion, m -> mm
LOG_FLOOR = 1e-30          # guards log10 of a zero amplitude (reads as -600 dB)

# --- Chain model for the mode-2 proxy (the model's own chain, not a recording's) ---------------
PICKUP_HP_HZ = 880.0       # pickup one-pole high-pass corner (crate::pickup::PICKUP_FC; pickup spec)
# Preamp gain vs frequency, `preamp-bench sweep --start 50 --end 16000 --points 40` (LDR 1 MΩ),
# 2026-10-29: flat within 1 dB to ~7.6 kHz, -3 dB at 16 kHz.
PREAMP_SWEEP_HZ = (50.0, 1500.9, 3144.4, 4900.5, 6587.4, 8854.9, 10266.4, 11902.8, 13800.2, 16000.0)
PREAMP_SWEEP_DB = (7.00, 7.02, 6.93, 6.77, 6.53, 6.12, 5.79, 5.36, 4.76, 3.96)

# --- The shipping fundamental decay law (tables::fundamental_decay_rate; UNVERIFIED there) --------
DECAY_COEF_DB_S = 0.005
DECAY_EXPONENT = 1.22
DECAY_FLOOR_DB_S = 3.0     # tables::MIN_DECAY_RATE

# --- Chain-identical calibration (plan §5d amendments) ------------------------------------------
CAL_VELOCITY = 127         # bench render velocity (output level only; the modal state is prescribed)
CAL_DURATION_S = 1.0
CAL_SENSOR_ONSET = (5, 900, 40)  # skip ms, window ms, band +-Hz: the survey's onset window (fit from ~85 ms)
CAL_SEARCH_LO = (20.0, 0.8)      # decay-range low edge: max(20 Hz, 0.8 f1)
CAL_SEARCH_HI = 1.1              # decay-range high edge: 1.1 f2
CAL_UNRESOLVABLE_HZ = 25.0       # a mode 2 this close to a harmonic is unresolvable, as in a recording
CAL_LINE_TOL = (4.0, 0.004)      # model line search: max(4 Hz, 0.4 % of f2) (voice detune is +-3 cents)
CAL_MAX_HARMONIC = 40            # harmonics checked for the resolvability test
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
    if midi - (MIDI_LOWEST - 1) <= BASS_REEDS:
        return [(f"bass {t:.3f}", t * INCH, t * INCH) for t in BASS_BLANKS_IN]
    return [(f"tip {t:.3f}", t * MM, ROOT_THICKNESS_IN * INCH) for t in TIP_THICKNESS_MM]


def draw(rng, n):
    out = {}
    for k, (kind, lo, hi) in PRIORS.items():
        out[k] = np.exp(rng.uniform(np.log(lo), np.log(hi), n)) if kind == "log-uniform" else rng.uniform(lo, hi, n)
    return out


def reed_data(midi, t_tip, t_root):
    r = reed(midi, t_tip, t_root, nm=N_MODES)
    om = r["om"][:N_MODES] * (r["f"] / r["fe1"])  # scaled to pitch, as in the model evaluation
    L = r["L"]
    lo, hi, step = XS_GRID_FRAC
    xs_grid = np.round(np.arange(lo, hi + step / 100, step), 2)
    ph_grid = np.array([patch(r, xs)[:N_MODES] for xs in xs_grid])
    xx = np.linspace(L - FINGER_REGION_MM[0] * MM, L - FINGER_REGION_MM[1] * MM, FINGER_REGION_PTS)
    ph_pick = np.array([np.mean(np.interp(xx, r["xe"], r["phi"][:, n])) for n in range(N_MODES)])
    ph_tip = r["phi"][-1, :N_MODES]
    return dict(f=r["f"], om=om, mu=r["mu"], fe1=r["fe1"], xs_grid=xs_grid, ph_grid=ph_grid,
                ph_pick=ph_pick, ph_tip=ph_tip, L=L, xe=r["xe"], phi=r["phi"][:, :N_MODES])


def strike_batch(rd, m_h, v, z_star, p, x_s, alpha=None, dt=None, t_max_periods=T_MAX_PERIODS):
    """Integrate S strikes in lockstep. Returns per-sample results (arrays of length S)."""
    S = len(m_h)
    om = rd["om"]
    T1 = 1.0 / rd["f"]
    ph = np.stack([np.interp(x_s, rd["xs_grid"], rd["ph_grid"][:, n]) for n in range(N_MODES)], axis=1)
    K = F_REF / z_star ** p
    if dt is None:
        # Resolve the stiffest contact in the batch: rigid-target compression time ~ delta_max / v.
        dmax = ((p + 1) * 0.5 * m_h * v ** 2 / K) ** (1 / (p + 1))
        dt = float(np.min(dmax / v)) / STEPS_PER_CONTACT
        dt = min(dt, T1 / STEPS_PER_T1_MIN)
    # Modes damped past critical don't ring; cap them just under it so the update stays defined.
    alpha = np.zeros(N_MODES) if alpha is None else np.minimum(alpha, OVERDAMP_CAP * om)
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
        tip_mm=amp * np.abs(rd["ph_tip"])[None, :] * MM_PER_M,
        pick_mm=amp * rd["ph_pick"][None, :] * MM_PER_M,  # signed coupling at the finger region
    )


CROWN_MM = 9.2  # measured felt-crown length along the reed (8.9-9.4 mm at the crown)
CROWN_PTS = 41  # crown sample points along the reed (crown-length changes resolve to <= 0.4 dB)


def strike_batch_w(rd, m_h, v, z_star, p, d_s_mm, crown_mm=CROWN_MM, alpha=None, dt=None,
                   t_max_periods=T_MAX_PERIODS, tau_d=None):
    """Pass-3 contact: a Winkler layer of felt on the hammer's blade. The crown spans crown_mm along
    the reed, centred d_s_mm from the clamp, sampled at CROWN_PTS points. Each point compresses
    locally, f_i = (F_REF / N) (delta_i / z*)^p while delta_i > 0, so a uniform compression gives the
    same total as the rigid form. Same kick-drift-kick integrator as strike_batch.

    tau_d (s, per sample): felt loss in Stulov's hereditary form (plan §5m), one memory state per crown point,
    carried through separations, force clamped at >= 0. None is the lossless law, unchanged."""
    S = len(m_h)
    om = rd["om"]
    T1 = 1.0 / rd["f"]
    L = rd["L"]
    off = (np.arange(CROWN_PTS) / (CROWN_PTS - 1) - 0.5) * crown_mm * MM  # crown points about its centre
    x = np.clip(d_s_mm[:, None] * MM + off[None, :], 0.0, L)  # (S, P) positions along the reed
    Phi = np.stack([np.interp(x.ravel(), rd["xe"], rd["phi"][:, n]).reshape(S, CROWN_PTS)
                    for n in range(N_MODES)], axis=2)  # (S, P, M)
    Phi_abs_max = np.abs(Phi).max(axis=1)  # (S, M): largest reach of the crown per mode
    x_end = x[:, -1]  # the crown's tip-ward end
    phi_end = np.stack([np.interp(x_end, rd["xe"], rd["phi"][:, n]) for n in range(N_MODES)], axis=1)
    kp = F_REF / CROWN_PTS / z_star ** p  # per-point stiffness
    if dt is None:
        dmax = ((p + 1) * 0.5 * m_h * v ** 2 / (F_REF / z_star ** p)) ** (1 / (p + 1))
        dt = min(float(np.min(dmax / v)) / STEPS_PER_CONTACT, T1 / STEPS_PER_T1_MIN)
    alpha = np.zeros(N_MODES) if alpha is None else np.minimum(alpha, OVERDAMP_CAP * om)
    wd = om * np.sqrt(1 - (alpha / om) ** 2)
    e = np.exp(-alpha * dt)
    c, s_ = np.cos(wd * dt), np.sin(wd * dt)

    def forces(yy, qq, phi, kk, pp):
        d = yy[:, None] - np.einsum("spm,sm->sp", phi, qq)
        f = np.where(d > 0, kk[:, None] * np.power(np.maximum(d, 0.0), pp[:, None]), 0.0)
        return f.sum(axis=1), np.einsum("spm,sp->sm", phi, f), d.max(axis=1)

    if tau_d is not None:
        # Stulov memory per crown point, advanced exactly for g linear over a step (first-order hold): the trapezoid
        # rule when dt << tau0, and g - tau0 dg/dt when dt >> tau0, so the loss survives steps longer than tau0.
        tau0 = np.asarray(tau_d, float) * (1 - STULOV_EPS) / STULOV_EPS
        E = np.exp(-dt / tau0)
        w_new = 1 - (1 - E) * tau0 / dt
        w_old = (1 - E) * tau0 / dt - E
        h_mem = np.zeros((S, CROWN_PTS))
        g_prev = np.zeros((S, CROWN_PTS))

        def forces(yy, qq, phi, kk, pp, a):  # noqa: F811 - the lossy law replaces the lossless one
            d = yy[:, None] - np.einsum("spm,sm->sp", phi, qq)
            g = np.where(d > 0, np.power(np.maximum(d, 0.0), pp[:, None]), 0.0)
            h = h_mem[a] * E[a, None] + g * w_new[a, None] + g_prev[a] * w_old[a, None]
            h_mem[a], g_prev[a] = h, g
            f = np.maximum(kk[:, None] / (1 - STULOV_EPS) * (g - STULOV_EPS * h), 0.0)
            return f.sum(axis=1), np.einsum("spm,sp->sm", phi, f), d.max(axis=1)

    q = np.zeros((S, N_MODES))
    qd = np.zeros((S, N_MODES))
    y = np.zeros(S)
    vy = v.copy()
    active = np.ones(S, bool)
    in_contact = np.zeros(S, bool)
    n_contacts = np.zeros(S, int)
    t_contact = np.zeros(S)
    t_first = np.zeros(S)  # the first contact's own width (plan §5n: reported beside the total, unscored)
    t_end = np.full(S, np.nan)
    d_peak = np.zeros(S)
    F_now = np.zeros(S)
    G_now = np.zeros((S, N_MODES))
    e0 = 0.5 * m_h * v ** 2
    idx = np.arange(S)
    t = 0.0
    for _ in range(int(t_max_periods * T1 / dt) + 1):
        a = idx[active]
        if a.size == 0:
            break
        phi_a, ma = Phi[a], m_h[a]
        vya = vy[a] - F_now[a] / ma * (dt / 2)
        qda = qd[a] + G_now[a] * (dt / 2)
        qa = q[a]
        q_new = e * (qa * c + (qda + alpha * qa) / wd * s_)
        qd_new = e * (qda * c - (wd * qa + alpha * (qda + alpha * qa) / wd) * s_)
        ya = y[a] + vya * dt
        F1, G1, dm = forces(ya, q_new, phi_a, kp[a], p[a]) if tau_d is None else \
            forces(ya, q_new, phi_a, kp[a], p[a], a)
        vya = vya - F1 / ma * (dt / 2)
        qd_new = qd_new + G1 * (dt / 2)
        q[a], qd[a], y[a], vy[a], F_now[a], G_now[a] = q_new, qd_new, ya, vya, F1, G1
        d_peak[a] = np.maximum(d_peak[a], dm)
        now = F1 > 0
        n_contacts[a[now & ~in_contact[a]]] += 1
        t += dt
        t_contact[a[now]] += dt
        t_first[a[now & (n_contacts[a] == 1)]] += dt
        in_contact[a] = now
        reach = np.einsum("sm,sm->s", Phi_abs_max[a], np.sqrt(q_new ** 2 + (qd_new / om) ** 2))
        done = (~now) & (vya < 0) & (n_contacts[a] > 0) & (ya < -reach)
        t_end[a[done]] = t
        active[a[done]] = False
    amp = np.sqrt(q ** 2 + (qd / om) ** 2)
    e_reed = 0.5 * np.sum(qd ** 2 + (om * q) ** 2, axis=1)
    return dict(
        dt=dt, unfinished=int(active.sum()), n_contacts=n_contacts, t_contact_T1=t_contact / T1,
        t_first_T1=t_first / T1, t_end_T1=t_end / T1, amp=amp, q=q, qd=qd, rebound=vy / v,
        d_peak_mm=d_peak * MM_PER_M,
        energy_err=(e_reed + 0.5 * m_h * vy ** 2 - e0) / e0,
        tip_mm=amp * np.abs(rd["ph_tip"])[None, :] * MM_PER_M,
        pick_mm=amp * rd["ph_pick"][None, :] * MM_PER_M,
        crown_end_swing_mm=np.abs(amp[:, 0] * phi_end[:, 0]) * MM_PER_M,
    )



# Felt loss (plan §5m): Stulov's hereditary law per crown point, F = F0 [g - eps h], h the felt's memory of g
# (relaxation time tau0). For contacts much longer than tau0 it is K [g + tau_d dg/dt] with
# tau_d = eps tau0 / (1 - eps); at fixed tau_d, eps moves the loop by < 0.2 % (physics review), so eps is held at the
# middle of Stulov 1995 Table II's fits (0.936-0.985) and tau_d carries the loss. z* stays on the relaxed (slow) law:
# F0 = K / (1 - eps).
STULOV_EPS = 0.95


# Self-test fixtures (test cases, not physics inputs): A1 on the .020" blank, a 3 g hammer at 2 m/s,
# a stiff and a soft felt, and the model evaluation's linear-spring cases.
SELFTEST = dict(midi=33, blank_in=0.020, batch=200, seed=1, m_h=3e-3, v=2.0, p=3.0, x_s=0.5,
                z_stiff=0.2e-3, z_soft=3e-3, dt_refine=4, tau_ratios=(0.3, 0.75),
                stulov_d_s_mm=42.0, stulov_tau_d_tiny=1e-9, stulov_tau_d=0.1e-3)


def selftest():
    """Energy balance (undamped), dt convergence, and agreement with the model evaluation's strike()."""
    from hammer_contact_eval import strike as eval_strike
    T = SELFTEST
    rd_a1 = reed_data(T["midi"], T["blank_in"] * INCH, T["blank_in"] * INCH)
    rng = np.random.default_rng(T["seed"])
    pr = draw(rng, T["batch"])
    res = strike_batch(rd_a1, pr["m_h"], pr["v_ff"], pr["z_star"], pr["p"], pr["x_s"])
    print(f"A1 batch of {T['batch']}: dt {res['dt']*1e6:.3f} us, unfinished {res['unfinished']}, "
          f"max |energy error| {np.max(np.abs(res['energy_err'])):.2e}")
    # dt convergence on a stiff and a soft case
    for zs, label in ((T["z_stiff"], "stiff"), (T["z_soft"], "soft")):
        one = dict(m_h=np.array([T["m_h"]]), v=np.array([T["v"]]), z_star=np.array([zs]), p=np.array([T["p"]]),
                   x_s=np.array([T["x_s"]]))
        r1 = strike_batch(rd_a1, one["m_h"], one["v"], one["z_star"], one["p"], one["x_s"])
        r2 = strike_batch(rd_a1, one["m_h"], one["v"], one["z_star"], one["p"], one["x_s"], dt=r1["dt"] / T["dt_refine"])
        print(f"  {label}: contacts {r1['n_contacts'][0]} vs {r2['n_contacts'][0]} (dt/4); tip mode1 "
              f"{r1['tip_mm'][0,0]:.4f} vs {r2['tip_mm'][0,0]:.4f} mm; mode2 re mode1 "
              f"{20*np.log10(r1['tip_mm'][0,1]/r1['tip_mm'][0,0]):.2f} vs "
              f"{20*np.log10(r2['tip_mm'][0,1]/r2['tip_mm'][0,0]):.2f} dB")
    # Linear spring (p = 1) against the model evaluation (its p = 1 path is closed form), 20 modes.
    r = reed(T["midi"], T["blank_in"] * INCH, T["blank_in"] * INCH, nm=N_MODES)
    for tau_ratio in T["tau_ratios"]:
        mh = T["m_h"]
        T1 = 1 / r["f"]
        K = mh * (np.pi / (tau_ratio * T1)) ** 2  # linear spring with rigid-target contact time tau_ratio * T1
        c7, d20, rb = eval_strike(r, T["x_s"], mh, T["v"], tau_ratio, nm=N_MODES)
        mine = strike_batch(rd_a1, np.array([mh]), np.array([T["v"]]), np.array([F_REF / K]), np.array([1.0]),
                            np.array([T["x_s"]]))
        print(f"  p=1 tau {tau_ratio} T1: contacts {len(c7)} vs {mine['n_contacts'][0]}; tip mode1 "
              f"{d20[0]*MM_PER_M:.3f} vs {mine['tip_mm'][0,0]:.3f} mm; mode2 {d20[1]*MM_PER_M:.4f} vs "
              f"{mine['tip_mm'][0,1]:.4f} mm; rebound {rb:+.3f} vs {mine['rebound'][0]:+.3f}")
    # Felt loss (plan §5m): tau_d -> 0 reproduces the lossless crown contact, and the lossy result converges in dt.
    one = lambda a: np.array([a])  # noqa: E731
    args = (rd_a1, one(T["m_h"]), one(T["v"]), one(T["z_soft"]), one(T["p"]), one(T["stulov_d_s_mm"]))
    lossless = strike_batch_w(*args)
    tiny = strike_batch_w(*args, tau_d=one(T["stulov_tau_d_tiny"]))
    lossy = strike_batch_w(*args, tau_d=one(T["stulov_tau_d"]))
    fine = strike_batch_w(*args, tau_d=one(T["stulov_tau_d"]), dt=lossy["dt"] / T["dt_refine"])
    db = lambda x, y: 20 * np.log10(abs(x) / abs(y))  # noqa: E731
    print(f"  Stulov tau_d {T['stulov_tau_d_tiny']:.0e} s vs lossless: pickup mode1 "
          f"{db(tiny['pick_mm'][0, 0], lossless['pick_mm'][0, 0]):+.5f} dB, mode2 "
          f"{db(tiny['pick_mm'][0, 1], lossless['pick_mm'][0, 1]):+.5f} dB")
    print(f"  Stulov tau_d {T['stulov_tau_d'] * 1e3:.2f} ms, dt/{T['dt_refine']}: pickup mode1 "
          f"{db(fine['pick_mm'][0, 0], lossy['pick_mm'][0, 0]):+.4f} dB, mode2 "
          f"{db(fine['pick_mm'][0, 1], lossy['pick_mm'][0, 1]):+.4f} dB; energy out/in {1 + lossy['energy_err'][0]:.3f}")


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
            m2_tip = 20 * np.log10(np.maximum(res["tip_mm"][:, 1], LOG_FLOOR) / res["tip_mm"][:, 0])
            m2_pick = 20 * np.log10(np.maximum(np.abs(res["pick_mm"][:, 1]), LOG_FLOOR) / np.abs(res["pick_mm"][:, 0]))
            for i in range(len(swing)):
                rows.append(dict(variant=name, m_h=pr["m_h"][i], v=pr["v_ff"][i], z_star=pr["z_star"][i],
                                 p=pr["p"][i], x_s=pr["x_s"][i], contacts=int(res["n_contacts"][i]),
                                 t_contact_T1=float(res["t_contact_T1"][i]), swing_mm=float(swing[i]),
                                 m2_tip_db=float(m2_tip[i]), m2_pick_db=float(m2_pick[i]),
                                 pick_mm=[float(x) for x in res["pick_mm"][i, :N_RENDER_MODES]],
                                 finished=bool(np.isfinite(res["t_end_T1"][i])),
                                 energy_err=float(res["energy_err"][i]),
                                 mode_ratios=list((rd["om"][:N_RENDER_MODES] / rd["om"][0]).round(4)),
                                 coupling=list((rd["ph_pick"][:N_RENDER_MODES] / rd["ph_pick"][0]).round(4))))
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
    hp = 20 * np.log10(f / np.sqrt(f ** 2 + PICKUP_HP_HZ ** 2))
    return hp + np.interp(f, np.array(PREAMP_SWEEP_HZ), np.array(PREAMP_SWEEP_DB))


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
            ratio = np.maximum(np.abs(r["pick_mm"][:, 1]), LOG_FLOOR) / np.maximum(np.abs(r["pick_mm"][:, 0]), LOG_FLOOR)
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
    f = A4_HZ * 2 ** ((midi - A4_MIDI) / 12)
    return max(DECAY_COEF_DB_S * f ** DECAY_EXPONENT, DECAY_FLOOR_DB_S)


def modal_spec_text(rd, res, i, midi):
    """The reed's modal state at separation as a `preamp-bench render --modal-spec` file, at the
    LOW-damping corner: every mode decays at the fundamental's rate (no extra loss on modes 2-7)."""
    om = rd["om"][:N_RENDER_MODES]
    q, qd = res["q"][i, :N_RENDER_MODES], res["qd"][i, :N_RENDER_MODES]
    R = np.sqrt(q ** 2 + (qd / om) ** 2)
    theta = np.arctan2(q, qd / om)
    disp = rd["ph_pick"][:N_RENDER_MODES] * R  # signed amplitude at the finger region, m
    swing_mm = abs(disp[0]) * MM_PER_M
    amps = disp / abs(disp[0])
    lines = [("ratios", om / om[0]), ("amplitudes", amps), ("phases", theta),
             ("decay_db_s", np.full(N_RENDER_MODES, fundamental_decay_db(midi)))]
    txt = "".join(f"{k} " + " ".join(f"{x:.9g}" for x in v) + "\n" for k, v in lines)
    return txt + f"swing_mm {swing_mm:.9g}\n", swing_mm, 20 * np.log10(max(abs(amps[1]), LOG_FLOOR))


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
                subprocess.run([bench, "render", "--note", str(midi), "--velocity", str(CAL_VELOCITY),
                                "--duration", str(CAL_DURATION_S),
                                "--no-poweramp", "--speaker", "0", "--no-mlp", "--modal-spec", spec, "--output", wav],
                               check=True, capture_output=True)
                # Same Sensor procedure as the survey's onset window; the model's line is taken at its
                # KNOWN frequency, within CAL_LINE_TOL. A mode 2 within CAL_UNRESOLVABLE_HZ of a harmonic is
                # unresolvable here, as it would be in a recording, and is not scored.
                f1n = A4_HZ * 2 ** ((midi - A4_MIDI) / 12)
                f2n = f1n * rd["om"][1] / rd["om"][0]
                h = np.arange(1, CAL_MAX_HARMONIC) * f1n
                if np.min(np.abs(h - f2n)) < CAL_UNRESOLVABLE_HZ:
                    o = dict(status="unresolvable")
                else:
                    lo = max(CAL_SEARCH_LO[0], CAL_SEARCH_LO[1] * f1n)
                    bands = sensor_bands(sensor, wav, *CAL_SENSOR_ONSET, lo, f2n * CAL_SEARCH_HI)
                    h1 = h1_of(bands, f1n) if bands else None
                    tol = max(CAL_LINE_TOL[0], CAL_LINE_TOL[1] * f2n)
                    near = [b for b in (bands or []) if abs(b["centre_hz"] - f2n) < tol]
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



def strike_w_chunked(rd, m_h, v, z_star, p, d_s_mm, chunk=STIFFNESS_CHUNK, tau_d=None):
    """strike_batch_w in chunks of similar contact stiffness, so the stiffest sample doesn't set every
    sample's step. Same physics and per-chunk step rule; results reassembled in the input order."""
    dmax = ((p + 1) * 0.5 * m_h * v ** 2 / (F_REF / z_star ** p)) ** (1 / (p + 1))
    order = np.argsort(dmax / v)
    keys = ("n_contacts", "t_contact_T1", "t_first_T1", "tip_mm", "pick_mm", "crown_end_swing_mm", "d_peak_mm",
            "energy_err")
    out = {}
    for i0 in range(0, len(order), chunk):
        idx = order[i0:i0 + chunk]
        r = strike_batch_w(rd, m_h[idx], v[idx], z_star[idx], p[idx], d_s_mm[idx],
                           tau_d=None if tau_d is None else np.asarray(tau_d)[idx])
        for k in keys:
            if k not in out:
                out[k] = np.zeros((len(m_h),) + np.shape(r[k])[1:], dtype=np.asarray(r[k]).dtype)
            out[k][idx] = r[k]
    return out


# Pass-3 strike-point law (plan §5f): d_s = d_ref (L / L_ref)^k mm from the clamp, k shared in [0, 2],
# per-hammer scatter +-1.5 mm, floor 5 mm. Anchor: one photographed hammer on a 206A, 7.1 mm from the
# bar edge; the L origin is a nuisance until that reed is identified.
ANCHORS = {"bar": (7.1, 31.5), "fillet": (13.2, 37.5), "fillet-reed38": (13.2, 40.1),
           "fillet-floor": (13.2, 40.1)}  # fillet-reed38: pass 3c (reed 38/39); fillet-floor: pass 3d
STRIKE_K_RANGE = (0.0, 2.0)   # strike-law exponent k, uniform (0: rail parallel to the bar; 1: constant fraction)
STRIKE_SCATTER_MM = 1.5       # per-hammer scatter, uniform +-, ASSUMED (physics review, G); also stands for crown tilt
STRIKE_FLOOR_MM = 5.0         # passes 3-3c: half the 9.2 mm crown plus clearance (bar-edge arithmetic; a form error
                              # under the fillet origin, see plan §5j)
# Pass 3d (plan §5j): the reed bar casting covers the first 5.9-6.3 mm from the fillet (photo read); the crown centre
# must sit at least half the widest felt (10.2-11.6 mm, photo read) past the bar edge.
STRIKE_FLOOR_BY_ANCHOR = {"fillet-floor": 11.5}


def keyboard3(notes, n, seed, out, split, survey, origin):
    rng = np.random.default_rng(seed)
    inst = draw_instruments(rng, n)
    inst["k"] = rng.uniform(*STRIKE_K_RANGE, n)
    d_ref, L_ref = ANCHORS[origin]
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
        rds = [reed_data(midi, t_tip, t_root) for _, t_tip, t_root in vs]
        if midi in measured:
            ratios = [r["om"][1] / r["om"][0] for r in rds]
            pick[:] = int(np.argmin([abs(r - measured[midi]) for r in ratios]))
        scatter = rng.uniform(-STRIKE_SCATTER_MM, STRIKE_SCATTER_MM, n)
        out_cols = {k: np.zeros(n) for k in ("swing_mm", "m2_out_db", "tip_mm", "crown_end_mm", "d_peak_mm",
                                              "d_s_mm", "t_contact_T1")}
        contacts = np.zeros(n, int)
        for kv, rd in enumerate(rds):
            sel = np.where(pick == kv)[0]
            if sel.size == 0:
                continue
            L_mm = rd["L"] * MM_PER_M
            d_s = d_ref * (L_mm / L_ref) ** inst["k"][sel] + scatter[sel]
            d_s = np.clip(d_s, STRIKE_FLOOR_MM, L_mm - CROWN_MM / 2)
            r = strike_w_chunked(rd, inst["m_h"][sel], inst["v"][sel], inst["z_star"][sel, g], inst["p"][sel, g], d_s)
            f1 = rd["f"]
            f2 = rd["om"][1] / rd["om"][0] * f1
            ratio = np.maximum(np.abs(r["pick_mm"][:, 1]), LOG_FLOOR) / np.maximum(np.abs(r["pick_mm"][:, 0]), LOG_FLOOR)
            out_cols["swing_mm"][sel] = np.abs(r["pick_mm"][:, 0])
            out_cols["m2_out_db"][sel] = 20 * np.log10(ratio) + preamp_tilt_db(f2) - preamp_tilt_db(f1)
            out_cols["tip_mm"][sel] = r["tip_mm"][:, 0]
            out_cols["crown_end_mm"][sel] = r["crown_end_swing_mm"]
            out_cols["d_peak_mm"][sel] = r["d_peak_mm"]
            out_cols["d_s_mm"][sel] = d_s
            out_cols["t_contact_T1"][sel] = r["t_contact_T1"]
            contacts[sel] = r["n_contacts"]
        res[midi] = {k: v.tolist() for k, v in out_cols.items()}
        res[midi].update(contacts=contacts.tolist(), grade=g, L_mm=float(rds[0]["L"] * MM_PER_M))
        print(f"MIDI {midi} (grade {g}) done", flush=True)
    with open(out, "w") as fh:
        json.dump(dict(split=split, origin=origin, seed=seed, n=n, priors=PRIORS,
                       instruments={k: np.asarray(v).tolist() for k, v in inst.items()}, notes=res), fh)



# Pass 3b (plan §5h): voiced offsets for F2/F3, one bass blank per instrument, F2/F3 at v_max.
BASS_BLANK_LEVELS_IN = (0.020, 0.022, 0.024, 0.026)
VOICED_OFFSETS_MM = np.arange(-1.5, 1.5001, 0.5)
VOICED_MAX_MIDI = 60
V_MAX_CEIL = 5.0


def keyboard3b(notes, n, seed, out, split, survey, origin, instruments=""):
    """`instruments`: a JSON of given instrument parameters (tools/hammer_contact_smc.py proposals) in place of prior
    draws; the per-note nuisances (FE variant, strike scatter) are still drawn from `seed`."""
    rng = np.random.default_rng(seed)
    if instruments:
        inst = {k: np.asarray(v, dtype=float) for k, v in json.load(open(instruments))["instruments"].items()}
        n = len(inst["m_h"])
    else:
        inst = draw_instruments(rng, n)
        inst["k"] = rng.uniform(*STRIKE_K_RANGE, n)
        inst["v_max"] = np.exp(rng.uniform(np.log(inst["v"]), np.log(V_MAX_CEIL)))
        inst["t_bass_in"] = rng.uniform(*BASS_BLANKS_IN, n)
    level = np.argmin(np.abs(inst["t_bass_in"][:, None] - np.array(BASS_BLANK_LEVELS_IN)[None, :]), axis=1)
    d_ref, L_ref = ANCHORS[origin]
    floor_mm = STRIKE_FLOOR_BY_ANCHOR.get(origin, STRIKE_FLOOR_MM)
    measured = {}
    if survey:
        for row in json.load(open(survey)):
            o = row.get("onset", {})
            if o.get("status") == "line":
                measured[row["midi"]] = o["ratio"]
    res = {}
    for midi in notes:
        g = grade_of(midi, split)
        if midi - (MIDI_LOWEST - 1) <= BASS_REEDS:  # bass blank: the instrument's own thickness
            vs = [(f"bass {t:.3f}", t * INCH, t * INCH) for t in BASS_BLANK_LEVELS_IN]
            pick = level.copy()
        else:
            vs = variants(midi)
            pick = rng.integers(0, len(vs), n)
        rds = [reed_data(midi, t_tip, t_root) for _, t_tip, t_root in vs]
        if midi in measured and midi - (MIDI_LOWEST - 1) > BASS_REEDS:
            ratios = [r["om"][1] / r["om"][0] for r in rds]
            pick[:] = int(np.argmin([abs(r - measured[midi]) for r in ratios]))
        scatter = rng.uniform(-STRIKE_SCATTER_MM, STRIKE_SCATTER_MM, n)
        nv = len(VOICED_OFFSETS_MM)
        cols = {k: np.zeros(n) for k in ("swing_mm", "m2_out_db", "tip_mm", "crown_end_mm", "d_peak_mm",
                                          "d_s_mm", "t_contact_T1", "t_first_T1", "tip_vmax_mm", "d_peak_vmax_mm")}
        voiced_tip = np.full((n, nv), np.nan)
        voiced_end = np.full((n, nv), np.nan)
        voiced_dpeak = np.full((n, nv), np.nan)
        contacts = np.zeros(n, int)
        for kv, rd in enumerate(rds):
            sel = np.where(pick == kv)[0]
            if sel.size == 0:
                continue
            L_mm = rd["L"] * MM_PER_M
            law = d_ref * (L_mm / L_ref) ** inst["k"][sel]
            zs, pp = inst["z_star"][sel, g], inst["p"][sel, g]
            # m_h shared across the keyboard (prior draws, stages up to 4) or graded per felt grade (stage 5, plan §5l)
            mh = inst["m_h"][sel, g] if inst["m_h"].ndim == 2 else inst["m_h"][sel]
            td = inst["tau_d"][sel] if "tau_d" in inst else None  # felt loss, s (plan §5m); absent = lossless

            def place(off):
                return np.clip(law + off, floor_mm, L_mm - CROWN_MM / 2)

            # (A) F1 and reports: random offset, v_ff.
            d_s = place(scatter[sel])
            r = strike_w_chunked(rd, mh, inst["v"][sel], zs, pp, d_s, tau_d=td)
            f1 = rd["f"]
            f2 = rd["om"][1] / rd["om"][0] * f1
            ratio = np.maximum(np.abs(r["pick_mm"][:, 1]), LOG_FLOOR) / np.maximum(np.abs(r["pick_mm"][:, 0]), LOG_FLOOR)
            cols["swing_mm"][sel] = np.abs(r["pick_mm"][:, 0])
            cols["m2_out_db"][sel] = 20 * np.log10(ratio) + preamp_tilt_db(f2) - preamp_tilt_db(f1)
            cols["tip_mm"][sel] = r["tip_mm"][:, 0]
            cols["crown_end_mm"][sel] = r["crown_end_swing_mm"]
            cols["d_peak_mm"][sel] = r["d_peak_mm"]
            cols["d_s_mm"][sel] = d_s
            cols["t_contact_T1"][sel] = r["t_contact_T1"]
            cols["t_first_T1"][sel] = r["t_first_T1"]
            contacts[sel] = r["n_contacts"]
            # (B) F2/F3 at v_max: voiced offsets at MIDI <= 60, the random offset above.
            if midi <= VOICED_MAX_MIDI:
                for j, off in enumerate(VOICED_OFFSETS_MM):
                    rv = strike_w_chunked(rd, mh, inst["v_max"][sel], zs, pp, place(off), tau_d=td)
                    voiced_tip[sel, j] = rv["tip_mm"][:, 0]
                    voiced_end[sel, j] = rv["crown_end_swing_mm"]
                    voiced_dpeak[sel, j] = rv["d_peak_mm"]
            else:
                rv = strike_w_chunked(rd, mh, inst["v_max"][sel], zs, pp, d_s, tau_d=td)
                cols["tip_vmax_mm"][sel] = rv["tip_mm"][:, 0]
                cols["d_peak_vmax_mm"][sel] = rv["d_peak_mm"]
        res[midi] = {k: v.tolist() for k, v in cols.items()}
        res[midi].update(contacts=contacts.tolist(), grade=g, L_mm=float(rds[0]["L"] * MM_PER_M),
                         voiced=bool(midi <= VOICED_MAX_MIDI))
        if midi <= VOICED_MAX_MIDI:
            res[midi].update(voiced_tip_mm=voiced_tip.tolist(), voiced_crown_end_mm=voiced_end.tolist(),
                             voiced_d_peak_mm=voiced_dpeak.tolist())
        print(f"MIDI {midi} (grade {g}) done", flush=True)
    with open(out, "w") as fh:
        json.dump(dict(split=split, origin=origin, seed=seed, n=n, priors=PRIORS, pass_="3b",
                       instruments={k: np.asarray(v).tolist() for k, v in inst.items()}, notes=res), fh)



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
    ap.add_argument("cmd", choices=("selftest", "run", "keyboard", "keyboard3", "keyboard3b", "calibrate"))
    ap.add_argument("--origin", choices=tuple(ANCHORS), default="fillet")
    ap.add_argument("--bench", default="target/release/preamp-bench")
    ap.add_argument("--sensor", default="")
    ap.add_argument("--workdir", default="")
    ap.add_argument("--survey", default="")
    ap.add_argument("--instruments", default="", help="keyboard3b: given instrument parameters (SMC proposals)")
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
    elif a.cmd == "keyboard3b":
        keyboard3b([int(x) for x in a.notes.split(",")], a.n, a.seed, a.out, a.split, a.survey, a.origin, a.instruments)
    elif a.cmd == "keyboard3":
        keyboard3([int(x) for x in a.notes.split(",")], a.n, a.seed, a.out, a.split, a.survey, a.origin)
    elif a.cmd == "keyboard":
        keyboard([int(x) for x in a.notes.split(",")], a.n, a.seed, a.out, a.split, a.survey)
    else:
        run([int(x) for x in a.notes.split(",")], a.n, a.damping, a.seed, a.out)
