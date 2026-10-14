# WS-10 Cross references: roadmap

## Scope
Live cross references derived from the model (never stored, no update step):
contact images on coils, coil location on contacts, interruption-point
partners, G / Shift+G / Alt+Left-Right navigation logic, selection
highlight + status line, "Link cross reference" as one undoable command,
tag lookup, issues for design checks (WS-14). Crate: `crates/novo-xref`.
Spec: docs/WORKFLOW.md J6 (PR #24) + Workflow thread's acceptance list
(P1 live refs, G, Shift+G, jump history; P2 highlight, linking, rename;
P3 too-many-contacts in the contact image).

## Done
- novo-xref crate: XrefIndex, LiveXref, Navigator/JumpHistory, link_command,
  XrefAction (one list for context menu + command bar), issues, find via
  Project::search. Built on #26 model API (location_of, cross_references,
  SetRole, PageFormat::columns). 19 integration tests + unit tests + doctest.
- #31 merged: DESIGN.md format (=A2/3.4) and go_to_tag (Ctrl+K exact).
- F## Next, in order
1. #89 merged. Get the contact-image PR (compact rows, Label.anchor) READY.
2. Review notes still open from #31:
   - Renaming a named interruption point on link can split a node: make it
     a proposal or a warning.
   - Reading order: call `Project::cross_references` instead of re-sorting,
     so the structure-first page order stays in step with core.
   - A terminal symbol is grouped only under its first terminal's strip.
   - Renumbering gives up silently when ordinals 1-9 are all used; report
     it as an issue once WS-14 checks consume `issues()`.
   - WS-03 to honour `Label.anchor` (Below/Right) in canvas `SymbolNote`.
3. Share one ContactKind classifier with WS-14 novo-checks (#50 copies it).
4. Help WS-03 / WS-01 draw `labels_on_page` and the highlight in the scene.
5. Primary link path: tag in place / drop a contact on a coil (WS-06).
6. Contact limits from parts (WS-07) instead of `contacts.*` device props.
7. Pinned-flow freeze (DESIGN.md §8) once WS-02 models it.
8. Incremental index only if the 10k build (~11 ms release) gets too slow.

Decided: no "other direction" retry before DeviceHasMain. The only other
direction moves a contact off a device that already has its coil.

e_structure()/Reference.structure, drop the
   `function`/`location` page props (new PR from main if #31 is READY).

## Open questions
- Who owns jump history: WS-08 is building one; novo-xref has JumpHistory
  that WS-08 can absorb or replace.
- docs/DESIGN.md (Workflow thread) may change interaction; keep UI thin.
