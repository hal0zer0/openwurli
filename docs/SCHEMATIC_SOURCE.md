# Obtaining the Wurlitzer 200A Schematic

## Required Schematic

This project references the **Wurlitzer Model 200A Electronic Piano Schematic**, drawing number **#203720-S-3**, valid for instruments starting at serial number **102905**.

The schematic PDF is **not included** in this repository due to copyright. You must obtain it separately.

## Where to Find It

The schematic is widely available from Wurlitzer service documentation archives:

1. **BustedGear** ([bustedgear.com](https://bustedgear.com)) — Free Wurlitzer service manual collection
2. **Electric Piano Forum** ([electricpianoforum.com](https://electricpianoforum.com)) — Community resource with schematics
3. **Original service manuals** — Sometimes available on eBay or from vintage keyboard dealers

Search for: *"Wurlitzer 200A schematic 203720"* or *"Wurlitzer 200A service manual"*

## Correct Version

Make sure you get the **200A** schematic (serial 102905+), not:

- The combined 200/203/206/207 schematic (different component numbering)
- The 200 schematic (tube-based, completely different topology)
- The 206A schematic (has C20/220pF cap not present on the 200A)

## Where to Place It

Save the PDF as:

```
docs/verified_wurlitzer_200A_series_schematic.pdf
```

This path is referenced by the schematic preprocessing tools in `tools/schematic_preprocess.py`. The file is in `.gitignore` and will not be committed.

## Key Component Values

If you don't have the schematic but need to verify the implementation, the critical component values extracted from it are documented in:

- `docs/preamp-circuit.md` — Complete component values with DC bias points
- `docs/output-stage.md` — Power amplifier and tremolo circuit
- `docs/pickup-system.md` — Electrostatic pickup parameters
