"""Model evaluation for the hammer-contact revision (2026-10-29): the FE reed (reed_fe.py, extended to N modes)
+ point-mass hammer + felt F = K d^p, one-sided, from below. Regime map and mode-count convergence."""
import sys, numpy as np, scipy.linalg as sl
from scipy.optimize import brentq
from reed_fe import (reed_len, width, RHO, E, A4_HZ, A4_MIDI, N_ELEMENTS, N_ELEMENTS_PITCH, PITCH_TOL,
                     SOLDER_BRACKET, INCH, MM_PER_M)

N_MODES_EVAL = 20         # modes solved (the evaluation compares 7 against 20)
PATCH_HALF = 0.1          # hammer patch half-length, fraction of L
PATCH_PTS = 61            # points per patch average
CT_SEARCH_STEPS = 4000    # steps per rigid-target contact time when solving K for p != 1
K_BRACKET = (1e-3, 1e18)  # stiffness search bracket for p != 1 (N/m^p)
STEPS_PER_T1 = 20000      # base step, refined further when the top mode exceeds 50 x f1
TOP_MODE_RATIO = 50
T_END_PERIODS = 20        # window cap: after the fix, the stop rule rather than this ends a strike

def fe_modes(L, W, t_root, t_tip, Mtip, ne=N_ELEMENTS, nm=N_MODES_EVAL):
    xe = np.linspace(0, L, ne+1); h = L/ne; nd = 2*(ne+1)
    K = np.zeros((nd, nd)); M = np.zeros((nd, nd))
    for e in range(ne):
        xm = (xe[e]+xe[e+1])/2; t = t_root+(t_tip-t_root)*xm/L; A = W*t; I = W*t**3/12
        k = E*I/h**3*np.array([[12,6*h,-12,6*h],[6*h,4*h*h,-6*h,2*h*h],[-12,-6*h,12,-6*h],[6*h,2*h*h,-6*h,4*h*h]])
        m = RHO*A*h/420*np.array([[156,22*h,54,-13*h],[22*h,4*h*h,13*h,-3*h*h],[54,13*h,156,-22*h],[-13*h,-3*h*h,-22*h,4*h*h]])
        i = 2*e; K[i:i+4, i:i+4] += k; M[i:i+4, i:i+4] += m
    M[-2, -2] += Mtip
    K = K[2:, 2:]; M = M[2:, 2:]
    w2, V = sl.eigh(K, M, subset_by_index=[0, nm-1])      # mass-normalised: V.T M V = I
    phi = np.vstack([np.zeros(nm), V[0::2, :]])           # displacement DOFs incl. clamp
    return np.sqrt(w2), xe, phi

def reed(midi, t_tip, t_root, nm=N_MODES_EVAL):
    L = reed_len(midi); W = width(midi); f = A4_HZ*2**((midi-A4_MIDI)/12)
    mass = RHO*W*L*(t_root+t_tip)/2
    om, xe, phi = fe_modes(L, W, t_root, t_tip, 0.0, N_ELEMENTS_PITCH, 1)
    mt = 0.0
    if om[0]/(2*np.pi) > f*PITCH_TOL:
        mt = brentq(lambda x: fe_modes(L, W, t_root, t_tip, x, N_ELEMENTS_PITCH, 1)[0][0]/(2*np.pi)-f, 0, SOLDER_BRACKET*mass)
    om, xe, phi = fe_modes(L, W, t_root, t_tip, mt, N_ELEMENTS, nm)
    return dict(L=L, f=f, fe1=om[0]/(2*np.pi), om=om, xe=xe, phi=phi, mu=mt/mass)

def patch(r, xs, half=PATCH_HALF):
    L = r['L']; xx = np.linspace((xs-half)*L, (xs+half)*L, PATCH_PTS)
    return np.array([np.mean(np.interp(xx, r['xe'], r['phi'][:, n])) for n in range(r['phi'].shape[1])])

def strike(r, xs, mh, v, tau_ratio, p=1.0, nm=7, half=PATCH_HALF, zeta=0.0):
    """tau_ratio = rigid-target felt contact time / T1, defining K at speed v (linear-equivalent)."""
    T1 = 1/r['f']; om = r['om'][:nm]*(r['f']/r['fe1']); ph = patch(r, xs, half)[:nm]
    tau = tau_ratio*T1
    # p=1: tau = pi sqrt(mh/K). For p != 1 match the same rigid-target contact time at speed v
    if p == 1.0:
        K = mh*(np.pi/tau)**2
    else:
        # rigid target: max compression dm from 0.5 mh v^2 = K dm^(p+1)/(p+1); time ~ c(p) dm/v
        # solve numerically for K giving contact time tau
        def ct(K):
            dt = tau/CT_SEARCH_STEPS; y = 0.0; vy = v; t = 0.0
            while True:
                F = K*max(y, 0)**p; vy -= F/mh*dt; y += vy*dt; t += dt
                if y <= 0 and t > dt: return t
        K = brentq(lambda K: ct(K)-tau, *K_BRACKET)
    dt = T1/STEPS_PER_T1/max(1, om[-1]/om[0]/TOP_MODE_RATIO)
    q = np.zeros(nm); qd = np.zeros(nm); y = 0.0; vy = v
    t = 0.0; contacts = []; incontact = False; tc0 = 0
    Tend = T_END_PERIODS*T1
    hist = []
    while t < Tend:
        ws = ph @ q; d = y-ws
        F = K*d**p if d > 0 else 0.0
        if F > 0 and not incontact: incontact = True; tc0 = t
        if F == 0 and incontact: incontact = False; contacts.append((tc0/T1, t/T1))
        a_h = -F/mh
        qdd = -om**2*q - 2*zeta*om*qd + ph*F
        vy += a_h*dt; y += vy*dt
        qd += qdd*dt; q += qd*dt          # semi-implicit Euler
        t += dt
        # Stop only when no recontact is possible: hammer moving away and below the patch's full reach
        # (sum |phi_n| x modal amplitude). The earlier rule (1 mm below the patch's current position)
        # missed late recontacts at tau_f <= 0.3 T1 (fixed 2026-10-29).
        if not incontact and contacts and vy < 0 and y < -np.sum(np.abs(ph)*np.sqrt(q**2 + (qd/om)**2)): break
    amp = np.sqrt(q**2 + (qd/om)**2)                       # modal amplitude (mass-normalised coords)
    tipdisp = amp*np.abs(r['phi'][-1, :nm])                # tip-displacement amplitude per mode
    return contacts, tipdisp, vy/v

# Evaluation grid printed by __main__ (the cases quoted in the plan's regime map).
EVAL_CASES = [(33, 'A1 bass .020', 0.020*INCH, 0.020*INCH), (60, 'C4 tip .585', 0.585e-3, 0.034*INCH)]
EVAL_XS = (0.30, 0.50)              # strike points, fraction of L
EVAL_MH = (1e-3, 5e-3)              # hammer masses, kg (the swing prior's 1-5 g ends)
EVAL_TAU = (0.1, 0.3, 0.75)         # rigid-target felt contact time, fraction of T1
EVAL_V = 2.0                        # hammer speed, m/s
EVAL_NM = (7, 20)                   # mode counts compared
LOG_FLOOR = 1e-30

if __name__ == '__main__':
    for midi, name, tt, tr in EVAL_CASES:
        r = reed(midi, tt, tr)
        om = r['om']; ratios = om/om[0]
        print(f"\n{name}: L {r['L']*MM_PER_M:.1f} mm, f {r['f']:.1f}, FE f1 {r['fe1']:.1f}, mu {r['mu']:.3f}")
        print('  FE ratios', np.round(ratios[:EVAL_NM[0]], 2))
        M1tip = 1/r['phi'][-1, 0]**2
        for xs in EVAL_XS:
            ph = patch(r, xs)
            print(f"  xs {xs:.2f} L: phi1(patch)/phi1(tip) {ph[0]/r['phi'][-1,0]:.3f}  M1tip {M1tip*MM_PER_M:.2f} g  Meff(xs) {MM_PER_M/ph[0]**2:.1f} g")
            for mh in EVAL_MH:
                for tr_ in EVAL_TAU:
                    c7, d7, rb = strike(r, xs, mh, EVAL_V, tr_, nm=EVAL_NM[0])
                    c20, d20, _ = strike(r, xs, mh, EVAL_V, tr_, nm=EVAL_NM[1])
                    dur = sum(b-a for a, b in c7)
                    up7 = 20*np.log10(np.maximum(d7[1:4], LOG_FLOOR)/d7[0])
                    print(f"    mh {mh*MM_PER_M:.0f} g tau/T1 {tr_:.2f}: contacts {len(c7)} (20 modes: {len(c20)}), "
                          f"total {dur:.2f} T1, span {c7[0][0]:.2f}-{c7[-1][1]:.2f} T1, hammer out {rb:+.2f} v, "
                          f"tip mode1 {d7[0]*MM_PER_M:.2f} mm (20m {d20[0]*MM_PER_M:.2f}), m2-4 re m1 dB {np.round(up7,1)}")
