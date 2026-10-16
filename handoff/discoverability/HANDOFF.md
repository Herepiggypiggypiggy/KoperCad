# Onboarding and discoverability: handoff

Repo herepiggypiggypiggy/kopercad, branch claude/project-thread-dghc5r. Brief: own discoverability;
draft PRs, small and often, no build. Start from docs/design/discoverability.md (audit table, order of work).

Coordinate with: the first-run thread (PR #428, session cse_019sSSXcKhY8k5uYwY3cgSyf; owns the walk-through
card, practice project, example), the workspaces and design-system threads (widgets, panel moves), and the
workflow thread (commands.rs, keymap). Do not edit PR #428's files.

Key finding: `view.shortcuts` (`?`) has no handler in novo-app; it falls to the "is not in the app yet" message
in `run_command` (app/workbench.rs).
