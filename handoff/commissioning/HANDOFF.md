# Commissioning thread: handoff

PRs: app #471 (stacked on #431 and #446, design reviewer), design #399 (`ws-commissioning-design`), engine #431 (`ws-commissioning-engine`, crate
`novo-commissioning`), storage #446 (`ws-commissioning-storage`, schema v3 `sidecar` table).

## How to reach it (entry points)
- Built in app PR #471 (`ws-commissioning-app`): palette/command `Commissioning` (`view.commissioning`) and the outline's fourth tab "Commissioning" (Checks and Punch list).
- Still to do: items below.
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
