# OpenWurli Documentation

Technical documentation for the OpenWurli Wurlitzer 200A virtual instrument plugin.

## Reading Order

**New to the project?** Start with the [project README](../README.md), then read the Signal Chain Architecture overview (Sections 1-3) to understand the full signal flow. Dive into per-stage docs as needed.

**Debugging a DSP issue?** Start with [SPICE-Rust Mapping](spice-rust-mapping.md), then the relevant stage doc.

**Evaluating sound quality?** Go to [Calibration and Evaluation](calibration-and-evaluation.md).

**Running the ML pipeline?** See [ml/README.md](../ml/README.md).

## Document Index

### Architecture

| Document | Description |
|----------|-------------|
| [Signal Chain Architecture](signal-chain-architecture.md) | Overall DSP specification: signal flow, stage interconnections, gain staging, parameters |

### Circuit Reference

| Document | Description |
|----------|-------------|
| [Preamp Circuit](preamp-circuit.md) | Two-stage BJT preamp: component values, DC bias points, feedback topology, harmonic analysis |
| [DK Preamp Derivation](dk-preamp-derivation.md) | Discretization-K method: 8-node MNA formulation, trapezoidal discretization, Sherman-Morrison |
| [DK Preamp Testing](dk-preamp-testing.md) | Five-layer test pyramid: matrix stamps, linear algebra, DC point, transfer function, time-domain |
| [Output Stage](output-stage.md) | Power amplifier, tremolo/LDR feedback, speaker cabinet model, volume control |

### Physics

| Document | Description |
|----------|-------------|
| [Pickup System](pickup-system.md) | Electrostatic pickup: 1/(1-y) nonlinearity, RC high-pass, capacitive coupling physics |
| [Reed and Hammer Physics](reed-and-hammer-physics.md) | Modal synthesis: Euler-Bernoulli beam theory, tip mass, dwell filter, onset ramp |

### Validation and Testing

| Document | Description |
|----------|-------------|
| [Calibration and Evaluation](calibration-and-evaluation.md) | OBM recording analysis, spectral targets, tiered metrics, evaluation methodology |
| [SPICE-Rust Mapping](spice-rust-mapping.md) | SPICE-to-Rust translation guide: node map, discretization reference, bug archaeology |

### Reference

| Document | Description |
|----------|-------------|
| [Schematic Source](SCHEMATIC_SOURCE.md) | How to obtain the Wurlitzer 200A schematic (#203720-S-3) |
