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
| Tip swing | **unknown** — no measurement or document gives it | An earlier visual guess from video ("of the order of the finger thickness or less") is WITHDRAWN as a bound (2026-09-26): a guess from a camera angle that cannot show vertical scale, not evidence. Only physics (hammer impact) or a measurement may bound it |
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
(t_p 1.08/1.275/1.44), inside voltron's ±0.05 of +0.28.

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
