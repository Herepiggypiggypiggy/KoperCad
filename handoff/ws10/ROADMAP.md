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
- Draft PR #31. DESIGN.md format (=A2/3.4) and go_to_tag (Ctrl+K exact).

## Next, in order
1. CI green on Linux + Windows for #31, mark ready, get reviewer READY.
2. After #31 merges: help Canvas (cse_013KCL5cYRtSrq4yndv3Z7TD, #28) draw
   `labels_on_page` and `set_related(highlight().on_page)`; help App shell /
   WS-08 (cse_01TAFG6LPGy2WMwhJYXmxqxF, #29) bind G, Shift+G, Alt+Left/Right
   and register XrefAction ids in the command bar. They own that code.
3. Primary link path per Workflow thread: giving a contact its device by
   typing the tag in place or dragging onto the coil (WS-06 owns editing;
   offer `link_command` / SetRole helpers).
4. Contact limits from parts (WS-07) instead of `contacts.*` device props.
5. Incremental index update from ChangeSet if 10k build (~11 ms release)
   ever becomes a frame-time problem.

6. Pinned-flow freeze (DESIGN.md §8): show frozen locations on frozen
   pages once WS-02 models revisions/freeze.
7. When WS-02's Page.structure PR (after #35) merges: use
   Location.structure in page_structure()/Reference.structure, drop the
   `function`/`location` page props (new PR from main if #31 is READY).

## Open questions
- Who owns jump history: WS-08 is building one; novo-xref has JumpHistory
  that WS-08 can absorb or replace.
- docs/DESIGN.md (Workflow thread) may change interaction; keep UI thin.
