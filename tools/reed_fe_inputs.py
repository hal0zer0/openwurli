"""Per-knot reed quantities for tools/swing_prior.py (2026-09-26): writes tools/data/reed_swing_inputs.json.
FE Euler-Bernoulli (reed_fe.py): blank widths and reed_length_mm from openwurli tables.rs;
bass reeds 1-20 uniform, variants .020" (VV) / .026" (openwurli code); reeds 21+ linear taper from
.034" root to tip t_r in {0.51, 0.585, 0.66} mm. Solder (tip mass) solved to put the reed on pitch
where the bare geometry is sharp; where it is flat (treble), bare geometry M1 with the true f1 is used.
Mode shape tip-normalised. phi_s = mean phi over the hammer patch [x_s-0.1L, x_s+0.1L];
phi_p = mean phi over [L-8 mm, L-2 mm]."""
import json, numpy as np
import os
from reed_fe import reed_len, width, fe, RHO
from scipy.optimize import brentq
def variants(m):
    r=m-32
    if r<=20: return [('bass_020',0.020*25.4e-3,0.020*25.4e-3),('bass_026',0.026*25.4e-3,0.026*25.4e-3)]
    return [(f'tip_{t:.3f}',t*1e-3,0.034*25.4e-3) for t in (0.51,0.585,0.66)]
out={}
for m in list(range(33,53,3))+[52,53,54]+list(range(57,97,3)):
    L=reed_len(m); W=width(m); f=440*2**((m-69)/12); row={}
    for name,t_tip,t_root in variants(m):
        mass=RHO*W*L*(t_root+t_tip)/2
        f0,xe,w,M1=fe(L,W,t_root,t_tip,0.0)
        mt=0.0
        if f0>f*1.002:
            mt=brentq(lambda x: fe(L,W,t_root,t_tip,x,40)[0]-f,0,20*mass); f0,xe,w,M1=fe(L,W,t_root,t_tip,mt)
        ph=lambda a,b: float(np.mean(np.interp(np.linspace(a,b,60),xe,w)))
        row[name]=dict(M1_g=M1*1e3, tip_mass_g=mt*1e3, mu=mt/mass, f1_Hz=f, f_fe_Hz=float(f0), on_pitch=bool(abs(f0/f-1)<0.01),
                       phi_s={f'{xs:.2f}':ph((xs-0.1)*L,(xs+0.1)*L) for xs in (0.40,0.45,0.50,0.55,0.60)},
                       phi_p=ph(L-8e-3,L-2e-3), L_mm=L*1e3, W_mm=W*1e3, t_root_mm=t_root*1e3, t_tip_mm=t_tip*1e3)
    out[m]=row
json.dump(out,open(os.path.join(os.path.dirname(os.path.abspath(__file__)),'data','reed_swing_inputs.json'),'w'),indent=1)
