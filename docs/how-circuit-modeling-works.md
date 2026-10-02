# How the Circuit Modeling Works

A non-technical overview of how OpenWurli turns a circuit schematic into real-time audio.

## The Problem

The Wurlitzer 200A has a distinctive sound — warm, barky, with a characteristic tremolo that changes the *tone*, not just the volume. That sound comes from specific electronic components (transistors, resistors, capacitors) wired together in specific ways. Change one resistor value and the whole character shifts.

Sample-based plugins record the output and play it back. That works, but you're locked into whatever the recording captured. Physical modeling recreates the *process* that makes the sound, so every parameter change produces the correct result — just like turning a knob on the real instrument.

The challenge: the 200A's preamp circuit has two transistor stages with feedback loops, capacitors that store energy across time, and nonlinear devices that behave differently depending on signal level. You can't model this with simple math. You need to solve the circuit.

## From Schematic to Math

Every electronic circuit obeys Kirchhoff's laws: current into a node equals current out, and voltages around a loop sum to zero. Engineers formalize this as **Modified Nodal Analysis (MNA)** — a system of equations with one equation per circuit node.

The default preamp solver tracks 9 nodes: TR-1's base, emitter and collector (which is also TR-2's base), TR-2's emitter, the junction in TR-2's emitter leg, TR-2's collector, the output-coupling-cap junction, the output, and the feedback junction where the tremolo LDR connects. (An opt-in 12-node solver generated from the SPICE netlist also exists.) Each component (resistor, capacitor, transistor) contributes terms to these equations. A 22K resistor between nodes A and B adds `(V_A - V_B) / 22000` to the current balance at both nodes. A capacitor adds a term involving the *rate of change* of voltage.

We write all of this down as matrices — one for resistive elements (G), one for capacitive elements (C), one for nonlinear devices (the transistors). The full system looks like:

```
C * dv/dt + G * v + i_nonlinear(v) = sources
```

This is a set of coupled differential equations. The G and C matrices are constant (they come straight from the schematic), but the transistor terms depend on the voltages we're trying to find.

## The DK Method

"DK" stands for Discretization-Kernel. It's a two-step technique for turning the continuous-time circuit equations into something a computer can evaluate sample-by-sample.

**Step 1 — Discretize time.** Capacitors involve derivatives (dv/dt). We approximate these using the trapezoidal rule, which converts each capacitor into an equivalent resistor plus a memory term from the previous sample. After this step, the differential equations become algebraic equations — no calculus, just arithmetic at each time step.

**Step 2 — Isolate the nonlinear kernel.** The transistor equations are nonlinear (exponential Ebers-Moll model), which means we can't solve the full 9-node system in one shot. But most of the circuit is linear — only the two transistors are nonlinear. The DK method uses linear algebra to reduce the 9-node system down to a 2x2 nonlinear kernel: one equation per transistor. (The tremolo LDR changes over time, and its effect is folded in with the Sherman-Morrison formula rather than by re-inverting the matrix.)

We solve that 2x2 system with Newton-Raphson iteration, then back-substitute to get all 9 node voltages. The output voltage is our audio sample.

## What This Gets Right

Because we're solving the actual circuit equations:

- **Frequency response** emerges naturally from the component values. The 100pF Miller capacitors (C-3, C-4) and the feedback network set the high-frequency rolloff (the SPICE reference puts the -3 dB point near 16.8 kHz) without us having to design a separate filter.

- **Harmonic distortion** comes from the transistors' exponential I-V curve, chiefly in stage 2 (the high-gain stage and the first to clip). We don't add distortion — the math produces it.

- **Tremolo interaction** works correctly because the LDR resistance modulates a feedback path *inside* the circuit. When the LDR changes, the entire gain, frequency response, and distortion character shifts together — exactly like the real instrument.

- **Coupling between stages** is captured. The direct coupling between Stage 1 and Stage 2 means the DC operating point of one stage affects the other. The DK method solves both stages simultaneously, preserving this interaction.

## The Rest of the Signal Chain

The preamp is the most complex piece, but the full model includes:

- **Reed oscillator** — each of the 64 keys drives a steel reed. We model this as a sum of 7 vibration modes (like harmonics, but slightly inharmonic because real reeds aren't perfect). Each mode is a sine wave with its own frequency, amplitude, and decay rate derived from beam physics.

- **Electrostatic pickup** — each reed sits between two fingers of a charged metal comb, resting just outside them. As it swings, it slides into the gap between the fingers on one half of its swing only, so the capacitance (and the signal) rises lopsidedly. We compute that capacitance-vs-position curve from a 2D electric-field simulation of the pickup's cross-section. That lopsided curve is the main source of the instrument's harmonics.

- **Power amplifier** — a Class AB push-pull amp with ±22 V rails. The default model is a behavioral closed-loop solve (Newton-Raphson on the feedback loop), including the crossover distortion where the NPN and PNP output transistors hand off to each other.

- **Speaker** — two small 4x8" oval ceramic speakers with bass rolloff, treble rolloff, cone breakup nonlinearity, and thermal compression. Basically everything that makes a cheap built-in speaker sound like a cheap built-in speaker — which is part of the charm.

## Performance

The DK preamp solver and the power amp run at 2x oversampling (88.2 kHz at a 44.1 kHz host rate). With 64 voices of reed oscillators, the full plugin runs comfortably in real time on any modern CPU. The trick is that the expensive nonlinear solve is on the *mono* signal (after all voices are summed), not per-voice.

## Further Reading

- [DK Preamp Derivation](research/dk-preamp-derivation.md) — the full mathematical derivation
- [Preamp Circuit Reference](research/preamp-circuit.md) — component values, DC bias points, measured vs modeled comparisons
- [Signal Chain Architecture](research/signal-chain-architecture.md) — complete DSP specification for every stage
