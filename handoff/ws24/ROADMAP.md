# WS-24 Import and export (`novo-interop`): roadmap

Scope: DXF import and export, EPLAN parts import, later Excel round trip. Pure
engines first; everything offline, no admin rights. Design: `docs/design/interop.md`.

## Done
- Design doc `docs/design/interop.md`.
- Crate `novo-interop`, module `dxf`: neutral `Drawing`, ASCII DXF writer (AC1009),
  reader (LINE, LWPOLYLINE, POLYLINE, CIRCLE, ARC, TEXT, MTEXT, INSERT explode,
  bulges to arcs, units, encodings, skipped-entity counts). Tests in `tests/dxf.rs`.

## Next, in order
1. `eplan` module: EPLAN parts XML and delimited export to `novo_parts::CatalogPart`
   plus a report; tests in `tests/eplan.rs`.
2. Bridge: `PageScene` primitives and labels to `dxf::Drawing` (y flip), export of
   a page or all pages; entry points for `novo-cli` / `novo-py`.
3. DXF as page background: needs a WS-02 decision on an underlay record.
4. Excel round trip for the parts list and the I/O list (after the PLC tag table export).

## Open questions
- Where a page keeps imported DXF geometry (WS-02).
- Real EPLAN export samples would harden `eplan` (none available offline).
