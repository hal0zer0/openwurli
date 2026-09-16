# Obtaining the Wurlitzer 200A Schematic

## Required Schematic

This project references the **Wurlitzer Model 200A Electronic Piano Schematic**, drawing number **#203720-S-3**. (An earlier revision of this doc said "valid for serial 102905+" — that serial belongs to the "LATE PRODUCTION AUX. OUTPUT CIRCUIT" note printed on the sheet, not to the schematic's coverage.)

**Provenance note (2026-09-13):** two different prints of this circuit circulate. The archive scan (the "p.66" surface) carries the actual title block *"MODEL 200A ELECTRONIC PIANO SCHEMATIC ... SCHEMATIC # 203720-S-3"* and is the surface we treat as authoritative. The Tropical Fish Vintage compendium's redraw (its p.23) has **no title block** and a different bias-adjust note — it is a different print and must not be cited as 203720-S-3, even though its topology has matched the archive scan on every instrumented read so far.

**Encoding caveat (2026-09-16):** the archive scan PDF is 1-bit **JBIG2** at 300 dpi with no text layer (check with `pdfimages -list`). JBIG2 is a lossy symbol coder that can substitute one glyph for a visually similar one (3/8, 1/7, 6/8), so any component value or drawing number read off it alone carries substitution risk. The Tropical Fish redraw is JPEG2000 (no symbol coding, resolution-limited only). Rule: cite the drawing number from the archive scan; accept a **value** from the archive scan only when it clears a same-sheet glyph control or agrees with the Tropical Fish print. The 2026-09 topology revision read both surfaces this way (e.g. R-2's "1 MEG" label, `docs/research/preamp-circuit.md` Note 1).

The schematic PDF is **not included** in this repository due to copyright. You must obtain it separately.

## Where to Find It

The schematic is widely available from Wurlitzer service documentation archives:

1. **BustedGear** ([bustedgear.com](https://bustedgear.com)) — Free Wurlitzer service manual collection
2. **Electric Piano Forum** ([electricpianoforum.com](https://electricpianoforum.com)) — Community resource with schematics
3. **Original service manuals** — Sometimes available on eBay or from vintage keyboard dealers

Search for: *"Wurlitzer 200A schematic 203720"* or *"Wurlitzer 200A service manual"*

## Correct Version

Make sure you get the **200A** schematic, not:

- The combined 200/203/206/207 schematic (different component numbering)
- The Model 200 schematic (drawing 201904-S-1-E-1 — solid-state but a different circuit; e.g. its volume control is a 3K part 201814, vs the 200A's 10K pot + 25K reed-bar trimmer)
- The 206A schematic (has C20/220pF cap not present on the 200A)

## Where to Place It

Save the PDF as:

```
docs/verified_wurlitzer_200A_series_schematic.pdf
```

This path is referenced by the schematic preprocessing tools in `tools/schematic_preprocess.py`. The file is in `.gitignore` and will not be committed.

## Key Component Values

If you don't have the schematic but need to verify the implementation, the critical component values extracted from it are documented in:

- `docs/research/preamp-circuit.md` — Complete component values with DC bias points
- `docs/research/output-stage.md` — Power amplifier and tremolo circuit
- `docs/research/pickup-system.md` — Electrostatic pickup parameters
