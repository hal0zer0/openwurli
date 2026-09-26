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
| Side gap g_s | .0025″ per side, band .002–.003″ (0.051–0.076 mm) | P (US 2,919,616 slot spec + spacer set); consistent with T ("one or two thousandths … makes a big difference"). 140B forum measurements (.008–.010″, different model) kept as a named outlier |
| Reed thickness t_r | reeds 1–20 .020″ unground (0.51 mm); 21–64 tongue .020–.026″ (0.51–0.66 mm) | S (Vintage Vibe case study; docwurly: thickening "reportedly … to be verified", undated) |
| Finger thickness t_p ordering | thick 1–20 · thin, bendable 21–42 · thick 43–64, uniform to the top | T (video, user read); S (docwurly: only F-21..D-42 thinned in 1963); D (frame at the break: bass edge 1.8–2.4× the section to its right) |
| t_p absolutes | 21–42: 0.4–0.8 mm · 1–20: 0.85–1.7 mm · 43–64: 0.85–2.0 mm | **G** — no measurement exists; middle bounded by "bendable by hand" and "somewhat thicker than the reed" (P, US 2,966,821); bass = middle × ratio; treble ≥ bass |
| Tip swing | at normal playing, of the order of the finger thickness or less (the reed and lead never swing fully clear of the band) | D (user's visual judgement of scale), low grade — a prior on the amplitude scale, not a value |
| Era | early flat bass pickups (later units: bass fingers angled up 20–30°, from about mid-1978) | T; S (docwurly) |

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
finger position. Tuned, because nothing measures them: the amplitude scale A at C4 (prior: swing ≲
t_p) and its register slope (the reed-compliance exponent), and at most g0 held **inside** its paper
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

- **Bark vs velocity:** real 200As bark more at ff than pp. The law's H2/H1 and H3/H1 trajectories
  over velocity (C2, C4, C6) must rise toward ff. A law that barks most at mf is a falsification of
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
- **Convergence gate** (static law, h vs h/2, within the swing prior): every harmonic above −30 dB
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

## 7. What this revision deliberately leaves alone

Only the pickup law changes. The amplitude path is shipped as it was — per-note swing =
`pickup_displacement_scale × PICKUP_MM_PER_DS` — and its recording-fitted pieces (the compliance
exponent, the velocity exponent, reed blank thicknesses) are recorded as top-down debt for the
excitation revision (per-note hammer–felt contact integration). One exception: the DS upper clamp is
removed, because it existed only to keep `1/(1−y)` away from its singularity. Consequence: bass ff
swings (~1.6 mm at C2) exceed the swing prior and reach the finger's far face, a region the tables
do not resolve — bass ff harmonics mean nothing until the swing scale is calibrated (a scope reading
of a real 200A, or the excitation revision).

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
