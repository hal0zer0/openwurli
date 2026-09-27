# Reed pitch: no random wander, a measured glide, and the clamp

Status 2026-09-27: the reed's random pitch jitter is **removed** (evidence below). A deterministic
pitch **glide** over the decay is **measured but not modelled**: the registered test that would admit
its one physical constant is too thin to grade on the data held. Nothing about the glide is in the
engine.

## 1. Random pitch wander: removed

The modal reed carried an Ornstein-Uhlenbeck frequency jitter on every mode (sigma 0.04 %, tau
320 ms; written as 20 ms but stepped every 16 samples). It was ear-voiced in v0.2.0 and never
derived. Tested under a decision rule fixed before measuring:

- **Deterministic amplitude-dependent frequency shift** of a 200A reed at the model's swings
  (geometric nonlinearity, electrostatic spring of the pickup field): <= ~0.003 % at bass ff, less
  above. It cannot stand in for a 0.04 % wander.
- **Measured common-mode partial-frequency wander** (H1 against the strongest other partial, so
  independent band noise drops out), Sensor Array `--mode partials`, 0.3-1.8 s after onset:

  | Source | Median | p90 | rho (median) |
  |---|---|---|---|
  | Model with the jitter (known answer) | 0.017-0.022 % | | 0.999 |
  | OBM, 13 notes (true pitch) | 0.0016 % | 0.015 % | +0.14 |
  | Keyboard Waves, mid layer | 0.0012 % | 0.0045 % | +0.10 |
  | Keyboard Waves, top layer | 0.0012 % | 0.0094 % | +0.26 |

  Real partials are 12-18x steadier than the jitter made them, at the estimator's noise. Every
  recording figure is an upper bound (chain, medium, sympathetic reeds add wander).

Outcome fixed in advance for "stable within resolution": remove. Levels and every harmonic's energy
are unchanged by the removal (C2/C4 0.00 dB; C6 <= 0.25 dB at -85 dBc); spectral lines sharpen and
the slow inter-mode beating is gone.

## 2. The glide: measured

Both instruments glide in pitch over the sustain (Sensor, same windows): above 200 Hz the pitch
**rises** as the note decays, median +0.108 % (OBM), +0.053 % (KW mid), +0.103 % (KW top), to
about +0.2 %; below 200 Hz the medians are near zero (+0.012 / +0.017 / -0.014 %), with a few
bass ff notes falling to about -0.13 %.

Known answers passed: synthetic decaying tones with no glide read <= 0.0001 %; imposed glides of
+0.10 / -0.15 % read back at 96 % (the fitted span is 96 % of the window); a constant-pitch model
across MIDI 33-93 at v70 and v127, bass fold-through included, reads <= 0.0004 % through the Sensor.
The treble rise is secure against estimator bias. The bass fall is less so: real bass ff spectra
(H2 above H1 by up to 18 dB) are more extreme than anything the model renders.

## 3. Mechanisms

- **(i) Partial slip at the clamp** (softening: slip lengthens the reed at large swing, so pitch
  rises as the swing decays). Derived from first principles (strip between flat jaws, fixed screw
  force, Coulomb friction, Rayleigh with a root compliance; general for taper and solder):
  under uniform clamp pressure the slip zone grows linearly with the root moment, the slipped
  length counts at exactly 1/2, and `df/f = -(l_c / (2 mu N)) * K'_note * A`, with
  `K'_note = omega^4 Q^3 / (M1 E I_root t_c)` (`tools/data/clamp_k.json`). Under edge-concentrated
  pressure the slip zone grows as the moment squared, so the exponent lies between 1 and 2 and is
  set by the clamp, not by physics in general. Uniform friction over-predicts the size by 10-100x
  for plausible clamp loads, so a real clamp carries the moment mainly by bearing and slip is a
  corner residual: the SIGN and the loss-to-glide ratio (below) are fixed by physics; the exponent
  and the per-note scaling `K'_note` are hypotheses. The same slip dissipates energy: extra loss
  factor 0.85-1.36 x |df/f| (exponent 1-2), i.e. an extra early decay of about 23-37 x f x |df/f|
  dB/s, falling with swing (at C4 ff with a 0.1 % glide: 6-10 dB/s, on top of air drag's ~2 dB/s).
  This loss-to-glide ratio is the robust test of the mechanism.
- **(ii) Edge wrap on a rounded jaw** (hardening). Wurlitzer's own patent, Miessner US 2,919,616
  p.13: with insecure clamp edges "the effective, vibratory length of the reed changes from L at
  low vibration amplitude to L' at high vibration amplitude. Since L' is shorter than L the reed
  frequency increases as the amplitude of vibration increases" — pitch falls as the note decays.
  Only its onset is derivable: a threshold root moment `M* = E I_root / R` (jaw edge radius `R`
  unknown), reached first by the thin bass roots at ff. Its size above onset is not derivable from
  held evidence.
- Excluded as the main term: reed heating from the hammer (~1e-6), pickup charge redistribution
  (<= 0.0014 %), air added-mass amplitude dependence (bounded by its own ~0.02-0.05 % total).

## 4. Admissibility and the test design

A constant set from recordings is admissible only as the identification of a physical constant:
the form comes from physics, the recordings set at most one constant per mechanism per instrument,
each identified individually, and the test must be able to fail. The constant lands only after a
graded out-of-sample test passes. Parsimony: slip alone first, on the secure data (f0 >= 200 Hz);
the wrap term only if slip fails and the bass fall is first secured by a known-answer with real bass
partial structure. Unit-to-unit differences in `c` are real (clamp torque): carried as a band, not
averaged.

Test design (agreed 2026-09-27; not run):

- **Prediction (uniform-model hypothesis):** `glide = -c * K'_note * A_start * (r - 1)` over the Sensor's fitted span, with
  `A_start = ff_table(note) x 0.98` and `r = A_end / A_start`. The graded set is the Keyboard Waves
  TOP zone per note (the key map's placement; taken as ff). There the velocity curve's s-shape is
  0.976-0.981 for every note, so its ear-voiced register exponent (which moves a mid-layer note
  scaling by ~25 %) drops out. `c` is conditional on the ff swing table and the 0.98 factor.
- **Measurement:** Sensor Array `--mode partials` (commit `bae8695`), 0.3-1.8 s after onset, three
  partials. These windows and exclusions were fixed for the wander measurement before the glide was
  seen. Exclusions: Sensor refusals (beating); the top two Keyboard Waves notes (tracking failed);
  f0 < 200 Hz. Mis-slotted zones: none can be named. The vendor re-assigned takes to zones by ear,
  and "top" is the key map's placement by definition.
- **Graded test:** leave-one-out over the qualifying top-zone notes: identify `c` from all but one,
  predict that one. Pass: at least 8 of 11 within +-30 % or +-0.03 % absolute (whichever is larger),
  and no common sign among the misses. Report the spread of the leave-one-out `c` (and any note that
  moves it), and residuals per register. Qualifying notes (after refusals): MIDI 57, 59, 60, 62, 64,
  67, 69, 71, 76, 86, 89; refused as beating: 65, 72, 74, 77, 79, 81, 83, 84, 88, 91, 93. Scope of a
  pass: the lower treble (8 of 11 notes are MIDI 57-71), with three notes of support above.
- **Report-only:** Keyboard Waves mid layer; OBM (labelled mf, so the exponent's shape enters its
  note scaling).
- **Mechanism tests to register with any future graded run:** (1) loss-to-glide ratio: the extra
  early decay at ff over pp must match 23-37 x f x |df/f| dB/s, or clamp slip is not the mechanism
  whatever the glide fit; (2) the slip exponent from the glide's dependence on swing across layers
  (1 = uniform pressure, 2 = edge-concentrated). Both need ff and pp decays from one unit.

## 5. Verdict: form unvalidated; awaiting admissible data

The design needs `r`, the swing's decay over the window, and the only graded set is Keyboard Waves.
Its quality audit found the files processed for playback (peak-normalised, truncated at 3-5 s, a
recording-chain 9.40 Hz amplitude modulation on every note), and their use was ruled out for level
and decay. Taking `r` from the model instead inherits its known mid-register decay error, so it is
not a validation. No admissible graded test exists on the data held. **The model stays glide-free.**

Report-only, permitted: an order-of-magnitude `c` with `r` from the model's own mode-1 decay,
labelled provisional. It does not put a glide in the model.

What would make it gradable: fortissimo recordings of at least 12 distinct notes above 200 Hz from
one unit, at a known dynamic, unprocessed and free of sympathetic beating (e.g. a bench DI with the
other reeds damped). Such data would also be blind. The design above runs on it unchanged. Keyboard Waves' within-file decay was also declined as a source under an integrity check (2026-09-27):
the samples are hot-voiced and processed for playback, and are not trusted as a reference. OBM's
chain (transformer and tube preamp) colours its harmonics. The wander conclusion in section 1 stands on OBM alone.

## 6. OBM loss-to-glide test: power check first (registered 2026-09-27, before computing)

The loss-to-glide ratio (section 3) has no free constant and is a within-file ratio, so OBM (one
unit, mf, decays admissible) could test the slip form: measure each qualifying note's glide and its
early-minus-late decay difference, and compare the latter with the slip prediction from the former.
Obstacle: the pickup law's H1 is proportional to swing only below ~0.2 mm (d ln H1 / d ln A = 0.96
at 0.2 mm, 0.86 at 0.4, 0.67 at 0.6 for reeds 21-42), and OBM's absolute swing is unknown. In
that range the output decays slower than the reed while the swing is large: an early-late
difference of the same kind as the slip excess, opposite sign. Swing from the velocity curve
(ear-voiced) or from harmonic-ratio inversion (OBM's chain colours H2) is not admissible.

Power check, model only, no OBM decay data: for each of the 7 qualifying OBM notes (true MIDI 58,
62, 66, 70, 74, 82, 86), simulate the reed's mode-1 swing over 0.3-1.8 s after onset,
`dA/dt = -A (alpha + beta A + s A) / 8.686`, and the output H1 through the note's section law (static
describing function, rest offset 0.085 mm, the section's sign). Early/late decay rates are
least-squares slopes of 20 log10 H1 over 0.3-1.05 s and 1.05-1.8 s; D = early rate minus late rate.
Bands: swing at 0.3 s in 0.2-0.6 mm; alpha = the model's linear rate x 1-3 (the model's mid-register
decay is too slow by up to ~2.5x); beta = the model's air-drag coefficient x 0.447-2.236. With slip:
`s = 27.3 f k c K`, k in 0.85-1.36, with `c K` set so the simulated glide over 0.3-1.8 s equals
the note's measured Sensor glide. Without slip: s = 0. Measurement tolerance 1 dB/s. **Run the OBM
test only if the with-slip and without-slip envelopes of D are disjoint by more than the tolerance on
at least 5 of 7 notes; otherwise park it and record this result as the reason.**

Also re-graded: criterion 1's attribution of the mid-register decay misses to the reed's decay rates
assumed output decay equals reed decay. With H1 compression of this size that holds only at matched
swing, and the model's swing is not matched: the attribution is **unresolved between reed decay
and law compression at mismatched swing**. The decay-law rebuild must compare reed decay through the
law at matched swing, not raw output decay.

**Power check result (computed after registration; no OBM decay data used): PARKED.** The with-slip
and without-slip envelopes of D are disjoint by more than 1 dB/s on 2 of 7 notes, not the 5
required:

| True MIDI | Glide | D without slip (dB/s) | D with slip (dB/s) | Separated |
|---|---|---|---|---|
| 58 | +0.102 % | -1.2 .. +0.1 | +0.9 .. +3.6 | no (0.9) |
| 62 | +0.034 % | -1.2 .. +0.2 | -0.3 .. +1.8 | no |
| 66 | +0.201 % | -1.2 .. +0.6 | +3.8 .. +8.2 | yes (3.2) |
| 70 | +0.189 % | -1.0 .. +1.2 | +3.9 .. +9.0 | yes (2.8) |
| 74 | +0.108 % | -0.9 .. +8.9 | +2.6 .. +20.8 | no |
| 82 | +0.121 % | -0.1 .. +63.5 | +6.1 .. +67.1 | no |
| 86 | +0.049 % | +0.1 .. +80.0 | +3.3 .. +79.9 | no |

Small glides give too little slip loss to separate; in the upper treble the plausible range of
linear decay and air drag alone spans the whole slip range. No OBM outcome could discriminate, so
the test is not run. It needs the absolute swing (a known dynamic) to narrow the no-slip envelope:
bench data again.
