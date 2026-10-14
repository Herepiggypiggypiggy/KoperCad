# WS-13 Reports engine: roadmap

Roadmap scope: generated-page framework, bill of materials, device list, wire
list, table of contents, report templates, Excel export. Done when reports
regenerate within a second of any edit (met: ~145 ms debug on 10k objects,
6 reports, 291 pages).

Crate: `crates/novo-reports` (owner WS-13). Engine only, no UI.

## Done
- #55 (merged ea47b10):
  - typed table model and the `Report` trait (the seam for generated pages);
  - LiveReports (ChangeSet-driven, never stale);
  - toc, bom, devices and wires reports;
  - paging plus mm layout;
  - xlsx (deterministic) and CSV (Excel formula-safe) export.
- #68 (READY from B and C at 02d6e29; awaiting the coordinator's merge):
  - TerminalDiagram and CableDiagram reports;
  - generated-page sheets (`20`, or `20.1`, `20.2`, ...);
  - PageSetup aligned to the canvas frame;
  - project-identity stamp in LiveReports;
  - xlsx cell-limit cut;
  - timing-test slack;
  - empty reports still print one page.

## Decisions
- Report pages are `PageKind::Generated { generator }` page records with live content (coordinator, from DESIGN.md §2). Command-only exports (xlsx/csv) stay as a convenience.
- Multi-sheet naming: the page's own name for a single sheet, `<name>.k` otherwise (proposed in #68; WS-03 and WS-09 still need to adopt it).

## Next, in order
1. #68 merge is with the coordinator (both READY).
2. After #72 merges: a BOM test with a strip-level terminal article.
3. B's #68 notes:
   - a sheet-name collision rule for `20.1` vs a real page `20.1`;
   - CableDiagram building screens once.
   - (C) pinned flow vs growing generated pages: keep sheet numbers stable after a revision.
4. Coordinate the drawing of generated sheets:
   - WS-03: canvas draws `layout_sheet` and the sheet name in the title block;
   - WS-09: PDF emits one page per sheet, with label and bookmark = sheet name;
   - WS-01: app owns one LiveReports per Document and calls notify/clear.
5. Report templates: column selection, order, headers and widths per report, stored in project info properties or a profile file.
6. BOM: joined-terminal accessories (connectors/jumpers from `rules.terminals.join_link`); an option for quantities per structure (=A1 / +CAB).
7. Wire list:
   - an option to sort by wire number;
   - freeze `page.column` after a revision or export (DESIGN "pinned flow").
8. Command-field verbs (`bom`, `wires`, `export xlsx`) in novo-workflow or novo-app; WS-28 already covers the CLI and Python (#65).
