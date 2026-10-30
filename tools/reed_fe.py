"""80-element Euler-Bernoulli reed model (from the physics review, 2026-09-26).

Geometry from tables.rs (reed_length_mm, blank widths); bass reeds 1-20 uniform at either disputed
blank thickness, reeds 21+ tapered from the .034" root to the tip thickness; tip solder solved to
put the reed on pitch where the bare geometry is sharp. Running this file prints the per-note ff
swing table that tables::PROVISIONAL_FF_SWING_MM was picked from (see tools/swing_prior.py)."""
import numpy as np, itertools, scipy.linalg as sl
from scipy.optimize import brentq
RHO,E=7850.0,200e9  # spring steel: density kg/m^3 and Young's modulus Pa (handbook values)
INCH=25.4e-3        # m per inch
N_REEDS=64; REED_OFFSET=32   # reed n = MIDI - 32 (MIDI 33 = reed 1)
# Reed free length, tables.rs reed_length_mm (reed-and-hammer-physics.md §1.3), inches:
# bass n = 1-20: 3.0 - n/20; treble n = 21-64: 2.0 - (n-20)/44.
LEN_BASS=(3.0,20); LEN_TREBLE=(2.0,20,44); BASS_REEDS=20
# Reed blank widths by range, inches (reed-and-hammer-physics.md §1, blank table): reeds 1-14 / 15-20 /
# 21-42 / 43-50 / 51-64.
WIDTH_BREAKS=(14,20,42,50); WIDTHS_IN=(0.151,0.127,0.121,0.111,0.098)
N_ELEMENTS=80       # FE elements along the reed
PITCH_TOL=1.002     # the bare reed counts as sharp (needs solder) above 0.2 % over pitch
SOLDER_BRACKET=20   # solder mass search bracket: up to 20x the reed's own mass
N_ELEMENTS_PITCH=40 # coarser mesh for the solder-to-pitch root search (the final solve uses N_ELEMENTS)
A4_HZ,A4_MIDI=440.0,69
PATCH_FRAC=(0.4,0.6)       # hammer patch for the swing prior: 20 % of L centred at 0.5 L
FINGER_REGION_M=(8e-3,2e-3)  # pickup finger region: from L-8 mm to L-2 mm, just behind the solder
SHAPE_AVG_PTS=50           # points per mode-shape average
MM_PER_M=1e3
# Reed thickness variants: bass blanks .020" (Vintage Vibe) / .026" (earlier code), uniform; reeds 21+
# ground from a .034" root to a tip in the pickup spec's t_r band, 0.51-0.66 mm.
BASS_BLANKS_IN=(0.020,0.026); ROOT_IN=0.034; TIP_BAND_M=(0.51e-3,0.66e-3)
# The swing prior's registered hammer priors (tools/swing_prior.py): grid and geometric centre.
GRID=dict(mh=(1e-3,2e-3,3e-3,4e-3,5e-3),vh=(1.4,2.45,3.5),e=(0.3,0.5,0.7),D=(0.33,0.57,0.8))
CENT=dict(mh=np.sqrt(1e-3*5e-3),vh=np.sqrt(1.4*3.5),e=0.5,D=np.sqrt(0.33*0.8))
TABLE_MIDI=range(33,97,3)  # knots printed by __main__
def reed_len(m):
    n=min(max(m-REED_OFFSET,1),N_REEDS)
    return (LEN_BASS[0]-n/LEN_BASS[1] if n<=BASS_REEDS else LEN_TREBLE[0]-(n-LEN_TREBLE[1])/LEN_TREBLE[2])*INCH
def width(m):
    r=min(max(m-REED_OFFSET,1),N_REEDS); b=WIDTH_BREAKS
    w=WIDTHS_IN[0] if r<=b[0] else WIDTHS_IN[1] if r<=b[1] else WIDTHS_IN[2] if r<=b[2] else WIDTHS_IN[3] if r<=b[3] else WIDTHS_IN[4]
    return w*INCH
# fe(): standard 2-node Hermite (Euler-Bernoulli) beam element, consistent mass (textbook matrices).
def fe(L,W,t_root,t_tip,Mtip,ne=N_ELEMENTS):
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
    L=reed_len(m); W=width(m); f=A4_HZ*2**((m-A4_MIDI)/12)
    mass=RHO*W*L*(t_root+t_tip)/2
    f0,xe,w,M1=fe(L,W,t_root,t_tip,0.0)
    if f0>f*PITCH_TOL:   # tune down with solder (tip mass) to pitch
        mt=brentq(lambda mt: fe(L,W,t_root,t_tip,mt,N_ELEMENTS_PITCH)[0]-f, 0, SOLDER_BRACKET*mass)
        f0,xe,w,M1=fe(L,W,t_root,t_tip,mt)
    else: mt=0.0
    ph=lambda a,b: np.mean(np.interp(np.linspace(a,b,SHAPE_AVG_PTS),xe,w))
    phs=ph(PATCH_FRAC[0]*L,PATCH_FRAC[1]*L)
    php=ph(L-FINGER_REGION_M[0],L-FINGER_REGION_M[1])
    return dict(L=L,W=W,f=f,f_bare=fe(L,W,t_root,t_tip,0.0,N_ELEMENTS_PITCH)[0],mu=mt/mass,M1=M1,phs=phs,php=php)
def swing(r,mh,vh,e,D):
    Meff=r['M1']/r['phs']**2
    V=(1+e)*mh*vh*D/(r['phs']*(mh+Meff)); V=min(V,vh*np.sqrt(mh/r['M1']))
    return V/(2*np.pi*r['f'])*r['php']*MM_PER_M   # mm at pickup
def thick(m):
    r=m-REED_OFFSET   # (t_tip, t_root) variants in m
    if r<=BASS_REEDS: return [(BASS_BLANKS_IN[0]*INCH,)*2,(BASS_BLANKS_IN[1]*INCH,)*2]
    return [(tt,ROOT_IN*INCH) for tt in TIP_BAND_M]
if __name__=='__main__':
    print('MIDI  L mm  f Hz  fbare(var) mu(var)   M1 g   phi_s phi_p |  central  p10   med   p90  (mm at pickup, ff)')
    for m in TABLE_MIDI:
        rs=[reed(m,*tv) for tv in thick(m)]
        a=[swing(r,*c) for r in rs for c in itertools.product(*GRID.values())]
        cen=np.exp(np.mean([np.log(swing(r,*CENT.values())) for r in rs]))
        print(f"{m:3d} {rs[0]['L']*MM_PER_M:5.1f} {rs[0]['f']:6.1f} {'/'.join(f'{r['f_bare']:.0f}' for r in rs):>9} {'/'.join(f'{r['mu']:.2f}' for r in rs):>9} {'/'.join(f'{r['M1']*MM_PER_M:.2f}' for r in rs):>9} {rs[0]['phs']:.3f} {rs[0]['php']:.3f} | {cen:6.2f} {np.percentile(a,10):5.2f} {np.median(a):5.2f} {np.percentile(a,90):5.2f}")
