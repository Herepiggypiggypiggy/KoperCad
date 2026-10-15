# Revision diff on the canvas: handoff

Branch `claude/project-thread-at2e2h`. First draft PR adds
`novo_revisions::change_list` (`list.rs`): flat `ChangeRow { page, id, label, kind }`
in page order, labels read from the newer state (older for removed objects).

Resume: `cargo test -p novo-revisions`. App side lives in
`crates/novo-app/src/app/changes.rs` (Changes mode, outline list); it still
shows per-page counts only, so step 1 of ROADMAP.md replaces that with rows.

Snapshot interface: `Backend::revision_snapshot(RevisionId) -> Option<Project>`
already exists; no new interface needed for the diff. Not yet agreed with the
sibling revisions thread whether a "compare two revisions" needs anything more.

## How to reach it (project rule: an engine without a UI entry is not done)
- Outline > Changes: named change rows, click reveals the item (palette `changes`). Done in the click-to-reveal PR.
- Changes mode overlay: green added, amber changed, red ghost removed, legend, next/previous jump. Partly on main (#305, #323), polish next.
- "Export change report (PDF)" command: change list per page between two revisions. Not built.
- From/To pickers: owned by the Revision tracking thread (session_01SRPgw1A9Q7haevj9AyydQD); `compare` engine too, after #401 merges.
- UI threads told: "UI integration: reports and checks", "Workspaces, menus and navigation".

## Split settled 2026-10-15 (Supervisor)
Revision tracking thread: engine #435 (compare/FieldChange), CLI #445, Python #450. This thread: ALL app UI: clickable change list (#442), From/To pickers (revision or now), Compare window, PDF change report, overlay legend and next/previous jump. Build on #435 once merged. Revisions menu entry: to be named with the "Workspaces, menus and navigation" thread (session_01WAQSEGkMbyreCPG18tsSPc).
