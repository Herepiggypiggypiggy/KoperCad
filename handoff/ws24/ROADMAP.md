# WS-24 Import and export (`novo-interop`): roadmap

Scope: DXF import and export, EPLAN parts import, later Excel round trip. Pure
engines first; everything offline, no admin rights. Design: `docs/design/interop.md`.

## Done
- Design doc `docs/design/interop.md`.
- Crate `novo-interop`, module `dxf`: neutral `Drawing`, ASCII DXF writer (AC1009),
  reader (LINE, LWPOLYLINE, POLYLINE, CIRCLE, ARC, TEXT, MTEXT, INSERT explode,
  bulges to arcs, units, encodings, skipped-entity counts). Tests in `tests/dxf.rs`.
- `eplan` module (PR 2, stacked on #344): tolerant XML reader, `import_xml`,
  `import_delimited`, `import_auto` to `novo_parts::ImportReport`; `tests/eplan.rs`.

## Next, in order
1. Bridge: `PageScene` primitives and labels to `dxf::Drawing` (y flip), export of
   a page or all pages; entry points for `novo-cli` / `novo-py`.
2. DXF as page background: needs a WS-02 decision on an underlay record.
3. Excel round trip for the parts list and the I/O list (after the PLC tag table export).

## Open questions
- Where a page keeps imported DXF geometry (WS-02).
- Real EPLAN export samples would harden `eplan` (none available offline).
