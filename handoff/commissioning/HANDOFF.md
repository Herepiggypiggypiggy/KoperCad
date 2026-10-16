# Commissioning thread: handoff

PRs merged: design #399, engine #431, storage #446, app #471, follow-up #494 (edit lock, two-step Pass all open, Large buttons in Settings). Nothing of mine is open except this handoff. Crate `novo-commissioning`, storage schema v3 `sidecar` table are on main.

## How to reach it (entry points)
- Built in app PR #471 (`ws-commissioning-app`): palette/command `Commissioning` (`view.commissioning`) and the outline's fourth tab "Commissioning" (Checks and Punch list).
- Follow-up PR #494: edit lock while the tab shows (canvas edits undone; command edits not yet), Pass all open, Large switch.
- Not mine: CLI command and File > Export entry (report thread session_015JWsgz1ouocSSfvtqPic6V, PR #452 export menu); read data with `ProjectFile::read_only(p)?.sidecar("commissioning")` + `CommissioningStore::from_json` + `to_report_input`.
- Open: Space/F keys (collide with pan and tool keys; with the Workspaces keymap), lock for command edits, `ProjectFile::create_as` does not copy sidecars (WS-28 scripted save-as).
- Mode switch "Commissioning" next to Changes mode (Workspaces, menus and navigation thread, session_01WAQSEGkMbyreCPG18tsSPc).
- Palette `Commissioning` / `Export commissioning report`; shortcut proposal Shift+K.
- Tick panel (Pass / Fail / Note) and Punch list tab in the mode's dock (built by this thread in novo-app).
- File > Export > "Commissioning report (PDF, CSV)" (UI integration: reports and checks, session_01TaTTPuTqhk8bHd277oJRFR) and `kopercad-cli commissioning-report`.

## State
- Decision: state lives in the sidecar document `commissioning` (JSON of `CommissioningStore`), written at once.
- Report thread (session_015JWsgz1ouocSSfvtqPic6V, #404 `CommissioningInput` in novo-reports) owns report + tablet layout; I owe `to_report_input(&Project, &CommissioningStore)` once #404 is on main.
- Reviewers: #431 and #446 with Main reviewer C (session_012GWMePFNbeNns8NbmSMaXJ).
- Done: to_report_input (in #431), app tab (#471). Next: edit lock while the mode is on, Space/F keys, compact tablet layout, export menu entry (reports UI thread, #452), CLI command.
- Resume: `cargo test -p novo-commissioning` (on the engine branch).

## Next step for a fresh session (not started, land-only mode at 80 % usage, 2026-10-15)
Move commissioning from the outline tab to `Workspace::Commissioning` (Workspaces thread agreed; #453 is merged, workspace.rs is on main):
- In `app/workspace.rs` set `is_built` true for Commissioning and drop its `workspace_unavailable` reason; remove the `"commissioning"` outline mapping in the layouts there.
- Draw the panel (`commissioning_outline`) in the right zone when `self.workspace() == Workspace::Commissioning`; delete `OutlineMode::Commissioning` and its tab/dispatch in `app.rs`.
- `commissioning_locks_edits()` becomes `self.workspace() == Workspace::Commissioning` (keep the lock; Undo/Redo stay disabled).
- Add a `view.commissioning` row in `menus.rs` (PLACEMENTS or PALETTE_ONLY); keep the Dutch words in `novo-workflow/language.rs`. Do not bind Shift+K; Ctrl+5 is the workspace key.
- Adapt `commissioning_tests.rs` (toggle test becomes a workspace switch). Touch layout slot belongs to Layout and branding.
- Known limits: the lock applies then undoes a canvas edit (needs a read-only flag in the editor view, WS-06, and a redo drop in `Document`, WS-02); only canvas edits are covered, not palette or key commands.
