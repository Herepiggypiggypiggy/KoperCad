# EPLAN layout import handoff
Based on PR #1031 (crane import); open a PR only after #1031 is merged (no stacked PRs).
Code: crates/novo-interop/src/eplan/pxf/{graphics,layout,layout_apply}.rs; reader arms in read.rs (O31 line, O89 rect, O32 arc, O34 polygon, O88 location box); `PxfFile.shapes`; plan.rs hooks (`graphics`, `layouts`).
Shapes go in the page properties `underlay.*` (the DXF background mechanism), because novo-core has no free-line object. Shapes wholly at or beyond the sheet edge (height index marks) are left out.
Mounting plate sheet = a page whose texts contain `Kast +<loc>: <type> WxHxD mm` (the crane configurator's layout_builder.sheet). Plate = biggest 0.35 pen rectangle (scale = its width / W); rails = pairs of 0.13 pen rectangles 35/20 mm high with the same span; ducts = other thin rectangles; devices = 0.25 pen rectangles, tag text (starts with `-`) at their left edge, `=code` text beside it.
Tests: crates/novo-interop/tests/graphics.rs (synthetic fixture synthetic-layout.epj), opt-in real: KOPERCAD_CRANE_EPJ (trimmed v95 has no layout pages: 23 schematic pages, 409 lines, 51 rectangles, 12 location boxes).
NOT verified: the real layout pages (full v95 epj with =DOC layout sheets is not in the cloud mount).
