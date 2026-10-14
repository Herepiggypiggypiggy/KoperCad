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
1. Done: PR #36 merged (9fa18ed); #67 (fixer) merged the first follow-ups
1b. Done (this thread, claude/project-thread-6encdr): last #36 review notes:
   unclosed CSV quote is an error, rows carry file line numbers, duplicate
   catalog headers refused, typed cells that don't parse are reported,
   Excel-formula tags skipped with advice
2. Done (fixer, claude/ws07-followups): terminal parts. assign_terminal_part
   sets Terminal.part; terminal_part() = Terminal.part, else the
   novo-terminals style (override or strip per kind), else the strip
   device's part; missing_parts lists terminals with none; unused_parts
   keeps parts the strip styles use. Open question for WS-11/WS-02: two
   stores for a terminal's part (Terminal.part vs novo-terminals' `part` /
   `terminal_part.<kind>` properties); one should go.
3. After DESIGN.md merges: thin UI on PropertySheet per the design (with WS-01)
4. EPLAN part import (WS-24 overlap) and catalog persistence UX

## Open questions
- Should the catalog live in SQLite for very large catalogs? JSON is fine for now.
