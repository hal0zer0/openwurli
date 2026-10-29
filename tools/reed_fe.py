"""80-element Euler-Bernoulli reed model (from the physics review, 2026-09-26).

Geometry from tables.rs (reed_length_mm, blank widths); bass reeds 1-20 uniform at either disputed
blank thickness, reeds 21+ tapered from the .034" root to the tip thickness; tip solder solved to
put the reed on pitch where the bare geometry is sharp. Running this file prints the per-note ff
swing table that tables::PROVISIONAL_FF_SWING_MM was picked from (see tools/swing_prior.py)."""
import numpy as np, itertools, scipy.linalg as sl
from scipy.optimize import brentq
RHO,E=7850.0,200e9
def reed_len(m): n=min(max(m-32,1),64); return (3.0-n/20 if n<=20 else 2.0-(n-20)/44)*25.4e-3
def width(m):
    r=min(max(m-32,1),64); w=0.151 if r<=14 else 0.127 if r<=20 else 0.121 if r<=42 else 0.111 if r<=50 else 0.098
    return w*25.4e-3
def fe(L,W,t_root,t_tip,Mtip,ne=80):
    xe=np.linspace(0,L,ne+1); h=L/ne; nd=2*(ne+1); K=np.zeros((nd,nd)); M=np.zeros((nd,nd))
    for e in range(ne):
        xm=(xe[e]+xe[e+1])/2; t=t_root+(t_tip-t_root)*xm/L; A=W*t; I=W*t**3/12
        k=E*I/h**3*np.array([[12,6*h,-12,6*h],[6*h,4*h*h,-6*h,2*h*h],[-12,-6*h,12,-6*h],[6*h,2*h*h,-6*h,4*h*h]])
        m=RHO*A*h/420*np.array([[156,22*h,54,-13*h],[22*h,4*h*h,13*h,-3*h*h],[54,13*h,156,-22*h],[-13*h,-3*h*h,-22*h,4*h*h]])
        i=2*e; K[i:i+4,i:i+4]+=k; M[i:i+4,i:i+4]+=m
    M[-2,-2]+=Mtip
    K=K[2:,2:]; M=M[2:,2:]
    w2,v=sl.eigh(K,M,subset_by_index=[0,0]); v=v[:,0]; w=np.concatenate([[0],v[0::2]])
    w=w/w[-1]; M1=float(v@M@v)/v[-2]**2
    return np.sqrt(w2[0])/(2*np.pi), xe, w, M1
def reed(m, t_tip, t_root):
    L=reed_len(m); W=width(m); f=440*2**((m-69)/12)
    mass=RHO*W*L*(t_root+t_tip)/2
    f0,xe,w,M1=fe(L,W,t_root,t_tip,0.0)
    if f0>f*1.002:   # tune down with solder (tip mass) to pitch
        mt=brentq(lambda mt: fe(L,W,t_root,t_tip,mt,40)[0]-f, 0, 20*mass)
        f0,xe,w,M1=fe(L,W,t_root,t_tip,mt)
    else: mt=0.0
    ph=lambda a,b: np.mean(np.interp(np.linspace(a,b,50),xe,w))
    phs=ph(0.4*L,0.6*L)                 # hammer patch 20% L centred 0.5 L
    php=ph(L-8e-3,L-2e-3)               # pickup region just behind solder
    return dict(L=L,W=W,f=f,f_bare=fe(L,W,t_root,t_tip,0.0,40)[0],mu=mt/mass,M1=M1,phs=phs,php=php)
def swing(r,mh,vh,e,D):
    Meff=r['M1']/r['phs']**2
    V=(1+e)*mh*vh*D/(r['phs']*(mh+Meff)); V=min(V,vh*np.sqrt(mh/r['M1']))
    return V/(2*np.pi*r['f'])*r['php']*1e3   # mm at pickup
IN=[3.0 if 0 else None]
def thick(m):
    r=m-32   # (t_tip, t_root) variants in m
    if r<=20: return [(0.020*25.4e-3,)*2,(0.026*25.4e-3,)*2]            # bass unground: VV .020 vs code .026, uniform
    tip = [0.51e-3,0.66e-3]                                               # openwurli pickup t_r band
    return [(tt,0.034*25.4e-3) for tt in tip]                             # ground tongue, root at blank .034
GRID=dict(mh=(1e-3,2e-3,3e-3,4e-3,5e-3),vh=(1.4,2.45,3.5),e=(0.3,0.5,0.7),D=(0.33,0.57,0.8))
CENT=dict(mh=np.sqrt(1e-3*5e-3),vh=np.sqrt(1.4*3.5),e=0.5,D=np.sqrt(0.33*0.8))
if __name__=='__main__':
    print('MIDI  L mm  f Hz  fbare(var) mu(var)   M1 g   phi_s phi_p |  central  p10   med   p90  (mm at pickup, ff)')
    for m in range(33,97,3):
        rs=[reed(m,*tv) for tv in thick(m)]
        a=[swing(r,*c) for r in rs for c in itertools.product(*GRID.values())]
        cen=np.exp(np.mean([np.log(swing(r,*CENT.values())) for r in rs]))
        print(f"{m:3d} {rs[0]['L']*1e3:5.1f} {rs[0]['f']:6.1f} {'/'.join(f'{r['f_bare']:.0f}' for r in rs):>9} {'/'.join(f'{r['mu']:.2f}' for r in rs):>9} {'/'.join(f'{r['M1']*1e3:.2f}' for r in rs):>9} {rs[0]['phs']:.3f} {rs[0]['php']:.3f} | {cen:6.2f} {np.percentile(a,10):5.2f} {np.median(a):5.2f} {np.percentile(a,90):5.2f}")
