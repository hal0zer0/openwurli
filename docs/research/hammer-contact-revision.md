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
- **All at the primary: 2 / 4 / 3 survivors** (code / damper-felt / blank-pickup): 9 rows but 5
  distinct instruments, because the splits share one set of draws (correction in §5j). They share:
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

## 5j. Pass 3d registration (2026-10-30, before any pass-3d run): the strike floor

- **Form error found in review:** the 5 mm strike floor ("half the crown plus clearance") was
  bar-edge arithmetic, set while the L origin was open. Under the fillet origin, the reed bar
  casting lies under the reed's first ~6 mm: the bar edge is 5.9–6.3 mm tip-ward of the fillet
  (photo read). A hammer striking from below cannot reach there.
- **Pass 3c's survivors at k ≈ 1.89 are therefore non-physical in the treble:** C7's crown at
  ~5–6 mm sits over the casting. In pass 3c's prior the crown starts within 1 mm of the bar edge for
  22 % of instruments at MIDI 74, 52 % at 79, 71 % at 84 and 87 % at 96.
- **Floor (fillet coordinates):** bar edge + half the widest felt, so d_s ≥ 11.5 mm with the widest
  felt (10.2–11.6 mm) touching the bar edge. The photo's 13.2 mm is the top of that band.
- **Law:** d_s = max(floor, d_ref·(L/L_ref)^k), with the voiced offsets clipped at the floor too. k
  then governs only the bass and middle; the treble strike sits at ~11.5–13.2 mm (C7 at 0.45–0.52 L,
  near the patent's mid-reed and the technician's "halfway").
- **Registered expectation (physics review, G):** moving the treble strike out lowers treble swing
  for a given felt, so F4 needs stiffer treble felt than pass 3c's 0.55 mm. **Failure clause:** if
  treble survivors keep z* ≥ 0.55 mm with F4 passing at the floor, the expectation is wrong.
- **Run:** otherwise identical to pass 3c (5000 per split, 3 splits). Next: pool the splits for a
  sequential (SMC) step if the survivors agree, or run a larger prior pass first if there are fewer
  than ~10.
- Side check (for the record): the FE clamp stays at the fillet. The bass mode-2 ratios fall inside
  the FE band for 4 of 5 notes with the clamp at the fillet, against 0 of 5 with it at the bar edge.

**Pass 3d results (2026-10-30; 5000 instruments × 26 notes per split, floor 11.5 mm, seed 51).**
- **Alone:** F1 8–11 %; F2 + F3 voiced 4.5–4.7 %; F4 83 %; validity flag 63–64 %.
- **All at the primary: 0 / 4 / 1 survivors** (code / damper-felt / blank-pickup), 5 distinct
  instruments.
  - z* bass 1.0–2.3, middle 1.0–2.2, treble 0.45–0.89 mm.
  - m_h 1.06–1.40 g, plus one at 3.9 g; v_ff 1.86–2.29 m/s; k 1.57–1.96.
  - The 3.9 g instrument passes F4 at the margin (C7 swing 0.015 mm against 0.012).
- **The registered expectation fails, by its own clause.**
  - All five survivors strike MIDI 84, 90 and 96 at the floor, and three keep z*_treble ≥ 0.55 mm
    (0.57, 0.79 and 0.89 mm).
  - Across the prior, the floor lowers top-treble swing: the median at MIDI 96 goes from 0.172 to
    0.097 mm, and at MIDI 90 from 0.38 to 0.29 mm.
  - But F4's z* boundary holds. The z*_treble where the local F4 pass rate falls below 50 % is
    0.76 mm in pass 3c and 0.79 mm in pass 3d, and per-bin pass rates above 0.55 mm agree within
    sampling noise. Below 0.55 mm the stiffest felt loses 3–5 points (99 → 95 % under 0.2 mm).
  - F4 failures are the long contacts, about 3 T1 (median) in both passes, against 1.0–1.4 T1 for
    strikes that pass. The floor moves the bulk of the swing distribution, not this tail. That is a
    reading of the data, not a derivation.
- **Correction to §5i: the splits are not independent.**
  - One seed draws the instruments for all three splits of a pass, and the splits differ only in the
    grade boundaries.
  - Pass 3c's 9 survivor rows are 5 instruments; two of them survive in all three splits. The
    cross-split agreement claimed there is one set of draws scored three ways, not replication.
  - Within a pass, the splits are a paired comparison of the boundary nuisance.
- **Next, by the §5j rule (fewer than ~10 survivors): the larger prior pass.**
  - Three more seeds (52, 53, 54) × 5000 instruments per split. Rules, priors and code are
    unchanged (`keyboard3b --origin fillet-floor`, scored with `tools/hammer_contact_score3c.py`).
  - Pooled with pass 3d to 20 000 instruments per split.
  - Survivors are counted as distinct instruments. The §5i/§5j rule then decides between an SMC
    step and a report.

**Pooled larger pass results (2026-10-30; seeds 51–54, 20 000 instruments per split;
`tools/hammer_contact_pool.py`).**
- **6 / 16 / 9 survivors** (code / damper-felt / blank-pickup): **19 distinct instruments**, 6 of
  which survive in all three splits.
  - Pass 3d's 0 in the code split was sampling. Every code survivor also survives in the other two
    splits.
- Survivors, p10 / 50 / 90:
  - z* bass 1.43 / 2.16 / 2.72 mm, middle 1.07 / 1.35 / 2.09 mm, treble 0.45 / 0.60 / 0.96 mm;
  - m_h 1.15 / 2.26 / 3.50 g; v_ff 1.68 / 1.99 / 2.78 m/s; k 1.59 / 1.81 / 1.95.
- **k piles against the prior edge:** all 19 sit in [1.5, 2.0), and 26 % are within 0.1 of 2
  (5 % under the prior). Bounded at 2, this gives only a lower bound on k; stage 4 widens the range
  (§5k).
- 7 of 19 pass F4 by less than 3 dB.
- 19 distinct survivors meets the ~20 of the §5i / §5j rule, so stage 4 runs, seeded by these 19.

## 5k. Stage 4 registration (2026-10-30, before any run): sequential importance sampling

- **Why:** pass 3d keeps ~0.1 % of prior draws. A prior pass large enough for a stable posterior
  is not affordable, but sampling the same target from a proposal adapted to the survivors is.
- **Physics reading of pass 3d (physics review):**
  - F4 sits on the shock-spectrum cliff, ~18 dB below the median swing at MIDI 96. The residual
    swing of a smooth push falls to −9.5 / −23.5 / −31 dB at contacts of 1 / 2 / 3 T1 (half-sine
    shock spectrum), with nulls between.
  - The strike point is a 5–10 dB lever at the top notes; contact time is a ~21 dB lever. So F4 is
    in effect a contact-time bound, τ ≲ 2 T1 at C7 (~1 ms), set by z*_treble and m_h.
  - No change to the floor (casting geometry) or to F4's form.
- **Target (rules unchanged):** prior × (1/3) Σ_split 1[pass].
  - "Pass" means the §5i checks (F1, F2 + F3 voiced, F4, the compression flag) at the primary
    let-off point, with §5j's fillet-floor strike geometry.
  - The felt-grade split is a discrete nuisance with a uniform prior, marginalised rather than
    chosen: every proposal is scored under all three splits.
- **One prior change, k uniform on [0, 2.5]** (was [0, 2]):
  - Pass 3d's survivors piled at the old edge: 40 % within 0.1 of 2, against 5 % under the prior.
  - The new edge is the pickup-comb ceiling at A1. The crown's tip-ward end must stay short of
    where the reed enters the comb, 0–5 mm before its tip (photo read, one reed; the bass comb is
    unread). With the +1.5 mm scatter, that is reached at k 2.52–2.64, so inside [0, 2.5] the
    ceiling never binds and is carried by the bound alone.
  - k > 2 puts A1 past ~0.6 L, against the technician's "about halfway" and the patent's
    mid-reed. That is reported as a check on the posterior, not imposed as a prior bound.
- **Sampler** (`tools/hammer_contact_smc.py`; ABC-PMC with the rules as an indicator likelihood):
  - Generation 0 is the pooled prior pass's distinct survivors (§5j). It only seeds the proposal,
    because it was scored with k ≤ 2.
  - Each generation draws N = 6000 proposals: 20 % fresh prior draws, and the rest a weighted
    survivor of the previous generation perturbed by a Gaussian kernel (2 × the weighted
    covariance plus a 1 % ridge), redrawn whole until inside the support. They are scored in 3
    chunks, one keyboard seed per chunk shared by its three splits.
  - Weight = prior / proposal density × (splits passed) / 3.
  - Bookkeeping check: with an always-pass likelihood, the weighted proposals reproduce the prior
    means within 0.6 % of each range, and E[prior / proposal] = 1.01.
  - Stop when a generation's effective sample size reaches 300, or after 4 generations. The
    posterior is that generation's weighted sample, with the previous generation's medians shown
    for stability.
- **Reported:**
  - weighted p10 / 50 / 90 for every parameter;
  - the split posterior;
  - the k mass above 2 and within 0.1 of 2.5;
  - the share with A1 struck beyond 0.6 L;
  - the share with an F4 margin under 3 dB.
- **Registered expectations (physics review):**
  - The split posterior favours damper-felt (46|47): pass 3d gave 0 / 4 / 1. That is weak with a
    handful of survivors, so it is falsified only if the posterior favours a different split.
  - If the k mass piles within 0.1 of 2.5 as well, the result gives only a lower bound on k and is
    reported as such.
- **Calibration caveat, found in pass 3d's F1 failures and registered before any stage-4 result:**
  - F1 fails in every grade, and among the pass-3d instruments that pass everything else it is the
    whole difference between the splits.
  - In the code and blank-pickup splits, the bass grade has 3 scored notes (MIDI 47, 48, 50), none
    of which may fail. MIDI 48 alone fails it in 173 of 202 instruments, at a median pickup swing of
    4.7 mm.
  - The chain renders behind F1's offsets use the shipping tables (u within ±4.5 mm, rest at
    u = −0.085 mm). So any pickup swing above 4.415 mm reaches a clamped law, and MIDI 48's offset
    there is fitted partly on clamped renders. The split posterior therefore leans on a provisional
    bass calibration.
  - Elsewhere the offsets are extrapolated in swing: for 30–45 % of the F1 failures at MIDI 71, 74,
    81, 84, 86 and 88, the swing lies beyond the calibrated range. The chain–proxy offset is within
    ±1 dB there, so the risk is smaller.
  - **Fix, registered now:** once the bass tables reach ±12 mm, recalibrate over the posterior's
    swing range and re-weigh every generation. Only the scores change, not the integration, and the
    weights stay valid for the new likelihood because every generation carries fresh prior draws.
    The re-weighed posterior supersedes the first, and both are reported.

**Bass calibration bracket results (2026-10-30; `tools/pickup_bracket.py`, overlap-law §6).**
- **Setup:**
  - Reeds 15–20 (MIDI 47, 48 and 50 are all in that table) were re-solved whole with the fixed
    solver to ±12 mm.
  - Eight corner tables: {plane −12 mm | converged} × {neighbours mirrored | at rest} × {no tilt |
    tilt-averaged, MIDI 48 mode shape: displacement 0.91–1.09 × pickup swing across the finger
    region}.
  - One preamp-bench build per corner, plus the shipping tables built the same way. Each was
    calibrated on MIDI 47, 48 and 50 with identical strikes (seed 7, n 90) and one Sensor build.
  - The same tools reproduce the original n 30 calibration exactly (0 of 90 rows differ).
- **The registered mechanism is refuted: the clamp does not matter.**
  - Paired per strike, the extended law with the plane at −12 mm matches the clamped shipping law
    to +0.0 dB at every swing band, up to 13 mm.
  - The mode-2 line is made where the law bends hardest, near the peak (u 0–3 mm), not in the
    tails.
- **The far-field closure does matter.** Paired against the shipping law, by MIDI:

  | Correction | MIDI 48 | MIDI 50 |
  |---|---|---|
  | Converged plane, swings 3–6 mm | +1.9 to +3.0 dB | +1.2 to +1.8 dB |
  | Converged plane + neighbours at rest, swings 3–6 mm | +2.6 to +4.6 dB | +1.9 to +2.4 dB |
  | Neighbours at rest, plane −12 | −0.9 to −1.0 dB | −0.3 to −0.9 dB |
  | Tilt (band medians) | ≤ 1.0 dB | ≤ 0.3 dB |

  - Below 3 mm every corner is within 0.2 dB.
  - The corrections are not additive: plane and neighbours interact by up to 3 % of range at
    u < −4.
- **Verdicts are scored on two corners; the other six are sensitivity only** (physics review):
  - Neighbours at rest is the known configuration for the reference samples, not an alternative.
    They are single notes, and at MIDI 47–50 the neighbours are damped.
  - Tilt is physics too: the FE gives the reed's bend across the overlap.
  - So the only unknown left is the closure. The two verdict corners are plane −12 and the
    converged plane, both with neighbours at rest and tilt. An instrument that survives both is
    stable; one that survives either alone is unresolved.
- **Verdicts** (the pooled pass re-scored; only MIDI 47, 48 and 50 changed):
  - **19 instruments are stable** — the same 19 as the pooled pass. They also survive in all
    eight corners.
  - **4 more survive only with the plane at −12 mm: unresolved.**
  - Survivors per split (code / damper-felt / blank-pickup): 12 / 19 / 18 at plane −12 against
    6 / 16 / 9 converged. Damper-felt leads clearly only with the converged plane, so **the split
    preference is not decided by the tables**.
- **Caveats still open:**
  - True 3D end fringing is unbracketed. It would act at the same few-percent-of-range level near
    the band as the closure does, so it sits beside the closure, not with the tails.
  - The clamp refutation covers the mode-2-re-H1 statistic only. The H1–H2 balance at large swing
    (criterion 6's swell, report-only) is shaped by the tails and is not covered.
- **Calibration sampling is a second limit.**
  - Re-measuring the shipping law with n 90 instead of 30 moves the same 4 instruments, because the
    original bass calibration rested on 6 and 9 resolved lines at MIDI 47 and 48.
  - Stage 4 is therefore weighed under each corner's n-90 calibration, and the spread is reported.

**Stage 4 results (2026-10-30; 4 generations × 6000 proposals; `tools/hammer_contact_smc.py`).**
- **Sampler:**
  - Each generation was weighed on both verdict corners, with plane −12 and the converged plane
    (both with neighbours at rest and tilt). The next proposal used the mean of the two weightings
    (`mix`).
  - Survivors kept: 6–7 % of proposals, against 0.1 % under the prior.
  - Effective sample size by generation (plane −12 / converged): 28 / 24, 212 / 207, 256 / 179,
    254 / 216. **The registered target of 300 was not reached in 4 generations**, so this is
    reported as registered.
  - From generation 3 to 4 the medians move by ≤ 8 % (largest: m_h, 1.71 → 1.84 g converged). The heavy particles in generation 1 sat at
    k > 2, where the generation-0 kernel (k ≤ 2) was thin; fresh draws carried 7 % of the weight.
- **Posterior (generation 4), p10 / 50 / 90, plane −12 | converged, against the prior:**

  | Parameter | Plane −12 | Converged | Prior |
  |---|---|---|---|
  | z* bass (mm) | 1.49 / 2.06 / 2.76 | 1.48 / 2.13 / 2.86 | 0.62 / 2.22 / 4.36 |
  | z* middle (mm) | 1.04 / 1.46 / 2.03 | 0.99 / 1.45 / 2.03 | 0.22 / 0.71 / 2.32 |
  | z* treble (mm) | 0.46 / 0.65 / 1.03 | 0.45 / 0.62 / 0.91 | 0.11 / 0.22 / 0.82 |
  | k | 1.49 / 2.02 / 2.31 | 1.55 / 2.06 / 2.35 | 0.25 / 1.25 / 2.25 |
  | m_h (g) | 1.08 / 1.87 / 2.83 | 1.08 / 1.84 / 2.94 | 1.17 / 2.24 / 4.26 |
  | v_ff (m/s) | 1.52 / 2.18 / 3.06 | 1.51 / 2.03 / 2.94 | 1.54 / 2.21 / 3.19 |
  | v_max (m/s) | 2.39 / 3.12 / 4.35 | 2.33 / 3.11 / 4.35 | 2.20 / 3.49 / 4.65 |
  | bass blank (mil) | 21.1 / 23.6 / 25.6 | 21.4 / 23.9 / 25.6 | 20.6 / 23.0 / 25.4 |
  | A1 strike (d_s / L) | 0.45 / 0.62 / 0.75 | 0.47 / 0.63 / 0.76 | — |

- **What the data constrain:**
  - The graded felt (z*) is constrained in every grade: three times narrower than the prior in
    the bass, and centred well above the prior in the middle and treble.
  - k is constrained to about 1.5–2.35, inside the widened range: 3–5 % of the mass lies within
    0.1 of 2.5, against 4 % under the prior.
  - m_h loses its upper tail.
  - The felt exponent p, v_ff and the bass blank stay essentially at their priors: these checks do
    not identify them.
- **Registered expectations:**
  - k piling at the new edge: **no**. k is bounded inside [0, 2.5].
  - Split posterior (blank-pickup / code / damper-felt): 0.38 / 0.23 / 0.39 at plane −12 against
    0.27 / 0.16 / 0.57 converged. The expectation (favours damper-felt) is not falsified: damper-felt
    leads in both, but only by 0.01 at plane −12. **Not decided until the closure is.**
- **Reported checks:**
  - 58–61 % of the mass strikes A1 beyond 0.6 L (median 0.62–0.63 L). That leans past the
    technician's "about halfway" and the patent's mid-reed; the lower tail (p10 0.45–0.47 L) agrees
    with them. A tension to resolve with strike-line evidence, not a falsification.
  - 26 % of the mass passes F4 by less than 3 dB.
- **Open, carried into §6 step 3:**
  - the far-field closure (it decides the split and 4 of the pooled verdicts);
  - 3D end fringing;
  - the large-swing H1–H2 balance (criterion 6, report-only);
  - p, v_ff and the bass blank, which these checks leave unidentified.

## 5l. Stage 5 registration (2026-10-30, before any run): graded hammer mass, ff speed fixed

- **Why (physics review, from the stage-4 proposals re-scored leave-one-out):**
  - F2 drives k, through m_h. With F2 + F3 voiced, the pass rate climbs from 0.09 (A1 struck
    before 0.45 L) to 0.51 (0.65–0.70 L), while F1 stays flat.
  - At m_h ≤ 1.5 g, F2 keeps rising out to 0.70 L. At 2.3–5 g it peaks at 0.50–0.55 L.
  - Heavy hammers are rejected by the other grades' mode 2: at 3.2–5 g, F1 treble passes 0.45
    against 0.71–0.77 for lighter ones. ρ(ln m_h, k) = −0.5.
  - With one mass for the whole keyboard, the treble sets the bass hammer's mass, and a light
    hammer reaches the let-off gap only when struck far out.
  - Graded mass is the general form: piano hammers run ~12 g bass to ~6 g treble (Hall &
    Askenfelt, doi:10.1121/1.395917; a piano source, transfer assumed).
- **Prior changes:**
  - **m_h per grade**, ordered bass ≥ middle ≥ treble, each log-uniform 1–5 g (the same range,
    sorted like z*).
  - **v_ff fixed at 2.9 m/s.** The ff speed is a convention, not a free parameter: stage 4 gives
    ρ(ln z*_t, ln v_ff) = +0.75, because F1 and F4 pin the ff contact time and the felt follows
    whatever speed is called ff.
    - The value is §4's derived one: action ratio ~2.9 × peak key speed 1 m/s, which agrees with
      Askenfelt & Jansson's forte 5 m/s scaled to the 200A's 27.4 mm blow.
    - v_max becomes log-uniform 2.9–5 m/s.
    - In stage 4, 13–20 % of the posterior lay at v_ff 2.6–3.2 m/s.
  - Everything else as stage 4: k on [0, 2.5], the rules, and the two verdict corners with their
    n-90 calibrations.
- **Sampler:** as §5k.
  - 12 coordinates, with two ordered triples (z* and m_h). Bookkeeping check with an always-pass
    likelihood: prior means within 0.8 % of each range, E[prior / proposal] = 0.99.
  - Generation 0 is stage 4's last-generation survivors at plane −12 with v_ff 2.6–3.2 m/s, their
    mass copied to all three grades. It seeds the proposal only.
  - Up to 4 generations × 6000 proposals; stop at ESS ≥ 300 on both corners.
  - **Amendment before generation 2 (sampler mechanics only):**
    - Generation 1 came out degenerate, with ESS 2 / 1. One fresh draw with graded mass
      (3.75 / 3.33 / 2.75 g) carries 65–83 % of the weight. The seeds' equal masses left the kernel
      exploring near-equal grades only (survivors' median bass / treble ratio 1.07).
    - A kernel built from those weights would collapse onto that single particle.
    - So when a generation's ESS is below 50, the next kernel uses tempered weights W^α, with α the
      largest giving ESS 50. The target and the importance weights are unchanged, since any
      proposal that keeps fresh draws is valid.
    - Stage 4 never triggers it: its proposals reproduce byte for byte.
- **Registered expectation (physics review), checked on both corners:**
  - the A1 strike median is ≤ 0.58 L, and the posterior median of m_h_bass / m_h_treble is ≥ 1.3.
  - **Failure clause:** the A1 median is ≥ 0.60 L, or the ratio is < 1.2.
- **Reported:** every parameter's p10 / 50 / 90; the split posterior; m_h_bass's mass within 10 %
  of its 5 g ceiling; k within 0.1 of 2.5; and the F4 margin. "About halfway" is reported only: it
  is verbal, of unknown origin, and reads ~0.54 L at A1 from the bar edge or 0.50 L from the fillet.
- **For §6 step 3 (physics review, registered now):**
  - Engine values are held until this run.
  - Ship one coherent instrument: a particle near the posterior's centre in what the checks pin
    (ff contact time per grade, bass reach, F4 margin), scored at both corners. The ear-gate
    corners are particles too, not a mix of marginal quantiles.
  - Freeze the hammer offsets per note, voiced at MIDI ≤ 60. A raw draw can land in a null: F#2's
    tip swing is 16.2 mm struck at 0.40 L but 1.3 mm at 0.50 L.
  - p: take the prior median, and hear the corners. With z* re-solved per p to hold the ff contact
    time, C4 at v/2, v/4 and v/8 reads −6.7, −14.3 and −23 to −25 dB across p 2.36–3.35.
  - **The treble is the velocity map's real problem.**
    - At C7 the centre's ff contact is 2.4 T1, past the cliff. At v/2 its swing drops 21–34 dB,
      then −37 to −88 dB in the nulls.
    - Report C6 / C7 swing against velocity before registering the map.
    - If the treble vanishes erratically, suspect pulse symmetry first: lossless felt gives the
      deepest nulls, and any loss fills them.
  - Velocity floor: the hammer must fly the 1/8″ let-off against gravity, so v ≥ ~0.25 m/s.

**Stage 5 results (2026-10-30; 4 generations × 6000 proposals).**
- **Effective sample size** by generation (plane −12 / converged): 2 / 1, 69 / 63, 77 / 63,
  158 / 79. The registered target of 300 was not reached, so this is reported as registered on
  generation 4.
- **Posterior (generation 4), p10 / 50 / 90, plane −12 | converged** (v_ff fixed at 2.9 m/s):

  | Parameter | Plane −12 | Converged |
  |---|---|---|
  | z* bass (mm) | 1.48 / 1.88 / 2.41 | 1.61 / 1.99 / 2.64 |
  | z* middle (mm) | 1.27 / 1.50 / 1.88 | 1.36 / 1.51 / 1.96 |
  | z* treble (mm) | 0.65 / 0.86 / 1.08 | 0.68 / 0.89 / 1.09 |
  | m_h bass (g) | 1.58 / 2.26 / 3.65 | 1.33 / 2.47 / 3.76 |
  | m_h middle (g) | 1.22 / 1.78 / 2.77 | 1.19 / 1.79 / 3.10 |
  | m_h treble (g) | 1.04 / 1.29 / 1.92 | 1.04 / 1.36 / 2.08 |
  | m_h bass / treble | 1.21 / 1.63 / 2.59 | 1.21 / 1.63 / 2.54 |
  | k | 1.45 / 1.95 / 2.24 | 1.53 / 2.00 / 2.24 |
  | v_max (m/s) | 2.98 / 3.27 / 4.26 | 2.97 / 3.21 / 4.24 |
  | A1 strike (d_s / L) | 0.44 / 0.60 / 0.71 | 0.45 / 0.61 / 0.72 |

  - p and the bass blank stay near their priors.
  - The treble felt is softer than in stage 4 (z* ~0.87 against 0.64 mm). That is the v_ff
    convention at work: F1 and F4 pin the ff contact time, so a faster ff blow needs softer felt.
  - The bass mass shows no pile-up at its ceiling: 2–3 % of the mass lies within 10 % of 5 g,
    against 18 % under the prior. k shows none at 2.5.
- **Registered expectation: half holds, half fails.**
  - Mass ratio: median 1.63 at both corners (bootstrap 90 %: 1.56–1.75), above 1.3. **Holds.**
  - A1 strike median: 0.598 L at plane −12 (bootstrap 90 %: 0.582–0.609) and 0.612 L converged
    (0.606–0.615). Neither reaches ≤ 0.58. The failure clause (≥ 0.60) triggers on the converged
    corner and sits on the line at plane −12. **Fails.**
  - **Reading:** graded mass is real and well constrained, but it moves the A1 strike only from
    0.62–0.63 L (stage 4) to 0.60–0.61 L, not to ≤ 0.58.
  - The A1 median also carries Monte Carlo error beyond one generation's bootstrap: generation 3
    had 0.56 / 0.58. A higher ESS, not a different rule, would settle the plane −12 case.
- **Split posterior** (blank-pickup / code / damper-felt): 0.38 / 0.18 / 0.44 at plane −12 against
  0.17 / 0.12 / 0.71 converged. It still depends on the closure.
- 19–24 % of the mass passes F4 by less than 3 dB.

**Why the A1 strike stays out (physics review, stage-5 proposals re-weighted one check at a time).**
- F2 (let-off reach) and F3 (tip ≤ 12 mm shield) together bound one mode-1 amplitude from both
  sides at the reach reeds. The window at A1 is shut below ~0.38 L and widens further out.
  - Drop the shield, and A1 moves to 0.46–0.48 L with m_h_bass 3.2–3.7 g. Drop F1 bass or middle,
    and it moves ≤ 0.01 L.
  - A heavy bass hammer at ≥ 2.9 m/s overshoots the shield unless struck where the window is wide.
- **The weak input is the 12 mm.** It is "~½ inch above" from video, with no datum and no read of
  whether upper-comb metal lies over the bass tip's path.
  - A1 median against clearance (plane −12 | converged): 11 mm 0.61 | 0.61; 12 mm 0.60 | 0.61;
    13 mm 0.57 | 0.58; 14 mm 0.54 | 0.56; 16 mm 0.50 | 0.47.
  - The let-off grid is a nuisance like the split; marginalised at 12 mm it gives 0.57 | 0.61.
- The posterior's p10–p90 is 0.37–0.72 L, so "about halfway" sits inside it; the median is a weak
  handle on this width.
- **Next stage (to register once the shield is read):** the clearance as a band read off the
  upper comb, with the let-off grid marginalised; run to ESS ≥ 300. Register a distribution (the
  share beyond 0.6 L), with bands wider than the generation-to-generation movement.

**Treble swing against hammer speed (stage-5 posterior, 300 weight-resampled particles per corner;
`tools/hammer_velocity_report.py`; report only).** Swing re the same particle's ff swing,
p50 [p10, p90] dB:

| MIDI | v / 2 | v / 4 | v / 8 |
|---|---|---|---|
| 84 (C6) | −11 [−14, −9] | −27 to −32 [−47, −22] | −54 to −55 [−71, −44] |
| 90 (F#6) | −17 to −20 [−33, −13] | −42 to −45 [−58, −32] | −58 to −61 [−76, −43] |
| 96 (C7) | −21 to −22 [−41, −9] | −37 [−53, −27] | −53 to −58 [−72, −42] |

- At v/8, 93–100 % of strikes fall more than 40 dB below their ff swing.
- At F#6 and C7, 7–12 % of strikes are not monotonic in speed: they land in nulls.
- The contact time lengthens from 1.3–2.4 T1 at ff to 2.8–5.6 T1 at v/8, far past the cliff.
- **This is the pulse-symmetry signature:** the model's felt is lossless, which gives the deepest
  nulls. Whether felt loss is needed is to be settled before the velocity map, because it would
  change every stage's contact law.

## 5m. Felt loss registration (2026-10-30, before any implementation or run)

- **Evidence held** (no measured loss or restitution figure exists for any hammer, piano or
  electric piano):
  - Stulov 1995 (doi:10.1121/1.411912) Table II fits a hysteresis constant ε 0.936–0.985 and a
    relaxation time τ0 1.9–20 µs to Yanagisawa & Nakamura's loops. The measured loops survive
    only as his Figs 1 and 2(a).
  - Giordano & Winans 2000 (doi:10.1121/1.428505) measure loading and unloading branches ~10 %
    apart on soft blows and 10–20 % at f–ff. They attribute much of the apparent hysteresis to
    force propagation within the hammer.
  - Hall & Askenfelt 1988 read their near-symmetric F(t) as "only a very minor role for
    dissipative forces".
  - The 200A's own patent (US 2,942,512, col. 2 ll. 63–67) wants the covering to have "some
    viscosity, to aid in the damping of upper partials".
- **Decision (physics review): carry loss as a nuisance band, in Stulov's hereditary form.**
  - Size: for contacts much longer than τ0, Stulov's law is K[g + τ_d dg/dt] with
    τ_d = ε τ0 / (1 − ε); at fixed τ_d, ε moves the loop < 0.2 %.
  - Giordano & Winans' gaps map to τ_d 0.02–0.09 ms; the Yanagisawa & Nakamura loops to
    ≥ 0.05–0.1 ms, and Stulov's fits to them to 0.10–0.29 ms.
  - **Prior: τ_d log-uniform 0.01–0.3 ms.** The evidence leans low.
  - **Why it matters even though it is "minor":** at Giordano & Winans' size, C4's ff mode 2
    drops 7–9 dB (F1) and C6's ff swing 3–5 dB at fixed contact (F4). A residual 40 dB down the
    contact cliff is set by small asymmetries.
  - **It does not cure the treble collapse.** v/8 stays 40–68 dB under its own ff, because the
    collapse is contact growth, ∝ v^−(p−1)/(p+1). Loss fills the deepest nulls (no +19 dB jumps)
    but leaves jags of up to ~20 dB.
- **Form:**
  - One relaxation state per crown point, with speed-independent constants.
  - F ≥ 0, with the state carried through separations: the felt is still recovering at a
    recontact.
  - z* is defined on the relaxed (slow) law, so the prior keeps its meaning. Stulov's F0 is the
    instantaneous law.
  - τ_d → 0 must reproduce the lossless integrator exactly.
- **Registered test of the form, on held data (to run when the digitised Stulov Fig. 2(a) loops
  arrive: A1 pliant, 0.52–1.43 m/s):**
  - The branch gap at half compression grows ×1.3–1.6 across those speeds under Stulov, and
    ×2.3–3 under Hunt–Crossley.
  - **Stulov fails at ≥ ×2.1.** Boutillon 1988 (being fetched) can take the same test.
- **Next stage (to register once the upper-comb read gives the shield band):** from the pooled
  pass, with τ_d, the shield clearance band and the let-off grid all marginalised. A verdict that
  flips across τ_d is unresolved.
  - **Registered prediction:** F1 binds the middle felt (dropping F1 moves z*_m from 1.50 to
    0.42 mm), so the C4 ff contact median is ≥ 10 % shorter for τ_d ≥ 0.1 ms than for
    ≤ 0.02 ms.
  - **Failure clause:** less than 5 % shorter, or longer.
  - **Reading (physics review, before any run):** at fixed felt the viscous stiffening shortens C4's
    contact by only 1.2 / 2.4 / 4.0 % at τ_d 0.02 / 0.05 / 0.1 ms. That costs C4's ff mode 2
    −2.9 / −5.4 / −8.3 dB with z* held on the relaxed law; this tool gives −4.8 dB at 0.05 ms in a
    neighbouring setup. So a shortening of ≥ 10 % has to come from F1 re-identifying stiffer felt,
    and that is the effect the prediction tests.
- **Operator's call, report-only:** whether a real 200A's top octave speaks at a light touch is an
  observation, not a model output. The model's answer (−40 to −68 dB at v/8 under its own ff) is
  reported beside it, not tuned to it.

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
