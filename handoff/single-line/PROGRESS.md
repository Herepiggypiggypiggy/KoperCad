2026-10-21 20:00 UTC engine, report, app view, command and fixture written; 35 tests green
2026-10-21 20:50 UTC QA scenario 58 at 1366x768 and 1920x1080, report in /mnt/project-files/qa/single-line-2026-10-21
2026-10-21 21:30 UTC rebased on main; fixed Dutch strings, CLI export single-line, report name test; fmt, clippy, tests for changed crates green
2026-10-21 22:00 UTC review round on #1281: Iz and ΔU marked ? when defaulted (drawing, schedule Assumed column, legend, notes); Ik shown as "Ik ≤" ceiling with legend line; legend bigger; schedule columns reordered; single-phase motor current uses typical eta and cos phi; no summing amps across a transformer or power supply (power carried to input voltage), drop restarts at converter output; Reports menu: plug list and single-line diagram in "More reports"
2026-10-21 22:30 UTC R3 round 2: Why column names the missing inputs for an incomplete check and sits before a short Assumed column; QA re-run on this tree

## 2026-10-22 single-phase loads
- Kinds Lamp and Sounder; mains test = control net reached from a stated AC line voltage >= 100 V over switched poles and carrying a lamp or sounder.
- One-conductor runs: phase voltage = line voltage / sqrt(3), marked calculated (assumed when no potential voltage is stated); note in the legend.
- Fixture `distribution_board_with_signals`, QA scenario 69, pictures in qa/single-line-loads-2026-10-22/.
2026-10-21 23:00 UTC follow-up PR 1 of 3 (short-circuit current along each run): Node.ik_ka/ik_exact/icu_status, report shows Ik per box, Icu check, notes and legend; QA scenario 58 re-run (/mnt/project-files/qa/single-line-ik-2026-10-21/)
