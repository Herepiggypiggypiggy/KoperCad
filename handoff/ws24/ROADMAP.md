# WS-24 Import and export (`novo-interop`): roadmap

Scope: DXF import and export, EPLAN parts import, later Excel round trip. Pure
engines first; everything offline, no admin rights. Design: `docs/design/interop.md`.

## Done
- Design doc `docs/design/interop.md`.
- Crate `novo-interop`, module `dxf`: neutral `Drawing`, ASCII DXF writer (AC1009),
  reader (LINE, LWPOLYLINE, POLYLINE, CIRCLE, ARC, TEXT, MTEXT, INSERT explode,
  bulges to arcs, units, encodings, skipped-entity counts). Tests in `tests/dxf.rs`.
- `eplan` module (#347, merged): tolerant XML reader, `import_xml`,
  `import_delimited`, `import_auto` to `novo_parts::ImportReport`; `tests/eplan.rs`.
- `bridge` module (PR #354): `page_drawing` / `scene_to_drawing`, page scene to DXF drawing (y flip, text per line); `tests/bridge.rs`.

## Next, in order
1. Entry points: CLI `export dxf` and File > Export DXF are in the entry-points PR; Python binding (`novo-py`) still open.
2. DXF as page background: `docs/design/dxf-background.md` proposes page properties (option B); needs the lead's nod, then reader-to-underlay, canvas layer, File > Import DXF.
3. Excel round trip for the parts list and the I/O list (after the PLC tag table export).

## Open questions
- Where a page keeps imported DXF geometry (WS-02).
- Real EPLAN export samples would harden `eplan` (none available offline).
