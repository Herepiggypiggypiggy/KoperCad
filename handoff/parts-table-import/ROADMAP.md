# Real part data import: roadmap

Scope: manufacturer part data into the parts library from open formats, with a preview, a
mapping step, validation and honest labelling of unknown fields. Never contacts a maker site.
EPLAN rule: no EPLAN macros, libraries, manual text or motor data are copied; customer-derived knowledge only.

## Done
- Table reader: CSV/TSV/TXT (UTF-8 or Windows-1252, delimiter by vote over the first lines) and `.xlsx` (several sheets), blank rows kept out but real row numbers kept.
- Mapping engine: suggestions from EN/DE/NL header words, units in headers, ETIM/ECLASS by shape; saved mappings by header signature.
- Preview engine: unit conversion, plausibility checks, duplicates, new/changed/unchanged against the catalog, field coverage, assumed units, plain-text report.
- App: File > Import parts > "Import parts from a table (CSV or Excel)", palette, Parts window button, .xlsx drop; Columns and Rows steps; Add.
- QA scenario `qa/scenarios/21-parts-table-import.qa`; docs `docs/PARTS-TABLE-IMPORT.md`.

## Next (in order)
1. Inbox bulk import reads `.xlsx` through the same mapping when a saved mapping or an unambiguous suggestion exists, else lists the file for the dialog.
2. Datasheet-to-part flow writes through the same preview (one validation path); show coverage there.
3. Real maker files: when Tadjiro sends one, add its header words and units to the suggestion tables and a golden test.
4. Row-per-terminal and row-per-variant tables (fold into one part).
5. `.xls`/`.ods` if a maker still ships them (calamine features are off; keep the build small).
6. ETIM feature-level mapping UI for BMEcat files that the recipes do not know.

## Open questions
- Which makers' Excel price lists does Tadjiro actually use? (header words, units, currency)
- Should "This file is" default to Maker data when the folder is a known maker? Today: Not stated.
