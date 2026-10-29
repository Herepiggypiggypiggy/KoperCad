2026-10-29 11:50 UTC Engine (inject, walkthrough) and 13 integration tests written; Defects card wired into the live view.
2026-10-29 12:10 UTC R2/R3 review points: clamp run time to 3600 s and skip idle ticks, Tripped closes signalling make contacts, clearer cut-wire text, neutral colour for 'no checks yet'.
2026-10-29 13:50 UTC F1b: machine-level hoist defects (`novo_sim::machine`): `machine` / `machine_repair` steps, `hook` checks, tick-by-tick run with hoists; the Defects card runs them with the project's hoists. 6 integration tests, 1 app test. Lost encoder, drive overcurrent, STO and radio link loss still open.
2026-10-29 15:20 UTC F1b follow-up (R2): hoist runs capped at 120 s (21.6 s debug at 600 s on the UI thread); hoist id keeps its case in the report.
