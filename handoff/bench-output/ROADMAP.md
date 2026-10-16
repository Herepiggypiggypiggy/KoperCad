# Bench output: roadmap

Scope (gap analysis item 2, Tadjiro 2026-10-16): printable output for a workshop that cuts wires and drills by hand.

Done in the first PR (branch `bench-output`):
- Ferrule standard (colour, contact length per cross-section, spares, twin limit), editable, stored in the project (`bench.ferrule_standard`).
- Ferrule table PDF: singles and twins per size and colour, spares, the standard printed on the sheet.
- Bench cut list PDF: groups by size and colour, tick boxes, strip lengths, ferrules, route + end allowance = cut, `~` on estimated lengths.
- Label sheets PDF: wire flags, terminal, device and cable labels on A4 sticker grids (5 presets, custom sizes), start slot for partly used sheets, copies, offsets, test outlines, tape widths 6 to 24 mm; Brother P-touch CSV per kind.
- UI: Reports > Bench output (Ctrl+K "bench output"), commands `view.bench`, `export.bench_cut`, `export.ferrules`, `export.labels`, button "Bench output" in Cabinet > Routes.

Next, in order:
1. Print a real sheet and a tape label on the workshop printers; correct preset pitches and tape behaviour.
2. Wire lengths become exact once part pin positions exist (PINS-UI, PARTS-Q): the `~` marks then disappear without code changes.
3. Weidmueller / Phoenix printer layouts (their own software imports CSV): add column maps once the workshop names its printer.
4. Bundle / loom view for hand wiring.
5. Drill template print pack belongs to WORK-1, not this lane.

Open questions: which label printer and sticker sheet the workshop owns; whether twin ferrules are wanted by default.
