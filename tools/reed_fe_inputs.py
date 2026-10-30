"""Per-knot reed quantities for tools/swing_prior.py (2026-09-26): writes tools/data/reed_swing_inputs.json.
FE Euler-Bernoulli (reed_fe.py): blank widths and reed_length_mm from openwurli tables.rs;
bass reeds 1-20 uniform, variants .020" (VV) / .026" (openwurli code); reeds 21+ linear taper from
.034" root to tip t_r in {0.51, 0.585, 0.66} mm. Solder (tip mass) solved to put the reed on pitch
where the bare geometry is sharp; where it is flat (treble), bare geometry M1 with the true f1 is used.
Mode shape tip-normalised. phi_s = mean phi over the hammer patch [x_s-0.1L, x_s+0.1L];
phi_p = mean phi over [L-8 mm, L-2 mm]."""
import json, numpy as np
import os
from reed_fe import (reed_len, width, fe, RHO, INCH, MM_PER_M, A4_HZ, A4_MIDI, REED_OFFSET, BASS_REEDS,
                     BASS_BLANKS_IN, ROOT_IN, PITCH_TOL, SOLDER_BRACKET, N_ELEMENTS_PITCH, FINGER_REGION_M)
from scipy.optimize import brentq
TIPS_MM=(0.51,0.585,0.66)        # tip thickness variants for reeds 21+ (pickup spec t_r band and its centre)
KNOTS=list(range(33,53,3))+[52,53,54]+list(range(57,97,3))  # the swing table's knots, incl. both sides of 52|53
XS_KNOTS=(0.40,0.45,0.50,0.55,0.60)  # strike points tabulated, fraction of L
PATCH_HALF=0.1                   # hammer patch half-length, fraction of L
SHAPE_PTS=60                     # points per mode-shape average
ON_PITCH_TOL=0.01                # 'on_pitch' flag: FE fundamental within 1 % of the target
def variants(m):
    r=m-REED_OFFSET
    if r<=BASS_REEDS: return [(f'bass_{round(t*1000):03d}',t*INCH,t*INCH) for t in BASS_BLANKS_IN]
    return [(f'tip_{t:.3f}',t*1e-3,ROOT_IN*INCH) for t in TIPS_MM]
out={}
for m in KNOTS:
    L=reed_len(m); W=width(m); f=A4_HZ*2**((m-A4_MIDI)/12); row={}
    for name,t_tip,t_root in variants(m):
        mass=RHO*W*L*(t_root+t_tip)/2
        f0,xe,w,M1=fe(L,W,t_root,t_tip,0.0)
        mt=0.0
        if f0>f*PITCH_TOL:
            mt=brentq(lambda x: fe(L,W,t_root,t_tip,x,N_ELEMENTS_PITCH)[0]-f,0,SOLDER_BRACKET*mass); f0,xe,w,M1=fe(L,W,t_root,t_tip,mt)
        ph=lambda a,b: float(np.mean(np.interp(np.linspace(a,b,SHAPE_PTS),xe,w)))
        row[name]=dict(M1_g=M1*MM_PER_M, tip_mass_g=mt*MM_PER_M, mu=mt/mass, f1_Hz=f, f_fe_Hz=float(f0), on_pitch=bool(abs(f0/f-1)<ON_PITCH_TOL),
                       phi_s={f'{xs:.2f}':ph((xs-PATCH_HALF)*L,(xs+PATCH_HALF)*L) for xs in XS_KNOTS},
                       phi_p=ph(L-FINGER_REGION_M[0],L-FINGER_REGION_M[1]), L_mm=L*MM_PER_M, W_mm=W*MM_PER_M, t_root_mm=t_root*MM_PER_M, t_tip_mm=t_tip*MM_PER_M)
    out[m]=row
json.dump(out,open(os.path.join(os.path.dirname(os.path.abspath(__file__)),'data','reed_swing_inputs.json'),'w'),indent=1)
