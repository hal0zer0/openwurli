"""Roots of the tip-mass cantilever frequency equation (no rotary inertia),
1 + cos b cosh b + mu b (cos b sinh b - sin b cosh b) = 0. They agree with the FE in
hammer_contact_eval.py to 4 decimals, and show tables.rs `eigenvalues` running sharp above mode 1
whenever mu > 0 (docs/research/hammer-contact-revision.md §2)."""
import numpy as np
from scipy.optimize import brentq
def ch(b, mu): return 1+np.cos(b)*np.cosh(b)+mu*b*(np.cos(b)*np.sinh(b)-np.sin(b)*np.cosh(b))
N_ROOTS = 7                     # the engine's mode count (tables::NUM_MODES)
BETA_SCAN = (0.3, 22.0, 40000)  # sign-change scan for roots: beta range and grid points (covers 7 roots)
MU_TABLE = (0.0, 0.01, 0.05, 0.10, 0.15, 0.20, 0.30, 0.50)  # tables::eigenvalues' tabulated tip-mass ratios
def roots(mu, n=N_ROOTS):
    g = np.linspace(*BETA_SCAN); v = ch(g, mu); bs = []
    for i in range(len(g)-1):
        if v[i]*v[i+1] < 0: bs.append(brentq(ch, g[i], g[i+1], args=(mu,)))
    return np.array(bs[:n])
if __name__ == '__main__':
    for mu in MU_TABLE:
        b = roots(mu); print(f"mu {mu:.2f}: betas {np.round(b,4)}  f_n/f_1 {np.round((b/b[0])**2,2)}")
