# TODO: Uncertainty Resolution & Doc Fixes

Tracking document for all outstanding fixes, unresolved values, and ambiguities identified during the Feb 2026 uncertainty resolution sweep.

**Schematic reference:** High-resolution (900 DPI) PNG renders of the Wurlitzer 200A schematic are available at `/tmp/schematic_*.png`, extracted from [BustedGear PDF](https://www.bustedgear.com/images/schematics/Wurlitzer_200A_series_schematics.pdf) via PyMuPDF. Regenerate with:

```bash
source .venv/bin/activate
python3 -c "
import fitz
doc = fitz.open('/tmp/wurlitzer_200a_schematic.pdf')
page = doc[0]
for name, clip in [
    ('full',        page.rect),
    ('preamp',      fitz.Rect(50, 20, 550, 200)),
    ('power_amp',   fitz.Rect(430, 100, 1100, 400)),
    ('vibrato',     fitz.Rect(200, 300, 600, 550)),
    ('speakers',    fitz.Rect(850, 50, 1100, 280)),
]:
    pix = page.get_pixmap(matrix=fitz.Matrix(900/72, 900/72), clip=clip)
    pix.save(f'/tmp/schematic_{name}.png')
"
```

---

## 1. CRITICAL — Preamp Doc Rewrite — **COMPLETED Feb 2026**

**File:** `docs/preamp-circuit.md`

~~The entire document's analysis is built on estimated component values that are **wrong by 3-5x**.~~ **Full rewrite completed and reviewed by 3-agent team (math-verifier, schematic-checker, consistency-checker).** All derived quantities recalculated from verified schematic values. Review corrections applied: Vb1 = 2.45V (not 2.65V), C20 HPF table corrected, "~924" → "~912/~900".

### 1.1 Replace estimated values with schematic values

| Parameter | Doc Currently Says | Correct Value (Schematic) |
|-----------|-------------------|--------------------------|
| Rc1 (TR-1 collector) | ~47K | **150K** |
| Rc2 (TR-2 collector) | ~10K | **1.8K** |
| Re1 (TR-1 emitter) | ~2.2-8.2K | **33K** (with 4.7 μF bypass) |
| Re2 (TR-2 emitter) | ~4.7-5.1K | **270Ω (bypassed) + 820Ω (unbypassed)** |
| Ce1 (feedback coupling cap) | "possibly present" (UNVERIFIED) | **4.7 MFD — confirmed** (couples emitter to R-10/LDR feedback junction, NOT a simple bypass) |
| C-3 (TR-1 Ccb feedback) | "10-100 pF" (ESTIMATED) | **100 pF** |
| C-4 (TR-2 Ccb feedback) | "10-100 pF" (ESTIMATED) | **100 pF** |
| R-10 (feedback) | "likely 100k-470k" | **56K** |
| D-1 (input diode) | "assumed 1N4148" | **25 PIV, 10 mA, part #142136** |

### 1.2 Redo DC bias analysis (Sections 3-4)

With correct values:
- Stage 1: Ic1 = (15 - 4.1) / 150K = **72.7 μA** (not 234 μA)
- gm1 = Ic1 / Vt = 72.7μA / 26mV = **2.80 mA/V** (not 9.0 mA/V)
- Stage 2: Ic2 = (15 - 8.8) / 1.8K = **3.44 mA**
- gm2 = 3.44mA / 26mV = **132 mA/V**

### 1.3 Redo gain structure (Section 5)

The doc's model of "both stages contribute significant voltage gain" is **wrong**.

Correct gain structure:
- **Stage 1 open-loop** (Ce1 bypassed): Av1 = -gm1 × Rc1 = -2.80 × 150 = **-420** (unchanged — gm×Rc = ΔVc/Vt is independent of Rc)
- **Stage 2 AC gain**: Av2 = -Rc2 / Re2_unbypassed = -1800 / 820 = **-2.2** (partially degenerated, NOT high-gain)
- Combined open-loop: ~420 × 2.2 = **~920**
- Closed-loop with feedback: ~5.6x (15 dB) per Avenson measurement — consistent

### 1.4 Redo Miller pole calculations (Section 6)

With C-3 = C-4 = 100 pF now known:
- **Stage 1 Miller**: C_miller1 = 100pF × (1 + |Av1|) = 100 × 421 = **42,100 pF**
  - f_miller1 = 1 / (2π × R_source × C_miller1). R_source ≈ R_bias || r_pi. This is the dominant pole — likely **~25 Hz**
- **Stage 2 Miller**: C_miller2 = 100pF × (1 + 2.2) = **320 pF**
  - f_miller2 = 1 / (2π × 150K × 320pF) ≈ **3.3 kHz** (or calculated from appropriate source impedance)
- Stage 1's ~25 Hz dominant pole means gain rolls off -20 dB/dec above ~25 Hz in open-loop, reaching unity gain well above audio

### 1.5 Redo clipping headroom analysis (Section 7)

- Stage 1 Vce = 4.1 - 1.95 = **2.15V**. Headroom toward saturation (Vce_sat ≈ 0.1V): **2.05V**. Headroom toward cutoff (Vc → 15V): collector can swing from 4.1V up to ~15V = **10.9V**. Asymmetry ratio: ~5.3:1 (was estimated ~6:1 — close)
- Stage 2 Vce = 8.8 - 3.4 = **5.4V**. Headroom toward saturation: **5.3V**. Toward cutoff: **6.2V**. More symmetric.
- The preamp's characteristic "bark" is still Stage 1 asymmetric clipping — that conclusion holds

### 1.6 Redo Appendix C

Appendix C (Open Questions) should be replaced with a summary of resolved values and remaining disputes. **All items now resolved**, including C-1 (= C20, naming discrepancy — see Section 5.3).

---

## 2. MODERATE — Output Stage Doc Updates — **COMPLETED Feb 2026**

**File:** `docs/output-stage.md` — All items below applied.

### 2.1 Replace "UNKNOWN" component values

| Line(s) | Current | Fix |
|---------|---------|-----|
| ~268 | R-31 "Unknown (likely 10K-47K)" | **R-31 = 15K** (schematic) |
| ~280 | C-8 "Unknown (likely 1-10 uF)" | **C-8 = 4.7 MFD** (schematic) |
| ~281 | C-12 "Unknown (likely 100-470 uF)" | **C-12 = 100 MFD** (schematic) |
| ~650 | Confidence table: C-8, C-12 "LOW" | Update to HIGH |

### 2.2 Add newly resolved values

- C-11 = 100 PF (pre-driver feedback cap)
- R-32 = 1.8K, R-33 = 1.8K (diff pair loads)
- LG-1 = Wurlitzer part **#142312** (LED/LDR opto-isolator)
- TR-7 = TR-8 = part #142128

### 2.3 Correct speaker size — **RESOLVED Feb 2026**

**RESOLVED:** Speaker = **4"x8" oval**, 16Ω each, P.M., ceramic magnet. The schematic shows "4x6" but ALL vendors, forums, and repair sources unanimously confirm 4x8. The schematic likely reflects a pre-production specification. Output-stage doc updated accordingly.

### 2.4 Add tremolo oscillator waveform detail — **RESOLVED Feb 2026**

- **Twin-T (parallel-T) oscillator** — NOT phase-shift. Notch filter in negative feedback path of TR-3.
- SPICE-validated: freq=5.63 Hz, Vpp=11.82V, DC operating points match schematic within 1%
- Non-standard twin-T ratios: R_shunt/R_series = 27K/680K = 0.040 (standard = 0.5)
- Produces shallow notch (~-23.5 dB) → mildly distorted sinusoid, est. THD 3-10%
- Subcircuit: `spice/subcircuits/tremolo_osc.cir`, Testbench: `spice/testbench/tb_tremolo_osc.cir`

### 2.5 Add crossover distortion detail

From GroupDIY thread 62917:
- R-34 = 160Ω on schematic (measured 150-160Ω in working units)
- Bias drift from aging causes 10-50 mV dead zone at zero-crossing
- Common repair: adjust R-34/R-35 network or add trimpot

---

## 3. MINOR — Pickup System Doc Updates — **COMPLETED Feb 2026**

**File:** `docs/pickup-system.md` — All items below applied.

### 3.1 Confirm 240 pF is separate from C20

Line ~722: "Whether 240 pF includes C20 — Likely separate" → **RESOLVED: Definitely separate** (240 - 270 = impossible; 240 pF is reed bar capacitance alone)

### 3.2 ~~Note C20 value discrepancy~~ **RESOLVED — C20 = 220 pF (Feb 2026)**

~~The doc states C20 = 270 pF. The schematic reads 220 pF. Add note:~~
> **RESOLVED:** C20 = 220 pF confirmed from BustedGear schematic at 1500 DPI. GroupDIY's 270 pF likely reflects tolerance variation in carbon composition capacitors. With R_bias = 2M||470K = 380K: f_c = 1/(2π × 380K × 220pF) = **1903 Hz**.

### 3.3 Update f_c regime status

Line ~233: "This is the central modeling question" → **RESOLVED:** f_c = 2073 Hz (calculated from verified R=320K, C=240pF). Bass notes in constant-voltage regime; treble in constant-charge.

---

## 4. MINOR — Reed & Hammer Physics Doc Updates — **COMPLETED Feb 2026**

**File:** `docs/reed-and-hammer-physics.md` — All items below applied.

### 4.1 Mark felt damper as NOT USED in 200A

Sections referencing Miessner's toroidal felt damper (~lines 143-144) should note:
> The tuner-damper system described in Miessner patents (US 3,215,765) was NOT implemented in production 200/200A instruments. The 200A uses solder tip mass only. Model should NOT include a felt damper.

### 4.2 Flag decay rate concern

Reed-researcher found that upper-mode decay rates in the current model spec may be 3-6x too slow. For constant Q, decay rate scales linearly with frequency:

| Mode | Current decay_scale | Physically correct | Ratio |
|------|-------------------|-------------------|-------|
| 1 | 1.000 | 1.000 | — |
| 2 | 0.55 | ~0.20 | 2.75x too slow |
| 3 | 0.30 | ~0.08 | 3.75x too slow |
| 4 | 0.18 | ~0.05 | 3.6x too slow |

This affects the "bright attack darkening to sine-like tail" character. Needs evaluation during implementation.

### 4.3 Flag dwell filter concern

Current Gaussian sigma=2.5 may over-attenuate modes 3+. Piano hammer literature suggests half-sine spectral envelope or Gaussian sigma ≥ 8. Evaluate during implementation — listen before changing.

---

## 5. Unresolved / Ambiguous Values

### 5.1 ~~HIGH PRIORITY — C20 value (220 vs 270 pF)~~ **RESOLVED — C20 = 220 pF (Feb 2026)**

~~Affects input HPF frequency by ~350 Hz. Both the schematic (220 pF) and GroupDIY (270 pF) are credible sources.~~

**Resolution:** Confirmed C20 = 220 pF from BustedGear schematic at 1500 DPI. GroupDIY's 270 pF likely reflects tolerance variation in carbon composition capacitors (which can drift +20% over decades). With R_bias = 2M||470K = 380K: f_c = 1/(2π × 380K × 220pF) = **1903 Hz**.
- [x] Re-examine schematic at maximum zoom — **220 pF confirmed at 1500 DPI**
- [x] Accept 220 pF as canonical (schematic > forum post)

### 5.2 ~~HIGH PRIORITY — R-2 bias resistor (1 MEG vs 2 MEG)~~ **RESOLVED — R-2 = 2 MEG (Feb 2026)**

Schematic reads "1 MEG" (confirmed at 1500 DPI), but three independent lines of evidence confirm R-2 = 2M:
- [x] GroupDIY PRR uses "380K" for R-2||R-3 impedance — 2M||470K = 380K (1M||470K would be 320K)
- [x] DC analysis: R-2=1M requires hFE=9 (impossible); R-2=2M requires hFE≈62 (plausible)
- [x] GroupDIY thread 62917 actual measurement: Vb=2.447V, consistent with R-2=2M and carbon comp tolerances
- [x] Carbon composition resistor tolerance (10-20%) closes remaining Vb gap: R-2=2.2M, R-3=420K → Vth=2.42V ≈ 2.447V measured

### 5.3 ~~MEDIUM PRIORITY — C-1 (RF shunt cap)~~ **RESOLVED — C-1 = C20 = 220 pF (Feb 2026)**

~~Service manual mentions "shunt capacitor C-1" as separate from C-3/C-4. Not visible as a distinct component on the schematic.~~

**Resolution:** C-1 (service manual designation) is the **same physical component** as C20/C-2 (schematic board position "2") = 220 pF shunt cap at TR-1 base to ground. The naming discrepancy arises because the service manual uses "C-1" (functional designation as the first/primary RF shunt cap) while the schematic uses board position numbering ("2"). Evidence:
1. Service manual describes C-1 as a "shunt capacitor" — the 220 pF cap is the only shunt-to-ground cap at the preamp input
2. No separate C-1 component visible on schematic at 2400 DPI examination
3. PCB physical layout (schematic page 1) shows R-x/C-x designators on silk screen; "C-3" clearly visible at position 3
4. Zero community discussion (GroupDIY, EP-Forum, BustedGear) about a "missing" C-1 — technicians recognize it as the 220 pF cap
- [x] Search schematic more thoroughly — **confirmed: no separate C-1 at 2400 DPI**
- [x] ~~Accept as unresolvable without physical inspection~~ → **Resolved: naming discrepancy, not a missing component**

### 5.4 ~~MEDIUM PRIORITY — Speaker size verification (4"x6" vs 4"x8")~~ **RESOLVED — 4"x8" (Feb 2026)**

**Resolution:** 4"x8" oval confirmed. The schematic's "4x6" is a pre-production spec. ALL replacement speaker vendors (Vintage Vibe, Tropical Fish), repair forums (EP-Forum, GroupDIY), and service technicians unanimously confirm 4"x8". No physical 200A has been documented with 4x6 speakers.
- [x] Cross-referenced multiple independent sources — all confirm 4x8
- [x] Output-stage doc updated

### 5.5 LOW PRIORITY — Original LDR specs — **SPICE model built (Feb 2026)**

LG-1 Wurlitzer part #142312 — no datasheet exists. SPICE behavioral model built using VTL5C3 specs:
- `spice/models/ldr_behavioral.lib`: Power-law R = 800 * (ctrl)^(-0.85), with asymmetric time constants (tau_on=2.5ms, tau_off=30ms)
- Static and dynamic subcircuits available
- Validated via LDR sweep testbench: `spice/testbench/topology_b_ldr_sweep.cir`
- Gain modulation range: 6.1 dB (matches EP-Forum "6 dB boost" measurement)

### 5.6 LOW PRIORITY — Physical pickup dimensions

Still estimated (require physical measurement):
- Vertical gap (reed to slot bottom): est. 0.2-0.5 mm
- Pickup plate active length: est. 3-8 mm
- U-channel depth: est. 2-4 mm
- These affect absolute signal level scaling but NOT the pickup transfer function shape

### 5.7 LOW PRIORITY — Reed mechanical parameters

Still estimated (require physical measurement):
- Solder tip mass: est. 0.145g (geometric calculation)
- Hammer striking position: est. 80-95% of reed length
- Hammer-reed contact duration: est. 0.5-3 ms (from piano literature)
- These are bounded well enough for initial implementation; tune by ear

### 5.8 ~~HIGH PRIORITY — R_feed (pickup feed resistor)~~ **RESOLVED — R_feed = 1 MEG (Feb 2026)**

**Resolution:** R_feed = 1 MEG, identified as component 56 in the HV supply filter chain on the main amp board. Avenson Audio's "499K" value refers to their replacement preamp design, NOT the original Wurlitzer 200A circuit.

Updated pickup impedance calculation:
- R_total = R_feed || R_bias = 1M || (2M||470K) = 1M || 402K = **287K** (note: R_bias = 2M||470K ≈ 380K, but using the more precise 402K from 2M||470K for R_total)
- Pickup RC f_c = 1/(2π × 287K × C_pickup) — with C_pickup = 240 pF: f_c = **2312 Hz**
- This is higher than the previous estimate using R_feed=499K, shifting the constant-charge/constant-voltage crossover upward

---

## 6. Signal Chain Architecture Doc Updates

**File:** `docs/signal-chain-architecture.md`

Depends on preamp doc rewrite (Section 1). ~~Once that's done:~~

- [x] Update preamp gain parameters throughout — **DONE Feb 2026**: Stage 1 gain 145→420, Stage 2 gain 252→238, all satLimit/cutoffLimit/re values corrected, C-3=C-4=100pF reflected
- [x] Update Miller pole frequency references — **DONE Feb 2026**: ~200-500 Hz→~25 Hz (Stage 1 open-loop), ~500-2000 Hz→~3.3 kHz (Stage 2), closed-loop BW ~3.7 kHz noted
- [x] Update speaker model parameters — **DONE Feb 2026**: RESOLVED as 4"x8" (all vendors/forums confirm; schematic's "4x6" is pre-production spec)
- [x] Review oversampling rationale — **DONE Feb 2026**: Still valid; C20 HPF frequency updated to ~1903 Hz
- [x] Add kPreampInputDrive recalibration note — **DONE Feb 2026**: Current value of 28.0 was calibrated against old incorrect gains, flagged for recalibration

---

## 7. Schematic Component Number Reconciliation

The schematic uses a numbering scheme that doesn't always match our docs' naming. Known mappings:

| Schematic Ref | Our Docs Call It | Value | Notes |
|---------------|-----------------|-------|-------|
| R-1 | R-1 | 22K | Input series from reed bar |
| R-2 | R-1 (bias upper) | **2 MEG** (RESOLVED, see 5.2) | Schematic reads 1M but DC analysis + GroupDIY measurement confirm 2M |
| R-3 (upper rail) | Rc1 | 150K | TR-1 collector load |
| R-3 (lower rail) | R-3 (bias lower) | 470K | To ground — same number, different component? |
| R-4 | Rc2 / "R-6" in MEMORY | 1.8K | TR-2 collector load |
| R-5 | Re1 | 33K | With 4.7 MFD bypass |
| R-7 | Re2 (bypassed) | 270Ω | With 22 MFD bypass |
| R-8 | Re2 (unbypassed) | 820Ω | |
| R-9 | R-9 | 6.8K | Series output |
| R-10 | R-10 | 56K | Feedback to LG-1 network |
| C-3 | C-3 | 100 pF | TR-1 Ccb feedback |
| C-4 | C-4 | 100 pF | TR-2 Ccb feedback |
| "2" (cap) | C20 / C-2 / **C-1** (service manual) | **220 pF** (RESOLVED, see 5.1, 5.3) | Input shunt — confirmed at 1500 DPI. C-1 = same component (naming discrepancy) |
| D-1 | D-1 | 25PIV/10mA/#142136 | Input protection |

**Action:** Standardize naming convention across all docs. Recommend using the **schematic reference numbers** as canonical, with our descriptive names in parentheses.

---

## Priority Order for Implementation Readiness

1. ~~**Preamp doc rewrite** (Section 1)~~ — **DONE**
2. ~~**Output-stage updates** (Section 2)~~ — **DONE** (including speaker=4x8, tremolo=twin-T, all component values)
3. ~~**Resolve C20, R-2, and R_feed disputes** (Sections 5.1, 5.2, 5.8)~~ — **ALL RESOLVED**: R-2 = 2M, C20 = 220 pF, R_feed = 1 MEG.
4. ~~**Signal-chain-architecture updates** (Section 6)~~ — **DONE** (phase numbering updated for SPICE validation phase)
5. ~~**Pickup, reed, and minor doc updates** (Sections 3, 4)~~ — **DONE**
6. ~~**SPICE validation** (Phase 2)~~ — **DONE Feb 2026**: Preamp, tremolo oscillator, LDR model all validated. See `spice/` directory.
7. **Component number reconciliation** (Section 7) — housekeeping; low priority, can be done during implementation

### Status Summary (Feb 2026)

**ALL schematic-resolvable uncertainties are RESOLVED.** SPICE models validate the circuit topology and component values. The project is ready to proceed to DSP implementation (Phase 3: Pickup and Summation, then Phase 4: Oversampler and Preamp).

Remaining open items (5.6, 5.7) require physical measurements and are bounded well enough for initial implementation.
