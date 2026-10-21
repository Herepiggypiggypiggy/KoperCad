2026-10-21 20:00 UTC engine, report, app view, command and fixture written; 35 tests green
2026-10-21 20:50 UTC QA scenario 58 at 1366x768 and 1920x1080, report in /mnt/project-files/qa/single-line-2026-10-21
2026-10-21 21:30 UTC rebased on main; fixed Dutch strings, CLI export single-line, report name test; fmt, clippy, tests for changed crates green
2026-10-21 22:00 UTC review round on #1281: Iz and ΔU marked ? when defaulted (drawing, schedule Assumed column, legend, notes); Ik shown as "Ik ≤" ceiling with legend line; legend bigger; schedule columns reordered; single-phase motor current uses typical eta and cos phi; no summing amps across a transformer or power supply (power carried to input voltage), drop restarts at converter output; Reports menu: plug list and single-line diagram in "More reports"
2026-10-21 22:30 UTC R3 round 2: Why column names the missing inputs for an incomplete check and sits before a short Assumed column; QA re-run on this tree
2026-10-21 23:30 UTC follow-up PR 2 of 3 (diversity factor): device and supply factors, Electrical inputs field, legend/notes/basis, fixture distribution-board-diversity.novo, QA scenario 65 (/mnt/project-files/qa/single-line-diversity-2026-10-21/)
2026-10-21 23:00 UTC follow-up PR 1 of 3 (short-circuit current along each run): Node.ik_ka/ik_exact/icu_status, report shows Ik per box, Icu check, notes and legend; QA scenario 58 re-run (/mnt/project-files/qa/single-line-ik-2026-10-21/)

## 2026-10-21 diversity factor, R3 fixes
- Own "Diversity" column in the schedule; Electrical inputs > Distribution field with hint "none" when empty.
- Notes for a factor outside 0.1 to 1 and for a factor with nothing to scale.
- Merged main and the short-circuit PR; QA at 1366x768 and 1920x1080 (scenarios 67, 68).
