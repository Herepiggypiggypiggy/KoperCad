# WS-09 PDF and print: roadmap

Scope (roadmap): vector PDF with clickable links and bookmarks, page sets, A3 and A4.
Done when the Gate 1 project prints to a clean PDF. Exports are never blocked by problems (DESIGN.md).
Crate: crates/novo-pdf (owner WS-09). Started 2026-10-14 by the coordinator session.

## Done
- novo-pdf crate: export_pdf / export_pdf_with_art / write_pdf, PdfOptions, PageSet, print_style.
- One page per project page (pages_in_order), size from PageFormat (A0-A4, landscape or portrait).
- Drawn from novo-canvas PageScene (no canvas API change), black on white.
- Helvetica (standard 14) with AFM widths; anchoring and rotation as on the canvas.
- Bookmarks per page, page labels = page names, GoTo links between device parts and interruption points.
- Deterministic bytes; an empty project or page set gives a blank A3 page.
- Gate 1 tests with lopdf; example `gate1`.

## Next, in order
1. Get the first PR (#49, branch claude/ws09-pdf-export) reviewed and merged.
2. Use LibraryArt once the editor PR (#38) merges: the example and the app pass it to export_pdf_with_art.
3. App command `export pdf` (with WS-01/WS-08): Documents folder, export summary listing warnings, never blocked.
4. Embed an Inter subset (novo-theme assets) as a CID font so any Unicode prints (pure Rust: subsetter + ttf-parser).
5. Per-entry cross-reference links once WS-10 draws reference labels (TODO in link_target).
6. Title block and column grid print automatically when WS-03's frame (#47) lands: check the Gate 1 render then.
7. Optional: compress content streams and keep determinism.
8. Print (OS print dialog) after PDF is solid; per user only, no admin rights.

## Open questions
- Monochrome vs colour option for wires (now always black via print_style; callers can pass any ModelStyle).
- Frozen pages after export (DESIGN §8 pinned flow): does export record a revision? Owner: WS-02 / design.
