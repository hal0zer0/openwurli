#!/usr/bin/env python3
"""Run harmonic analysis at multiple input levels and parse results."""
import subprocess
import re
import os

NETLIST_TEMPLATE = """.title Wurlitzer 200A Preamp - Harmonics at {label}
.MODEL Q2N5089 NPN(
+  IS=3.03E-14 BF=1434 NF=1.005 VAF=98.5 IKF=0.01358
+  ISE=2.88E-15 NE=1.262 BR=4.62 NR=1 VAR=22 IKR=0.1
+  ISC=1.065E-11 NC=1.41 RB=57 RE=0.518 RC=2.58
+  CJE=3.22E-12 VJE=0.65 MJE=0.33 TF=5.4E-10
+  CJC=1.35E-12 VJC=0.4 MJC=0.33 XCJC=0.63 TR=5E-08
+  XTB=1.38 EG=1.11 XTI=3 FC=0.5)
.MODEL D1N4148 D(IS=2.52E-09 RS=0.568 N=1.752 BV=100
+  IBV=100E-06 CJO=4E-12 VJ=0.7 M=0.45 TT=6E-09)
Vcc vcc 0 DC 15
Vin in_sig 0 DC 0 SIN(0 {amp} {freq})
R1 in_sig node_A 22K
Cin node_A base1 0.022U
R2 vcc base1 2MEG
R3 base1 0 470K
C20 base1 0 220P
D1 0 base1 D1N4148
Q1 coll1 base1 emit1 Q2N5089
Rc1 vcc coll1 150K
Cc3 coll1 base1 100P
Re1 emit1 0 33K
Ce1 emit1 fb_junct 4.7U
Rldr_path fb_junct 0 {rldr}
R10 out fb_junct 56K
Q2 coll2 coll1 emit2a Q2N5089
Rc2 vcc coll2 1.8K
Re2a emit2a emit2b 270
Ce2 emit2a emit2b 22U
Re2b emit2b 0 820
Cc4 coll2 coll1 100P
R9 coll2 out 6.8K
Rload out 0 100K
.control
tran 0.5u 200m
meas tran vout_max max v(out) from=150m to=200m
meas tran vout_min min v(out) from=150m to=200m
meas tran vc1_max max v(coll1) from=150m to=200m
meas tran vc1_min min v(coll1) from=150m to=200m
meas tran ve1_max max v(emit1) from=150m to=200m
meas tran ve1_min min v(emit1) from=150m to=200m
fourier {freq} v(out)
fourier {freq} v(coll1)
.endc
.end
"""

# Test cases: (label, amplitude_V, frequency_Hz, rldr_path_ohm)
tests = [
    # Velocity sweep at 440 Hz, no tremolo (Rldr=1M)
    ("pp_440_notrem",     0.0005, 440, 1e6),
    ("mf_440_notrem",     0.002,  440, 1e6),
    ("f_440_notrem",      0.005,  440, 1e6),
    ("ff_440_notrem",     0.010,  440, 1e6),
    ("fff_440_notrem",    0.050,  440, 1e6),
    ("extreme_440_notrem",0.200,  440, 1e6),
    # Different frequencies at mf, no tremolo
    ("mf_220_notrem",     0.002,  220, 1e6),
    ("mf_880_notrem",     0.002,  880, 1e6),
    ("mf_1760_notrem",    0.002, 1760, 1e6),
    ("mf_3520_notrem",    0.002, 3520, 1e6),
    # Tremolo bright (Rldr_path=19K) at different levels
    ("mf_440_trem_bright",0.002,  440, 19e3),
    ("ff_440_trem_bright",0.010,  440, 19e3),
    ("fff_440_trem_bright",0.050, 440, 19e3),
]

print("=" * 100)
print(f"{'Test':<25} {'Vin_pk(mV)':>10} {'Vout_pp(mV)':>12} {'Gain':>8} {'THD%':>10} "
      f"{'H2/H1(dB)':>10} {'H3/H1(dB)':>10} {'Vc1_pp(mV)':>11} {'SatHead(V)':>11}")
print("=" * 100)

for label, amp, freq, rldr in tests:
    netlist = NETLIST_TEMPLATE.format(label=label, amp=amp, freq=freq, rldr=rldr)
    cir_path = f"/tmp/harm_{label}.cir"
    with open(cir_path, 'w') as f:
        f.write(netlist)

    result = subprocess.run(['ngspice', '-b', cir_path],
                          capture_output=True, text=True, timeout=120)
    output = result.stdout + result.stderr

    # Parse meas results
    vout_max = vout_min = vc1_max = vc1_min = ve1_max = ve1_min = None
    for line in output.split('\n'):
        m = re.match(r'vout_max\s+=\s+([0-9eE.+-]+)', line)
        if m: vout_max = float(m.group(1))
        m = re.match(r'vout_min\s+=\s+([0-9eE.+-]+)', line)
        if m: vout_min = float(m.group(1))
        m = re.match(r'vc1_max\s+=\s+([0-9eE.+-]+)', line)
        if m: vc1_max = float(m.group(1))
        m = re.match(r'vc1_min\s+=\s+([0-9eE.+-]+)', line)
        if m: vc1_min = float(m.group(1))
        m = re.match(r've1_max\s+=\s+([0-9eE.+-]+)', line)
        if m: ve1_max = float(m.group(1))
        m = re.match(r've1_min\s+=\s+([0-9eE.+-]+)', line)
        if m: ve1_min = float(m.group(1))

    # Parse Fourier for v(out) — first fourier block
    thd = None
    harmonics = {}
    in_fourier_out = False
    for line in output.split('\n'):
        if 'Fourier analysis for v(out)' in line:
            in_fourier_out = True
            continue
        if in_fourier_out and 'THD:' in line:
            m = re.search(r'THD:\s+([0-9eE.+-]+)\s+%', line)
            if m: thd = float(m.group(1))
        if in_fourier_out:
            m = re.match(r'\s+(\d+)\s+[\d.]+\s+([\d.eE+-]+)\s+([\d.eE+-]+)', line)
            if m:
                h_num = int(m.group(1))
                h_mag = float(m.group(2))
                harmonics[h_num] = h_mag
                if h_num >= 4:
                    in_fourier_out = False  # done with this fourier block

    vout_pp = (vout_max - vout_min) * 1000 if vout_max and vout_min else 0
    vc1_pp = (vc1_max - vc1_min) * 1000 if vc1_max and vc1_min else 0
    gain = (vout_max - vout_min) / (2 * amp) if vout_max and vout_min else 0
    sat_head = vc1_min - ve1_max - 0.1 if vc1_min and ve1_max else 0

    import math
    h2_db = 20 * math.log10(harmonics.get(2, 1e-20) / harmonics.get(1, 1e-20)) if harmonics.get(1, 0) > 0 else -999
    h3_db = 20 * math.log10(harmonics.get(3, 1e-20) / harmonics.get(1, 1e-20)) if harmonics.get(1, 0) > 0 else -999

    print(f"{label:<25} {amp*1000:>10.2f} {vout_pp:>12.4f} {gain:>8.3f} {thd or 0:>10.4f} "
          f"{h2_db:>10.1f} {h3_db:>10.1f} {vc1_pp:>11.4f} {sat_head:>11.3f}")

print()
print("Notes:")
print("- Vin_pk is the peak input amplitude in mV")
print("- Gain = Vout_pp / (2 * Vin_pk)")
print("- H2/H1 and H3/H1 are harmonic-to-fundamental ratios in dB")
print("- SatHead = Vc1_min - Ve1_max - 0.1V (headroom before TR-1 saturates)")
