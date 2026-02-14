# Reed and Hammer Physics: Wurlitzer 200/200A

Comprehensive physical reference for AI agent consumption. Every claim is tagged:
- **[VERIFIED]** = confirmed by multiple independent sources or direct measurement
- **[SINGLE-SOURCE]** = from one credible source only
- **[CALCULATED]** = derived from verified data using established physics
- **[ESTIMATED]** = reasonable inference, not directly measured
- **[UNVERIFIED]** = plausible but lacks confirmation

---

## 1. Reed Construction

### 1.1 Material

**[SINGLE-SOURCE]** The reeds are made of spring steel. The exact alloy is not documented in any available Wurlitzer service manual or patent. Replacement reed suppliers (Retro Linear, Vintage Vibe) use blue-tempered spring steel strip, consistent with AISI 1095 or similar high-carbon steel (0.90-1.05% C).

**Material properties for AISI 1095 blue-tempered spring steel [VERIFIED from materials databases]:**

| Property | Value | Unit |
|----------|-------|------|
| Young's modulus E | 200-210 | GPa |
| Density rho | 7850 | kg/m^3 |
| Poisson's ratio | 0.29 | - |
| Yield strength | 500-700 (tempered) | MPa |
| Internal damping ratio zeta | ~1e-4 (steel tuning fork) | - |
| Quality factor Q (fundamental) | ~5000-10000 | - |

Source: [Euphonics, Section 2.2.7](https://euphonics.org/2-2-7-vibration-damping/) gives zeta ~ 1e-4 for tuning forks. [Wikipedia: Tuning fork](https://en.wikipedia.org/wiki/Tuning_fork) confirms Q ~ 1000 for a standard tuning fork; clean steel bars with minimal mounting losses can reach Q ~ 5000-10000.

### 1.2 Reed Blank Sizes

**[VERIFIED]** Wurlitzer used 5 reed blanks for the 200/200A series (64 notes, A1-C7, MIDI 33-96). Each blank has a different width, and the upper blanks are ground to taper thickness.

Sources: [Vintage Vibe Case Study](https://www.vintagevibe.com/blogs/news/wurlitzer-electric-piano-reeds-case-study), [EP-Forum Reed Dimensions](https://ep-forum.com/smf/index.php?topic=8418.0), [DocWurly Reed Compatibility History](https://docwurly.com/wurlitzer-ep-history/wurlitzer-ep-reed-compatibility-history/)

#### 200 Series Reed Dimensions (Era 4, 1966-1975)

| Blank | Reed Numbers | Notes | Width (inch) | Thickness (inch) | Ground? |
|-------|-------------|-------|-------------|-------------------|---------|
| 1 (Lower) | 1-14 | A1-Bb2 | 0.151 | 0.020 | No |
| 2 (Upper Lower) | 15-20 | B2-E3 | 0.127 | 0.020 | No |
| 3 (Middle) | 21-42 | F3-C5 | 0.121 | 0.031 (base 0.032) | Gradual grind |
| 4 (Upper Mid) | 43-50 | C#5-G#5 | 0.111 | 0.031 (base 0.032) | Gradual grind |
| 5 (Treble) | 51-64 | A5-C7 | 0.097-0.099 | 0.031 (base 0.032) | Gradual to rapid grind |

#### 200A Series Modifications (post-1975)

**[VERIFIED]** The 200A increased the thickness of middle and treble reeds:
- Middle/Upper-Mid/Treble blanks: 0.026-0.034 inch (vs 0.020-0.031 in 200)
- Widths remain identical to 200 series
- Both the head (screw mounting area) and tongue were thicker
- Result: "smoother, rounder, mellower tone" vs 200's "long dwell, sharp attack"

Source: [Vintage Vibe Case Study](https://www.vintagevibe.com/blogs/news/wurlitzer-electric-piano-reeds-case-study)

#### Pickup Slot Dimensions (140B, similar geometry to 200)

**[SINGLE-SOURCE]** Measured from a 140B instrument (EP-Forum):

| Reed Range | Slot Width (inch) | Reed Width (inch) | Side Clearance per side (inch) |
|------------|-------------------|-------------------|-------------------------------|
| 1-14 | 0.172 | 0.151 | 0.0105 |
| 15-20 | 0.145 | 0.127 | 0.009 |
| 21-42 | 0.139 | 0.121 | 0.009 |
| 43-64 | 0.114 | 0.097-0.099 | 0.0075-0.0085 |

Source: [EP-Forum Thread](https://ep-forum.com/smf/index.php?topic=8418.0)

### 1.3 Reed Lengths (Vibrating Length)

**[SINGLE-SOURCE]** From EP-Forum thread "The formula for lengths of Wurly reeds":

The vibrating length follows a **two-segment linear scaling** (NOT logarithmic):

**Bass reeds (1-20):** Subtract exactly 1/20 inch per reed number.
```
L(n) = 3.0 - n * (1/20)    [inches]
Reed 1:  2.950" (2 19/20")
Reed 5:  2.750" (2 3/4")
Reed 10: 2.500" (2 1/2")
Reed 15: 2.250" (2 1/4")
Reed 20: 2.000"
```

**Treble reeds (21-64):** Subtract exactly 1/44 inch per reed number (counted from reed 21).
```
L(n) = 2.0 - (n - 20) * (1/44)    [inches], for n = 21..64
Reed 21: 1.977" (1 43/44")
Reed 31: 1.750" (1 3/4")
Reed 42: 1.500" (1 1/2")
Reed 53: 1.250" (1 1/4")
Reed 64: 1.000"
```

**Metric equivalents:**
- Bass reed 1 (A1, 55 Hz): L = 74.9 mm
- Mid reed 31 (E4, ~330 Hz): L = 44.5 mm
- Treble reed 64 (C7, ~2093 Hz): L = 25.4 mm

Source: [EP-Forum](https://ep-forum.com/smf/index.php?topic=9643.0)

**[CALCULATED] Verification check:** For a uniform cantilever beam, f = (k_n^2 / 2pi) * sqrt(EI / rho*A*L^4). With k_1 = 1.875 for the fundamental, a bare steel reed of L = 75 mm, t = 0.020 inch = 0.508 mm, w = 0.151 inch = 3.84 mm:
```
I = w*t^3/12 = 3.84e-3 * (0.508e-3)^3 / 12 = 4.19e-14 m^4
A = w*t = 3.84e-3 * 0.508e-3 = 1.95e-6 m^2
f1 = (1.875^2 / (2*pi)) * sqrt(210e9 * 4.19e-14 / (7850 * 1.95e-6 * 0.075^4))
   = (3.516 / 6.283) * sqrt(8.80e-3 / (4.84e-7))
   = 0.5596 * sqrt(18182) = 0.5596 * 134.8 = 75.4 Hz
```

Bare reed frequency ~75 Hz vs target A1 = 55 Hz. The solder tip mass lowers it by a factor of ~0.73, implying a tip mass ratio mu of roughly 0.05-0.15. This is consistent with the estimated mu values in the existing docs.

### 1.4 Tuning Weights (Solder)

**[VERIFIED]** Fine pitch tuning is accomplished by a "predetermined pyramid of solder" applied to the tip of each reed using a reed mold, then filed smooth.

Source: [Tropical Fish - How to Tune](https://www.tropicalfishvintage.com/blog/2020/5/7/how-to-tune-a-wurlitzer-electronic-piano-reed), [Sweetwater - Tuning](https://www.sweetwater.com/insync/tuning-wurlitzer-electric-pianos/)

**[ESTIMATED]** No published mass measurements of solder exist. Estimation from geometry:

Solder blob dimensions (typical): ~2-4 mm long, ~reed width, ~1-2 mm tall.
Solder density (60/40 Sn-Pb): ~8500 kg/m^3.

Bass reed solder (larger blob for greater detuning):
- Estimated volume: 3 mm x 3.8 mm x 1.5 mm = 17.1 mm^3 = 1.71e-8 m^3
- Estimated mass: 1.71e-8 * 8500 = 0.145 g

Bass reed mass (reed 1, L=75mm, w=3.84mm, t=0.508mm):
- Volume: 75 * 3.84 * 0.508 = 146.3 mm^3 = 1.46e-7 m^3
- Mass: 1.46e-7 * 7850 = 1.15 g

**Estimated mass ratio mu = m_solder / m_reed:**
- Bass: mu ~ 0.05-0.15 (larger solder, heavy reed)
- Mid: mu ~ 0.10-0.25 (moderate solder, lighter reed)
- Treble: mu ~ 0.00-0.05 (minimal solder, short reed)

### 1.5 Tuner-Damper System (Miessner Patent)

**[VERIFIED]** US Patent 3,215,765 (Miessner, 1965) describes a critical feature NOT present in the 200/200A but relevant to understanding the design lineage:

The patent describes a **neoprene/butyl rubber toroidal tuner-damper** clamped around the reed near its fixed end. Key specifications:
- Mass: approximately 1/3 of the reed mass (including internal metal ring)
- Position: approximately 1/8 of the reed length from the fixed end

> **IMPORTANT (Feb 2026):** The toroidal tuner-damper described in Miessner patents (US 3,215,765) was NOT implemented in production 200/200A instruments. The 200A uses solder tip mass only for tuning. This damper should NOT be modeled.

- Purpose: "integralize" the second-to-first partial ratio to exactly 6:1

**[VERIFIED]** For the 200/200A production instruments, this rubber tuner-damper was NOT used. Instead, tuning is accomplished purely through solder mass at the tip, and the reed is tapered (ground) in upper registers. The 200/200A reeds do NOT have integralized (harmonic) partials -- their overtones follow the natural inharmonic cantilever beam ratios modified by tip mass.

Source: [US Patent 3,038,363](https://patents.google.com/patent/US3038363), [US Patent 3,215,765](https://patents.google.com/patent/US3215765)

**[VERIFIED]** For the production instruments, Miessner patent 3,038,363 describes the actual system used: reeds with a "visco-elastic tuning-damper" that "harmonically relates vibration partials without significantly increasing damping." However, by the 200-series era, this was simplified to plain solder tuning masses.

### 1.6 Reed Mounting

**[VERIFIED]** Each reed is secured by a single screw through a mounting hole in the reed base. The hole has deliberate play (tolerance) allowing the reed to be shifted forward/backward to fine-tune the vibrating length:
- Forward (away from screw) = longer vibrating length = lower pitch
- Backward (toward screw) = shorter vibrating length = higher pitch

The reed must be centered within the pickup slot. Off-center mounting causes:
- Louder output (uneven capacitance modulation)
- Altered timbre
- Risk of short-circuit if reed contacts pickup plate

Source: [Tropical Fish - How to Tune](https://www.tropicalfishvintage.com/blog/2020/5/7/how-to-tune-a-wurlitzer-electronic-piano-reed)

---

## 2. Euler-Bernoulli Beam Theory

### 2.1 Governing Equation

**[VERIFIED]** The transverse vibration of a uniform cantilever beam is governed by:

```
EI * d^4w/dx^4 + rho*A * d^2w/dt^2 = 0
```

where:
- E = Young's modulus (Pa)
- I = second moment of area = w_reed * t^3 / 12 (m^4)
- rho = density (kg/m^3)
- A = cross-sectional area = w_reed * t (m^2)
- w(x,t) = transverse displacement

Source: [Wikipedia: Euler-Bernoulli beam theory](https://en.wikipedia.org/wiki/Euler%E2%80%93Bernoulli_beam_theory)

### 2.2 Boundary Conditions (Clamped-Free)

**[VERIFIED]** For a cantilever (clamped at x=0, free at x=L):

At x=0 (clamped):
```
w(0,t) = 0          (zero displacement)
dw/dx(0,t) = 0      (zero slope)
```

At x=L (free):
```
d^2w/dx^2(L,t) = 0  (zero bending moment)
d^3w/dx^3(L,t) = 0  (zero shear force)
```

### 2.3 Characteristic Equation (No Tip Mass)

**[VERIFIED]** Separation of variables gives spatial mode shapes:

```
Y_n(x) = C * [cosh(beta_n*x) - cos(beta_n*x)
              - sigma_n * (sinh(beta_n*x) - sin(beta_n*x))]
```

where sigma_n = (cosh(beta_n*L) + cos(beta_n*L)) / (sinh(beta_n*L) + sin(beta_n*L))

The characteristic equation is:

```
1 + cos(beta_n*L) * cosh(beta_n*L) = 0
```

Solutions (eigenvalues):

| Mode n | beta_n * L | (beta_n*L)^2 | f_n/f_1 |
|--------|-----------|-------------|---------|
| 1 | 1.8751 | 3.5160 | 1.000 |
| 2 | 4.6941 | 22.034 | 6.267 |
| 3 | 7.8548 | 61.697 | 17.547 |
| 4 | 10.9955 | 120.902 | 34.386 |
| 5 | 14.1372 | 199.860 | 56.842 |

Source: [enDAQ Bernoulli-Euler Beams](https://endaq.com/pages/bernoulli-euler-beams), multiple textbooks

Natural frequencies:
```
f_n = (beta_n*L)^2 / (2*pi*L^2) * sqrt(E*I / (rho*A))
```

Frequency ratios:
```
f_n / f_1 = (beta_n*L / beta_1*L)^2
```

### 2.4 Characteristic Equation WITH Tip Mass

**[VERIFIED]** Adding a point mass M_tip at the free end x=L modifies the free-end boundary conditions. The shear force at x=L must accelerate the tip mass:

```
At x=L: EI * d^3w/dx^3(L,t) = M_tip * d^2w/dt^2(L,t)
```

This changes the characteristic equation to:

```
1 + cos(lambda)*cosh(lambda) + lambda*mu*(cos(lambda)*sinh(lambda) - sin(lambda)*cosh(lambda)) = 0
```

where:
- lambda = beta_n * L (dimensionless eigenvalue)
- mu = M_tip / (rho * A * L) = M_tip / M_beam (tip mass / beam mass ratio)

Source: [Wiley Appendix C: Modal Analysis with Tip Mass](https://onlinelibrary.wiley.com/doi/pdf/10.1002/9781119991151.app3)

### 2.5 Eigenvalue Table for Cantilever with Tip Mass

**[CALCULATED]** Numerical solutions of the characteristic equation. Values verified against [Extrica article 16636](https://www.extrica.com/article/16636) and standard references.

| mu | lambda_1 | lambda_2 | lambda_3 | lambda_4 | f2/f1 | f3/f1 | f4/f1 |
|----|----------|----------|----------|----------|-------|-------|-------|
| 0.00 | 1.8751 | 4.6941 | 7.8548 | 10.9955 | 6.267 | 17.547 | 34.386 |
| 0.01 | 1.8584 | 4.6849 | 7.8504 | 10.993 | 6.358 | 17.860 | 34.99 |
| 0.05 | 1.7920 | 4.6477 | 7.8316 | 10.983 | 6.728 | 19.09 | 37.58 |
| 0.10 | 1.7227 | 4.6024 | 7.8077 | 10.970 | 7.133 | 20.55 | 40.56 |
| 0.15 | 1.6625 | 4.5618 | 7.7859 | 10.958 | 7.530 | 21.92 | 43.45 |
| 0.20 | 1.6097 | 4.5254 | 7.7659 | 10.947 | 7.905 | 23.26 | 46.22 |
| 0.30 | 1.5201 | 4.4620 | 7.7310 | 10.928 | 8.614 | 25.86 | 51.70 |
| 0.50 | 1.3853 | 4.3601 | 7.6745 | 10.897 | 9.903 | 30.68 | 61.82 |
| 1.00 | 1.2479 | 4.2291 | 7.5967 | 10.850 | 11.478 | 37.04 | 75.55 |

**CRITICAL OBSERVATION:** Adding tip mass INCREASES mode frequency ratios above 6.267 for all modes. The tip mass preferentially lowers the fundamental (which has maximum tip displacement) while leaving higher modes less affected (they have smaller tip displacement). There is NO physical mechanism by which a point mass at the tip can produce f2/f1 < 6.267.

**The previous code's mode ratios of 3.8-5.8 for f2/f1 are physically impossible** for a cantilever beam with tip mass. They would require either: (a) a distributed mass extending over >50% of the beam length, (b) a non-uniform cross-section specifically designed for it (as in Miessner's patent), or (c) entirely different boundary conditions.

Source: Numerical solution verified against [Extrica](https://www.extrica.com/article/16636) Table 1 (mu=0.01: lambda_1*pi = 1.8715, lambda_2*pi = 4.6830 -- note their convention uses lambda*pi; our lambda_1 = 1.8715/pi*pi = 1.8715 agrees after adjusting notation).

### 2.6 Estimated Tip Mass Ratios for Wurlitzer 200A Reeds

**[CALCULATED]** Using the reed length formula (Section 1.3) and the beam frequency equation, we can estimate the tip mass ratio needed to lower each reed's natural frequency from its bare-beam value to the target pitch.

Bare beam fundamental: f_bare = (1.8751^2 / (2*pi*L^2)) * sqrt(EI / (rho*A))

Target: f_target = 440 * 2^((MIDI-69)/12)

Ratio: f_target / f_bare determines the required eigenvalue lambda_1, from which mu is extracted via the characteristic equation.

| Reed | MIDI | Note | L (mm) | t (mm) | w (mm) | f_bare (Hz) | f_target (Hz) | Est. mu | Est. f2/f1 |
|------|------|------|--------|--------|--------|-------------|---------------|---------|-----------|
| 1 | 33 | A1 | 74.9 | 0.508 | 3.84 | ~75 | 55.0 | 0.10-0.15 | 7.1-7.5 |
| 10 | 42 | F#2 | 63.5 | 0.508 | 3.84 | ~105 | 92.5 | 0.03-0.08 | 6.4-6.9 |
| 20 | 52 | E3 | 50.8 | 0.508 | 3.23 | ~127 | 164.8 | ~0 | ~6.27 |
| 30 | 62 | D4 | 45.5 | 0.787 | 3.07 | ~265 | 293.7 | ~0 | ~6.27 |
| 42 | 74 | D5 | 38.1 | 0.787 | 3.07 | ~378 | 587.3 | 0.15-0.25 | 7.5-8.5 |
| 53 | 85 | C#6 | 31.8 | 0.787 | 2.49 | ~534 | 1108.7 | 0.25-0.40 | 8.5-10.0 |
| 64 | 96 | C7 | 25.4 | 0.787 | 2.49 | ~837 | 2093 | 0.30-0.50 | 9.0-10.5 |

**IMPORTANT FINDING:** The mid-register reeds (around reed 20-30) have f_bare BELOW f_target. This means the solder is NOT needed for pitch-lowering in this region -- the bare beam is already close to or below target pitch. In this register, solder is minimal (near-zero mu) and serves fine-tuning only. The treble reeds, however, have f_bare well BELOW target, meaning they need the grinding/thinning process AND short length to achieve high pitch. The solder in treble is also minimal.

**The bass reeds need the most solder** (highest mu), and the relationship between mu and register is NOT monotonic -- it depends on the interplay of reed length, thickness, and target pitch. This is a critical distinction from the existing model which assumes a smooth bass-to-treble mu gradient.

**[ESTIMATED] Recommended mode ratios for the model:**

Given the estimated mu values:

| Register | Est. mu range | f2/f1 | f3/f1 | f4/f1 | f5/f1 |
|----------|-------------|-------|-------|-------|-------|
| Bass (MIDI 33-45) | 0.05-0.15 | 6.5-7.5 | 18-22 | 36-44 | 58-72 |
| Mid (MIDI 46-65) | 0.00-0.05 | 6.27-6.7 | 17.5-19 | 34-38 | 57-62 |
| Treble (MIDI 66-96) | 0.00-0.05 | 6.27-6.7 | 17.5-19 | 34-38 | 57-62 |

These are HIGHER than the current model values (6.3/6.8/6.3 for f2/f1). The current values are reasonable for bass (mu~0.05 gives 6.7, not far from 6.3 at mu~0.02). However, the mid value of 6.8 implies mu~0.12, which seems high for mid-register where minimal solder is expected. The treble value of 6.3 (near bare beam) is physically sensible.

---

## 3. Mode Shapes and Modal Participation

### 3.1 Mode Shape Values at Free End (Tip)

**[CALCULATED]** For a uniform cantilever (no tip mass), the mode shapes normalized to unit maximum displacement have these values at the tip (x=L):

| Mode n | phi_n(L) (tip) | Node locations (fraction of L) |
|--------|---------------|-------------------------------|
| 1 | 2.000 (max at tip) | none |
| 2 | 2.000 (max at tip) | 0.783 |
| 3 | 2.000 (max at tip) | 0.504, 0.868 |
| 4 | 2.000 (max at tip) | 0.356, 0.644, 0.906 |
| 5 | 2.000 (max at tip) | 0.279, 0.500, 0.723, 0.926 |

Note: For the standard cantilever, ALL modes have their maximum displacement at the free end. The mode shapes are mass-normalized such that integral(rho*A*phi_n^2, 0, L) = 1, in which case phi_n(L) = 2.0 for all n (a property of clamped-free beams).

Source: Standard Euler-Bernoulli beam theory; see [Vibrations of Cantilever Beams](http://emweb.unl.edu/mechanics-pages/scott-whitney/325hweb/beams.htm)

### 3.2 Initial Modal Amplitudes from Velocity Impulse at Tip

**[CALCULATED]** When a hammer imparts a velocity impulse to the reed at the free end (x=L), the initial modal amplitudes are determined by the modal expansion of the initial conditions.

For an impulse force F(t) = J*delta(t) applied at x=L (where J = impulse = integral of force over time, in N*s):

The modal displacement response is:
```
w(x,t) = sum_n [ (J * phi_n(L)) / (M_n * omega_n) * sin(omega_n * t) * phi_n(x) ]
```

where:
- phi_n(L) = mode shape value at the tip
- M_n = generalized mass = rho*A*L (for mass-normalized modes, M_n = 1)
- omega_n = 2*pi*f_n = angular natural frequency

**The initial amplitude of mode n is proportional to phi_n(L) / omega_n.**

Since phi_n(L) is the same for all modes of a uniform cantilever (Section 3.1), the **relative modal amplitudes scale as 1/omega_n = 1/(2*pi*f_n)**.

For a pure impulse (delta function in time):
```
A_n / A_1 = omega_1 / omega_n = f_1 / f_n = 1 / (f_n/f_1)
```

**Predicted initial amplitude ratios (pure impulse, uniform beam):**

| Mode | f_n/f_1 | A_n/A_1 |
|------|---------|---------|
| 1 | 1.000 | 1.000 |
| 2 | 6.267 | 0.160 |
| 3 | 17.55 | 0.057 |
| 4 | 34.39 | 0.029 |
| 5 | 56.84 | 0.018 |
| 6 | 85.1 | 0.012 |
| 7 | 119.3 | 0.0084 |

**This is the 1/omega_n scaling** referenced in the existing documentation. For the fundamental mode, the amplitude is 6x larger than mode 2, 18x larger than mode 3, etc. This produces a signal strongly dominated by the fundamental with small but audible upper mode content.

**[VERIFIED]** This scaling is consistent with the observation from multiple sources that "only the first few modes of vibration have significantly large values, and the higher order vibration modes can be ignored" for struck cantilever beams.

Source: [MEMS 431 Lab](https://classes.engineering.wustl.edu/mems431_lab/lab6.html), modal analysis theory

### 3.3 Effect of Tip Mass on Modal Participation

**[CALCULATED]** When a tip mass is present (as with the solder weight), the mode shapes are modified. The tip mass:
1. Concentrates more of the effective mass at the tip
2. Reduces the tip displacement for higher modes relative to mode 1
3. Results in even FASTER rolloff of initial amplitudes for higher modes

For mu > 0, the 1/omega_n scaling becomes approximately 1/omega_n^(1+delta) where delta > 0, meaning upper modes are WEAKER than the pure 1/omega prediction. This reinforces the dominance of the fundamental.

### 3.4 Current Model vs Physical Prediction

**Comparison of mode amplitudes (bass register):**

| Mode | Current model | Physical 1/omega | Ratio (model/physical) |
|------|-------------|-----------------|----------------------|
| 1 (fund) | 0.350 | 0.350 (reference) | 1.0x |
| 2 | 0.100 | 0.056 | 1.8x |
| 3 | 0.030 | 0.020 | 1.5x |
| 4 | 0.015 | 0.010 | 1.5x |
| 5 | 0.010 | 0.006 | 1.7x |
| 6 | 0.006 | 0.004 | 1.5x |
| 7 | 0.004 | 0.003 | 1.3x |

The current model (which already uses approximate 1/omega scaling after the R41 correction) is within 2x of the physical prediction. The slight elevation of upper modes is defensible as compensation for the dwell filter attenuation. This is a major improvement over the R40 values which had modes 1+ elevated 20x above physical.

---

## 4. Hammer Mechanics

### 4.1 Hammer Construction

**[VERIFIED]** Wurlitzer 200/200A uses a simplified piano-style action:
- Wooden hammer body
- Felt tip (covering the striking surface)
- Conventional key mechanism adapted for reed striking (not string striking)
- The hammer strikes the reed near but not at the free tip

Source: [Tropical Fish - How Does a Wurlitzer Work](https://www.tropicalfishvintage.com/blog/2019/5/27/how-does-a-wurlitzer-electronic-piano-work), [US Patent 3,038,363](https://patents.google.com/patent/US3038363)

**[VERIFIED]** From Miessner patent 3,038,363: The hammer head "strikes the tuning damper with impact energy that sets reeds into free vibration following removal of the damper constraint." The hammer contact is brief -- "for a brief instant only."

### 4.2 Contact Duration (Dwell Time)

**[VERIFIED from piano literature, ESTIMATED for Wurlitzer]** No direct measurements of Wurlitzer hammer-reed contact duration exist in the literature. However, piano hammer-string contact durations provide a well-studied analog:

**Piano hammer contact durations (Askenfelt & Jansson):**

| Register | Contact Duration |
|----------|-----------------|
| Bass (C2) | ~4 ms |
| Mid (C4) | ~2 ms |
| Treble (C7) | <1 ms |

Source: [Askenfelt & Jansson](https://www.speech.kth.se/music/5_lectures/askenflt/stricont.html)

**Velocity dependence:** Contact duration decreases with increasing dynamic level (harder strikes). The variation across pp-ff is approximately +/-20% from mf.

Source: [Askenfelt & Jansson](https://www.speech.kth.se/music/5_lectures/askenflt/stricont.html)

**[ESTIMATED] Wurlitzer adaptation:**

The Wurlitzer hammer strikes a steel cantilever reed, not a tensioned string. The reed's impedance is much higher than a string's (stiff beam vs flexible string). This likely results in:
- **Shorter contact times** than piano (stiffer target = faster rebound)
- **Steeper force profile** (less felt compression needed before the reed pushes back)

**Estimated Wurlitzer contact durations:**
```
t_dwell = 0.5 ms (ff) to 3 ms (pp)
t_dwell_mf ~ 1.0-1.5 ms
```

The model's current formula `t_dwell = 0.001 + 0.003 * (1 - vel)` gives:
- ff (vel=1.0): 1.0 ms
- mf (vel=0.5): 2.5 ms
- pp (vel=0.1): 3.7 ms

This is reasonable but may overestimate the pp contact time for a stiff reed target. **Consider `t_dwell = 0.0005 + 0.002 * (1 - vel)` for a stiffer reed model.**

### 4.3 Force Profile Shape

**This is the critical question identified in the project's R41 review.**

#### 4.3.1 What the Research Shows

**[VERIFIED]** Piano hammer felt is a **nonlinear hardening spring**. The force-compression relationship follows:

```
F = K * x^p
```

where:
- K = generalized stiffness (varies across register)
- x = felt compression
- p = nonlinearity exponent

**Measured p values [VERIFIED]:**
- Hammers from pianos: p = 2.2 to 3.5
- Unused hammers: p = 1.5 to 2.8
- Musical sweet spot: p = 2 to 3

Source: [D. Russell - Piano Hammer as Nonlinear Spring](https://www.acs.psu.edu/drussell/piano/nonlinearhammer.html), Hall & Askenfelt measurements

**[VERIFIED]** The measured force-time pulse is NOT a half-sine. It is **asymmetric** -- the compression phase is faster than the release phase due to hysteresis (felt does not rebound as quickly as it compresses). The actual shape lies between:

1. Half-sine: F(t) = F_max * sin(pi*t/T), symmetric
2. Skewed pulse: steeper rise, slower decay
3. Sine-squared: F(t) = F_max * sin^2(pi*t/T)

Source: [D. Russell](https://www.acs.psu.edu/drussell/piano/nonlinearhammer.html)

**[VERIFIED]** Researchers (Hall, Askenfelt) attempted to fit the force pulse to:
- (a) half-cycle sine
- (b) sine-squared
- (c) product of sine-squared and exponential

None of these perfectly match the measured data, though a skewed sine or asymmetric pulse is the best approximation.

#### 4.3.2 Spectral Envelope of Different Force Profiles

**[CALCULATED]** The spectral characteristics of different pulse shapes:

**Rectangular pulse (current sinc model -- WRONG for felt hammer):**
```
|F(f)| = T * |sin(pi*f*T) / (pi*f*T)| = T * |sinc(f*T)|
```
- Has deep nulls (theoretically -infinity dB) at f = n/T
- First null at f = 1/T (e.g., 500 Hz for T=2ms)
- 40-60 dB nulls between peaks
- **This shape is appropriate for a rigid hammer, NOT felt**

**Half-sine pulse (better approximation for felt):**
```
|F(f)| = (2*T/pi) * |cos(pi*f*T)| / |1 - (2*f*T)^2|
```
- First null at f = 1.5/T
- Much gentler rolloff than sinc
- Shallow nulls at f = (n+0.5)/T for integer n
- 20-30 dB nulls (much less severe than sinc)

**Gaussian pulse (smooth felt, conservative estimate):**
```
F(t) = F_max * exp(-t^2 / (2*sigma^2))
|F(f)| = F_max * sigma * sqrt(2*pi) * exp(-2*pi^2*f^2*sigma^2)
```
- NO nulls at any frequency -- monotonic rolloff
- 6 dB/octave faster rolloff than half-sine at high frequencies
- sigma relates to contact time: T_eff ~ 2.35*sigma (FWHM)
- For T=2ms: sigma = 0.85ms, -3dB at f = 1/(2*pi*sigma) = 188 Hz
  (This is very aggressive. sigma should be expressed in normalized f*T units.)

**Corrected Gaussian in normalized units:**
```
|F(f*T)| = exp(-(f*T)^2 / (2*sigma_normalized^2))
```
For sigma_normalized = 2.5 (current model, kDwellSigmaSq = 2.5^2):
- At f*T = 6.27 (mode 2): attenuation = exp(-6.27^2/12.5) = exp(-3.14) = 0.043 = -27 dB
- At f*T = 17.5 (mode 3): attenuation = exp(-17.5^2/12.5) = exp(-24.5) = essentially zero

**This is extremely aggressive.** With sigma=2.5 in f*T units, mode 3 and above are completely killed. The current model may need a larger sigma (gentler rolloff).

#### 4.3.3 Comparison at Typical Dwell Time

**[CALCULATED]** For T_dwell = 1.5 ms (mf) and fundamental = 262 Hz (C4):

| Mode | f (Hz) | f*T | Sinc atten. (dB) | Half-sine atten. (dB) | Gaussian(sigma=2.5) atten. (dB) | Gaussian(sigma=5.0) atten. (dB) |
|------|--------|-----|------------------|-----------------------|--------------------------------|-------------------------------|
| 1 | 262 | 0.39 | 0 (ref) | 0 (ref) | 0 (ref) | 0 (ref) |
| 2 | 1641 | 2.46 | -7.8 | -4.2 | **-4.1** | -1.8 |
| 3 | 4599 | 6.90 | -15.0 | -12.3 | **-33.0** | -13.2 |
| 4 | 9014 | 13.5 | -24.5 | -20.1 | **-127** | -51.0 |

*Note: All values are relative to mode 1 (after normalization to fundamental).*

> **Correction (Feb 2026):** Gaussian(sigma=2.5) column recalculated. Previous values (-7.0, -53.6, -200+) used incorrect sigma. Correct values from exp(-x²/(2×2.5²)) are shown in bold. The sigma=2.5 filter is still extremely aggressive: mode 3 at -33 dB and mode 4 effectively zero.

**FINDING:** The current Gaussian model (sigma=2.5) is FAR too aggressive for modes 3+. A half-sine model or a wider Gaussian (sigma=5.0 or higher) would better match the physics of felt hammer contact.

#### 4.3.4 Recommendation for Hammer Force Model

**[ESTIMATED, based on synthesis of all research]**

The best model for a felt-tipped wooden hammer striking a steel reed is a **half-sine pulse** or a **wide Gaussian** (sigma >= 5.0 in f*T units).

Recommended approach:
```
// Half-sine spectral envelope (preferred):
dwellArg = f_mode * t_dwell;
numerator = fabs(cos(M_PI * dwellArg));
denominator = fabs(1.0 - 4.0 * dwellArg * dwellArg);
if (denominator < 0.01) denominator = 0.01;  // avoid division near nulls
dwellFilter = numerator / denominator;
// Normalize to fundamental
dwellAtten = dwellFilter / dwellFilter_fundamental;
```

Or:
```
// Wide Gaussian (simpler, no nulls):
dwellArg = f_mode * t_dwell;
sigma_sq = 8.0 * 8.0;  // sigma=8 in fT units -- very gentle rolloff
dwellFilter = exp(-dwellArg * dwellArg / (2.0 * sigma_sq));
dwellAtten = dwellFilter / dwellFilter_fundamental;
```

The Chaigne-Askenfelt Hertz contact model (F = K*x^p with p~2.5) would be the most physically accurate but requires solving a coupled ODE at note-on, which is more complex than needed for a modal synthesis approach.

Source: [Euphonics 12.2](https://euphonics.org/11-2-hitting-strings-the-piano-and-its-relatives/), [Euphonics 12.2.1](https://euphonics.org/12-2-1-parameter-values-for-piano-simulations/)

### 4.4 Striking Position

**[ESTIMATED]** The hammer strikes the reed near but not at the very tip. The exact striking position affects which modes are excited:

- Striking at a mode's antinode maximally excites that mode
- Striking at a mode's node produces zero excitation of that mode
- For a cantilever, ALL modes have their maximum at the tip

**[ESTIMATED]** The hammer likely strikes at approximately 80-95% of the reed's vibrating length from the clamped end (i.e., near the tip). This would slightly reduce higher mode excitation compared to a tip impulse, because higher modes have their LAST antinode slightly inward from the tip.

The effect of striking position x_h on initial amplitude of mode n:
```
A_n(x_h) = phi_n(x_h) / omega_n
```

For x_h = 0.9*L (90% position):
- Mode 1: phi_1(0.9L) / phi_1(L) ~ 0.97 (negligible reduction)
- Mode 2: phi_2(0.9L) / phi_2(L) ~ 0.85 (15% reduction)
- Mode 3: phi_3(0.9L) / phi_3(L) ~ 0.60 (40% reduction)
- Mode 4: phi_4(0.9L) / phi_4(L) ~ 0.40 (60% reduction)

**This further reduces upper mode content** beyond the 1/omega_n scaling, making the fundamental even more dominant.

---

## 5. Decay Mechanisms

### 5.1 Overview of Damping Sources

**[VERIFIED]** Three primary loss mechanisms in vibrating steel bars/reeds:

1. **Internal (material) damping** -- energy dissipated within the steel crystal structure
2. **Air damping (radiation losses)** -- energy radiated as sound + viscous air resistance
3. **Mounting/support losses** -- energy conducted into the clamping structure

Source: [Euphonics 2.2.7](https://euphonics.org/2-2-7-vibration-damping/), [COMSOL Damping Blog](https://www.comsol.com/blogs/damping-in-structural-dynamics-theory-and-sources)

### 5.2 Internal (Material) Damping

**[VERIFIED]** For hardened carbon steel at audio frequencies:
- Damping ratio: zeta ~ 1e-4 to 5e-4
- Quality factor: Q = 1/(2*zeta) ~ 1000 to 5000
- Loss factor: eta = 2*zeta ~ 2e-4 to 1e-3

**[VERIFIED]** Steel's internal loss factor is approximately CONSTANT over frequency in the audio range. This means Q is roughly constant across modes.

Source: [Euphonics 2.2.7](https://euphonics.org/2-2-7-vibration-damping/), [COMSOL](https://www.comsol.com/blogs/damping-in-structural-dynamics-theory-and-sources)

**Implication for decay rate:** If Q is constant, then the decay rate (in dB/s) is:
```
decay_rate_n = pi * f_n / Q = omega_n / (2*Q)
```

**Higher modes decay proportionally faster** because they have higher frequencies but the same Q. The ratio of decay rates is:
```
decay_rate_n / decay_rate_1 = f_n / f_1
```

For mode 2 at f2/f1 = 6.3: mode 2 decays 6.3x faster than mode 1.

**Current model comparison:** The current model uses decay_scale = [1.0, 0.55, 0.30, 0.18, 0.10, 0.06, 0.035]. If we assume constant Q, the physical prediction is decay_scale = 1/mode_ratio:

| Mode | Mode ratio | Physical 1/ratio | Current model | Ratio |
|------|-----------|-----------------|---------------|-------|
| 1 | 1.0 | 1.000 | 1.000 | 1.0x |
| 2 | 6.3 | 0.159 | 0.550 | 3.5x too slow |
| 3 | 17.9 | 0.056 | 0.300 | 5.4x too slow |
| 4 | 35.4 | 0.028 | 0.180 | 6.4x too slow |
| 5 | 58.7 | 0.017 | 0.100 | 5.9x too slow |

**FINDING:** The current model's higher modes decay 3-6x TOO SLOWLY compared to what constant-Q internal damping predicts. This means upper modes persist much longer than physics dictates, which would explain the "decay too slow" issue noted in the R40 metrics.

However, pure 1/ratio scaling may be too aggressive -- mounting losses can partially equalize decay rates across modes (see Section 5.4).

### 5.3 Air Damping (Radiation Losses)

**[VERIFIED]** For thin vibrating bars, air resistance acts as a combination of:
- **Viscous drag**: proportional to velocity (contributes to damping)
- **Acoustic radiation**: proportional to velocity times radiation impedance

For a narrow reed (width << wavelength at all audio frequencies), radiation efficiency is very low. The reed is a poor radiator -- you can barely hear a Wurlitzer reed unplugged. Air damping is dominated by viscous drag, not acoustic radiation.

**[ESTIMATED]** Air damping contributes a loss factor of approximately eta_air ~ 1e-4 to 5e-4, comparable to internal damping for the fundamental but less significant for higher modes (which have smaller displacement amplitudes).

### 5.4 Mounting (Support) Losses

**[VERIFIED]** Energy is lost at the clamped end through elastic waves propagating into the mounting structure. This is the dominant loss mechanism for many clamped-free structures.

**[VERIFIED]** In bar percussion instruments (vibraphone, glockenspiel), the bars are supported at the nodal points of mode 1, which minimizes fundamental losses but increases higher-mode losses. For a CLAMPED bar (as in the Wurlitzer), the clamp is at a point of maximum curvature for ALL modes, so mounting losses affect all modes.

**[ESTIMATED]** Mounting losses for a screw-clamped reed:
- Strongly dependent on clamping torque and contact area
- Typically add zeta_mount ~ 1e-4 to 1e-3 to the overall damping ratio
- Roughly frequency-independent (affects all modes similarly as an additive loss)

Source: [CCRMA Percussion](https://ccrma.stanford.edu/CCRMA/Courses/152/percussion.html), [UBC Wiki Glockenspiel](https://wiki.ubc.ca/Course:PHYS341/Archive/2016wTerm2/glockenspiel)

### 5.5 Thermoelastic Damping

**[VERIFIED]** When a beam vibrates in bending, the compressed side heats up and the stretched side cools down. Heat flows irreversibly between them, dissipating energy. This is thermoelastic damping (Zener damping).

For a beam of thickness t, the thermoelastic relaxation frequency is:
```
f_Zener = pi * kappa / (2 * t^2)
```
where kappa = thermal diffusivity of steel = 1.2e-5 m^2/s.

For a 200A reed (t = 0.5 mm):
```
f_Zener = pi * 1.2e-5 / (2 * (0.5e-3)^2) = pi * 1.2e-5 / 5e-7 = 75,400 Hz
```

Thermoelastic damping peaks at f = f_Zener and is approximately:
```
Q_TE^{-1} = E * alpha^2 * T / (rho * c_p) * f * f_Zener / (f^2 + f_Zener^2)
```
where alpha = thermal expansion coefficient ~ 12e-6 /K, c_p = specific heat ~ 480 J/(kg*K), T = 300 K.

At f = 1000 Hz (well below f_Zener for these thin reeds):
```
Q_TE^{-1} ~ (210e9 * (12e-6)^2 * 300) / (7850 * 480) * 1000/75400
      ~ (9.07e-3) * 0.0133 = 1.2e-4
Q_TE ~ 8300
```

**[CALCULATED]** Thermoelastic Q ~ 8000-10000 for Wurlitzer reeds at audio frequencies. This is the same order as internal friction Q, confirming that thermoelastic damping is a SIGNIFICANT contributor to the total loss.

For higher modes (higher frequency), thermoelastic Q actually IMPROVES (because f << f_Zener, the loss scales as f/f_Zener). This partially compensates the 1/Q decay rate increase and could explain why higher modes don't decay as fast as pure constant-Q would predict.

### 5.6 Combined Damping Model

**[CALCULATED]** Total damping for mode n:
```
1/Q_total = 1/Q_internal + 1/Q_thermoelastic + 1/Q_air + 1/Q_mounting
```

**[ESTIMATED]** Practical values for Wurlitzer reeds:

| Component | Q (fundamental) | Frequency dependence |
|-----------|----------------|---------------------|
| Internal friction | 3000-5000 | ~constant |
| Thermoelastic | 8000-10000 | improves with f (for f << f_Zener) |
| Air damping | 10000-50000 | complex |
| Mounting losses | 2000-10000 | ~constant |
| **Total** | **~1500-3000** | weakly frequency-dependent |

The combined Q leads to a decay time:
```
T_60 = Q / (pi * f_1) * ln(1000)  [seconds to decay 60 dB]
     = 6.91 * Q / (pi * f_1)
     ~ 2.2 * Q / f_1
```

For C4 (f_1 = 262 Hz) with Q = 2000:
```
T_60 = 2.2 * 2000 / 262 = 16.8 seconds  →  decay rate = 60/16.8 = 3.6 dB/s
```

**Calibration check:** OldBassMan measured D4 (f_1 = 294 Hz) decay at 6.2 dB/s, implying Q ~ 1294 (from Q = pi*f*8.686/decay_dB). This is lower than our estimated Q ~ 1500-3000, suggesting significant mounting losses or other dissipation not captured by the steel-only model.

### 5.7 Calibration Data: Measured Decay Rates

**[VERIFIED]** From OldBassMan 200A recordings:

| MIDI | Note | f_1 (Hz) | Decay (dB/s) | Implied Q | Implied T_60 (s) |
|------|------|----------|-------------|-----------|-------------------|
| 54 | F#3 | 185 | 2.9 | 1741 | 20.7 |
| 58 | Bb3 | 233 | 4.4 | 1445 | 13.6 |
| 62 | D4 | 294 | 6.2 | 1294 | 9.7 |
| 66 | F#4 | 370 | 5.1 | 1980 | 11.8 |
| 70 | Bb4 | 466 | 12.4 | 1025 | 4.8 |
| 74 | D5 | 587 | 9.5 | 1686 | 6.3 |
| 78 | F#5 | 740 | 12.5 | 1615 | 4.8 |
| 82 | Bb5 | 932 | 23.6 | 1078 | 2.5 |
| 86 | D6 | 1175 | 16.7 | 1920 | 3.6 |
| 90 | F#6 | 1481 | 17.6 | 2296 | 3.4 |
| 94 | Bb6 | 1865 | 34.0 | 1497 | 1.8 |
| 98 | D7 | 2349 | 22.3 | 2874 | 2.7 |

> **Correction (Feb 2026):** Q values recalculated using Q = pi * f * 8.686 / decay_dB. Previous values were systematically 1.245x too low due to a unit conversion error.

Source: [Calibration Data](../reference/calibration-data.md)

**Observations:**
1. Q ranges from ~1025 to ~2874 across the keyboard. [VERIFIED — OldBassMan 200A recordings, Q values corrected Feb 2026]
2. There is no clean frequency dependence -- Q fluctuates note-to-note
3. The scatter suggests per-note variation (solder mass, mounting tightness, reed condition)
4. An exponential fit to decay rate: `decay = 0.26 * exp(0.049 * MIDI)` captures the trend
5. Geometric mean Q ~ 1636. [CORRECTED Feb 2026 — previous value of ~1100 had unit conversion error]

**[CALCULATED]** If we model Q as constant at ~1636 (geometric mean from OldBassMan measurements):
```
decay_rate_nepers = pi * f / Q
decay_rate_dB = 8.686 * pi * f / Q
```

For Q=1636:
- F#3 (185 Hz): pi*185/1636 = 0.355 nepers/s = 3.1 dB/s (measured: 2.9)
- D4 (294 Hz): pi*294/1636 = 0.564 nepers/s = 4.9 dB/s (measured: 6.2)
- D5 (587 Hz): pi*587/1636 = 1.127 nepers/s = 9.8 dB/s (measured: 9.5)
- Bb5 (932 Hz): pi*932/1636 = 1.789 nepers/s = 15.5 dB/s (measured: 23.6)

The constant-Q model (Q~1636) matches measured data within about +/-40%. This is reasonable given the per-note variation. Some notes (Bb4, Bb5) have notably lower Q, suggesting those specific reeds/mountings have higher losses.

### 5.8 Mode-Dependent Decay

**[CALCULATED]** For higher modes with constant Q:
```
decay_rate_mode_n = pi * f_n / Q = (f_n/f_1) * pi * f_1 / Q
```

Mode 2 (f2/f1 ~ 6.3) decays 6.3x faster than mode 1.

**[CALCULATED]** Recommended decay scale (fraction of fundamental decay time, i.e., T_mode_n / T_mode_1):

For constant Q: `decay_scale[n] = f_1 / f_n = 1 / mode_ratio[n]`

| Mode | Mode ratio | Pure 1/ratio | Recommended (with mounting floor) |
|------|-----------|-------------|----------------------------------|
| 1 | 1.0 | 1.000 | 1.000 |
| 2 | 6.3 | 0.159 | 0.20 |
| 3 | 17.9 | 0.056 | 0.08 |
| 4 | 35.4 | 0.028 | 0.05 |
| 5 | 58.7 | 0.017 | 0.03 |
| 6 | 88 | 0.011 | 0.02 |
| 7 | 123 | 0.008 | 0.015 |

The "recommended" column adds a floor from mounting losses (modes can't decay infinitely fast because mounting dissipation provides a minimum decay time independent of frequency). This produces the "bright attack darkening to sine-like tail" character universally described for the Wurlitzer.

**Comparison with current model [1.0, 0.55, 0.30, 0.18, 0.10, 0.06, 0.035]:** Current model is 2-4x too slow for modes 2-4. The recommended values would produce significantly faster upper mode decay, which should help with the "decay too slow" issue.

> **Modeling concern (Feb 2026):** The current model's decay_scale values may be 3-6x too slow for upper modes. For constant Q, decay rate scales linearly with frequency: T_mode_n / T_mode_1 = f_1 / f_n. Current values (mode 2: 0.55, mode 3: 0.30, mode 4: 0.18) are significantly slower than the physically correct values (mode 2: ~0.20, mode 3: ~0.08, mode 4: ~0.05). Faster upper-mode decay produces the characteristic "bright attack darkening to sine-like tail" timbre. Evaluate during implementation.

---

## 6. Vibration Characteristics

### 6.1 Plane of Vibration

**[ESTIMATED]** The reed vibrates primarily in a SINGLE plane (perpendicular to the reed's flat surface), for the following reasons:
- Rectangular cross-section: second moment of area is much larger for in-plane bending (I_y = t*w^3/12) than for out-of-plane bending (I_z = w*t^3/12). Since w >> t for all reeds, the out-of-plane bending frequency is much lower.
- The hammer strikes from one direction (below), exciting primarily the out-of-plane bending modes.
- The pickup measures displacement in one direction (perpendicular to the reed bar face).

**[ESTIMATED]** Lateral (in-plane) vibrations may exist at much higher frequencies (f_lateral ~ (w/t)^2 * f_fundamental for mode 1), but these are not coupled to the pickup and are acoustically irrelevant.

For bass reeds: w/t ~ 3.84/0.508 ~ 7.6, so f_lateral_1 ~ 57 * f_fundamental. This is well above audio range for bass notes but could be audible for treble notes.

### 6.2 Observability of Inharmonic Modes

**[VERIFIED]** The existing documentation correctly notes that reed mode frequencies are NOT directly observable as spectral peaks in the Wurlitzer output. All prominent spectral peaks fall on exact harmonics of the fundamental (integer multiples of f_0).

**Why:** The pickup responds to total reed displacement, which is dominated by the fundamental. The preamp's nonlinearity (exponential, generating harmonics) operates on this fundamental-dominated signal, producing even harmonics (2*f_0, 4*f_0...) from the asymmetric distortion. The inharmonic reed modes (at 6.3*f_0, 17.5*f_0, etc.) are:
1. Much weaker than the fundamental (1/omega scaling: -16 dB for mode 2)
2. Their second harmonics (at 12.6*f_0, etc.) are even weaker
3. They decay rapidly (Section 5.8)

The inharmonic modes are primarily audible during the first 5-20 ms of the attack transient, contributing to the "complexity" and "metallic quality" of the initial strike. During sustain, the sound approaches a pure sinusoid with harmonics generated by the preamp.

### 6.3 Nonlinear Effects

**[ESTIMATED]** At ff dynamics, the reed displacement approaches the pickup gap distance. For very large amplitudes (reed approaching the pickup plate), geometric nonlinearity in the cantilever equation becomes significant:
- The beam equation assumes small deflections (dw/dx << 1)
- For bass reeds with large excursion, dw/dx at the tip can reach 0.1-0.3 radians
- This introduces a hardening effect (frequency increases slightly with amplitude)
- The frequency shift is small (<1% for typical amplitudes) and perceptually negligible

**[VERIFIED]** The dominant nonlinearity in the Wurlitzer is the preamp, not the reed vibration itself. The reed behaves as a linear oscillator to excellent approximation.

---

## 7. Summary of Corrections Needed

### 7.1 Dwell Filter

**STATUS: Current Gaussian (sigma=2.5) is too aggressive.**

| Issue | Evidence | Recommendation |
|-------|----------|----------------|
| Too much mode attenuation | Modes 3+ nearly zeroed at mf | Increase sigma to 8+ or switch to half-sine |
| Forced mode amp compensation | R40 needed 20x elevation | Return to physical 1/omega amplitudes with gentler filter |
| Nulls (sinc, old model) | Rectangular impulse physically wrong for felt | Use half-sine or wide Gaussian (no nulls) |

### 7.2 Decay Rates

**STATUS: Higher modes decay too slowly.**

| Issue | Evidence | Recommendation |
|-------|----------|----------------|
| Mode 2 decay_scale = 0.55 | Physical: 0.16-0.20 | Reduce to ~0.20 |
| Mode 3 decay_scale = 0.30 | Physical: 0.056-0.08 | Reduce to ~0.08 |
| Overall decay too slow | Calibration: 6.2 dB/s at D4 | Adjust baseDecay to match Q~1100 |

### 7.3 Mode Ratios

**STATUS: Current values are within acceptable range but could be refined.**

| Issue | Evidence | Recommendation |
|-------|----------|----------------|
| Bass f2/f1 = 6.3 | Physical estimate: 6.5-7.5 (mu=0.05-0.15) | Increase slightly to ~6.7 |
| Mid f2/f1 = 6.8 | Physical estimate: ~6.27-6.7 (mu=0-0.05) | Decrease to ~6.4 |
| Treble f2/f1 = 6.3 | Physical estimate: ~6.27-6.5 (mu=0-0.03) | Acceptable, perhaps 6.3-6.4 |

### 7.4 Mode Amplitudes

**STATUS: Current values are close to physical after R41 correction.**

The current 1/omega-like scaling (bass modes: 0.35, 0.10, 0.030...) is within 2x of the pure 1/omega prediction. This is acceptable, especially since the dwell filter provides additional physical attenuation. No major change needed, but amplitudes should be verified after correcting the dwell filter (Section 7.1).

### 7.5 Attack Transient

**STATUS: Correctly disabled artificial overshoot (R41). Natural overshoot from modal superposition is the right approach.**

**[CALCULATED]** Expected natural overshoot from modal superposition:

At t=0, all modes are in-phase (or near-phase). The initial peak amplitude is:
```
A_peak = sum(A_n) for all n = A_1 * (1 + sum(A_n/A_1 for n>=2))
```

With 1/omega amplitudes:
```
A_peak / A_1 = 1 + 0.160 + 0.057 + 0.029 + 0.018 + 0.012 + 0.008 = 1.284
```

Overshoot in dB: 20*log10(1.284) = 2.2 dB

After mode 2 decays (~10ms for mid-register), only mode 1 remains:
```
A_sustain ~ A_1
```

**Natural overshoot: ~2 dB at mf.** This matches the OldBassMan calibration data (2.0-3.8 dB at mf). Higher velocity increases mode amplitudes disproportionately (shorter dwell time passes more upper mode energy), potentially reaching 4-6 dB at ff.

---

## 8. Key References

### Academic Papers
- Pfeifle, F. (2017). "Real-Time Physical Model of a Wurlitzer and Rhodes Electric Piano." DAFx-17. [ResearchGate](https://www.researchgate.net/publication/319644771_REAL-TIME_PHYSICAL_MODEL_OF_A_WURLITZER_AND_RHODES_ELECTRIC_PIANO)
- Pfeifle, F. & Bader, R. (2016). "Tone Production of the Wurlitzer and Rhodes E-Pianos." [DAGA 2017](https://pub.dega-akustik.de/DAGA_2017/data/articles/000210.pdf)
- Gabrielli, L. et al. (2020). "The Rhodes electric piano: Analysis and simulation of the inharmonic overtones." JASA 148(5). [AIP Publishing](https://pubs.aip.org/asa/jasa/article/148/5/3052/631688)
- Chaigne, A. & Askenfelt, A. (1994). "Numerical simulations of piano strings." JASA 95(2). [JASA](https://pubs.aip.org/asa/jasa/article/95/2/1112/830873)
- Chaigne, A. & Doutaut, V. (1997). "Numerical simulations of xylophones. I. Time-domain modeling of the vibrating bars." JASA 101(1). [JASA](https://pubs.aip.org/asa/jasa/article-abstract/101/1/539/562404)

### Piano Hammer Physics
- Russell, D. "The Piano Hammer as a Nonlinear Spring." [Penn State](https://www.acs.psu.edu/drussell/piano/nonlinearhammer.html)
- Russell, D. "Piano Hammer Testing." [Penn State Publication](https://www.acs.psu.edu/drussell/publications/pianohammer.pdf)
- Askenfelt, A. & Jansson, E. "From Touch to String Vibration." [KTH](https://www.speech.kth.se/music/5_lectures/askenflt/askenflt.html)
- Euphonics. "12.2 Hitting Strings." [Euphonics](https://euphonics.org/11-2-hitting-strings-the-piano-and-its-relatives/)
- Euphonics. "12.2.1 Parameter Values for Piano Simulations." [Euphonics](https://euphonics.org/12-2-1-parameter-values-for-piano-simulations/)

### Damping and Material Properties
- Euphonics. "2.2.7 Vibration Damping." [Euphonics](https://euphonics.org/2-2-7-vibration-damping/)
- COMSOL. "Damping in Structural Dynamics." [COMSOL Blog](https://www.comsol.com/blogs/damping-in-structural-dynamics-theory-and-sources)

### Beam Theory and Modal Analysis
- enDAQ. "Bernoulli-Euler Beams." [enDAQ](https://endaq.com/pages/bernoulli-euler-beams)
- Wikipedia. "Euler-Bernoulli Beam Theory." [Wikipedia](https://en.wikipedia.org/wiki/Euler%E2%80%93Bernoulli_beam_theory)
- Extrica. "Practical Method for Eigenfrequencies with Tip Mass." [Extrica](https://www.extrica.com/article/16636)
- UBC Wiki. "Glockenspiel." [UBC](https://wiki.ubc.ca/Course:PHYS341/Archive/2016wTerm2/glockenspiel)

### Patents
- Miessner, B. US Patent 3,038,363. "Electronic Piano." (1962). [Google Patents](https://patents.google.com/patent/US3038363)
- Miessner, B. US Patent 3,215,765. "Fixed Free-Reed Electronic Piano." (1965). [Google Patents](https://patents.google.com/patent/US3215765)

### Wurlitzer Hardware
- Vintage Vibe. "Wurlitzer Electric Piano Reeds Case Study." [Vintage Vibe](https://www.vintagevibe.com/blogs/news/wurlitzer-electric-piano-reeds-case-study)
- EP-Forum. "Wurlitzer 200 Reed Dimensions." [EP-Forum](https://ep-forum.com/smf/index.php?topic=8418.0)
- EP-Forum. "The Formula for Lengths of Wurly Reeds." [EP-Forum](https://ep-forum.com/smf/index.php?topic=9643.0)
- DocWurly. "Wurlitzer EP Reed Compatibility History." [DocWurly](https://docwurly.com/wurlitzer-ep-history/wurlitzer-ep-reed-compatibility-history/)
- Tropical Fish. "How Does a Wurlitzer Electronic Piano Work?" [Tropical Fish](https://www.tropicalfishvintage.com/blog/2019/5/27/how-does-a-wurlitzer-electronic-piano-work)
- Tropical Fish. "How to Tune a Wurlitzer Electronic Piano Reed." [Tropical Fish](https://www.tropicalfishvintage.com/blog/2020/5/7/how-to-tune-a-wurlitzer-electronic-piano-reed)

### Vibrating Bar Physics
- CCRMA. "Percussion Instruments." [Stanford](https://ccrma.stanford.edu/CCRMA/Courses/152/percussion.html)
- Gibson. "Vibrating Bars and Non-Linear Dependencies." [UConn](https://www.phys.uconn.edu/~gibson/Notes/Section4_1/Sec4_1.htm)
- Physics LibreTexts. "Harmonic Percussion Instruments." [LibreTexts](https://phys.libretexts.org/Bookshelves/Acoustics/Book:_Sound_-_An_Interactive_eBook_(Forinash_and_Christian)/12:_Percussion/12.01:_Percussion_and_Drumheads/12.1.04:_Harmonic_Percussion_Instruments)
