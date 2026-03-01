# OpenWurli Documentation

## User Guides

| Document | Description |
|----------|-------------|
| [How the Circuit Modeling Works](how-circuit-modeling-works.md) | Non-technical overview of the DK method preamp solver and signal chain |
| [How the MLP Corrections Work](how-mlp-corrections-work.md) | Non-technical overview of the per-note neural network correction layer |
| [Schematic Source](SCHEMATIC_SOURCE.md) | How to obtain the Wurlitzer 200A schematic (#203720-S-3) |
| [Release Codenames](release-codenames.md) | Version codename candidates and history |

## Wurlitzer 200A Research (`research/`)

Circuit analysis, physics models, and DSP derivations for the 200A signal chain.

| Document | Description |
|----------|-------------|
| [Signal Chain Architecture](research/signal-chain-architecture.md) | Complete DSP specification: signal flow, stage interconnections, gain staging |
| [Preamp Circuit](research/preamp-circuit.md) | Two-stage BJT preamp: component values, DC bias, feedback topology |
| [DK Preamp Derivation](research/dk-preamp-derivation.md) | Discretization-K method: 8-node MNA, trapezoidal discretization, Sherman-Morrison |
| [Output Stage](research/output-stage.md) | Power amplifier, tremolo/LDR feedback, speaker cabinet model |
| [Pickup System](research/pickup-system.md) | Electrostatic pickup: 1/(1-y) nonlinearity, RC high-pass, capacitive coupling |
| [Reed and Hammer Physics](research/reed-and-hammer-physics.md) | Modal synthesis: Euler-Bernoulli beam theory, tip mass, dwell filter |

## Agent Reference (`reference/`)

Working docs for AI agents and developers modifying the codebase.

| Document | Description |
|----------|-------------|
| [DK Preamp Testing](reference/dk-preamp-testing.md) | Five-layer test pyramid strategy |
| [Calibration and Evaluation](reference/calibration-and-evaluation.md) | OBM recording analysis, spectral targets, evaluation methodology |
| [SPICE-Rust Mapping](reference/spice-rust-mapping.md) | SPICE-to-Rust translation guide: node map, discretization, bug archaeology |
| [Parameter Tuning Guide](reference/parameter-tuning-guide.md) | Parameter interaction chains, calibration workflow, sensitivity analysis |
