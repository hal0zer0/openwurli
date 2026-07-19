# Reed–Pickup Displacement (d₀) — What We Know, What We've Inferred, and Why

> **Status: LIVING PROVENANCE DOCUMENT.** The reed-to-pickup rest gap d₀ — the
> single most load-bearing unknown in the whole model — is **not documented in
> any patent, service manual, or factory record we have found** (Wurlitzer's
> factory records were destroyed in 1988). Because the outside world cannot tell
> us this number, our value for it is only as trustworthy as the reasoning
> recorded here. This document exists so that reasoning is explicit, auditable,
> and not re-litigated from scratch every time the bark is questioned.
>
> **Deep-research complete (2026-07, workflow `wg18kbnp4`).** Verdict: d₀ is
> genuinely undocumented for the 200A in every primary source, and the back-calc
> is *structurally* underdetermined — see §3/§4. The research also corrected three
> inputs we'd been carrying (Pfeifle "1.5 mm/EP300" misattribution, unverified
> Avenson 2–7 mV, refuted 240 pF); those are fixed here and in pickup-system.md.
>
> See also: [Pickup System](pickup-system.md), [Reed & Hammer Physics](reed-and-hammer-physics.md),
> [Preamp Circuit](preamp-circuit.md).

---

## 0. Why this number matters (and why it's dangerous to guess)

The 200A's "bark" — its defining aggressive midrange growl at velocity — is
**overwhelmingly a product of the capacitive pickup's `1/(1−y)` transfer
nonlinearity**, where `y = x / d₀` is the reed displacement normalized to the
rest gap (project finding, cross-checked repeatedly: preamp/power-amp/speaker
add negligible harmonics; loop gain ~450 linearizes the preamp to ~0.04% THD).

- If `y_peak` is large (reed swings close to the pickup, small d₀), `1/(1−y)`
  bends hard → strong H2 → lots of bark.
- If `y_peak` is small (large d₀), the transfer stays nearly linear → weak bark.

So d₀ is the master knob for bark. **The catch:** the pickup is the FIRST stage
of the chain, so any change to d₀ (via the `DISPLACEMENT_SCALE` constant that
stands in for it) cascades through pickup → preamp → power amp → speaker →
output gain. This is precisely why past bark fixes were frustrating: a small
front-end tweak re-levels everything downstream, and the fix gets chased around
the gain staging instead of pinned at the source. Documenting the physics-based
bound for d₀ is the way out of guess-and-check.

---

## 1. What we KNOW (documented facts, with sources)

| Fact | Value | Source / confidence |
|---|---|---|
| Pickup type | Electrostatic (capacitive), senses displacement (gap change), `1/(1−y)` transfer | Established; multiple patents + physics. HIGH |
| Lateral slot clearance | 0.005" wider than reed → **~0.0025"/side (0.064 mm)** | US 2,919,616 (Andersen, 1960). HIGH — but this is LATERAL, not the vertical gap |
| Vertical rest gap d₀ | **UNDOCUMENTED — for the 200A specifically, in every primary source** | Deep-research pass (2026-07, 99 agents): confirmed absent from the 200/200A service manual, all Miessner/Andersen/Vintage-Vibe patents, and the Pfeifle paper. HIGH confidence, unanimous. The load-bearing unknown |
| ~~"Only published d₀ = Pfeifle 1.5 mm (EP300)"~~ | **RETRACTED — this was a false citation we carried** | The Pfeifle DAFx-17 paper models the **EP200**, contains NO "1.5 mm" and NO "EP300" and NO rest-gap value at all (it computes C by FEM field-slices, no gap in mm). The "1.5 mm/EP300" attribution is unsupported; corrected in pickup-system.md |
| Polarizing voltage | **147 V DC** via 1 MΩ feed resistor (R_feed) — 200A **manual spec** | 200A service manual. HIGH as a spec. **Caveat (new):** Pfeifle measured an EP200 at **130 V vs its 170 V manual spec** — real instruments can sit ~20–25% below nameplate, so the effective polarizing V in a back-calc may be < 147 |
| U-channel geometry | Reed surrounded on **3 faces** (bottom + 2 sides) | Pickup-system.md §; patents. HIGH. Makes C(y) depend on vertical AND lateral position |
| Pickup axial position | Patent-specified ~**0.22L from tip** (node of 2nd partial) | US 2,966,821. MED — production comb may place it at/near the tip instead |
| Proximity → tone direction | **closer reed = louder AND harsher (more harmonic)** | Confirmed HIGH: service manual over-loud fix bends pickup ends **UP 1/32″–1/16″** (0.031″–0.062″); Vintage Vibe US 11,475,868 says pickup-end position "modulates the tone toward the harmonic or the fundamental." Validates the `1/(1−y)` direction and gives the only concrete geometry-change scale |
| Measured output level | **2–7 mV AC at the volume-pot output** | Brad Avenson. NOTE: this figure entered via our own brief; the research found **no independent source** that pins it. Treat as a single-rig report, not verified |
| Pickup capacitance | ~240 pF total (used for TAU = 68.88 µs, f_c ≈ 2312 Hz) | **Single GroupDIY forum report — adversarially REFUTED** as unverifiable hearsay. Our RC corner rests on this one number; no primary source gives a pF value for the 200A |

**Bottom line of §1:** the pickup MECHANISM is fully documented and confirmed,
but three of the specific numbers we'd been leaning on turned out to be either a
misattribution (Pfeifle 1.5 mm), an unverified single-rig figure (Avenson 2–7 mV),
or refuted forum hearsay (240 pF) — and the one number that sets the bark, d₀,
is genuinely absent from every primary source.

---

## 2. What we've INFERRED (our derivations + reasoning)

### 2.1 The `DISPLACEMENT_SCALE` constant is our stand-in for d₀
Because d₀ is unknown, the model does not use a physical gap. Instead
`pickup.rs` works in normalized `y` directly, and a per-note constant
`DISPLACEMENT_SCALE` (a.k.a. DS) scales the reed model's displacement units into
`y`. DS **absorbs the unknown d₀** (and the unknown reed-amplitude scaling, and
the pickup-geometry constant) into one calibratable number.

- Current: `DS_AT_C4 = 0.85`, per-note law `ds = DS_AT_C4 · (C/C_ref)^DS_EXPONENT`
  (DS_EXPONENT = 0.75), clamped to `[0.02, 0.95]` (`tables::pickup_displacement_scale`).
  (Clamp upper raised 0.88 → 0.95 in the 2026-07 bass calibration so bass drives
  closer to the pickup on peaks → sharper spikes; see CHANGELOG + memory reed-pickup-displacement-research.)
- Safety rail: `PICKUP_MAX_Y = 0.98` with a smooth-saturation knee at
  `PICKUP_KNEE_Y = 0.94` (raised 0.85 → 0.94 in the 2026-07 bass calibration so
  the saturation stops rounding the bark spike tips; the `1/(1−y)` pole is at
  y=1, and the model must never reach it).
- The bass edge of `velocity_exponent` was also compressed (1.3 → 0.55) so the
  reed drives hard enough to spike at moderate velocity, not only ff — the bark
  appears in normal playing. Treble edge stays 1.3 (unchanged).

### 2.2 DS was calibrated by EAR/spectrum, not physics — this is the gap we're closing
DS history (from `memory/calibration-history.md`), all OBM/Dr-Dawgg-driven:
`DS_AT_C4`: 0.70 → **0.85** (Feb 2026, "H2/H1 5–8 dB too clean") → 0.75 (reshape)
→ **0.85** again (Apr 2026 retune, clamp raised 0.82 → 0.88) → clamp **0.88 → 0.95**
(2026-07 bass calibration, waveform-grounded — sharper spikes). Every move was
"match the reference spectrum/waveform," never "match a measured gap." **That is
exactly the guess-and-check this research is meant to replace with a physics bound.**

### 2.3 The voltage back-calculation (our tightest independent constraint)
We can bound `y` from the measured output voltage, working backward through the
(now-corrected) gain chain:

- Avenson measured **2–7 mV AC at the volume pot** (post-preamp).
- **Corrected 2026-07** preamp no-vibrato gain is **≈14 dB (≈5×)** — NOT the
  ≈6 dB (2×) previously assumed. (This correction fell out of the tremolo
  LDR-divider work; see [preamp-circuit.md §7.3](preamp-circuit.md) and the
  CHANGELOG. It also makes the pickup estimate agree with Avenson's own ~15 dB
  front-end figure rather than contradict it.)
- ⇒ **pickup-plate output ≈ 0.4–1.4 mV AC** (2–7 mV ÷ 5).
- With the 147 V polarizing voltage, the small-signal pickup output is
  `v_out ≈ 147 V · (ΔC/C) · (divider factor)`, and for a `1/(1−y)` capacitor
  `ΔC/C ≈ y` for small y. So `y_peak` is bounded by how much of 147 V survives
  the pickup's own high-impedance divider (R_feed 1 MΩ + the RC network) down to
  ~1 mV. **This is the lever the deep-research must close:** pin the pickup
  rest capacitance and the reed amplitude, and this inverts to a real d₀ bound.

> ⚠️ The exact ΔC/C-to-voltage transfer through the 1 MΩ / 240 pF / 147 V
> network is what turns "0.4–1.4 mV" into a d₀ number. That derivation, with the
> pickup capacitance from research §3, goes in **§4 (pending)**.

---

## 3. Deep-research verdict (2026-07, workflow `wg18kbnp4` — 99 agents, adversarially verified)

A five-angle fan-out (patents, academic modeling, practitioner measurements,
service manual/signal levels, forum/teardown), 17 sources fetched, 60 claims
extracted, top 25 verified by 3-vote adversarial refutation. Outcome:

1. **d₀ is genuinely undocumented for the 200A** — confirmed unanimously across
   15+ claims. Not in the 200/200A service manual (which specifies voicing only
   qualitatively: reed flat-face **coplanar** with the pickup face, lateral
   centering by feeler gauge to no stated dimension), not in Miessner US 3,215,765
   ("a horizontal plane slightly offset vertically" — qualitative), not in the
   Andersen (US 2,952,179 / US 2,974,555) or Vintage Vibe (US 11,475,868, which
   disclaims its drawings are to scale) patents, not in Pfeifle.
2. **Mechanism confirmed** (HIGH, unanimous): grounded reed vibrating in a
   DC-polarized plate cutout; C varies inversely with reed–plate distance — the
   physical root of `1/(1−y)`. This validates the model's transduction topology.
3. **Direction confirmed** (HIGH): closer = louder AND harsher; the factory
   over-loud remedy raises the pickup ends by **1/32″–1/16″** — the only concrete
   gap-change scale anywhere, and it confirms the `1/(1−y)` sign.
4. **Three of our inputs corrected** (see §1): Pfeifle "1.5 mm/EP300" is a
   misattribution (that paper is EP200, no gap value); Avenson 2–7 mV has no
   independent verified source; the 240 pF pickup capacitance was refuted as
   single-source forum hearsay.
5. **Reed amplitude, pickup capacitance, and node voltages: all unquantified**
   by any surviving primary source for the 200A. Pfeifle even filmed reed motion
   with a high-speed camera but plots it on a **normalized (unitless)** deflection
   axis — no mm. His pickup model is a *linearized* `i(t) = u₀·∂C/∂t`, so it
   cannot be mined for a `y`-range either.

Sources (primary, verified): 200/200A Service Manual (archive.org / Shopify CDN
mirrors); US 3,215,765; US 2,952,179; US 2,974,555; US 11,475,868; Pfeifle DAFx-17
(`dafx17.eca.ed.ac.uk/papers/DAFx17_paper_79.pdf`).

## 4. BEST-SUPPORTED CONCLUSION: d₀ is not derivable from available data — and here's exactly why

**We cannot pin d₀, reed amplitude, or `y_peak` for the 200A from any sourced
data, and the reason is structural, not "we didn't look hard enough."** The
voltage back-calc (§2.3) is **underdetermined**:

- The signal at the pickup plate is `ΔV ≈ V_pol · y_eff`. Reaching the preamp
  input, it is attenuated by the plate→preamp divider, whose factor **`A`
  depends on the pickup capacitance** (the cap's reactance sets the source
  impedance against the ~380 kΩ bias / 1 MΩ feed network).
- So `V_preamp_in ≈ V_pol · y · A`. We observe `V_preamp_in ≈ 0.4–1.4 mV`
  (Avenson 2–7 mV ÷ the corrected ~5× gain) and `V_pol ≈ 147 V` (maybe less).
  That is **one equation in two unknowns** (`y` and `A`), and `A` is set by the
  unknown, refuted pickup capacitance.
- Consequently a wide family of `(d₀, reed-amplitude, C_pickup)` triples all
  reproduce the observed millivolts. A small physical `y` with low attenuation,
  or a large physical `y` with heavy attenuation (small C, high source-Z), are
  both consistent with the data. **The observable levels do not choose between
  them.**

**What this means for the model.** `DISPLACEMENT_SCALE` (DS ≈ 0.85, `y_peak ≈ 0.85`)
is therefore an **honest calibration constant, not a measured physical ratio** —
it is the single free parameter that the OBM spectral match tunes, and it
legitimately absorbs the jointly-unknown `(d₀, amplitude, C_pickup)`. That is not
a defect to be "fixed" by finding a number; it is the correct structure given
what is knowable. The one thing that WOULD collapse the ambiguity is a **direct
measurement** — a caliper on a disassembled 200A reed bar (d₀ and reed geometry),
or an LCR-meter reading of the reed-bar capacitance — neither of which exists in
any source and both of which would require hardware we don't have.

**Tightest defensible statement:** the reed operates in a regime where the
`1/(1−y)` transfer produces the measured, audibly-strong bark; the factory
"closer = harsher" evidence and the model's OBM match both place the effective
`y_peak` in the **moderate-to-strong nonlinear range (roughly 0.7–0.9 effective)**,
but whether that reflects a large *physical* displacement/gap ratio or a smaller
physical ratio amplified by the pickup's charge dynamics **cannot be resolved
without a hardware measurement**. DS stays where OBM calibration puts it.

## 5. Confidence ledger

| Quantity | Value in model | Status | Confidence |
|---|---|---|---|
| Pickup mechanism (`1/(1−y)`, capacitive) | core of pickup.rs | Confirmed by patents + Pfeifle | **HIGH** |
| "Closer = louder + harsher" direction | DS sign, bark model | Confirmed (service manual + patent) | **HIGH** |
| Vertical rest gap d₀ (absolute) | not used (DS instead) | Undocumented everywhere; underdetermined | **UNKNOWN (structural)** |
| `DISPLACEMENT_SCALE` (DS ≈ 0.85) | tables.rs per-note | Calibration constant (OBM-tuned), not measured | **Calibrated, not physical** |
| `y_peak ≈ 0.85` effective | pickup.rs | OBM-consistent; physical vs effective unresolved | **MED (as effective)** |
| Polarizing voltage 147 V | model input | Manual spec; real may be ~20% lower | **MED** |
| Pickup capacitance 240 pF (→ TAU) | pickup.rs | Single forum source, adversarially refuted | **LOW — flagged** |
| Reed amplitude (mm) | embedded in DS + BASE_MODE_AMPLITUDES | Unquantified in any source | **UNKNOWN** |
| Avenson 2–7 mV output | back-calc anchor | Single unverified rig | **LOW–MED** |

**Open questions (need hardware to close):** (1) caliper d₀ + reed geometry on a
disassembled 200A reed bar; (2) LCR measurement of reed-bar capacitance (would
also validate/replace the refuted 240 pF and the TAU corner); (3) an
oscilloscope trace of the pickup-plate node at a known note/velocity to anchor
the voltage chain independently of Avenson. Until then, DS is the right home for
the ambiguity and must not be "back-solved" from a single assumed input.

---

## 6. How to change DS responsibly (the cascade rule)

Whatever d₀/DS we land on, changing it re-levels the entire downstream chain
(pickup → preamp → power amp → speaker → POST_SPEAKER_GAIN). Any DS change MUST
be followed by re-checking: (a) the `1/(1−y)` `y_peak` stays below the
`PICKUP_MAX_Y = 0.98` pole with margin; (b) the engine-peak-≤-1.0 invariant at
vol=1.0 (recompensate POST_SPEAKER_GAIN as a pure output trim, exactly as the
tremolo work did); (c) the OBM harmonic comparison. Never chase the resulting
level change by re-tweaking DS — fix level downstream, keep DS at its
physics-justified value.
