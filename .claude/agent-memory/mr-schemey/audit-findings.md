# Circuit Topology Audit — Detailed Findings (Feb 2026)

## C1: Missing C20 Input HPF
- C20 (220pF) + R-2||R-3 (380K) = HPF at 1903 Hz
- SPICE netlist line 69: `C20 base1 gnd 220P`
- docs/preamp-circuit.md Section 5.2 documents it
- signal-chain-architecture.md lists it as stage [F]
- Neither pickup.rs, preamp.rs, voice.rs, nor lib.rs implements it
- Fix: Add OnePoleHpf(1903, os_sample_rate) before preamp in oversampled loop

## C2: Bark Source Attribution Error
- pickup.rs lines 11-19: "PRIMARY source of Wurlitzer bark"
- lib.rs lines 19-23: "bark comes from HERE, not the preamp"
- Contradicts: docs/pickup-system.md Section 4.4 (pickup H2 = -26 dB at mf)
- Contradicts: docs/preamp-circuit.md Section 6 (preamp H2 = -10 to -20 dB)
- Contradicts: docs/signal-chain-architecture.md ("preamp is primary source of H2")
- Both contribute; preamp dominates by 10-15 dB at normal dynamics

## I1: Stage 1 Miller Pole Inside Feedback Loop
- bjt_stage.rs line 70: miller_freq=23 Hz (open-loop dominant pole)
- Applied inside BjtStage::process() via miller_lpf
- preamp.rs uses prev_output feedback with one-sample delay
- 23 Hz pole + 1-sample delay = 9.5 deg phase margin at crossover
- Result: BW narrowing, especially at tremolo bright (5.4 kHz vs 8.3 kHz SPICE)
- Post-loop 16 kHz BW LPF compensates partially but can't fix in-loop resonance
- Fix option: set stage1 miller_freq very high, rely solely on post-loop BW LPF

## I3: Power Amp Model Lacks Gain Stage
- SPICE: R31=15K, R30=220, C10=22uF -> AC gain = 1+15K/220 = 69x (37 dB)
- DSP: PowerAmp has unity gain, crossover_width=0.0005, rail_limit=1.5
- preamp_gain param (default 40.0) compensates for missing power amp gain
- Crossover distortion thresholds are in uncalibrated arbitrary units

## M5: Oversampler Under-Specified
- BRANCH_A_COEFFS: 3 values, BRANCH_B_COEFFS: 3 values = 6 allpass sections
- Architecture spec calls for 12 sections (~100 dB rejection)
- Actual: ~28 dB at 30 kHz near transition band
- Preamp BW LPF at 16 kHz mitigates (content above 16 kHz already attenuated)
