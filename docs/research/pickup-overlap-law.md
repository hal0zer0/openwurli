# Pickup Overlap Law — the 200A pickup as a through-slot comb

> **Status: DESIGN SPEC for the pickup-law revision (replaces the `1/(1−y)` parallel-plate law).**
> Every number below carries a source and a grade. Where the evidence gives a band rather than a
> value, the model is built and checked at both ends of the band; a conclusion counts only if it
> holds across the band.
>
> Grades: **F** factory document · **P** patent (1950s prototype, not the 200A) · **A** academic ·
> **T** technician, first-hand observation on a real 200A · **S** secondary/practitioner text ·
> **D** read off a photo or video frame · **G** guess, bounded by argument only.

## 1. What changed and why

The shipping pickup (`crates/openwurli-dsp/src/pickup.rs`) models the reed as one plate of a
parallel-plate capacitor closing on a slot floor: `C = C0/(1−y)`, `y = x/d₀`. That geometry has no
source. Every primary description of the pickup says otherwise:

- The service manual's voicing section (§18, printed pp.12–13) sets "the flat surface of the steel
  reed and the face of the pickup ... in the same plane", warns that "when a reed at rest is
  positioned inside the pickup, a very strong undesirable harmonic will be generated", and uses a
  feeler gauge for equal clearance on each **side** of the reed. **F**
- US 2,919,616 (Miessner): pickup ends in the plane of the reed → the output doubles in frequency
  and the fundamental disappears; ends offset above or below → essentially only the fundamental. **P**
- The EP200 finite-element study (Pfeifle & Muenster, DAGA 2017) finds the capacitance maximal near
  rest and falling at both extremes — impossible for a monotone `1/(1−y)`. **A**
- A 200A technician's narrated video of a disassembled reed bar (private communication, 2026-09):
  every reed sits laterally **between** two pickup fingers; the reeds are never bent; techs voice by
  bending the fingers. **T**

So the reed does not approach a floor. It moves vertically past the edge of a band of metal (the
finger) across a small lateral gap, and the capacitance follows how much of the reed's thickness
overlaps the finger's thickness — an **overlap law**. The old "vertical rest gap d₀", which no
source ever gave, has no counterpart in this geometry and is retired.

## 2. Geometry — evidence

| Quantity | Value | Grade · source |
|---|---|---|
| Reed position | laterally between two fingers of the lower comb (the upper comb, ~½″ above, is a grounded RF shield) | T (video); F (manual feeler-gauge procedure) |
| Vertical rest offset g0 | "a piece of paper … but just barely": ≈ 0.07–0.10 mm | T (bass, narrated); treble **assumed** equal (implied, not shown) |
| Side of the band | bass reeds (1–20) rest just **below** the fingers, solder lead down; treble (21–64) just **above**, lead up | T (video, user read); F (manual: "bass with the lead down and the treble with the lead up"); S (docwurly: "bass pickups are above the reeds and the non-bass pickups are below them") |
| Solder lead | faces away from the pickup and lies beyond the finger's end (fingers end about where the solder starts; drift toward the tip up the treble) | T (video, user read) |
| Hammer | strikes from below → first swing is up: into the band for bass, away from it for treble | T |
| Side gap g_s | **shipped: 0.19–0.27 mm per side by section** (reeds 1–14: 0.267, 15–20 and 21–42: 0.229, 43–64: 0.19), from one 140B comb's measured reed and slot widths (Electric Piano Forum topic 8418; a different model, method and serial unknown) | Re-ruled (§9 and the steepness tests below): the gap now stands on the 140B measurement alone. The patent's .0025″ per side (band .002–.003″, US 2,919,616 slot spec and spacer set) is the superseded design reference, kept for the band-case study. Doc's "one or two thousandths … makes a big difference" concerns lateral centring and fits either value |
| Reed thickness t_r | reeds 1–20 .020″ unground (0.51 mm); 21–64 tongue .020–.026″ (0.51–0.66 mm) | S (Vintage Vibe case study; docwurly: thickening "reportedly … to be verified", undated) |
| Finger thickness t_p ordering | thick 1–20 · thin, bendable 21–42 · thick 43–64, uniform to the top | T (video, user read); S (docwurly: only F-21..D-42 thinned in 1963); D (frame at the break: bass edge 1.8–2.4× the section to its right) |
| t_p absolutes | 21–42: 0.4–0.8 mm · 1–20: 0.85–1.7 mm · 43–64: 0.85–2.0 mm | **G** — no measurement exists; middle bounded by "bendable by hand" and "somewhat thicker than the reed" (P, US 2,966,821); bass = middle × ratio; treble ≥ bass |
| Tip swing | **unknown** — no measurement or document gives it | An earlier visual guess from video ("of the order of the finger thickness or less") is WITHDRAWN as a bound (2026-09-26): a guess from a camera angle that cannot show vertical scale, not evidence. Only physics (hammer impact) or a measurement may bound it |
| Era | early flat bass pickups (later units: bass fingers angled up 20–30°, from about mid-1978) | T; S (docwurly) |
| Polarizing voltage | 147 V DC via a 1 MΩ feed (200A spec); real units may sit lower: Pfeifle measured an EP200 at 130 V against its 170 V spec | F (200A service manual); P (Pfeifle & Muenster, DAFx-17, EP200, measured) |
| Pickup axial position | ~0.22 L from the reed tip (node of the 2nd partial); production combs may sit nearer the tip | P (US 2,966,821); production placement not documented |
| Proximity → tone | closer reed = louder and more harmonic; the over-loud remedy bends the pickup ends up 1/32″–1/16″, and pickup-end position "modulates the tone toward the harmonic or the fundamental" | F (service manual voicing remedy); P (Vintage Vibe US 11,475,868) |

## 3. The law

**Capacitance vs position comes from a 2D field solve, not a fitted formula.** `tools/pickup_field2d.py`
solves Laplace's equation on the cross-section (grounded reed W × t_r, fingers held at the plate
potential, neighbouring reeds by mirror symmetry, the grounded RF shield above) and returns the
reed-to-pickup capacitance per unit length as a function of u, the reed's penetration past the
finger's near face. A Lorentzian-smoothed trapezoid fits these curves only to 2–7% of range, with
the worst error near the rest point — where soft-playing harmonics are made — so the solved curve
itself is the law, compiled in as a per-section table with C²-continuous (cubic spline)
interpolation. The field solve is 2D: it treats the finger as long compared with g_s and ignores
the 3D fringe at the finger's end.

In the model, with `disp` the reed's modal displacement (model units, ≈1 at ff):

```text
x = sign · A · disp            sign = +1 bass (band above), −1 treble (band below)
u = x − g0                     penetration past the near finger face
s = (C(u) − C(−g0)) / ΔC_full  source term, 0 at rest, ~1 at full overlap
```

`s` replaces `y/(1−y)`; the network that follows (the charge dynamics with the 880 Hz corner) is
unchanged. `C_REED0` keeps its role as the capacitance swing at full overlap — a gain convention
tying the source to the plate node (the geometric value is nearer 0.4 pF than 3 pF; the node stays
near-LTI either way).

**What is evidence and what is tuned.** Pinned from evidence (with bands): g_s, g0, t_r, t_p, sign,
finger position. Tuned, because nothing measures them: the amplitude scale A at C4 (no evidence bound; see §2)
and its register slope (the reed-compliance exponent), and at most g0 held **inside** its paper
band. The fringe width is not a knob — it comes out of the field solve. If the model cannot reach
reality inside these bands, that is the result: some physics is still missing, and the bands are not
widened to close it.

## 4. Reference instrument ("the average 200A")

There is no single correct 200A: the design changed across the run and every unit was hand-voiced.
The model targets the most typical setup of one configuration:

- **Early flat bass pickups** (pre-mid-1978); the later angled bass fingers are a documented
  variant for later.
- Side gap .0025″, rest offset about a paper's thickness, reed thicknesses as §2.
- Unit-to-unit and note-to-note differences — the lateral centring that sets a note's loudness, and
  the finger height techs bend to voice it — belong in per-note variation, not in the base values.

## 5. Checks this law must pass

- **Harmonics vs velocity:** real 200As bark more at ff than pp. The law's H2/H1 and H3/H1 trajectories
  over velocity (C2, C4, C6) must rise toward ff. A law whose harmonics peak at mf is a falsification of
  its parameters, not a voicing choice.
- **Held-out reference:** the OldBassMan recordings (13 notes, one velocity, recorded through a
  transformer and tube preamp that add their own H2) are used for trends only — register slope,
  continuity across the E3/F3 break, and the H3-vs-H2 pattern (`1/(1−y)` forces H3/H1 = (H2/H1)²;
  an overlap law allows H3 above H2 when the reed reaches the band's far side).
- **Band robustness:** every conclusion is checked at both ends of every band.
- **Convergence:** see §6 (gate stated in absolute terms relative to H1).

## 6. Numerics — how the tables are made, and what they can resolve

- **Solver:** 2D finite-volume Laplace (`tools/pickup_field2d.py`), mirror-symmetric half period,
  knots every 0.025 mm in u. Shipping tables are solved at h = 0.0025 mm; h = 0.005 mm is the
  comparison grid. C(u) itself agrees between the two to ≤ 0.6 % of range everywhere.
- **Smoothing:** the solver's discretization noise is tiny in C (~1e-4 of range) but double
  differentiation amplifies it in C″, and harmonics come from C″ — the raw tables injected
  spurious harmonics (≈30 dB too much H3 at pp, a false 15–24 dB "click band" rise). Each table is
  therefore denoised with a smoothing spline whose strength is chosen by generalized
  cross-validation; the chosen λ and the residual (≤ 3.2e-4 of range) are recorded in the generated
  `pickup_tables.rs`. The two grids are smoothed independently, so part of their difference is
  smoothing.
- **Convergence gate** (static law, h vs h/2, swings 0.02 mm to min(t_p, 1 mm) — a range set by the
  withdrawn swing guess; extend it to the reachable swing once the swing scale is known): every harmonic above −30 dB
  re H1 agrees to 0.5 dB, and the absolute (linear) difference is ≤ −45 dB re H1 everywhere.
  (`tools/pickup_field2d.py --check A.json B.json`.) h 0.005 vs 0.0025: bass −65.8 dB / 0.06 dB,
  middle −47.2 dB / 0.34 dB, pass; the treble misses marginally at small swings (−43.4 dB /
  0.59 dB). A targeted re-solve of the treble near rest (`--refine treble -1.0 0.8`, u −1.0 to
  0.8 mm) at h 0.0025 vs 0.00125 passes at −69.5 dB / 0.03 dB. The miss was the h 0.005
  grid's error, not the shipping table's. The shipped treble window checked directly against
  h 0.00125 gives −73.9 dB (smoothed) and −62.9 dB (raw, unsmoothed).
- **Resolution limit:** in the middle and treble sections at small swings, H3/H4 below about
  −35 dB re H1 are table-limited (±4 dB). No conclusion — including the reference recordings'
  H3-vs-H2 pattern — may rest on a model harmonic in that range. H2, and anything above −30 dB, is
  resolved.
- **Aliasing:** none attributable to the law — pickup harmonics rendered at 44.1 kHz and
  176.4 kHz agree to 0.1 dB (C3, C5 at v120).
- **Far-field closure (found 2026-10-30; open):**
  - The solve grounds a flat plane on the reed's far side from the fingers, at y = −12 mm
    (`FAR_BELOW`), or 0.6 mm below the reed when the reed sits lower than that. Nothing in the
    evidence puts a conductor there; it is a numerical closure. The RF shield on the finger side is
    physical.
  - Reeds 15–20 (fixed solver, below; h 0.005 mm), plane at −12 / −36 / −60 / −120 mm:
    C(−4.6) = 14.69 / 16.29 / 16.53 / 16.69 pF/m. The plane is converged to ~1 % by −60 mm.
  - Against the converged plane, the −12 mm plane reads low by a near-constant 2.6–2.8 % of the
    law's range from u = −4.6 to 0. That shrinks to 0.2 % at +4.5 and grows to 4.6 % at −12.
  - Static H2–H5 at rest −0.085 mm, −12 against −120 mm (a shape comparison on coarse knots):
    - ≤ 0.3 dB at swings up to 2 mm;
    - 0.4–0.8 dB at 3 mm;
    - 0.7–1.5 dB at 4 mm;
    - 1.5–1.9 dB at 6 mm;
    - 2.7–3.2 dB at 8 mm.
  - Beyond the finger region's length along the reed (~6 mm; about l/2 ≈ 3 mm by a rule of thumb),
    a 2D cross-section may not be a fair model of the far field. The mirror cell also makes the
    neighbouring reeds swing with this one, whereas real neighbours rest in their slots. The
    reed's tilt across the 6 mm overlap (displacement varying 0.13–0.50 × the pickup swing, by
    section) is noted only qualitatively (§8: it averages the law over u) and has not been sized.
  - Open: the real far-side conductors per section, and the law's validity at large swings
    (closure, neighbours at rest and tilt, to be taken together). The shipping tables are unchanged.
  - Search result (2026-11-02): no held document dimensions any conductor around the pickup.
    - The service manual has no dimensioned section or exploded view. Its reed-vs-pickup sketches
      (Figs. N and O) are undimensioned, and the parts list gives spacer part numbers only (PDF p74).
    - What it does state: the reeds are grounded (PDF p16) and the pickup is polarised at 147 V
      (PDF p64). A metal shield is screwed over the reed bars (PDF pp. 5, 17), with its height and
      potential unstated.
    - The photo frames have no usable scale for vertical spacing.
- **Solver defect, fixed 2026-10-30:**
  - The reed and gap edges inserted into the x grid coincided with core grid lines up to rounding,
    and both were kept: a 1e-14 mm column whose conductance made the operator near-singular. The
    direct solve's relative residual reached 2e-2 at h 0.01 and 7e-5 at h 0.005, and deep-plane
    solves returned nonsense.
  - Lines closer than 1e-9 mm are now merged: residuals ~1e-15, and h 0.0025 / 0.005 agree to
    0.004 %.
  - The shipping tables were solved before the fix. Their data sit high by a smooth 0.12 % (reeds
    15–20) and 0.19 % (21–42) of range, at 14 / 12 re-solved knots.
  - On resolved harmonics (above −35 dB re H1), static H2–H5 move by ≤ 0.17 dB, inside the 0.5 dB
    gate. The shipping tables stand, emitted from their stored data. New solves use the fixed
    operator, so a table extended later must be re-solved whole, keeping one solve path.

## 7. What this revision deliberately leaves alone

The revision first shipped the law alone, keeping the recording-fitted amplitude path
(`DS_AT_C4 × compliance^0.75`, anchored at C4 ff ≈ 0.45 mm). The ear test showed that anchor was
load-bearing (§8), so the per-note ff swing is now a provisional bottom-up table. What stays
recording-fitted, and belongs to the excitation revision (per-note hammer–felt contact
integration): the velocity curve (`velocity_scurve`, `velocity_exponent`) and the reed blank
thicknesses.

Second consequence: **less velocity span.** The overlap law is linear in swing over the operating
range (fundamental ∝ A^1.00±0.06 up to about 0.45 mm in all three sections), while `1/(1−y)`
expanded toward its singularity, and the shipped velocity curve was voiced around that expansion.
Measured at the preamp input against v0.8.0 with the level anchored at C4 v100: pp (v20) +1.5 dB in
every register; C4 v64 +1.9 dB, v110 −1.4 dB; v127 −5 to −9 dB above C2 (G6 −9.2 dB). Because
`output_scale` normalises every note to C4 at the same velocity, C4's curve sets the span for the
whole keyboard. Restoring the span belongs to the excitation revision, which replaces the velocity
curve with hammer physics. Re-voicing it here would fit the new law to the old one's error.

Nothing else in the pickup or preamp expands level with swing. The tip arc changes the overlap
length by about 0.2 % at a 1 mm swing. Reed tilt across the overlap averages the law over u,
which smooths it. The electrostatic back-force is ~1e-5 N, and the preamp is linear at the
~18 mV C4 level. The one geometric exception is a rest point out on the fringe tail
(g0 ≳ edge width): mildly expansive at pp. That applies to the later angled bass fingers, not to
the early flat units modelled here. The span the excitation revision should supply comes from the
hammer contact. With a power-law felt pad (F ∝ δ^p; p ≈ 2–3.5 is typical of piano felt, not
measured for these pads), contact time shortens with hammer velocity. Near Miessner's design point
(contact ≈ ¾–1 fundamental cycle, US 2,932,231), that raises the fundamental's drive by about
3–6 dB over a 2.5× hammer-velocity span, on top of the linear growth. How much span a MIDI
velocity range "should" have is a controller convention. Physics fixes level against hammer
velocity, so the excitation revision maps MIDI velocity to physical hammer velocity once and lets
the span emerge.

## 8. The swing is load-bearing: provisional bottom-up swing (2026-09-26)

**Ear test.** With the law in and the old anchor (C4 ff ≈ 0.45 mm), the core waveform read as
more Wurlitzer-like than any earlier build, but the whole instrument sounded muffled. Measured: not
a filter (energy above 1 kHz rose in every band; onsets were not duller). The odd harmonics had
collapsed in the middle and treble: at C4 v100, H2 −7.3 dB and H3 −42.6 dB (v0.8.0: −15.0). An
ideal half-wave-rectified sine gives H2 −7.4 dB and no odd harmonics at all.

**Mechanism.** With the reed resting just outside the band, the law is a biased half-wave
rectifier. Odd harmonics come from only two places: the conduction angle departing from 180°
(which scales with g0/A), and the plateau once the reed reaches full overlap (~0.6 mm in the
middle section). No setting of g0 or swing inside the old guesses restored H3. Averaging effects
(bent or tilted fingers, mode shape along the overlap) only smooth the law. Lateral reed motion
contributes nothing at 3f (its mode sits near 4.7 f1, and the side-gap capacitance is even in the
lateral offset). Wurlitzer's own comb-pickup patent (Andersen, US 2,974,555, filed 1955) states
the same physics: a reed swinging through the plate gives capacitance peaks twice per cycle,
one-sided coupling gives the fundamental, and the timbre changes as the swing decays across the
band.

**Provisional swing.** `tables::pickup_swing_mm` is a per-note table from a bottom-up
hammer-impact model (see the `PROVISIONAL_FF_SWING_MM` doc comment for the model and inputs). The
ff hammer speed comes from the service manual's action dimensions (blow distance 27.4 mm, key dip
9.5 mm). The other inputs are guess-grade bands, and the value is picked by a rule fixed before
any render was heard: the geometric centre of each input band. Central values run from A1 3.26 mm
through C4 1.19 mm to C7 0.22 mm, with p10/p90 corners carried alongside. An ear-test conclusion
counts only if it holds at both corners. The same hammer inputs are assumed for every note; that
is an assumption, since the patents describe graded coverings. Bench measurements (a weighed
hammer assembly, key speed) replace the table. The level anchor (C4 v100 at the preamp input) is
unchanged: the swing changes timbre, not the level evidence.

**Convergence for the new swings.** The bass table was re-solved whole over u −4.5..+4.5 mm, per
point, at h 0.005 and 0.0025. One solve path per table: the two paths differ by ~0.05 %, and a
splice would show in C″. The gate is read as confirmed on 2026-09-26: harmonics above −30 dB re H1
agree to 0.5 dB, and harmonics at or below −30 dB differ by ≤ −45 dB re H1 (linear). As first
worded, the linear bound also bit on fold-through harmonics louder than H1, where it demands
~0.05 dB. Reach = central ff swing × ~1.19 (per-note offset and mode sums):

| Section | Reach | h 0.005 vs 0.0025 | Verdict |
|---|---|---|---|
| bass | 3.86 mm | 0.44 dB / −54.9 dB | pass (3 misses at A ≥ 3.88 mm, H6 ~−25 dB) |
| middle | 1.6 mm | 0.34 dB / −48.4 dB | pass |
| treble | 0.85 mm | 0.59 dB near rest | pass on the SHIPPED table: its window checked directly against h 0.00125 (−73.9 dB smoothed, −62.9 dB raw) |

**What is and isn't gated.** The p10/p90 band corners are not gated beyond these reaches (bass p90
runs to 9.5 mm), so no conclusion about bass ff holds beyond "central pick only; band not checked".
Fold-through also depends on finger thickness. Bass t_p (1.8–2.4× the thin section) and middle t_p
(0.4–0.8 mm) are bands, and bass or C4-ff conclusions need renders at both band ends first.

**First results** (full metric set, `local-docs/pickup-rev/metrics-swing-v1.json`; level
re-anchored, `target_db` −32.53, C4 v100 at the preamp input unchanged):
- C4 v100 H3 −18.1 dB (law with the old anchor: −42.6; v0.8.0: −15.0). The odd harmonics return.
- C4 v127: fold-through of the 0.6 mm middle band. H2 −19.5, H3 −2.6, H4 −4.4 dB: the fundamental
  weakens against its harmonics, the regime Andersen describes.
- C2 ff: gross fold-through, every harmonic 1–15 dB above H1 (central pick only; band not checked).
- Velocity span C4 v64→v110 at the pickup: 6.7 dB (v0.8.0 12.0, law-only 8.7). The plateau now
  clips ff; the span belongs to the contact integration (§7).


**Decay criteria, fixed before measuring (2026-09-26).** The ear test found the central pick's ff
sustain organ-like. "A 200A note decays audibly" is admitted only as a FALSIFIER of the joint pick
{swing, finger thickness, g0, 2D law}, never as a way to choose a value.
1. *v≈80, against the recordings (13 notes, MIDI 50–98):* fundamental decay rate fitted over
   0.1–1.5 s, the same window and fit as the reference analysis (`ml/harmonics.json`,
   `decay_rate_dB_s`). Pass per note: within ±30 % of the recording's rate, or ±1.5 dB/s,
   whichever is larger (the recording chain adds a transformer and tube preamp; this tolerance is
   set before any comparison).
2. *ff (v127), no recording:* full-chain RMS in 100 ms windows from 0.1 s to 2.1 s. Pass at C2 and
   C4 if (a) no window rises more than 0.5 dB above the one before it, and (b) the total fall is at
   least half the reed's own mode-1 decay over the same 2 s. That is, output tracks the reed's
   swing with a slope of at least 0.5.
Longitudinal averaging (the finger spanning a stretch of the mode shape) was checked as a
candidate and is not first-order: even a ±25 % amplitude spread moves the bass output slope at
3–3.5 mm only from 0.00 to +0.12.

**Results as registered (central pick).** (1) v≈80 decay: 7/13 notes pass. v0.8.0 scores 7/12
(MIDI 98 excluded: its fundamental reaches zero before 1.5 s). Both models decay slower than the
recordings at MIDI 54–62 and 78–82, so the misses are the reed decay rates, not the pickup law.
(2) ff falloff: C2 v127 falls 4.5 dB (3.0 required, largest rise +0.3 dB); C4 v127 falls 5.7 dB
(4.5 required). Both pass, so the central pick is not falsified.

*For anyone comparing by ear:* v0.8.0's bass at v127 fell 21.9 dB over 2 s while its reed decays
6 dB. The old law's expansion roughly tripled the audible decay rate. Part of the "piano falloff"
remembered from the old model was that expansion, not reed physics.

*Why it still reads as organ-like:* the 2D static law cannot give strong H3 and a strong ff
falloff together. Across four middle-section variants, H3 of −15..−20 dB at the preamp always
coincides with an output slope of 0.4–0.65 against swing; both are faces of the same knee. The
leading missing physics is amplitude-dependent reed loss (air drag, clamp micro-slip), which
belongs to the excitation revision. The swing table is not moved to buy decay.

**Registered now for the next round:**
3. *Fundamental alone at ff:* at C2 and C4, v127, report the fundamental's early rise (size and
   duration, 100 ms windows) as a MEASUREMENT, not pass/fail. Miessner (US 2,966,821) states that
   tones from electrostatically translated struck reeds "frequently" show "slight initial
   increments", so a rise may be correct physics. At v≈80 the recordings are roughly flat over the
   first 0.25 s: MIDI 50–78 fall about 1 dB, and the model matches (−0.5 to −1.4 dB at MIDI
   50–62). No mf early decrement is missing. The ff and sub-MIDI-50 behaviour has no recording to
   check against.
   (The 200A's mode-2 tilt at the pickup is ~0.1°, far inside Miessner's face-proportion rule, so
   his increment mechanism is minor here. The model's early rise at C4 v127 comes from
   fold-through.)
4. *Re-derive criterion 2's threshold* from the reed decay rates in force once the excitation
   revision changes them. Where the reed decays too slowly, the "half the reed's decay" bar is
   too lenient by the same factor.
5. *Expected treble reading after air drag (registered before the change):* the treble already
   falls faster early than the recordings, and drag adds ~5–6 dB/s at MIDI 78–96 at the central ff
   swing (less at v80), so the treble gap widens. That points at the linear decay rates
   (`0.005 f^1.22`, fitted through the old expansive law, which inflated apparent output decay),
   recorded top-down debt. It is not a drag regression, and the drag coefficient is not the thing
   to cut.

**Air drag added (mode 1, C_d 4.47 centre; `tables::PROVISIONAL_AIR_DRAG_DB_PER_MM`).** Level
anchor re-held (`target_db` −32.34). Registered checks: (1) v≈80 decay 8/12 (was 7/12); every note
from MIDI 50 to 82 moved toward the recordings. (2) ff falloff, threshold re-derived from the reed's
decay with drag: C2 5.85 dB (needs ≥ 3.71), C4 7.67 dB (needs ≥ 5.45); both pass. (3) Reported:
the fundamental's early rise is +1.5 dB at C4 v127 (peak at 0.5 s) and +11.7 dB at C2 v127, still
rising at 2.0 s. At ff the bass fundamental starts weak (the reed folds through its fingers) and
swells as the swing decays. Andersen (US 2,974,555) describes that same defect for the in-plane
design ("the fundamental may reach a peak as much as two seconds later"), and the 200A's offset
design exists to prevent it. Treble: as pre-registered, the early-window gap to the recordings
widens slightly (linear-rate debt).

**Bass ff fundamental — falsified, and a criterion registered (2026-09-26).** Andersen reports, in
a Wurlitzer comb pickup, that the bass fundamental "may reach a peak as much as two seconds later
than the second harmonic", names it a defect, and designs it out. The central pick reproduces it
(C2 v127: +11.7 dB, still rising at 2.0 s), so the bass central pick at ff is FALSIFIED. The
falsification covers the joint pick (swing, bass finger thickness, bass geometry), not the swing
alone: Andersen's cure was geometry. Registered before the next pick is rendered:
6. *Bass ff fundamental swell:* at C2 and C3, v127, the fundamental (100 ms windows from onset) must
   peak within 0.5 s and rise no more than 3 dB above its first window. Sources: the recordings
   at v≈80 show no rise at all on their five lowest notes (MIDI 50–66; the peak is in the first
   window, then monotonic decay, MIDI 50 −2.6 dB over 2 s). Miessner allows "slight initial
   increments" (C4's +1.5 dB by 0.5 s was accepted as such). Andersen's two-second swell is the
   defect end. Evidence standing: the recordings are MIDI 50–66 at v≈80, above C2/C3 and softer,
   so they support "no rise is normal" only by extrapolation across register and velocity.
   Andersen is what speaks directly to the ff bass. The criterion is deliberately lenient (a
   falsifier should fire only on clear defects).

**The finding of this round, and the bass stopgap (Rule B).** The register-independent hammer
inputs over-drive the bass relative to Andersen's defect boundary, so that assumption is refuted
for the bass (jointly with bass finger thickness and geometry). The fundamental starts to swell at a
swing of g0 + t_p and reaches +1 dB at g0 + t_p + 0.28 mm (verified on four tables). The admissible
bass band is therefore [all-inputs-low corner, that bound]: 1.45 / 1.64 / 1.80 mm at t_p
1.08 / 1.275 / 1.44. The bass swing is the geometric centre of that band, the rule already in force:
A1 0.99, C2 0.98, C3 0.95, E3 0.88 mm at t_p 1.275. It is a stopgap inside the surviving band, not a
physics-derived bass. Clipping at the bound (Rule A) was rejected because it would let the falsifier
select a value. No source compares bass swing with treble swing, so the bass sitting below C4's
1.19 mm contradicts nothing on record. The inputs that would settle it are bass hammer data (mass,
covering grading) and the bass finger thickness (a photo). Band ends travel with the pick. Bass:
low = the all-inputs-low corner, high = the swell bound; treble: the grid's p10/p90.
Bound confirmed on the band-end tables: the +1 dB swell edge sits g0 + t_p + 0.261/0.262/0.265 mm
(t_p 1.08/1.275/1.44), inside the physics reviewer's ±0.05 of +0.28.

**Rule B results (as registered; level anchor unchanged, C4 v100 −55.64 dBFS at the preamp input).**
Criterion 6 (bass swell): C2 central +0.03 dB (pass), low pass, high +1.58 dB peaking at 0.7 s
(fail on timing: the high end sits at the static +1 dB edge by construction). C3 passes at all
three. Criterion 2 (ff falloff, thresholds re-derived per pick): C2 passes at all three; C4 passes
at central and low and FAILS at its high end (3.04 mm), where the fundamental swells +14.3 dB. The
same Andersen signature in the middle section: its upper band is not admissible either. The swell
bound formula assumes t_p ≥ t_r, so it does not transfer directly to the thin middle fingers.

**Swing pick, final rule for this revision (2026-09-26; replaces Rule B and the corner
convention).** One rule for every note. (1) Prior: independent distributions per hammer input,
log-uniform for m_h (1–5 g), v_h (1.4–3.5 m/s), e (0.3–0.7) and D (0.33–0.8), uniform for the
strike point (0.4–0.6 L), stated in code before any render. (2) The falsifier conditions the prior:
samples whose ff swing exceeds the note's dynamic admitted edge (the largest swing passing criteria
2 and 6, bisected per knot) are discarded. (3) Pick = median of the surviving samples; band for the
ear-test robustness check = their p10/p90. Notes where fewer than ~10 % of samples survive are
reported as a finding (prior and falsifier in conflict). Corners were dropped: they assume every
input is at its extreme at once, which is improbable for physically independent inputs, and they
make the band so wide that the robustness check is vacuous. Dynamic edges (mm, finger region): 33
1.76, 36 1.57, 39 1.67, 42 1.84, 45 1.80, 48 1.85, 51 1.74, 52 1.89 | 53 1.05, 54 1.02, 57 1.11, 60
1.26, 63 1.36, 66 1.42, 69 1.44, 72 1.35 | 75 2.50, 78 2.77, 81 2.53, 84 2.74, 87 3.28, 90 3.50, 93
4.39, 96 ≥ 5.99.

**Registered before the next pick is rendered:**
7. *Harmonic lower bound, v≈80, against the recordings:* at MIDI 50 and 54, the model's H2/H1 and
   H3/H1 (full chain, speaker passthrough; the recordings' early-sustain window, 50–200 ms, same
   method) may not fall more than 6 dB below the recordings'. Recordings: MIDI 50 H2 −2.4, H3 −6.2
   dB; MIDI 54 H2 −9.0, H3 −22.1 dB. Lenient on purpose: the recording chain (transformer and tube
   preamp) is unbounded.
*Report, not a criterion:* the user's ear says the bass at ff lacks growl compared with real units
(heard on the corner-based Rule B build, bass 0.9–1.0 mm, which is superseded). No recording or
number exists for ff bass, so no threshold is set. The technician's bass data is what answers it.
The previous release is never the reference, in either direction.

**Conditioned-prior pick: results as registered (level anchor re-held, `target_db` −30.06).**
Survival in the prior: 15–44 % in the bass, 39–90 % in the middle, 100 % in the treble (MIDI 72 up
the edges sit above the whole prior). Criteria 2 and 6 pass at the pick and at both band ends at
C2, C3 and C4 (worst: C3 high end +1.42 dB at 0.5 s). Criterion 1: 7/12. **Criterion 7 FAILS on
H3 at both notes** (MIDI 50: −15.7 dB against a floor of −12.2; MIDI 54: −35.8 against −28.1): the
pick is too clean at v≈80 in the low register, the same direction as the user's "bass lacking"
report. Criterion 7 conditions the prior from below too. Scanning ff swing at v≈80 gives the
smallest passing swing at 1.54 mm (MIDI 50; H2 and H3 trade off as fold-through begins) and
0.98 mm (MIDI 54). With the upper edges (~1.78 and 1.02 mm) the admissible windows are 0.24 mm and
0.04 mm wide. Prior survival in them: 8–10 % (MIDI 48–51 reed data) and 2.9 % (MIDI 54), both
under the 10 % flag. **Finding:** at the lowest recorded notes, the ff swell bound and the v≈80
harmonic floor nearly collide. Candidates, none tested yet: the recording-fitted velocity curve,
which sets the v≈80/ff swing ratio; the guessed middle finger thickness (0.6 mm, below the reed's
own thickness); and the unbounded recording chain (transformer and tube can add H3).

**Admissibility, and the cost of the shipped pick.** Every criterion gets the same test: does it
speak to the variable being picked (the ff swing) through measured or sourced links only, with the
sanctioned ruler? Criteria 2 and 6 pass it (measured at ff, sourced by Andersen and practitioner
knowledge), so they condition the prior. Criterion 7 fails it twice. It is measured at v≈80, and
the link from v≈80 to ff is the velocity curve. And the Sensor Array refuses the recordings (no
frame of a decaying note passes its stationarity test; reported to its maintainers). So criterion 7
is reported, not conditioned. **Cost, stated on the shipped label:** conditioning on the upper
bounds only biases the pick LOW relative to a world where criterion 7 were admissible, which is the
same direction as the criterion-7 miss and the user's "bass lacking" report.

**The velocity curve inherits the retired law.** `velocity_exponent` was voiced by ear in 2026-07
through the old `1/(1−y)` law, and `velocity_scurve` cites Miessner's pads only qualitatively. It
fixes the v≈80/ff swing ratio (0.53 at MIDI 50, 0.50 at 54, 0.47 at C4), which the analysis ranks
first-order for H3 at v≈80 (13–17 dB across ratios 0.5–0.8). Every velocity-dependent comparison
in the new model inherits a fit to a law that no longer exists, so any v≈80-versus-ff statement is
provisional, including the ear test's "at every velocity from v40". The curve moves into the
excitation revision, re-derived from the contact physics. The technician's DI notes at stated
dynamics (pp/mf/ff on one note, read with the Sensor Array) would give the mf/ff ratio directly.
Check (1) done: MIDI 54's H3 miss is not table-limited. At its v≈80 swing (0.37 mm) the shipped
middle table and an h 0.00125 re-solve of the near-rest window agree to 0.1 dB (−36.1 vs −36.2 dB
at the preamp input), and the window passes the gate (−65.0 dB / 0.08 dB). With (2) blocked by the
analyser and (3) finding no source, the conflict stands as the finding: at the lowest recorded
notes the prior and the falsifiers disagree.

**Criterion 7 through the Sensor Array (2026-09-26).** The analyser's fundamental lock took the
loudest peak, an octave error whenever H2 ≥ H1, the recordings' low notes. Its maintainers traced
it and supplied a cross-checked `--fundamental` option. At FFT 8192 with the nominal pitch, every
frame confirmed, window 600 ms from 50 ms after onset, full chain (power amp in, speaker
passthrough): recording MIDI 50 H2/H3 −2.41/−6.23 dB (matching the reference extraction exactly),
model −5.38/−19.25, so H3 is 13 dB short. Recording MIDI 54 −10.80/−13.99, model −6.24/−39.84: H3
26 dB short. Bypassing the power amp moves the model's H3 by 0.1–0.2 dB (the shipping behavioural
amp does not yet carry the factory-bias crossover). At MIDI 54 the gap exceeds what the v≈80/ff
swing ratio (13–17 dB across its plausible range), middle finger thickness (4–6 dB) and a crossover
of plausible size (H3 ~−35 dB) could close together. **Finding: an odd-harmonic source is missing
from the model at MIDI 54**, not only a wrong swing. At MIDI 50 (13 dB) the velocity link alone
could reach it. (An earlier reading that the recordings' H3 grows as the note decays came from
comparing a 600 ms with a 1500 ms window and is retracted.) Successive 600 ms windows through the
analyser show the opposite. At MIDI 50, H1 falls 5.6 dB from 0.1 s to 2.5 s while H3 falls 23 dB
(H3/H1 −6.4 → −24.1). The same sign appears at MIDI 58, 62, 70 and 74, and at equal output level
H3/H1 depends strongly on register (−6.4 dB at MIDI 50 to −28.9 at MIDI 62). The recordings' odd
source is therefore level-increasing, steep (H3 roughly ∝ H1⁴ at D3) and concentrated in the low
register. That is a threshold-type, pickup-side behaviour, and it rules out an under-biased
output-stage crossover (which would be level-inverse and register-independent).
**Joint constraint (framing, 2026-09-26).** The two low-register findings together say more than
"the swing is too small". The real bass reaches a steep odd-harmonic threshold at mf, and it shows
no delayed fundamental at ff. In the current 2D geometry these pull the swing in opposite
directions (the near-empty admissible windows at MIDI 50/54), so no single swing value satisfies
both. The pair is a joint constraint on the geometry or law (rest gap, finger shape, Andersen-style
ears or angled fingers, longitudinal overlap) together with the swing. It is not only an amplitude
correction.

## 9. The reference recordings are an octave below their assigned notes (2026-09-26)

All 13 reference recordings (Freesound pack 5726) sound one octave below the notes our analysis
assigned them. Every file carries a full harmonic series on half the assigned pitch (floor between
those partials −50 to −80 dB); in the treble that half-pitch partial is 17–30 dB louder than the
assumed fundamental. The file labelled d7 was assigned MIDI 98, above a 200A's top note (C7, MIDI
96). The pack's octave labels run one above scientific pitch, and the extraction refined the
fundamental within ±8 cents of the label, so it locked onto the true second harmonic. **True note
= assigned MIDI − 12** (D2–D6, MIDI 38–86). Found when the Sensor Array's corrected fundamental
lock settled on half the nominal pitch.

**Void until re-extracted at the true pitch:** criterion 1 (decay rates were of the true H2),
criterion 7 and its Sensor re-measurement (the "H2/H3" were the true H4/H6 of a note an octave
lower), the steepness tests at MIDI 50/54/62, the "odd-harmonic source missing at MIDI 54"
finding, the recordings' "H3 > H2" pattern, and the early-H3-rise observation. The side-gap
re-ruling stands on the 140B measurement alone until the steepness test is redone. Beyond this
revision: every constant or training target documented as "OBM-calibrated" or "fitted to OBM"
inherits the same octave error and is suspect until re-derived.

**Re-registered before any re-extracted number is looked at (2026-09-26).**
- *Note map:* true MIDI = assigned − 12: 38, 42, 46, 50, 54, 58, 62, 66, 70, 74, 78, 82, 86
  (reeds 6–54). Row by true pitch (reed = MIDI − 32): 1–14 → 38, 42, 46; 15–20 → 50; 21–42 → 54,
  58, 62, 66, 70, 74; 43–64 → 78, 82, 86.
- *Criterion 1 (decay):* same thresholds (±30 % or ±1.5 dB/s), same method (fundamental in 100 ms
  windows at 0.1/0.3/0.5/0.8/1.0/1.5 s, log-linear fit), now at the true fundamental, all 13 notes.
  A note is dropped only if its fundamental falls below the floor before 1.5 s, reported as such.
  The Sensor Array's decay mode is run alongside as a cross-check and reported, not graded.
- *Criterion 7 (harmonic floor):* same 6 dB floor, same tap (full chain, speaker passthrough, v≈80,
  MLP off), measured with the Sensor Array (true fundamental, FFT 8192, 600 ms window from 50 ms
  after onset, the window the analyser can judge), at the two lowest true notes: MIDI 38 and 42.
- *Steepness (scale-free, a consistency check):* post-attack rule for every note: start at the
  recording's 0.7 s window, end at its 1.3 s window, H1 drop = the recording's own H1 change over
  that interval (Sensor, true fundamental, 600 ms windows). Notes, one per row, chosen now: 38
  (1–14), 50 (15–20), 62 (21–42), 78 (43–64).
- The velocity-link objection to conditioning on criterion 7 is unchanged: it concerns the
  ear-voiced velocity curve, not the recordings.
- *Suspect until checked for recording input:* the velocity curve, the register trims, and the level
  anchor's provenance.

**First real-instrument finding at the true pitch: the bass fundamental swells at mf.** Relative
to its first 200 ms window, the fundamental of the reference recordings rises +1.9 dB (D2, peak at
2.25 s), +3.4 dB (F#2, 1.65 s), +2.3 dB (A#2, 1.05 s) and +2.0 dB (D3, 0.85 s), and not at all
from F#3 up, while H2 and the overall level fall steadily (F#2: H2 −22 dB by 2 s). The
fundamental is strong from the start. Caveats: one unit, v≈80, voicing unknown, and a transformer
+ tube recording chain that, if compressive at that level, could lift the fundamental slightly as
H2 decays. That is carried as a caveat, not a refutation (a linear chain cannot make a swell).

**Criterion 6 re-registered (evaluation referee, 2026-09-26).** A falsifier that rejects the reference
instrument is wrong by construction: the old criterion (peak within 0.5 s, rise ≤ 3 dB) rejects
the real unit, and its "the recordings show no rise" support was the octave error. Andersen's defect
is a WEAK early fundamental ("a very strong harmonic … with a very weak fundamental", peaking up to
two seconds later), and lateness alone no longer discriminates. New criterion: at C2 and C3, v127
(100 ms windows from onset), the fundamental's rise above its first window must not exceed 6 dB;
no timing clause. The 6 dB is a judgement, not a sourced number (about 2.6 dB of headroom over the
real unit's largest mf swell for ff growth, well below the falsified +11.7 dB); the operator can
override it. The old bass pick stays falsified. The bass prior is reconditioned on the new
criterion, so the bass pick moves up.

**Criterion 2 re-checked against the recordings at the true pitch:** it passes the real unit at all
13 notes (no window-to-window rise above 0.5 dB; the level falls 5–57 dB over 2 s against the
required 3–28 dB), so it stands. Side observation: the real notes' level falls far faster than the
model reed decays in the middle register (true MIDI 58: 25.6 dB in 2 s against the reed's 7.8 dB),
consistent with the linear decay law having been fitted an octave off.

**Steepness at the true pitch, as re-registered (consistency check, coarse h 0.005 tables).** True
D3 (MIDI 50, 15–20 row): recording −11.6 dB; 140B gap (0.229 mm) −12.5; patent gap −14.8. True
D4 (MIDI 62, 21–42 row): recording −34.4; 140B gap −38.1 (inside the ±4 dB table-limited zone
at that small swing); patent gap −57.0. The 140B gaps pass; the patent gap fails at D4 by 23 dB.
The earlier "21–42 misses by 15 dB" came from the octave error. At true D2 (38) and at MIDI 78 the
test is undefined as registered: the recording's fundamental rises over 0.7→1.3 s at D2, and its
H3 rises at 78. Reported, not redefined.

**Registered before any fine-table run (2026-09-26): the no-fit swell check.** Hypothesis: the
real bass swell is the transducer operating past its fundamental peak as the swing decays through
it. Per bass note (true MIDI 38, 42, 46, 50):
- *Identify:* solve for the initial swing A0 and the reed's decay rate from the recorded
  fundamental swell's SIZE and PEAK TIME only (two observables, two unknowns, nothing else
  fitted). If the solution is not unique, report every solution; a cell where solutions disagree
  on a prediction is INDETERMINATE, not a pass.
- *Predict, out of sample:* (ii) the H2 change and (iii) the RMS change at the recording's 2.05 s
  point, with the same windows and analyser settings as the recording values (D2: H2 −3.3, RMS
  −5.4; F#2: −22.3, −7.6; A#2: −13.2, −9.4; D3: −16.7, −10.6 dB).
- *Band:* compute each prediction at the input band ends (bass finger thickness band, side gap
  ±0.05 mm, rest offset 0.07–0.10 mm), re-solving A0 and the decay at each end. A cell passes if
  the recording lies inside [min, max] of the predictions, widened by ±2 dB for H2 and ±1 dB for
  RMS.
- *Verdict:* the transducer explanation is SUPPORTED if at least 6 of the 8 cells pass AND the
  misses show no common sign (all model-high or all model-low). A systematic sign means a missing
  mechanism even if the count passes. All 8 are reported either way.
- *Use:* if supported, the identified A0 and decay are measurement-derived inputs at the
  recording's own dynamic, the first swing in this revision with no velocity curve in its path. If
  not supported, nothing is calibrated from the swell.

**Four-range tables (140B gaps): numerics.** The per-point solver re-aligns its mesh at every u;
at h 0.005 that makes a table jagged (median second difference 0.64 pF/m, with a 19 pF/m spike at
u 1.55 mm in the 43–64 table), while h 0.0025 is smooth (0.03 pF/m). Halving h cut the jitter
~20×, where a smooth O(h²) error would fall ~4×, so the 0.005/0.0025 pair is not asymptotic and
its gate failures measure mesh jitter, not the shipping table's error. The gate is therefore run
as h 0.0025 against an h 0.00125 re-solve of each section's reach window (bass u −3.1..+2.9,
middle −1.9..+1.7, treble −1.6..+1.4), same thresholds, in the order bass, middle, treble. Results
from a section read before its window passes are conditional. If the h/2 windows show the same
jitter ratio, the tool needs fixing (a fixed background mesh with u stepped in whole cells), not a
finer grid. The steepness check is unchanged on the smooth tables: true D3 −12.5 dB (recording
−11.6), true D4 −37.1 dB (recording −34.4).

**Measurement taps (registered 2026-09-26, before any measurement at a new tap).** Principle: measure
each claim at the tap where its source speaks; the tap is part of the criterion's definition, and
changing it is a method change that is re-registered first.
- Recording comparisons (criteria 1, 7, steepness, swell check): at the tap matching the recording
  (full chain). Caveat on every one: the recording chain beyond our full-chain tap (transformer and
  tube) is unmodelled.
- Criterion 6 (Andersen): at the PICKUP tap, since his text concerns the pickup's capacitance;
  the full-chain value is reported beside it. A linear chain cannot change H1's time envelope, so
  they should agree; a disagreement is a finding about the chain.
- Criterion 2 (ff falloff): the verdict stays at the full chain (a heard property); its threshold
  ("half the reed's mode-1 decay") is MEASURED at a reed-only tap, with the computed value (decay
  formula + drag) reported beside it. A difference is a finding about the implementation.
- The table gate: no tap (a static-law property).
- Every registered check reports two values: its registered tap and the pickup tap. Once per build,
  a Sensor/Emitter characterisation of each stage (reed → pickup → preamp → amp → speaker):
  small-signal response and level-dependent distortion at 3–4 levels including real ff signal
  levels.
- Stimuli: (1) a pickup describing-function map (the law alone, clean sinusoidal swing, amplitude
  swept at fixed frequency, per section and band end; H1..H6 against A); (2) check 2a: the swell
  check driven by a pure exponentially decaying swing through the law alone (as the committed tool
  does), then confirmed on the full render, same tolerances; (3) a known-answer gate: a full render
  at a given swing must match the describing-function map at that A, modulo the RC tilt.
- The reed tap exposes modal amplitudes (mode 1, mode 2), not only displacement. Every tap renders at
  a stated seed and velocity, recorded with the result.
- *Physics additions (2026-09-26).* A linear, time-invariant stage multiplies each harmonic by a
  fixed complex gain. So self-normalised TIME-COURSE metrics (a harmonic's decay, swell size and
  timing, each relative to itself) are identical at every tap downstream of the last nonlinearity,
  while RATIO metrics (H2/H1, spectral balance) change at every linear stage. The recording chain
  gives every harmonic an unknown fixed gain, so recording criteria are preferably built on
  self-normalised time courses (the bass swell is chain-invariant; H2/H1 is not). Ratio criteria
  against the recordings stay at the full chain and carry the unknown chain EQ as an open error
  term. Planned taps and tests: a reed tap exposing per-mode displacement at the finger region, the
  total swing, an operating-point trace (fraction of each cycle past full overlap and past the far
  face) and instantaneous frequency; a raw pickup tap before the RC network as well as the node
  after it; polyphonic ff bass chords (all reeds share one plate node, so chords drive the preamp
  harder than any single note); volume swept in the stage characterisation (the real amp clips at
  high volume: 60 mV in for 4.75 V out); the factory-bias crossover added to the behavioural amp
  before low-level results are read; a Sensor-measured pickup atlas (amplitude sweep per section
  and gap), a decaying-sweep stimulus for the swell check, two-tone attack intermod, and Emitter log
  sweeps per linear stage.

**Adopted (2026-09-26): recording criteria as self-normalised trajectories.** Criteria against the
recordings are registered preferentially as self-normalised trajectories (a harmonic relative to
itself over time) or differences of them. Absolute ratios against the recordings stay at the
full-chain tap, carry the recording chain's EQ as an explicit error term, and are REPORT-ONLY
while that term is unbounded (criterion 7 is report-only for this second reason too). Validity
conditions, stated on every trajectory criterion: (1) the invariance holds only after the last
nonlinearity. The recording's transformer + tube is not known to be linear at that level, and a
compressive recovery would break the invariance exactly as it would create a swell, so that caveat
stands until the recording chain is known. (2) The chain's time constants must be short against
the envelope change. That is true for the audio stages; any coupling or DC-block corner near a
bass fundamental (73 Hz) would add a small group-delay effect on its decaying envelope, probably
negligible, stated rather than assumed.

**Steepness test re-registered.** Its end reading (the change in H3/H1) is a trajectory difference
and is chain-invariant, but its start point located A0 from an absolute H3/H1, which is
chain-dependent. From now on A0 is identified from trajectories only: where the note swells, from
the fundamental's swell size and time (as in the swell check); elsewhere, jointly from the
fundamental's decay trajectory and the Δ(H2/H1) trajectory. The Δ(H3/H1) trajectory is then
predicted, with the same tolerances as before. Earlier steepness numbers used the old start rule
and are superseded by this definition once re-run. Criterion 2: the threshold is defined as half
the reed's mode-1 decay; the reed tap measures that quantity directly, so where computed and
measured values differ, the measured one is the definition's value and the computation is the bug.


**Multi-velocity reference: Keyboard Waves "Wurlitzer 200a" (registered 2026-09-26, before any
criterion number is extracted).** One 200A, described by the vendor and heard by the user as
"hot" (barkier than typical), sampled through its own electronics (tap point, tremolo state and
per-layer processing unknown); 38 white notes, true A1–C7 (the file labels run one octave BELOW
true pitch); 12 velocity layers (vendor labels 0x0a–0x7e, some missing); 16-bit mono, the lowest 16
notes at 22.05 kHz; truncated at ~3–5.5 s. Data QA: a small steady ~9.4 Hz amplitude modulation
(typically 0.2–0.4 dB p5–p95) at the same rate on every note, cause unknown (not the 5.63 Hz
vibrato rate).
0. *Split:* IDENTIFICATION notes = every other white note from A1 (A1, C2, E2, G2, B2, D3, F3, A3, C4, E4, G4, B4, D5, F5, A5, C6, E6, G6, B6);
   HELD-OUT = B1, D2, F2, A2, C3, E3, G3, B3, D4, F4, A4, C5, E5, G5, B5, D6, F6, A6, C7. Held-out notes are not extracted until the model built from the
   identification half is frozen (commit recorded), then scored once on (a)–(c) at these tolerances.
1. *Claim:* per note, the measured ff/pp swing ratio (hardest layer over softest), read as a LOWER
   BOUND on the instrument's physical span (the sampling may not reach the extremes). Layers are
   vendor labels, not key speeds, so the velocity curve's SHAPE stays ear-voiced and is labelled so;
   only its unsourced SPAN is replaced.
2. *Acceptance per layer:* a layer's identified swing enters the map only if its out-of-sample
   Δ(H2/H1) and Δ(H3/H1) trajectory predictions pass within ±2 dB. Non-unique identifications are
   INDETERMINATE and left out.
3. *Robustness:* the span ratio is computed at the g0, finger-thickness and side-gap band ends; it
   is admissible where it moves by no more than ±15 % across them. Absolute swings are reported as
   unit-specific only (a hot unit likely differs in g0).
4. *Cross-layer ratio differences at a fixed window:* admissible only if per-layer processing was a
   scalar gain, tested against the per-layer trajectories; if they disagree, these are void for
   this set.
5. *Instrument:* identification runs through the model's full chain (the samples include the unit's
   own preamp and amp); the last-nonlinearity caveat applies (a hot unit drives its preamp harder).
6. *Housekeeping:* analysis windows are integer multiples of the ~9.4 Hz modulation period (e.g.
   0.213 s) or every tolerance widens by ±0.4 dB; windows end ≥ 0.3 s before truncation; each file is
   checked for a fade-out; the lowest notes use the analyser's fundamental pinned to the true pitch.

**Registered 2026-09-26 (evening), before extraction: steepness mechanics.** No numeric tolerance had
ever been registered for the steepness test; the earlier "the 140B gaps pass" readings were informal,
so the side gap's consistency support is PENDING until this runs, and the side-gap re-ruling rests on
the 140B measurement alone. Recording side: Sensor, true fundamental pinned, FFT 16384 below ~100 Hz
else 8192, 600 ms windows starting 0.7 s and 1.3 s after onset. Model side: a pure exponentially
decaying swing through the section's static law and the pickup's high-pass, the same Sensor
settings on the synthesised audio. Swelling notes (38, 50) reuse the swell check's (A0, r) solution
set; non-swelling notes (62, 78) identify (A0, r) from ΔH1 and Δ(H2/H1) over 0.7→1.3 s. Every
solution is kept; disagreement is INDETERMINATE. Predicted: Δ(H3/H1) over 0.7→1.3 s at every band
end; pass = the recording inside [min, max] ± 2 dB. A predicted H3/H1 below −35 dB re H1 at either
window (middle/treble) is table-limited: reported, not graded. Note 78 without its r43_64 band-end
tables is "partial band" and not graded. Consistency check only; it cannot select g_s. Tool:
`tools/steepness_check.py`.

**Registered 2026-09-26 (evening), before extraction: multi-velocity identification mechanics.**
Two collisions with item 5 above: the full chain pins the reed decay to the model's (suspect) law,
and band-end tables are not compiled into the engine. Resolution: (1) Sensor windows 0.639 s (3 ×
0.213 s; 0.426 s yields no FFT-8192 frame at 22.05 kHz), hop 0.213 s, from 0.2 s after onset to
≥ 0.3 s before the usable end (fade-out detected per file); the last window is the last where H1,
H2 and H3 are all reported; fewer than 3 usable windows = TOO SHORT. Spans are therefore
layer-dependent: each result carries its span, and Δ values are never compared across layers with
different spans without saying so. (2) Unknowns per layer (A0, r) as in the swell and steepness
checks; r is an EFFECTIVE decay over that file's span (a fit statistic, not the mode-1 constant).
A layer swells if its H1 peak is not in the first window and is ≥ 0.5 dB above it. Swelling
layers: identify from swell size + peak time, predict Δ(H2/H1) and Δ(H3/H1); others: identify
from ΔH1 and Δ(H2/H1), predict Δ(H3/H1); ±2 dB; mixed = INDETERMINATE. (3) Search on the law-only
path, then CONFIRM each accepted layer through the full chain at the identified (A0, r)
(`preamp-bench render --decay-db-s`, analysis-only: mode 1 at a fixed decay, drag off); a layer
that fails confirmation is CHAIN-REJECTED and reported apart from identification failures.
(4) Band robustness of the span ratio on the law-only path. (5) r is reported for the decay-law
rebuild with the "effective" label. Early-window misses systematic across layers point at mode 2:
the sign pattern is reported. Every prediction uses its own file's span. The vendor files carry a
non-standard (legal) fmt chunk the analyser refused; they are analysed as a byte-identical re-wrap
(PCM data hashes recorded). **Pre-stated:** if the measured span (admitted layers, band-robust)
implies bass ff swings that criterion 6 rejects, the measurement wins: criterion 6's magnitude is
re-set from the measured ff swell, and the old +11.7 dB pick stays falsified only if it exceeds what
the measured span produces. Criterion 6's binding fraction is reported beside the span.

**Results so far (CONDITIONAL: the h/2 table gate has not run on the four-range tables).**
- *Edge-regime test re-derived.* Its −22 dB H2 floor had no source and was set on the patent gap;
  the 140B gaps give −21.6..−26.0 dB at 0.2 mm. The test now checks the sourced claims
  (fundamental dominant, H3 well below H2, H2/H1 growing with swing).
- *GCV residual.* Up to ~1 % of range, but concentrated past the far face (u > t_r + t_p + 0.3 mm,
  outside every reach window), where the per-point solver's mesh jitter sits at the same u in
  different tables (residual lag-1 autocorrelation −0.67: noise removed). Inside the reach windows:
  rms ≤ 7.5e-4 of range. In r21_42 the global GCV λ is ~150× the bass λ and its residual is
  positively correlated (+0.72: real curvature smoothed). Choosing λ by GCV on the reach window
  instead moves the operating-range harmonics by ≤ 0.47 dB (above −30 dB) and ≤ −51 dB linear:
  inside the gate thresholds, though at small swings in r21_42 smoothing alone uses most of the
  0.5 dB budget. The §6 figure "≤ 3.2e-4" belongs to the earlier tables. Whether the residual is
  within the solver's own error is read against the h/2 gate solves.
- *Dynamic edges re-bisected* (`tools/swing_edges.py`; criterion 2 threshold measured at the reed
  tap, agreeing with the computed value to ~0.03 dB; criterion 6 at the pickup tap, agreeing with
  the full chain to ~0.1 dB): bass 2.71–3.26 mm, middle 2.13–2.48 mm, treble ≥ 2.89 mm (search
  cap). Criterion 6 binds at every knot up to MIDI 72; criterion 2 binds nowhere. Applying the
  6 dB judgement (registered at C2/C3) to middle knots follows the per-knot edge rule.
- *Prior reconditioned:* bass picks 1.75–2.08 mm (survival 47–77 %), middle 0.76–1.10 mm
  (88–100 %), C4 1.08 mm, treble unchanged; no note under 10 %. Level anchor re-held (C4 v100 at
  the preamp tap −55.644 dBFS): `target_db` −30.06 → −29.41.

**Operator ruling on the multi-velocity set (2026-09-26, supersedes the identification above).**
The operator's words: "let's not make the OBM mistake of treating it as a gold standard to work
toward. just get the genuinely useful data". Read (a reading, not the operator's list) as: the set
is not a calibration target and not a sound to match; only within-note spectral shape against
mapped velocity is used, as one hot-voiced, peak-normalised instrument; no levels, no decays, no
fitting to its timbre. A decay-based use would be put to the operator directly. Remaining use,
report-only: Sensor, one fixed early window (0.2 s + 0.639 s) per key-map zone, H2/H1 and H3/H1
against mapped zone order beside the model's same-window trend at the same notes: a picture, not a
score (no thresholds, no parameter moves, no match/miss language). The identification/held-out split
is dropped; the set is now fully seen and cannot serve as a blind test. The per-layer swing identification (which fitted swing, decay and
a per-unit rest offset to decay trajectories, and whose span was to re-set criterion 6) is stood
down before any unseen note was extracted; criterion 6 stays as registered. Its 6 dB magnitude is a judgement with no source number, and it binds the bass ff pick at every knot up to MIDI 72, which puts the bass ff pick near the swing the recordings' mf swell implies. An instrument with ff ≈ mf swing has almost no velocity span, so this is an OPEN issue, for bench hammer data or physics to settle. Two facts from the
audit carry forward: every file is peak-normalised, and the filename velocity codes do not match the
key map (the vendor re-slotted takes). Observation kept from the smoke run (not a criterion): at C4
the softest zone is odd-dominant (H3 −28, H2 −45 dBc at 0.2 s) and its H2 falls faster than A²
while H3 falls as A³, the small-signal signature of a rest point near the law's inflection in one
hot unit. The inflection's location is not converged between the raw and smoothed tables (smoothed:
one C″ zero per section, u 0.084–0.137 mm; raw: dozens of ripple zeros); an h 0.00125 re-solve of
u −0.4..+0.6 mm per section is the derivative-convergence check.

**Registered checks, results (2026-09-26 evening; CONDITIONAL on the h/2 gate).**
- *No-fit swell check (all band ends; synth high-pass corrected to the engine's bilinear form):*
  3/8 cells pass (H2 at 42, 46, 50). Misses: H2 at 38 (model-low) and RMS at all four notes, every
  one model-high (the model's RMS falls 1–3 dB less over 2 s than the recording's). Count fails and
  the RMS misses share a sign: the transducer explanation is **NOT SUPPORTED** as registered, and
  nothing is calibrated from the swell. The identified swings (2.0–2.6 mm) are recorded, not used.
- *Steepness (Δ(H3/H1) over 0.7→1.3 s):* 38 PASS (−1.42 in [−4.40, −0.05]); 50 FAIL model-high
  (−8.09 against [−7.86, −1.15]); 62 table-limited (would read pass); 78 no solution (the
  recording's H3/H1 rises −49.3 → −44.5 dB while H1 falls 11 dB; no decaying swing through the law
  does that). The side gap's consistency support is mixed; the gap stands on the 140B measurement.
- *Criterion 1 (decay, true pitch, `tools/decay_check.py`):* 5/13 pass (54, 66, 74, 78, 86). True
  38–50: the recordings' fundamental rises over the window (+0.6..+2.4 dB/s, the mf swell) while the
  model's decays (−1.3..−2.5). 58, 62, 70, 82: the recordings decay much faster (e.g. −23.6 against
  −9.6 dB/s at 70): the linear decay law, fitted an octave off. The Sensor decay-mode cross-check
  cannot run on this span (it refuses: needs ≥ 4.5 s at its band; over the whole tail the
  fundamentals are two poles ~0.1 Hz apart).
- *Criterion 7 (report-only):* true 38: recording H2/H1 +9.3, H3/H1 +6.9 dB; model −32.2, −5.5.
  True 42: recording +3.9, +6.4; model −23.4, −8.1.
- *Known-answer check (render against the atlas):* H3–H6 agree within ±0.4 dB with mixed signs
  (including the fold-through cell, C2 v127). H2 reads high in 17 of 20 cells (to +2.3 dB) where
  the law's H2 has a notch; driving the law with the reed tap's own trace instead matches the
  render's H2 to ≤ 0.10 dB, so the bias is the atlas method (a 0.5 s window averaging across a
  notch), not the engine. The remaining tilt (to +0.38 dB at H6, rising with register) was the
  reference's own backward-Euler high-pass; the engine's network matches the analog one-pole to
  ≤ 0.02 dB. Cells below the atlas's 0.02 mm floor (C6 after 1.3 s) are outside the map.
- *Known-answer gate, re-registered and run:* the gate as first registered (render against the
  steady-swing atlas) FAILED on H2 for a method reason (a 0.5 s window averaging across H2's notch;
  diagnosed). Re-registered trace-driven: the render against the law driven by the reed tap's own
  trace through the engine's network (node-capacitance modulation included, since it is intended
  physics), ±0.5 dB on H2..H6 above −40 dBc, with the sign rule applying above a 0.05 dB floor (the
  first sign rule had no floor and tripped at −0.01 dB, below resolution). Result, C2..C6 v127,
  four windows each: every residual 0.00 dB, no shared sign: **PASS**. The atlas comparison stays
  in the report for H3..H6, ungated (within ±0.4 dB except where the swing leaves the map).

**Criterion 6's magnitude: an operator decision (2026-09-26 evening, pending).** A no-fit reading
from the recordings (inverting the chain-invariant mf swell for the swing, then predicting the
chain-dependent harmonic ratios at that swing) puts the real bass at mf in fold-through (swing
~2.0–2.35 mm > g0 + t_r + t_p = 1.87 mm): F#2 H2/H1 +2.7, H3/H1 +5.7 dB predicted against +3.9,
+6.4 recorded; D2 short by ~10 dB on H2. Graded PLAUSIBLE, NOT ESTABLISHED (it rests on the same
inversion as the unsupported swell check; the new support is one note on a report-only ratio).
Criterion 6's existence rests on Andersen's documented defect; its 6 dB magnitude has no ff
evidence. Two builds for the operator, both at the same level anchor:
- **A** — criterion 6 report-only. Edges = the solved tables' reach (criterion 2 never binds), so
  the upper tail is cut by numerics, not physics. Bass ff 1.98–2.46 mm, C4 1.11 mm; `target_db`
  −29.56. Branch `pickup-overlap-law-variant-A`. C2 v127 fundamental rise +4.2 dB (peak 1.5 s).
- **B** — criterion 6 kept (a judgement). Bass ff 1.75–2.08 mm, C4 1.08 mm; `target_db` −29.41.
  This branch. C2 v127 fundamental rise +1.7 dB.
Both central picks pass criteria 2 and 6. At C2 v100 (100–500 ms) H2/H1 is −4.3 dB in A against
−15.3 in B; the middle is identical within 0.5 dB. What settles it with evidence: a bench ff DI of
a bass note (the fundamental's time course), or, if the operator allows it, the multi-velocity
set's ff-layer H1 time course, report-only.

**h/2 gate: like-for-like method (registered 2026-09-26 evening, before any reach window was
scored).** A dry run showed the original check compares smoothers, not grids: each table was
GCV-smoothed over its own u range, and the same h 0.0025 data smoothed full-range versus over a
window already differs by 0.78 dB (r21_42, H2, small swing), while h and h/2 smoothed alike agree
to 0.16–0.17 dB. Registered: (1) the gate compares the shipping-grid data cropped to the window
with the h/2 window, both smoothed with the same λ, the smallest decade (1e-7..1e-3) at which C″
agrees between the grids within 10 % (smoothing chosen by h-convergence); thresholds unchanged
(`tools/pickup_field2d.py --check-window`). (2) The shipping table's own smoothing error (full-range
GCV against the converged window smoothing) is reported beside it. (3) The shipping tables are
regenerated in this revision with λ chosen by h-convergence per section (GCV was the last place the
data chose the smoothing); the level is re-anchored and the alias baseline and known-answer gate
re-run before the listening build. Near-rest windows (u −0.4..0.6, all four sections): λ 1e-4;
grid 0.07–0.33 dB (PASS); shipping smoothing error 0.13–0.54 dB.

**Field-solver fix and table regeneration (2026-09-26 night).** The bass h/2 gate failed past the
far face with jitter of the same size at h and h/2: a per-point solve's uniform fine region followed
the reed, so for u > t_r + 0.6 mm the finger's bottom face sat in graded cells whose positions moved
with u. Fixed (the fine region always holds the finger); for u ≤ t_r the mesh is unchanged, so every
per-point table was re-solved above t_r and merged (one solve path per table). On the fixed data the
like-for-like h/2 gate passes on every section's FULL reach (bass −3.1..2.9, middle −1.9..1.7,
treble −1.6..1.4) at λ 1e-4: grid 0.32 / 0.33 / 0.28 / 0.14 dB above −30 dB, linear ≤ −58 dB.
Shipping tables regenerated with λ = 1e-4 per section (h-convergence; `SMOOTH_LAM`), fit residual
~1e-4 of range. Re-run on the regenerated tables: level anchor re-held (`target_db` −29.40 B,
−29.55 A); known-answer gate PASS (0.00 dB); alias baseline refreshed (note 72 plateau 8.5 → 2.4 dB,
improved; no worsening); edges moved ≤ 0.03 mm, B picks ≤ 0.01 mm. Registered verdicts unchanged:
swell check 3/8 NOT SUPPORTED (RMS model-high at all four), steepness 38 pass / 50 fail (0.25 dB) /
62 table-limited / 78 no solution, criterion 1 5/13, criterion 7 model H2/H1 −31.1 against +9.3 at
true D2. Option C unchanged within 0.1 dB (flat C2 ff swell +6.4 dB at 1.9 s, mf +2.6 dB; 20° with
L_f 6–8 mm removes both the swell and the H2/H3 dominance; L_f 3 mm keeps +2.8..+3.4 dB).

**Chain characterisation (build B, regenerated tables; `tools/chain_characterise.py`).** Stage
ladder (C2/C4/C6 × v40/80/127): the preamp adds 7 dB of gain and moves H2..H5 by ≤ 0.1 dB; the full
chain moves bass H2/H3 by ≤ 1 dB. The exception is the quiet treble (C6 v40 H3 +11 dB at the full
tap, the behavioural amp's low-level floor). Volume sweep (C2, C4 v127, volume 0.2–1.0): H3 does not
move and the peak stays ≤ −35 dBFS, so a single ff note never reaches the power amp's nonlinear
region. Bass chord (C2+G2+C3 v127): with matched per-voice seeds, the shared-stage interaction is
−90 dB (preamp) / −86 dB (full chain). Each voice has its own pickup network; no shared plate node is
modelled. At the levels the model produces, every audible nonlinearity is at the pickup. Whether
the real preamp/amp contribute at these dynamics rests on the level anchor's provenance, which is
flagged suspect; open.

**Operator decision (2026-09-27): variant A ships on this branch.** After listening to both builds,
the operator chose A (criterion 6 report-only): "my ears prefer and think [it] sounds more real";
B had read as "a little light on the bass". Recorded as a product decision between two
evidence-free options, not as tuning. Criterion 6 is REPORT-ONLY from here: its magnitude is
re-registered from evidence when a bench ff DI of a bass note exists. The criterion-6-conditioned
edges (B) are kept in `tools/swing_prior.py` for the record. This branch models early flat fingers.
Option C (angled bass fingers) is closed, not pursued: the operator has no interest in a cleaner,
less swelling bass unless it fixes a problem, and it does not. It is a later production variant,
not a correction. The computation stays on record (`tools/angled_fingers.py`) should a problem
later point at the bass geometry.

**Review of the tooling (2026-09-27): verdict-logic fixes and the steepness re-run.** A review of
this revision's tools found defects in two gates' verdict logic; the fixes change no registered
threshold. (1) The steepness check synthesised a stride subsample of each case's solutions (which
can drop the extremes) and decided "partial band" from the case names rather than from the cases
that produced predictions. Its registration said both "every solution is kept; disagreement is
INDETERMINATE" and "pass = inside [min, max] ± 2 dB" without saying which spread each governs.
Ruled on review, as an interpretation of the registration, not a change: [min, max] spans the
input-band ends (g0, t_p, g_s), which is genuine input uncertainty; non-unique identification at one
setting must never widen the band. Within each case every solution is judged (recording within its
prediction ± 2 dB): all pass or all fail makes the case determinate, mixed makes it INDETERMINATE.
The note is graded on the determinate cases' [min, max] ± 2 dB only if every case is determinate
and every band end solved. Re-run under this rule (all solutions, the regenerated tables):
38 PASS (7/7 cases determinate, every one of 321 solutions within ± 2 dB); 50 FAIL model-high
(7/7 determinate, 0 of 2360 solutions within ± 2 dB; −8.09 against [−7.88, −1.10]). The earlier
verdicts stand. Noted, not graded: at 50 the solutions that reproduce the swell verify Δ(H2/H1)
anywhere from −12 to +13 dB against the recording's −4.8, so the swell alone constrains the
identification weakly there. (2) The known-answer gate could print PASS with no graded cell (a
Sensor refusal was skipped silently). It now reads INCOMPLETE if any registered cell is refused and
NO DATA if nothing is graded. The recorded PASS had every cell graded and stands. Also fixed:
`--check` / `--check-window` in `tools/pickup_field2d.py` fell through into the default field
sweep; `--check-window`'s "shipping smoothing error" column now measures the shipping smoother
(`law_smooth`), not GCV; criterion 7 in `tools/decay_check.py` now refuses an unconfirmed
fundamental; `preamp-bench render-poly --no-mlp`, used by the chord interaction so the chord and
its single-note references run alike (no effect on the recorded C2+G2+C3 result: the MLP is
identity below MIDI 53).
