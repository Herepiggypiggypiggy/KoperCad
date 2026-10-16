# Onboarding and discoverability: handoff

Repo herepiggypiggypiggy/kopercad, branch claude/project-thread-dghc5r. Brief: own discoverability;
draft PRs, small and often, no build. Start from docs/design/discoverability.md (audit table, order of work).

Coordinate with: the first-run thread (PR #428, session cse_019sSSXcKhY8k5uYwY3cgSyf; owns the walk-through
card, practice project, example), the workspaces and design-system threads (widgets, panel moves), and the
workflow thread (commands.rs, keymap). Do not edit PR #428's files.

Key finding: `view.shortcuts` (`?`) has no handler in novo-app; it falls to the "is not in the app yet" message
in `run_command` (app/workbench.rs).

DISC-1 (2026-10-16): the sheet is `app/shortcuts_sheet.rs`; `sections(keymap, filter)` is the pure part, the window is `NovoApp::shortcuts_window`. Gestures that are not one key press live in `GESTURES` there; add a line when a hold/tap behaviour is added. DISC-2 continues with `tip()`, hint counters, What's new and the dead-command audit. onboarding.rs untouched.

DISC-2 PR 1 (2026-10-16): hints live in `HINTS` in app/tips.rs; add a hint with one line. Still to do: What's new pill/Help badge (assets/whats-new.md, last seen version in Settings, never opens by itself), tooltip coverage test over remaining controls, board/first-wire hints, error wording pass. Keep the Help badge consistent with the Learn menu structure (lesson-plan thread cse_01GtrgYmDXm75UfvZzUV67rz).
