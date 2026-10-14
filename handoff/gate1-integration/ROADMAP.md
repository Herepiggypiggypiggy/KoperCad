# Gate 1 integration roadmap

## Scope
Host the Gate 1 engines in the app (crates/novo-app, owned by WS-01). Gate 1 in the real app: create or open a project, place a DOL starter over two pages, wire it, see terminals and potentials, save, reopen and export PDF.

## Done (PR #53, head 67637b7, awaiting final READY from both reviewers)
- Editor hosted (EditorCanvas). Command field and keymap. Terminals drawer and proposals as marks. xref labels and navigation. Issue pill, list, F8 and problem marks. Export PDF. Headless e2e test.

## Next, in order (WS-01, new PRs from main after #53 merges)
1. PDF: don't replace a user's own `<stem>.pdf` (check /Producer or use a distinct name); write to a temp file and rename.
2. Focus scope: canvas keys only when the canvas has focus or hover, not when a panel button does. Needs the canvas widget id (ask WS-03/WS-06).
3. Remove the `proposal.accept` + "all" workaround in app/workbench.rs, which #63 makes redundant.
4. Cables drawer (Ctrl+4, novo-cables) and Devices drawer (Ctrl+5). Selection link drawer → drawing.
5. Dismiss a terminal proposal (remembered, DESIGN §2). Issues as an outline mode with filter chips.
6. Inspector (DESIGN §3) and tool dock with hint and proposal count. Space-held pan.
7. xref labels drawn by the canvas scene instead of the app overlay (with WS-03).

## Open questions
- Should Enter take the proposal nearest the pointer (where the hint shows) rather than the Tab-cycled one?
- Single click selection under Xvfb: does it work on Windows?
