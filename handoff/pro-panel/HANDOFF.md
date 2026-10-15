# Pro Panel research: handoff

Branch `claude/project-thread-i979ll`, draft PR with `docs/PRO-PANEL-GAP-ANALYSIS.md` (docs only, no code).

State: doc written from public EPLAN pages; no EPLAN data copied. Ranked plan R0 to R9 in section 6, threads in section 8. R7 (#436) and R8 (#451) are implemented and merged; their app entry points are still to do (ROADMAP.md, Next).

Resume: read the doc and ROADMAP.md Next. Run no build (owner said no builds until asked).

Decisions made: 2D first, 3D after the enclosure depth and door data exist (R2). Machine formats and maker online tools are skipped until a machine is named.

Traps: thermal formulas in R3 must be checked against IEC 60890 before numbers are shown; the doc says so.

## State at 19:50 UTC (usage limit near; this thread is idle)

- #379 (this doc plus handoff): out of draft, with Reviewer B (session_017hRHV6ik9P7Wcan8CARcGN), no verdict yet. Docs only; the integration lead merges.
- #436 R7 place rules engine: merged. #451 R8 assembly checklist with the Drill step: merged.
- #513 Export assembly checklist (File menu, palette `export.assembly`): READY on 10af2ff, out of draft, with the lead and merger B. One nit rides the next app PR: `ASSEMBLY_HINT` in app.rs sits under the doc comment of `SHEETS_HINT`; move the comment down.
- #516 Place rules in the app (place-rules.txt, Place rules section in the Plate card): READY on 43e38b0 for its own commit, kept in draft on purpose. Base is #409's branch; when #409 merges: retarget #516 to main, merge main (rebase-only delta is accepted without re-review), undraft, and send the head SHA to the lead (cse_01FiLvRBKEcNNHbQEGhYyL1S) and merger B (session_011NKUp2tgBn8uVWK6WHsc7k).
- Pre-existing failing test seen on main, not from these PRs: `the_watcher_picks_up_a_file_named_after_the_article` in novo-app `parts_import.rs` (fails alone too); unconfirmed against a clean main.

Details per feature: handoff/place-rules and handoff/assembly-checklist (in #516 and #513).
