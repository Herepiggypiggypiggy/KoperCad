# Print preview and plot sets: roadmap

Scope: a real print preview for sheets, reports and the workshop pack, and multi-sheet plot sets to PDF.
Source: audit/creative/round5-skeptical-buyer.md, blocker 6 ("Print quality is never seen until export").

## Done (PR from branch claude/project-thread-xx2ycy)
- novo-pdf `print` module: `PrintSetup` (paper A4..A0, Letter, Legal, Tabloid or "as the sheet"; orientation; fit or fixed scale; margins; centring; page numbers) and `place()`, the one placement function used by preview and writer.
- `PdfOptions.print`; `draw_sheets()` draws once, the writer and the preview share it; footers "Page n of N", sheet name, "Printed at 65 %"; cut-off sheets are clipped to the printable box and reported.
- App: File > Export print > Print preview and plot sets... (palette: "print preview", "plot set"); also Reports > Export > Print preview..., PDF options > Paper and scale..., Bench output > Workshop pack > Preview.
- Plot sets: named page choices + setup, saved per user per project file (settings), "Save every plot set" writes one PDF per set.
- Workshop tab: each mounting plate cut into template tiles on A4 or A3, with tile names and page counts.

## Next (not done)
- Keep plot sets in the project file (needs a WS-02 model change; today they live in the per-user settings keyed by the project file path hash).
- Preview the other four workshop pack files (cut list, ferrules, labels, assembly): they are written by their own exporters and have no draw list yet.
- Report PDFs written by `export_report_pdf` (own paper settings) are not in the preview; only reports added as drawing pages are.
- Optional: line-weight styles per layer, tiling a large sheet over several small papers, a "smallest paper that fits at 100 %" suggestion, sending to the printer (needs the Windows print verb, no admin).

## Open questions
- Should a plot set also remember the checks appendix and revision row? Today both are off in the preview (appendix) and follow the PDF options (revision row).
