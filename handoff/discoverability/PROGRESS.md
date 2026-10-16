# Onboarding and discoverability: progress

- 2026-10-15: audit of main 074138a done; docs/design/discoverability.md written (draft PR).
- 2026-10-16: DISC-1 shortcut sheet built (app/shortcuts_sheet.rs). `?`, Help > Keyboard shortcuts and Ctrl+K "shortcuts" open it. Generated from the live keymap (user keymap.txt included) and COMMANDS; filter matches English and Dutch words and keys; Hold and tap text rows; unrunnable commands greyed "Comes later"; a click runs the command. Tests in shortcuts_sheet_tests.rs.
- 2026-10-16: DISC-1 merged (#564). DISC-2 PR 1: tips.rs (tip_text, hint counters, Show hints again, empty-page hint), dead-command audit test, doc addendum. Dock tooltips now read keys from the keymap. Audit found four commands with no handler: project.rules, draw.tidy, edit.renumber, nav.changes (listed in COMES_LATER).
