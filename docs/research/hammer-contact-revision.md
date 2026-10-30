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
  - τ_f ≥ 0.75 T1: a single contact in every evaluated case.
  - Recontact at τ_f ≤ 0.3 T1 is not confined to heavy hammers. A 3 g hammer (m_h/M_eff ≈ 0.64)
    recontacts too. (Corrected 2026-10-29: the first evaluation's stop rule ended a strike 1 mm
    below the patch's current position and missed late recontacts. It now stops only below the
    patch's full reach, and the numbers here are from the rerun.)
  - Mode 2 re mode 1: about −41…−62 dB at τ_f 0.75 T1, bracketing the old −46 dB. Chatter lifts it
    to tens of dB higher; one case is 5 g at A1 mid-reed, τ_f 0.3 T1: 2 contacts, mode 2 −17.8 dB.
    **Contact time sets the plink.**
  - Energy: the swing edges condition m_h, v_h and x_s jointly. Rerun examples, A1 mid-reed at
    2 m/s:
    - 5 g, τ_f 0.3 T1: 1.95 mm tip, 2 contacts.
    - 3 g with piano-stiff felt (0.25–0.5 ms): 5.3–9.6 mm undamped, 5.8–6.3 mm with damping,
      9–12 contacts. That is still above the 4.29 mm bass edge.
  - Mode-count convergence holds for point and patch contact (7 / 12 / 24 modes), and damped
    14-mode runs equal 20-mode runs. Undamped stiff chatter is not converged at 14 modes (2–9 %
    off), so 20 modes are integrated.
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
  (rigid-target contact ~0.2–0.6 ms; τ ∝ m^(1/(p+1)), not √m) puts the bass in a chatter regime.
  The numbers in this bullet came from the evaluation before its stop-rule fix; the prototype's
  first pass supersedes them.
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

**Registered priors for the prototype's first pass (2026-10-29, before any prototype run).** ff
only; the velocity map is registered separately, before any non-ff render. Every value here is
ASSUMED unless marked.
- m_h: log-uniform 1–5 g, the same for every note (as in the swing prior). Survivors are reported
  per note, so a register trend shows up rather than being imposed.
- Felt, F = 10 N·(δ/z*)^p:
  - z* log-uniform 0.1–5 mm. That spans stiffer than piano felt (0.42–0.56 mm, Knabe) to very soft;
    the upper edge stands for felt thickness, which is unmeasured.
  - p uniform 2.2–3.5 (Hall & Askenfelt's measured range).
- x_s: uniform 0.25–0.60 L, over a ±0.1 L patch.
- v_ff: log-uniform 1.4–3.5 m/s.
- Reed geometry: equal weight on the FE variants. Tips 0.51 / 0.585 / 0.66 mm (a discrete stand-in
  for the uniform tip prior); bass blank .020″ / .026″, 50/50.
- Modes: 20 integrated, 7 rendered.
- High-mode damping during contact, run at both corners and reported side by side:
  - floor: undamped;
  - upper: ω² anchored at C4's mode-2 loss, the only anchor (α ≈ 4–10 /s at 1466 Hz, upper end).
    Pass 1 ran with the anchor briefly moved to G3 (23 /s at 1295 Hz). That was withdrawn once G3's
    line proved fixed. The two corners agreed within ~1 dB in pass 1, so the result stands.
- Offline falsifier for this pass: the ff swing at the pickup must stay within each note's
  dynamic admitted edge (the swing prior's EDGES, which are the pickup tables' reach). Checks 10a
  and 10b need chain renders and come after.

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
10. **Mode-2 line, real-200A evidence (Keyboard Waves, Sensor Array decay mode), measured before
    any prototype render.**
    - C4 (label c3), from 85 ms (skip 5 ms, window 900 ms, band ±40 Hz): a line at 1466–1472 Hz,
      ratio 5.63, inside the FE band 5.53–5.80. Re H1: −59.8 dB (v126), −64.1 (v105), −66.5 (v95),
      −67.0 (v74). Over 0.3–2.8 s it is still there at −74…−83 dB with a stable frequency, and below
      v52 it is at the noise floor.
    - Identity check, G3 (label g2). **CORRECTED by the survey below.** G3's line at 1294.5–1296.3 Hz
      (6.61×, inside its FE band) is a FIXED line: the same frequency appears in 10 other notes'
      samples, against a chance level of 1.2–1.4 at offset controls. It is not G3's own mode 2.
      - C4's 1466 Hz line does pass the same test (1 other note, chance level 1.2), so C4 stays
        the anchor.
      - G3 also shows a line at 1151 Hz (5.88×, −50…−57 dB), unexplained; recorded, not used.
    - Also seen: the undamped top reeds ringing in sympathy (1665 / 1764 / 1868 Hz in the C4 and E4
      samples), confirming that those keys have no dampers.
    - C4 at 5.63× also refutes the uniform beam there: the shipping engine (μ 0) puts C4's mode 2 at
      6.27× (1640 Hz, +186 c). That is the taper, separate from the §2 table bug, and another reason
      the frequencies come from the FE.

    **Falsifiers (registered 2026-10-29, after physics review):**
    - **10a, level, scored at the top layer only** (recording v126 against the model at v_ff).
      Measure the model's C4 mode-2 line at the preamp tap with the identical Sensor fit.
      - Its onset intercept re H1 must not exceed the recording's by more than 10 dB (≤ −50 dB).
      - Decouple the test from the unsourced decay: for this scoring only, render mode 2 at the
        recording's fitted rate, or with no extra loss, and compare onset intercepts from identical
        fits. Then the test is about the hammer alone.
      - One-sided. The v95 layer and the slope across layers are report-only, because they depend
        on the velocity convention.
      - Expected: tip-level chatter (−11…−32 dB at the tip) lands 30–50 dB above the recording;
        Miessner-soft contact (≤ −41 dB) lands near it.
    - **10b, persistence:** at v126 over 0.3–2.8 s, the model's C4 mode-2 LEVEL re H1, measured
      identically, must be ≥ the recording's −74 dB minus 15, so ≥ −89 dB. Level, not prominence:
      a clean render has no noise floor. A decay law that kills mode 2 before 0.3 s (for example
      today's 326 dB/s) fails by tens of dB.
    - Report only: the shipped model's mode 2 was voiced to −36 dB at the pickup output.
    - **High-mode damping prior, two corners.**
      - Lower: the physics floor. Thermoelastic is flat at ~5 dB/s. Air radiation is dipole-weak,
        ~f², ≲ 3 dB/s at 1.5 kHz, uncertain ~10×. Viscous and slot-shear loss is ~√f and unsized:
        the Stokes layer (~57 µm at 1.5 kHz) is comparable to the 50–270 µm finger gaps. Clamp
        microslip is empirical.
      - Upper: whatever 10b admits.
      - A sourced upper corner later, once the Sensor quotes a rate: ω² anchored at the MEASURED
        mode-2 rate, not at H1. That is 4–9× below today's law at mode 2.
11. The ear gate (the user), at the pick and both corners.

## 5b. First pass results (2026-10-29, `tools/hammer_contact.py`, registered priors, 1000 strikes per note)

All strikes finished (60 T1 window). Energy is conserved to ≤ 2e-9 at the undamped corner. The two
damping corners agree to within ~1 dB everywhere, so high-mode damping during contact is not a
sensitive input.

- **Bass (A1–E3):** 35–71 % of strikes stay within the swing edge. Survivors: 52–59 % single
  contact, contact p50 0.14–0.43 T1, mode 2 at the pickup p50 −35…−40 dB.
- **Middle (G3–C5):** 77–98 % survive; 76–84 % single contact; mode 2 at the pickup p50
  −46…−57 dB.
- **C4, PROXY of falsifier 10a.** The proxy is mode 2 at the pickup + 9.6 dB of RC tilt, against
  the recording + 10 dB, i.e. ≤ −59.4 dB at the pickup. It is not the registered render test.
  - 42 % of the edge survivors pass.
  - They select **soft felt: z* p10/50/90 1.08 / 2.40 / 4.36 mm**, 2–8× softer than piano felt
    (0.42–0.56 mm). m_h, v, p and x_s stay unconstrained.
  - The passing strikes are 98 % single contact, with contact p10/50/90 0.48 / 1.02 / 1.45 T1.
    That is Miessner's ¾–1 cycle, reached from the recording through the model rather than
    assumed.
- **One felt cannot serve the whole keyboard.**
  - Soft felt (z* 1–5 mm), as C4 selects: in the treble, contact lasts 2–5 T1, a quasi-static push,
    so the reed barely swings (C6 p50 0.02 mm, C7 ≈ 0). In the bass it is mostly single contact
    (70–86 %), mode 2 −45…−54 dB.
  - Stiff felt (z* < 0.6 mm): the treble speaks (C7 p50 0.16 mm, near the old table's 0.22), but
    the bass chatters (32–39 % single contact) and mode 2 is too loud for C4.
  - So the felt must be graded, harder toward the treble. That matches the three hammer-tip grades
    restorers buy and piano practice.
- **Next registration:** a per-grade felt prior. The grade boundaries are undocumented for hammer
  tips, so the code's sections (≤ 52 / 53–72 / ≥ 73) are proposed, ASSUMED. Each grade then needs
  evidence. The proposal is a Keyboard Waves survey of the mode-2 line across notes in every
  section (where the FE band clears the harmonics and the sympathetic reeds). The same survey
  gives per-reed mode-2 decay for the damping prior.
- Field notes: Emma's restorations describe tip felt compacting after decades ("a little
  impacted", sanded back to fresh felt), hammers as "light", and bare wood sounding wrong. These
  are qualitative (auto-captions).

## 5c. Mode-2 survey of the reference set (2026-10-29, `tools/mode2_survey.py`)

Keyboard Waves, every white key at the v126 layer (the set has no black keys).
- Method: Sensor decay mode, onset window (skip 5 ms, window 900 ms, band ±40 Hz) and late window
  (0.3–2.8 s).
- Search band: the FE mode-2 band widened 5 %, clear of harmonics and the sympathetic top reeds by
  12 Hz, with prominence ≥ 12 dB.
- Levels are the Sensor's windowed candidate levels re H1. Its fitted intercepts and rates are
  mostly refused ("maybe a blend"), so none is quoted.
- Identity: a line counts as the struck reed's own only if the same frequency (±1.5 Hz) does not
  appear in the other notes' samples above chance. Chance level = matches at 8 offset control
  frequencies, mean + 3σ.

Results:
- **FIXED lines (excluded):** 1296 Hz (seen in G3 and A3, 10 other notes) and 1719 Hz (D4 and E4,
  13–14 other notes). Their origin is unknown: a structural resonance, or one under-damped reed's
  mode 2 driven through the bar.
- **Ambiguous:** A2 (681 Hz) and E3 (1061 Hz).
- **No usable onset line:** A1–G2 (the H1 or line is not resolved at harmonic spacing ≤ 98 Hz).
- **Own lines, onset level re H1:**
  - Bass: B2 −35.6, C3 −56.3, D3 −32.7 (E3 −35.3, ambiguous).
  - Middle: F3 −57.1 (ratio 7.37, above its FE band, doubtful), B3 −39.6, C4 −59.8, F4 −55.9,
    G4 −65.0, A4 −63.9, B4 −62.6, C5 −86.1.
  - Treble: D5 −76.1, E5 −58.4, F5 −47.7, G5 −70.0, A5 −65.6, B5 −71.3, C6 −67.0, D6 −81.2,
    E6 −58.3, F6 −104.6, G6 −85.2, A6 −96.0, B6 −95.2, C7 −95.9.

**Pattern test.** The physics reviewer predicted the pattern without seeing these data (the survey
had already been run here):
- with discrete grades, τ is roughly fixed within a grade while T1 shrinks, so mode 2 re H1 FALLS
  with pitch inside each grade and JUMPS up at each boundary;
- one felt gives a monotone fall to near nothing;
- continuous grading gives a smooth curve.

Reading (corrected after the physics review's fit; the first reading here overstated it):
- **Middle:** falls from B3 −40 to C5 −86. **Middle → treble:** a rise at D5–F5 (−76, −58, −48),
  then a fall to about −96 at the top.
- **Statistically suggestive, not established.** Fitted over the 21 values B3–C7 (F3 dropped):
  - one falling line, −1.1 dB per semitone, rms 11.5 dB;
  - a break at 72|73 is no better (ΔBIC +0.7);
  - the best break is 75|76 (+27 dB jump, rms 8.9, ΔBIC −4.6), but it was found by searching, on
    one unit.
- **74|75 is the pickup's finger-thickness boundary** (reeds 42|43). A pickup-section change moves
  mode 2 re H1 at the output even under one felt, so the raw pattern cannot locate felt grades.
  Only the chain-identical per-note test, carrying each section's pickup law, can.
- **Treble ratios:** D5–E6 drift smoothly from 5.75 to 4.86, consistent with one mode-2 family.
  F6–C7 sit at −85…−105 dB with erratic ratios (5.64, 5.73, 5.61, 4.96, 5.74) and are doubtful.
  The FE band is unreliable above MIDI 72 anyway (the length table).
- **The fixed-line test can't tell a structural resonance from one reed's mode 2 rung through the
  bar on every strike.** Each fixed line sits in one reed's FE mode-2 band: 1296 Hz in G3's,
  1719 Hz in D4's, 1151 Hz in F3's.
  - Discriminator (to do): the line's level re the undamped top reeds' 1760 Hz line, which is a
    bar-excitation reference present in every sample. A reed's own mode 2 should stand higher in
    its own sample; a flat ratio across samples means a structural resonance.
  - G3 and D4 stay excluded until then.
- **Bass:** too sparse and noisy (±10 dB per-reed scatter) to show a fall, and the bass/middle
  boundary is unresolved: G3 and A3 are lost to the fixed line, and F3 is doubtful.
- Caveats: one unit, white keys only, and windowed levels rather than intercepts. E6 (−58) is an
  outlier in its run.

## 5d. Pass 2 design: the keyboard pass, scored chain-identically

- **Instruments.** One prior sample is one instrument:
  - felt graded z*_bass ≥ z*_mid ≥ z*_treble (three log-uniform draws, sorted); the order is
    documented in US 2,942,512 (pad thickness growing toward the bass); p per grade;
  - m_h and v_ff shared across the keyboard (contact time is degenerate in m_h and z*, so the
    per-grade z* absorbs any mass grading);
  - x_s and geometry per reed.
  - Grade boundaries are a nuisance over three candidate splits: 46|47 · 72|73 (the damper-felt
    grades), 52|53 · 72|73 (the code's sections), and 52|53 · 74|75 (the reed-blank and
    pickup-section splits).
- **Per-note test.** Falsifier 10a's template applied at every note with a clean own line,
  scored at the LOW-damping corner so heavy model damping can't hide a loud contact; the high
  corner is reported alongside.
  - Notes: B2, C3, D3, B3, C4, F4, G4, A4, B4, C5, D5–E6.
  - Excluded: G3, A3, D4, E4 (fixed lines); A2, E3 (ambiguous); F3 (ratio outside the band);
    F6–C7 (doubtful); A1–G2 (unresolved).
- **Robust rule:** an instrument is falsified if, within any grade, more than a quarter of these
  notes have the model's mode 2 above the recording's + 10 dB. Per-reed scatter is ±10 dB, so
  judge the grade, not single notes. Swing edges apply at every note.
- **Chain-identical scoring.** A bench path renders a prescribed modal excitation (per-mode
  amplitude, phase and FE frequency at separation) through the real pickup and preamp, measured
  by the same Sensor procedure as the recording. The renders calibrate the cheap proxy (pickup-
  region ratio + chain tilt) per note, including each section's pickup law. The calibrated proxy
  then scores every instrument.

**Amendments, registered before any keyboard run (2026-10-29):**
- **Geometry conditioning.** At each note with an own line, the measured mode-2 ratio is evidence
  about that reed's geometry. The FE variant nearest the measured ratio is used there; elsewhere
  the variants stay equally weighted.
- **Calibration.** About 30 prototype strikes per note are rendered through the bench at the
  LOW-damping corner (`render --modal-spec`, preamp tap, power amp and speaker bypassed) and
  measured with the survey's onset-window Sensor procedure, at the model line's known frequency
  (±4 Hz). The chain − proxy offset is fitted per note as a linear function of swing (at C4:
  median −3.4 dB, r = −0.89 with swing, from the pickup's gating).
  - A mode 2 below the Sensor's detection counts as passing the one-sided test.
  - A mode 2 within 25 Hz of a harmonic is unresolvable, and the geometry conditioning above
    removes it.
- **Recording side.** The per-note thresholds are the survey's windowed onset levels re H1 at
  v126 (§5c), + 10 dB.

## 5e. Pass 2 results (2026-10-29, 1000 instruments × 23 notes per split, registered rules)

`tools/hammer_contact_score.py`, three boundary splits.
- **Survivors:** 0.6 % (damper-felt split), 0.2 % (code), 0.4 % (blank-pickup). The swing edge
  falsifies 90 %; the mode-2 rule falsifies 95–98 %.
- **The survivors are consistent across splits:**
  - graded felt in the documented direction: z* about 2.4–4.0 mm (bass), 1.6–1.8 (middle) and
    0.7–0.9 (treble);
  - single contacts through the middle, about 0.8–1.3 T1;
  - light hammers (1.1–1.8 g) at low ff speed (1.5–1.9 m/s), both at their prior floors.
- **That edge-hugging traces to the swing edge, which is a numerical limit** (the pickup tables'
  reach, per the swing prior's own comment), not physics. Under the mode-2 rule alone:
  - survivors are 2.5–5.0 %, and m_h and v span their priors;
  - **the felt grading is robust:** z* about 3.6–4.3 / 2.3 / 0.9–1.2 mm;
  - C4 contact is 1.0–1.15 T1 (Miessner again);
  - A1's ff swing is median 4.6–5.6 mm, up to 8–11 mm, beyond the tables.
- **Open questions:**
  - **(i) The bass ff swing bound.** What physically limits it (damper clearance, the comb,
    crit 6's swell defect, which is report-only by operator choice), or do the tables need
    extending to render larger swings? A numerical edge must not condition physics inputs.
  - **(ii) The top treble.** It barely swings in every survivor (C7 ≈ 0.00–0.03 mm against the
    old table's 0.22 mm): a quasi-static push at 2–3 T1. Either the real treble is that weak and
    the per-note voicing makes up the level, or the top notes need harder felt or a lighter
    hammer than the measured D5–E6 lines allow.
    - Proposed evidence: relative note levels from the reference set's noise floors (peak
      normalisation at a constant recording gain shifts each file's floor by its own gain).
    - Self-test first: the layers of one note must order by velocity.
    - **Self-test run 2026-10-29: FAILS.** C4's third-octave bands from 6.3 to 16 kHz are flat and
      time-invariant (noise), and they fall with velocity, but by only ~6 dB from v10 to v126
      (−97.4 to −103.2 dBFS). The three softest layers share −97.4, which looks like a floor added
      after normalisation, and v84 is out of order. The noise floor can't recover relative note
      levels in this set, so (ii) needs other evidence.
- **(ii) Evidence for the top treble (2026-10-29)**, sought instead of a design-intent assumption:
  - **Factory target (D):** service manual §18: voicing "insures the tonality, volume, and ring time
    of the individual notes are uniform and consistent. Loud notes, soft notes, and notes that have
    a piercing tone, will be eliminated." The means are reed and pickup alignment, lateral centring,
    and small pickup bends. The manual gives no tolerances. Trouble-shooting covers "extra loud"
    notes (bend the pickup ends 1/32–1/16″) and "duds"; it has no weak-note entry. Treble pickups
    are thick and effectively not bendable (Doc Wurly: only F-21 to D-42 flex).
  - **Physics (model evaluation on the shipping law):** raising a top-treble reed toward the band
    (from g0 0.10 to 0, since resting inside the pickup is forbidden) adds only 1.2–1.4 dB, because
    the R43_64 slope is nearly flat there. Even loudness must therefore come from the swing.
  - **Minimum top-treble swing** to sit within N dB of C4 (0.6–1.2 mm swing, g0 0.085) at the best
    voicing: 6 dB → 0.06–0.08 mm; 10 dB → 0.04–0.05; 20 dB → 0.012–0.015.
    - Pass-2 survivors at C7 (0.00–0.03 mm) sit 12–24+ dB below C4 even fully voiced.
    - Anything under ~0.012 mm fails at any tolerance up to 20 dB, i.e. 100× weaker in power.
  - **Proposed falsifier (for the operator):** report the tolerance visibly, at 6 / 10 / 20 dB.
    Reject only at the lenient end: a top-grade note more than 20 dB below C4 at its best voicing.
    No reading of "uniform volume" admits a 100× power deficit. The caveats (voicing by ear on the
    speaker, equal-loudness contours) shift this by a few dB, not 20.
  - Doc's "one or two thousandths makes a big difference" concerns the lateral side gaps (his
    words), not height, so it does not conflict with the flat height sensitivity.
- **Photo measurements (2026-10-29).** 4K stills from Emma Repairs videos, with a steel rule in
  frame (±4 %), read by the schematic reader:
  - **Hammer head:** a felt strip folded into an inverted U over a thin wooden blade, the U's axis
    along the reed.
    - Contact width along the reed: crown 8.9–9.4 mm (MEASURED), widest 10.2–11.6 mm.
    - Across the reed: ~10.7 mm (DERIVED, from an assumed 13.75 mm hammer pitch).
    - Felt over the wood: crown 3.0–3.3 mm, legs 3.7–4.3 mm (DERIVED).
    - No size difference visible between rows (sections unidentified).
  - **Strike point** (MEASURED, one hammer, one Model 200, upper middle or treble, ≈ reeds 38–56):
    - crown centre 7.1 mm from the reed bar's edge;
    - 0.21–0.24 L from the bar edge, or 0.33–0.37 L from the clamp fillet; the convention depends
      on where the length table's L starts, which is undocumented.
    - That is clamp-ward of the 0.40–0.60 L prior and of "about halfway", and consistent with
      Miessner's 0.25–0.35 L.
  - **Dampers:** V-notched pads over the reed just tip-ward of the clamp screw (qualitative).
  - **Implications:** the contact patch becomes a physical width (~9 mm, so ~0.12 L in the bass and
    ~0.25–0.3 L in the upper middle); the x_s prior must reach nearer the clamp; and ~3 mm of felt
    bounds z* well below the 4 mm that pass 2's bass survivors used (bass felt is thicker by the
    patent, but unsized).
- **Geometry sources** (shelf search, 2026-10-29):
  - No damper lift, no damper-to-reed contact point and no hammer-head dimension anywhere in the
    manual or parts listings.
  - The damper sits on the reed above the hammer. Practitioners say the bass solder tips face down
    so that hard-hit reeds don't "whack the dampers", so the damper is a plausible but unsized
    upward limit.
  - The strike line is set per hammer at the factory and voiced by ear in service (manual §9 and
    §18: slide the reed bar, move the hammer "to improve the tone and sustain"). That supports a
    per-note x_s prior.
  - No side-on action photo with a scale is held. The candidate frames are in Emma's restoration
    videos (side-on action segments with known timestamps), which would have to be downloaded.
- 1000 instruments leave 2–6 survivors per split. A stable posterior needs sequential sampling
  around them (an ABC-SMC step) once (i) is settled.

## 5f. Pass 3 registration (2026-10-29, before any pass-3 run)

**Model form** (physics review answers to §5e and the photo reads):
- **Contact: a Winkler layer on the blade.** The crown is a physical length along the reed
  (9.2 mm; band 8.9–11.6 mm, length changes ≤ 0.4 dB), sampled at 41 points.
  - Each point compresses locally, δ_i = y_h − w(x_i), with f_i = (10 N / 41)·(δ_i / z*)^p while
    δ_i > 0, so a uniform compression δ gives the same total as the rigid form.
  - Modal forces are Σ_i φ_n(x_i) f_i. Crown tilt (unmeasured) is carried as strike-point scatter.
- **Strike point in mm from the clamp,** shared across the keyboard: d_s = d_ref·(L / L_ref)^k.
  - k shared, uniform 0–2. (0 = rail parallel to the bar; 1 = a constant fraction of L; > 1 = the
    bass struck further out, as Emma's let-off evidence requires.)
  - Per-hammer scatter ±1.5 mm (uniform), floor d_s ≥ 5 mm.
  - Anchor (the photo, one hammer on a 206A): crown centre 7.1 mm from the bar edge. The L origin
    is unknown, so it is carried as a nuisance with two cases, pending identification of the
    photographed reed:
    - bar edge: d_ref 7.1 mm at L_ref = 31.5 mm;
    - fillet: d_ref 13.2 mm at L_ref = 37.5 mm.
- **Felt and hammer:** felt graded z*_bass ≥ z*_mid ≥ z*_treble, p per grade, m_h and v_ff shared,
  boundaries over the three splits, as in §5d.

**Falsifiers** (each a separate term, so any can be dropped to see whether they agree):
- **F1 (mode 2):** the §5d per-note rule at the own-line notes, chain-identically calibrated.
- **F2 (let-off, Emma, 206A, practitioner):** on a loud strike (v_ff), the swing at the crown's
  tip-ward end must reach the let-off gap for the lowest n_b reeds.
  - Gap: 1/8″ ± 1/32″ = 2.4–4.0 mm, a nuisance. n_b: 7–13, a nuisance.
  - It is a LOWER bound; above n_b the absence of complaint is report-only.
- **F3 (shield):** tip swing ≤ 12 mm (the grounded shield ~½″ above the comb).
- **Validity flag:** peak felt compression d_max ≤ h_grade / 2, with h_treble 3.0–3.3 mm (photo).
  Bass and middle are uncapped until their felt thickness is known.

**Report only, pending operator decisions:** the bass fundamental's swell against the reference
set's v126 bass samples, and the treble voicing bound at 6 / 10 / 20 dB. The numerical table edge
is no longer a falsifier.

## 5g. Pass 3 results (2026-10-29, 1000 instruments × 26 notes, 2 L origins × 3 splits)

Integration is chunked by contact stiffness: each group of 64 samples takes its own step, and the
result is identical to one batch within 4e-6. Each falsifier alone, then in combination:
- **F1 (mode 2) alone:** 5.8–12.1 %.
- **F2 (let-off, 10 reeds, 3.2 mm) alone:** 2.1 % (fillet origin), 0.7 % (bar edge).
- **F3 (shield):** 83–90 %. **Validity flag:** 94.9 %.
- **F1 + F3 + valid:** 34–81 survivors. **F2 + F3 + valid:** 4 (fillet), 0 (bar).
- **All four at the primary (10, 3.2 mm): 0 in every case.** Across the nuisance grid, only (7, 2.4)
  leaves 1–2, fillet origin only.
- **The mode-2 evidence and the let-off evidence do not agree under the current model and priors.**
  Not yet interpretable:
  - (a) F1's chain calibration for B2, C3 and D3 covers swings only to ~4 mm. The large bass swings
    that F2 demands are extrapolated and lie beyond the pickup tables (the bass extension had
    finished 1 of 4 ranges when paused).
  - (b) F2 alone is barely reachable within the m_h (1–5 g) and v_ff (1.4–3.5 m/s) priors, so
    either the priors are low or the let-off reading is too strict.
  - Both go to the physics review before any change.
- **L origin (photo read, 2026-10-29): fillet.** Counting reeds from the bass-to-treble solder
  switch at a comb-section joint puts the measured hammer under reed #4 or #5 bass-ward of it, at
  ≤ 49 / ≤ 48. The fillet set (39–47) fits; the bar-edge set survives only as reed 49 under narrow
  conditions. So the fillet-origin runs are the relevant ones.
  - The absolute reed depends on where the 200A comb splits: 42|43 gives reeds 38/39, 50|51 gives
    46/47. A 207VA frame weakly favours 42|43.
  - A parts drawing of the comb split would settle it.

## 5h. Pass 3b registration (2026-10-30, before any pass-3b run)

Physics review of the pass-3 conflict: not the calibration extrapolation. In the let-off regime
(m_h comparable to the bass M_eff, soft felt) contact lasts ~0.8 T1, and at some strike points the
reed hands its energy back to the hammer. Tip swing then jumps with the strike point by up to ~12×
within ±2.5 mm (F#2: 16 / 9.7 / 1.3 / 8.9 mm at 0.40 / 0.44 / 0.50 / 0.56 L). Random per-hammer
offsets make every bass note a lottery.

Three changes, no new physics:
1. **Voiced per-hammer offsets for F2 and F3.**
   - §9 and §18 set each hammer by ear for tone, volume and sustain, and nobody leaves a dead note
     or a reed on the shield. So F2 and F3 pass at a note if ANY offset in {−1.5, −1.0, …, +1.5} mm
     about the strike-point law satisfies both, at the same offset.
   - This applies at MIDI ≤ 60. In pass 3, tips above that stayed ≤ 7 mm. Above 60, F3 uses the
     random offset.
   - F1 keeps the random offset at v_ff.
2. **One bass blank per instrument:** thickness uniform .020–.026″, applied to every reed 1–20,
   evaluated on four FE levels (.020 / .022 / .024 / .026″). The measured mode-2 ratios at MIDI
   47–50 (6.50–6.54) fall between the .020″ (6.27) and .026″ (6.80–6.94) FE values. Geometry
   conditioning on the measured ratio stays for reeds 21 and up.
3. **F2 and F3 at v_max, not v_ff.**
   - The choke and the shield concern the hardest real playing.
   - v_max: log-uniform from v_ff to 5 m/s, per instrument. That is Askenfelt & Jansson II's
     forte 5 m/s over a 47 mm blow, scaled by the 200A's 27.4 mm blow: ~2.9 m/s at forte and ~4.9
     at the fastest key, graded G.
   - F1 stays at v_ff (the reference set's top layer).

**F4 (operator-approved 2026-10-30):** treble voicing. At the top-grade sample notes (MIDI 84, 90 and
96) the pickup swing at v_ff must be ≥ 0.012 mm, the lenient end of the 20 dB bound. 6 and 10 dB are
reported.

**Not used (operator decision 2026-10-30):** the reference set's bass fundamental swell. Criterion 6
stays report-only.

**Only if pass 3b still leaves no survivors:** grade m_h with the felt (bass ≥ mid ≥ treble), registered
before that run. The physics review withdrew its earlier "share m_h", because a swing bound breaks
the m_h–z* degeneracy.

**Pass 3b results (2026-10-30, 1000 instruments × 26 notes, 2 origins × 3 splits).**
- **F2 + F3 (voiced, at v_max):** 14.6 % pass alone at the fillet origin (2.1 % in pass 3) and 1.9 %
  at the bar edge. F1 alone 8–16 %; F4 83–86 %; validity flag 95 %.
- **All registered checks at the primary (10 reeds, 3.2 mm):** fillet 2 / 5 / 4 survivors (code /
  damper-felt / blank-pickup), bar edge 1 in each split.
- **Fillet/code survivors:**
  - k 1.3–1.85, so the strike runs from 0.55 L at A1 through 0.36–0.40 L in the middle to 0.28 L
    at the top;
  - z* about 4.0 / 3.5 / 0.84 mm (bass / middle / treble);
  - m_h 1.2–2.0 g, v_ff 2.5–3.3 m/s, v_max 4.1–4.5 m/s, bass blank .021–.024″.
- **Bass at v_ff:** pickup swing 6–8 mm, and only 50 % single contact at A1–F#2. That is beyond the
  shipped tables, in the regime where the overlap law predicts the Andersen swell (criterion 6,
  report-only) and where the physics review flagged fatigue.
- **Middle:** single contact, 0.8–1.7 T1.
- **Treble:** C7 pickup swing 0.049 mm (within 10 dB of C4; not 6 dB).
- **Open:**
  - A middle z* of 3.5 mm implies very soft middle felt. The middle felt's thickness is unknown, and
    the validity flag covers only the treble.
  - 2–5 survivors per split is too few for a pick. The next pass needs more samples under the
    refined anchor below.

**Anchor refinement (2026-10-30, registered after pass 3b was launched with the §5f anchor and
before any pass-3b result was read).** The manual's service parts list (PDF p74) lists the pickup as
four parts by reed range: 1–20, 21–32, 33–42 and 43–64. So a comb joint sits at 42|43, and 50|51 is
not a boundary. Doc Wurly's "F-21 to D-42 much thinner" corroborates 20|21 and 42|43.
- The photographed hammer, reed #4/#5 bass-ward of a comb joint with the solder switch, is
  therefore reed 38 or 39 (MIDI 70/71).
- Its length-table length is 39.8–40.4 mm, consistent with the photo's 35–40 mm at the fillet
  origin.
- Anchor for the next pass: d_ref = 13.2 mm from the fillet, L_ref = 40.1 mm. The bar-edge origin is
  dropped.
- Caveat: early 200-era "reed plates" at reeds 43–52 could mimic a joint in a photo. The read joint
  showed comb hardware (a slotted hole and bar bolt), and the 206A is 200A-era.

## 5i. Pass 3c registration (2026-10-30, before any pass-3c run)

- **Correction:** the 3.0–3.3 mm felt thickness was read off a 207VA hammer of unknown section,
  scaled by an assumed pitch. It was never measured on the 206A hammer, which is now known to be
  reed 38/39, a middle-grade hammer. The treble-only validity flag rested on that misattribution.
- **Validity flag, all grades, from geometry.**
  - Hammers sit side by side at the reed pitch. A folded strip is 2·h_leg + blade wide, and its
    crown is no thicker than its legs. So h ≤ (13.75 − 2.9 − 0.5)/2 ≈ 5 mm in any section (pitch
    assumed; the reed pitch read is pending).
  - The 206A frame shows pads at 0.74–0.81 of the pitch, with no row visibly different, which puts h
    nearer 3–4 mm.
  - Flag: **peak felt compression ≤ 2.5 mm at every note and grade, at each check's own speed** (v_ff
    for the random strike; v_max for the voiced offsets and the above-60 shield strike). A voiced
    offset counts only if it also satisfies the flag.
- **Registered expectation** (physics review, from pass-3b data): in the middle notes d_peak ≈ 0.27·v·τ,
  so a 2.5 mm cap at v_ff 2.9 m/s allows τ ≤ ~3.1 ms (0.8 T1 at C4, 1.4 at A4). F1's weak middle mode 2
  wants longer contacts. Graded m_h cannot relieve this. Only a lower v_ff, more mode 2, or a mode-2
  loss during contact can.
- **Run:** fillet origin only, anchor 13.2 mm at L_ref 40.1 mm (reed 38/39); 3 splits × 5000
  instruments; otherwise §5h. A sequential (SMC) step only once a split has ~20+ survivors.
- Report only: crit 6 (the swell) once the ±12 mm tables are in; until then any bass output reading at
  6–8 mm is extrapolated. Fatigue at the bass clamp edge (0.4–0.85 GPa at A1, 7–8.4 mm tip) is at the
  endurance edge, and bass breakage is documented.

**Pass 3c results (2026-10-30; 5000 instruments × 26 notes per split, fillet anchor at reed 38/39).**
- **Alone:** F1 9–12 %; F2 + F3 voiced, with the compression flag inside the offset choice, 4.6 %;
  F4 85 %; validity flag (all grades, ≤ 2.5 mm) 64–65 %.
- **All at the primary: 2 / 4 / 3 survivors** (code / damper-felt / blank-pickup), 9 in total. The
  survivors agree across splits:
  - **graded felt, z* about 2.2 / 1.8 / 0.55 mm** (bass / middle / treble). The flag removed pass 3b's
    soft 3.5 mm middle.
  - m_h 1.2–2.0 g; **v_ff low at 1.6–2.2 m/s**; v_max 2.6–3.7 m/s;
  - **k ≈ 1.89**: strike at 0.57 L at A1, 0.31–0.37 L in the middle, 0.20 L at the top;
  - bass blank .021–.024″.
- **Contacts:** single contact nearly everywhere. Bass contact 0.2–0.45 T1, middle 0.6–1.25 T1,
  treble 0.6–1.8 T1.
- **Swings:** A1 pickup swing ~6 mm at v_ff. C7 0.059 mm (within 10 dB of C4).
- **This matches the registered expectation:** with the compression cap, only a lower v_ff keeps the
  middle contacts long enough for the weak middle mode 2.
- **Open:**
  - 9 survivors is below the ~20 per split needed for a sequential (SMC) step.
  - The bass swing (~6 mm) awaits the ±12 mm tables for the swell report and calibration.
  - Hammer mass, felt thickness and the reed pitch from the technician or a photo would narrow the
    prior most.

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
