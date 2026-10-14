# WS-07 Parts and properties: roadmap

Scope (roadmap WS-07): part catalog (manufacturer, order number, dimensions,
technical data), device-to-part assignment, property schema behind a future
properties UI, spreadsheet-style bulk edit, CSV import. Done criterion: every
device in the Gate 1 circuit carries a real part.

Spec sources: docs/WORKFLOW.md J12 (and J2/J5/J7 where parts appear), the
roadmap "Lessons from EPLAN" table, the workflow thread's criteria (P1-P3),
and docs/DESIGN.md once it lands (owned by the workflow thread). Tadjiro's
direction: NovoElectric is its own program, not an EPLAN copy; until
DESIGN.md merges, build engines and commands only, no UI layout.

## Done (PR #36, crate novo-parts)
- Catalog + search + suggest by letter code; 30-part starter catalog; user
  catalog at %APPDATA%\NovoElectric\parts.json; preview_merge for imports
- assign_part / assign_cable_part / clear_parts / missing_parts /
  remove_unused_parts as Commands (one undo step each)
- PropertySheet: generic rows from record JSON, filter by name/synonym/value,
  mixed values, validated edits, tag rules (J4), part-data overrides with
  reset_to_part (principle 12: part is the style of a device)
- BulkTable: TSV copy, apply_table paste, set_cell, fill (fill-down)
- import_catalog: CSV with EN/DE/NL headers, delimiter detection
- egui panel REMOVED on purpose (design doc first)

## Next, in order
1. Drive PR #36 to READY and merged (no pushes after READY; new PR from main)
2. After #35 merges: terminal parts (Terminal.part) in assigned_parts(),
   assign_part for terminals, missing_parts includes terminals (J5)
3. After DESIGN.md merges: thin UI on PropertySheet per the design (with WS-01)
4. EPLAN part import (WS-24 overlap) and catalog persistence UX

## Open questions
- Should the catalog live in SQLite for very large catalogs? JSON is fine for now.
