# Reviewer R4 handoff (2026-10-16)
Thread "Reviewer R4 live view, Learn, commissioning", spawned by the coordinator. I review, never push or merge.
Lane: live view and simulation UI, Learn window and lessons, commissioning, onboarding (taken over from R3).
Routine: same as handoff/reviewer-r3/HANDOFF.md. Fetch refs with `git fetch origin +pull/N/head:refs/review/prN` in /home/claude/kopercad, read the diff, check the exact head SHA (full 40 chars) and ubuntu CI on that head (mcp__github__pull_request_read get_check_runs). Only rerun tests locally for touched crates when CI is not green. Post a COMMENT review "**Verdict: READY on <sha>**" or CHANGES NEEDED, then send_message lead A cse_01FiLvRBKEcNNHbQEGhYyL1S and merger B cse_011NKUp2tgBn8uVWK6WHsc7k. Skip PRs with READY on the current head.
Lessons standard: every step says what, why, exact how; Show me demonstrates start/end; keys from keymap; Dutch twins; status early shows with a badge, draft hidden.
