# Reviewer A: roadmap

## Scope
Independent correctness review of app, core, storage, workflow, editor, terminals, parts, tests and process PRs (scripts/, workflows). New-crate PRs belong to reviewer B, overflow to C, and UI/DESIGN.md to the design reviewer.

## Done (2026-10-14)
28 reviewed heads; see PROGRESS.md. Process: the merge-tree-based scripts/check-mechanical-merge.sh (#51, #61).

## Next, in order
1. **#72** (fixer): one terminal_part source for strip rows, BOM and parts.
   - Check for BOM double counting.
   - Check that a deleted style part never resolves.
   - Gate 1 test: the BOM's terminal count equals the strip's terminal count.
2. Any re-head of #53.
3. Open non-blocking follow-ups worth a PR (none tracked elsewhere yet):
   - **#53:**
     - The PDF next to the .novo overwrites a user's own Plant.pdf: check /Producer or use a distinct name; write a temp file and rename.
     - Focus::Canvas applies while a panel button has focus; needs a canvas focus id from novo-editor/novo-canvas.
     - lopdf → [workspace.dependencies] (coordinator).
   - **#46:**
     - kept-untitled-*.novo can't be deleted from the UI, so the UNSAVED list grows.
     - The Save dialog defaults to "Untitled".
   - **#34:**
     - Save As onto the untitled temp file deletes it on Unix.
     - .broken untitled autosaves are only logged.
   - **#45:**
     - Location::qualified reads a bare "3.4" for an unstructured target from a structured page (WS-02/WS-10 decision).
     - set_text(None, None) records an empty undo step.
   - **#38:**
     - proposal_holds ignores crossings and symbol bodies.
     - Deleting a Device id doesn't tidy its pages.
     - The old "structure" page property isn't migrated to Page.structure.
   - **#29:**
     - Renumbering suffixed tags turns -K1.1/-K1.2 into -K1.1/-K2.2.
     - TagMode::Keep renumbers terminals.
     - split_verb slices bytes after to_lowercase (latent; Kelvin sign).
   - **#28:**
     - A 2-wire straight run draws a junction dot.
     - junction_glyph Tee{stem>=253} overflows.
   - **#36/#67:**
     - An unterminated CSV quote swallows the rest of the file.
     - Skipped-row line numbers count rows, not lines.
     - import_catalog: duplicate headers, last one wins.
     - Excel turns -K1 / =A1 into formulas.
   - **#47:**
     - marks::layout runs every frame, O(marks×primitives).
     - PageFormat.columns is unbounded; cap it in core.
   - **#49:**
     - PDF draws in id order, but the canvas draws in storage order.
     - Latin-1 glyph widths are approximate.
   - **#31:**
     - Renaming a named interruption point splits its same-name node.
     - The EN 50005 ordinal clash check compares pin names only.
   - **#41:** package-windows no longer runs on crate changes; schedule a weekly run.
