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
4. Multi-sheet generated pages in the PDF, with WS-03 (canvas) for the title
   block's sheet name and "n of m".

## Later (from ws9/ws13/ws14 roadmaps)
- PDF: embedded Inter subset for Unicode; per-entry xref links; compressed
  streams; OS print.
- Reports: BOM test with a strip-level terminal article; report templates;
  wire list by wire number; xlsx "Created" from the project's save time.
- Checks: optional pins, cross-section vs protective device (need WS-02),
  shield landing on a deleted terminal (from reviewer C).
