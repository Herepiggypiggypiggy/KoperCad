# Outputs thread (WS-09 PDF, WS-13 reports, WS-14 checks): roadmap

Owns `novo-pdf`, `novo-reports` and `novo-checks`, which cloud workers built
(their history: handoff/ws9, ws13, ws14). Started 2026-10-14 21:26 UTC.
Reviewer: main reviewer (cse_016C9nTT8cN7voW8t1NQUG4W). Merges: integration
lead (cse_01FiLvRBKEcNNHbQEGhYyL1S).

## Queue from the integration lead, in order
1. Sheet names `20.1` colliding with a real sub-page: done (#86).
2. Cable diagram building every cable screen twice: done (#86).
3. Pinned flow vs growing generated pages: decided (DESIGN.md #88): they
   reflow; bookmarks name sheets by content. Done in the PDF PR.
4. Multi-sheet generated pages in the PDF: done (#96). Title block name
   and per-page "n of m" via WS-03's set_sheet: staged (pdf-sheet-content).

## Later (from ws9/ws13/ws14 roadmaps)
- PDF: embedded Inter subset for Unicode; per-entry xref links; compressed
  streams; OS print.
- Reports: BOM test with a strip-level terminal article; report templates;
  wire list by wire number; xlsx "Created" from the project's save time.
- Checks: shield landing on a deleted terminal (from reviewer C).

## Queued by the lead (21:56 UTC)
- PE terminal on a non-PE block: in the checks PR (with #82 feeds and the
  undersized-wire check).
- Title-block numbering per generated page: staged, PR after the checks PR.
- Sheet letters 20a, 20b (DESIGN #108/#110): done (#115).
- Contents list names each generated sheet (DESIGN #88): staged (toc-sheets).
