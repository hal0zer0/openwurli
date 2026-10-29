# Hammer–reed contact revision (plan, reviewed DRAFT)

Status: plan only. Nothing here is implemented. Sound change: ships as a minor version. Physics
review 2026-10-29; its answers are folded in below.

## 1. Goal

Replace the hand-set and recording-fitted parts of the excitation with a bottom-up integration of
the felt hammer striking the reed. Today the strike is described by five separate pieces, none
derived from the hammer:

| Piece | Where | Provenance |
|---|---|---|
| Dwell filter: Gaussian, σ 8 in f·T, contact ¾–1 fundamental cycle | `hammer::dwell_attenuation` | Contact time from Miessner US 2,932,231 (a design statement for the patent's foam pad, not a production measurement); the Gaussian shape and σ are hand-set |
| Onset ramp: 1–2 periods, 2 ms floor, raised cosine with a velocity exponent | `hammer::onset_ramp_time`, `ModalReed` onset | Hand-set; voiced by ear (2026-07) |
| Velocity curve: sigmoid k 1.5, then a per-register power law | `tables::velocity_scurve`, `velocity_exponent` | Ear-voiced around the retired 1/(1−y) pickup law; old-law debt (pickup-overlap-law.md §7) |
| Relative mode amplitudes | `tables::BASE_MODE_AMPLITUDES` | Calibrated against the reference recordings, which were an octave mislabelled; mode 2 halved by ear |
| ff swing at the pickup | `tables::PROVISIONAL_FF_SWING_MM` | A conditioned prior through a short-contact impulse law (restitution and contact-spread factors stand in for the felt) |
| High-mode decay: rate × ratio^2.0 | `tables::MODE_DECAY_EXPONENT` | Chosen by ear to cut the plink short. Its Zener basis rests on an arithmetic slip (reed-and-hammer-physics.md §5.5), and a real 200A's C4 mode 2 lingers for seconds (§5, check 10) |

The sixth row is in scope because the mode-2 evidence (§5, check 10) can't be read without it.
After the revision, one model produces the first five: contact time, the attack waveform, the
velocity response, the modal spectrum and the swing. All five retire. Two stay as report-only
cross-checks:
- **The swing table:** same priors, so a new ff swing outside its p10–p90 band needs explaining.
- **Mode 2 at −46 dB re mode 1:** the old calibration.

The experimental `set_hammer_contact_scale` becomes a felt-stiffness (hardness) multiplier.

## 2. A shipping defect this revision fixes

`tables::eigenvalues(mu)` is wrong above mode 1 whenever the tip mass ratio μ > 0. Its table
disagrees with the roots of the textbook tip-mass equation
1 + cos β cosh β + μβ(cos β sinh β − sin β cosh β) = 0. Those roots agree with the FE beam
(`tools/hammer_contact_eval.py`) to 4 decimals, and with an independent root solve
(`tools/beam_eig_check.py`).

The table runs sharp:

| μ | Mode 2 | Mode 3 |
|---|---|---|
| 0.01 | +23 c | +27 c |
| 0.05 | +100 c | +109 c |
| 0.10 (A1) | +156 c | +162 c |
| 0.20 | +218 c | +220 c |
| 0.50 | +289 c | +311 c |

β1 also drifts from μ 0.15 up (1.3853 against 1.4200 at μ 0.5).

- `mode_shape()` pairs the μ = 0 σ with the μ > 0 β, so `spatial_coupling_coefficients` inherits
  the error. It also takes |κ| and clips at 1, which discards each mode's polarity at the pickup.
- Effect in 0.9.0: the bass inharmonic partials run up to ~1.6 semitones sharp, falling to 0 by
  E3. The treble runs +23–45 c (μ 0.01–0.02).

The revision replaces the analytic beam with the FE model (§3), so it fixes this. Fixing the table
on its own first would be a separate sound change, then a second one when the FE lands.

## 3. Model

- **Reed (FE).** Frequencies f_n, mass-normalised modes and shapes φ_n all come from ONE
  eigenproblem: the 80-element Euler–Bernoulli reed (`tools/reed_fe.py`), with the tip solder
  solved to pitch.
  Mixing FE shapes with analytic ratios would give a mode set that belongs to no beam.
  - Tools: `tools/reed_fe.py` (the beam), `tools/reed_fe_inputs.py` (regenerates
    `tools/data/reed_swing_inputs.json` byte for byte) and `tools/hammer_contact_eval.py` (N modes,
    the contact model evaluation quoted below).
  - Caveat 1, the bass-blank dispute: .020″ against .026″. At A1, μ 0.20 puts mode 2 at 6.97×f0
    and μ 0.50 at 8.38×f0. A Sensor check on a Keyboard Waves bass attack can discriminate
    (§5, check 9).
  - Caveat 2: above about MIDI 72 the tabulated reed lengths can't reach pitch even bare, so no
    beam model is trustworthy there. Treat the treble modes as model debt and report it.
- **Modes: 7** (model evaluation: going 7 → 12 → 24 modes never changed the contact count, moved
  contact time ≤ 0.003 T1, mode 1 ≤ 3 % (worst case, 5-contact chatter) and modes 2–3 ≤ 0.3 dB).
  A 7-vs-14-mode check at the stiff corner is registered (§5).
- **Hammer.** An effective point mass m_h = I/r² (moment of inertia about the pivot over the
  pivot-to-felt distance squared) at speed v_h, striking from below over a patch (±0.1 L by
  default). Let-off 1/8″ (manual) confirms free flight at contact.
- **Felt: a plain power law F = K δ^p**, one-sided (δ = y_h − w(x_s) > 0). Hysteresis is left
  out: Giordano & Winans put most of the apparent hysteresis in shank bending and core transit, and
  soft blows show little. Production hammers are felt (service manual).
- **Reference mass:** the hammer meets the modal mass at the strike point,
  M_eff = M1/φ1(x_s)², not M1. A1 is 4.7 g at 0.5 L and 28 g at 0.3 L; C4 is 3.2 g and 22 g. So
  m_h/M_eff runs 0.2–1.6 mid-reed and 0.04–0.2 at 0.3 L.
- **Contact regime** (model evaluation). The target is a spring-mass: no reflections, and the
  energy stays in mode 1 and throws the hammer back. The string criteria (Hall III's C/C0, α)
  don't map. The map variables are m_h/M_eff and τ_f/T1, where τ_f is the felt's contact time on a
  rigid target.
  - τ_f ≥ 0.75 T1: a single contact everywhere.
  - Recontact (2–8 contacts) happens only when m_h ≳ M_eff and τ_f ≤ 0.3 T1. That chatter lifts
    mode 2 to −13…−25 dB.
  - Mode 2 re mode 1 is −41…−62 dB at τ_f 0.75 T1 in 7 of 8 cases, bracketing the old −46 dB. The
    outlier is C4 mid-reed with 5 g, at −23 dB. At τ_f 0.1 T1 it's −13…−38 dB. **Contact time sets
    the plink.**
  - Energy binds: 5 g at 2 m/s, mid-reed, lossless felt and τ_f 0.3 T1 swing the A1 tip 18 mm.
    The swing edges will now condition m_h, v_h and x_s jointly.
- **Pickup coupling: moves now**, to the FE φ_n averaged over the finger region, with signs. The
  contact gives FE modal coordinates, and their polarity at the pickup matters once phases are
  real.
- **Integration.** At note-on, step the coupled hammer + 7-mode system at an oversampled rate until
  the hammer separates and is moving away, checking for recontact. Fixed step budget, no allocation.
- **Output.**
  - The reed's motion during contact, written to a per-voice buffer (fixed size, tens of ms at most).
  - Each mode's amplitude and phase at separation. The free modal oscillators start from these
    (`ModalReed` gains per-mode phases).
- **Unchanged:**
  - the pickup law and network, the preamp, tremolo, power amp and speaker
  - the decay law (separately unverified), air drag and the damper
  - the C4 v100 level anchor
- **MIDI velocity → hammer speed** is a controller convention, registered before any render, not
  tuned: log-linear from v_pp to v_ff. `output_scale`'s velocity-aware RMS proxy follows the new
  velocity → swing map.

## 4. Inputs: priors stated before any render

The hammer has no direct measurement. The sources are piano measurements on strings (papers listed
under each input), so the
transfer is ASSUMED and labelled.

- **Felt stiffness, as τ_f/T1** (rigid-target contact time in fundamental periods, at the
  reference speed), plus p. Not as z*: Giordano & Winans' 0.42–0.56 mm belongs to 7–15 g Knabe
  hammers with thick felt and does not carry across.
  - p: uniform 2.2–3.5. Hall & Askenfelt V measure unused hammers at 1.5–2.8 and used ones at
    2.2–3.5; dynamic p runs 2.2 (bass) to 3.0 (C5 and up). Giordano & Winans suggest 3.0.
  - τ_f/T1: the band is set when the prototype is registered. The felt's real constraints are the
    falsifiers (§5) and Fig. 5 (report-only). Miessner's ¾–1 cycle is report-only.
- **The felt prior decides the regime (open, 2026-10-29).** Piano felt scaled to a 1–5 g hammer
  (rigid-target contact ~0.25–0.5 ms) puts the bass in a chatter regime.
  - A1: 2–16 contacts, total contact 0.03–0.15 T1, mode 2 at −11…−32 dB re mode 1. The result is
    erratic in m_h, and 7 against 14 modes disagree (6.17 vs 2.45 mm at one corner).
  - C4: 1–12 contacts, total contact 0.09–0.42 T1.
  - Miessner's ¾–1 cycle at A1 needs felt 30–60× softer (τ_f ≈ 0.75 T1 ≈ 14 ms).
  - The prior should sit on absolute felt properties per grade, so that τ_f/T1 emerges per note.
    Bounding it is the open physics question. The manual's hard-felt "clicking" (§5D) may be
    this chatter.
- **Hardness (the setter)** scales K. One measured series anchors the range: Hall & Askenfelt
  pricked a hammer in stages from "definitely too hard" to "ruined". Each step raised compliance
  about 10 %, with p from 2.3 (hardest) to 2.8 (softest), then 3.3 after ruining.
- **m_h** (= I/r² at the felt). Piano hammers are 6–15 g. The 200A's are not published; the swing
  prior used log-uniform 1–5 g. Vintage Vibe sells three tip grades (bass, middle, treble), so some
  grading is real. Grading by register is decided at registration.
- **x_s: uniform 0.25–0.60 L** (widened in review). After m_h it is the input that matters most:
  M_eff moves 6× between 0.3 and 0.5 L. Neither patent position is a production fact:
  - Miessner gives 0.25–0.35 L from the clamp (US 2,919,616) and "near mid-reed" (US 2,942,512).
  - The technician (Doc Wurly, voice message, 2026-09-25) says techs aim for an antinodal point
    "about halfway down" the reed, and reposition hammers to keep it there.
  - The reed doc's old "80–95 % from the clamp" visual estimate disagrees with both.

  A photo of an open 200A action with a scale would narrow it.
- **v_h.** The manual gives blow distance 27.4 mm and key dip 9.5 mm: an action ratio of about
  2.9, DERIVED, ignoring let-off and lost motion. Piano key speeds are 0.1 / 0.4 / 0.6 m/s at
  p / mf / f, peaking at 1 m/s; piano hammer speeds are 1 / 2 / 5 m/s (Askenfelt & Jansson 1991).
  The ff prior stays log-uniform 1.4–3.5 m/s.

The pick rule is the swing prior's: sample the priors, discard samples that fail the registered
falsifiers (§5), pick the median of the survivors, and carry the p10 / p90 corners. An ear-test
conclusion counts only if it holds at both corners.

Technician measurements would narrow the priors most (the ask goes through the user):
- weigh the hammer head and the shank separately, and measure the pivot-to-felt distance (for I/r²);
- the felt thickness by section;
- whether a hard bass strike double-contacts (slow-motion video);
- a photo of the strike line with a scale.

## 5. Checks, registered before rendering

Measured with the Sensor Array. Recordings are trend checks only.
1. pickup-overlap-law.md criteria 2 (ff falloff) and 6 (bass ff fundamental swell) stay in force
   as falsifiers. Criteria 3 and 7 are report-only.
2. Contact time against Miessner's ¾–1 fundamental cycle: report only.
3. Onset at the pickup, Pfeifle DAFx-17 Fig. 5, voltage trace, report only. The figure's two
   traces are two strikes (F2 camera, G2 voltage), so reed-to-voltage timing can't be read.
   - The physics review's reading: the voltage trace is G2, reed 11, in the bass, where the push
     goes INTO the band. Below an RC corner well above 98 Hz the output is ~dC/dt, so lobe height
     follows the reed's speed at band entry. Under soft felt the reed starts 0.085 mm from the edge
     and enters slowly (a weak, slow event); the free swings then cross at full speed.
   - Score the register asymmetry on the source term s, not the network output.
   - Registered prototype check: G2 at τ_f ≈ 0.75 T1, through a high-pass well above 98 Hz,
     should give slow features 0.5–1.0 T apart at 0.1–0.5× the first sharp lobe. If every corner
     instead gives a sharp lobe ≥ 0.7× steady within 0.3 T of contact, the reading fails: record
     the pre-excursion, don't model it.
   - Open: the trace shows ≥ 1.39 T before the first sharp lobe, longer than the evaluated
     contacts (≤ 1.1 T).
   - The traces were digitised from the figure's vector paths; the digitised data is kept outside
     the repo (derived from a published figure).
4. Attack shape by register against the Keyboard Waves samples, recorded through a 200A's own
   circuit. They are peak-normalised and trimmed, so shape only.
5. Velocity span: emerges from the physics, reported against v0.9.0, not tuned.
6. Upper-mode content at the pickup: mode 2 against the old −46 dB, report only.
7. Mode count: 7 against 14 modes at the stiff corner. Contact count, contact time and modes 1–3
   must agree to the evaluation's tolerances.
8. CPU: note-on cost of a 10-note chord, against the engine's budget.
9. Bass blank: Sensor check of a Keyboard Waves A1 attack for an inharmonic partial at
   6.5–7.5×f0 (.020″) or 7.8–9.0×f0 (.026″). The band edges are a model guess.
   **Run 2026-10-29: inconclusive.** Sensor decay mode, 405–520 Hz, layers 4a/5f/74/7e, fitted
   over 0.13–0.77 s. It found only H8 (441 Hz), H9 (496 Hz) and their ±10 Hz sidebands (the set's
   known ~9.4 Hz amplitude modulation). Nothing near the 462 Hz the .026″ blank predicts. That
   doesn't confirm .020″: mode 2 may sit below detection at the pickup, and the .020″ line (384 Hz)
   lies within 2 Hz of H7, which this window cannot resolve.
10. **Mode-2 line at C4, falsifier (proposed 2026-10-29, pending physics review).** Keyboard Waves C4
   (label c3), measured with the Sensor Array before any prototype render:
   - Method: decay mode, skip 5 ms, window 900 ms, band ±40 Hz, so the fit starts at 85 ms.
   - Result: a line at 1466–1472 Hz (ratio 5.63, inside the FE band 5.53–5.80) at −59.8 dB re H1
     (v126), −64.1 (v105), −66.5 (v95) and −67.0 (v74).
   - In the 0.3–2.8 s window it falls to −74…−83 dB with a stable frequency, and below v52 it is
     at the noise floor. It decays faster than H1, as a higher reed mode should.
   - The same runs show the undamped top reeds ringing in sympathy (1665 / 1764 / 1868 Hz, also in
     E4's sample), which independently confirms that those keys have no dampers.
   - Falsifier: rendered at the preamp tap with the same measurement, the model's C4 mode-2 line
     must not exceed the recording's by more than 10 dB (≤ −50 dB re H1 at v126, ≤ −56 at v95).
     One-sided: a cleaner model is not falsified.
   - Expected reading: tip-level chatter (−11…−32 dB) would sit 30–50 dB above this; Miessner-soft
     contact (≤ −41 dB at the tip) is near it.
   - Report only: the shipped model's mode 2 was voiced to −36 dB at the pickup output.
   - Persistence, the second half of the same evidence: from 0.3 to 2.8 s the line stands 30–43 dB
     above the noise, falling only ~15 dB faster than H1 over half a second. The Sensor refused to
     quote a T60 ("maybe a blend"), so only presence is evidence, not a rate.
   - At `MODE_DECAY_EXPONENT` 2.0 the model's C4 mode 2 decays at ~326 dB/s and is gone before
     85 ms, so the one-sided level falsifier above has no teeth until the high-mode decay is
     sourced.
   - Proposed second falsifier: the model's C4 mode-2 line must still be measurable (prominence
     ≥ 12 dB) in the 0.3–2.8 s window at v126.
   - High-mode damping enters the prototype as an explicit prior with two corners: a floor of
     thermoelastic ~5 dB/s (flat) up to whatever the falsifiers admit.
11. The ear gate (the user), at the pick and both corners.

## 6. Steps

0. Done: the source readings, the Fig. 5 digitisation, and the physics review with the FE code.
1. Commit the FE tools under `tools/`. Run check 9.
2. Offline prototype, `tools/hammer_contact.py`, no engine change:
   - per-note contact over the priors;
   - contact time, the modal amplitudes and phases, the signed pickup coupling, the swing at the
     pickup, and the attack waveform;
   - the pick under the registered falsifiers.
3. Register the τ_f band, the m_h grading and the velocity map. The user sees the plan with
   numbers, before any Rust.
4. Rust, on a `hammer-contact` branch:
   - the contact integrator in `hammer.rs`; per-mode phases in `ModalReed`; FE mode data replacing
     `eigenvalues` and the analytic coupling;
   - remove the retired pieces; tests that pin the physics.

   Downstream library hosts that build against this checkout must be told while the branch is
   checked out.
5. Re-hold the level anchor. Run the registered checks. Independent review of the diff.
6. Ear gate. MLP retrain (blocked until the reference recordings are recovered). Release as a
   minor version.
