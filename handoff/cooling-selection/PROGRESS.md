# Cooling selection progress

- 2026-10-22: engine with 13 hand-calculated tests, panels, standards report lines, QA scenario 44 at 1366x768 and 1920x1080 (report in qa/cooling-selection).
- 2026-11-04 E5 cooling (#1902): the Cooling selection reads the outside size from the enclosure record (`novo_layout::spec_cooling`), shows "Enclosure size: <weakest source>." plus the estimate notice while any record number is catalogue or default, and names a missing number in one line. The error line uses `pal.error` for now; lane T6 (problem triangles) moves it to a flag. QA scenarios 9520 and 9521 (fixtures from `qa/fixtures/make_cooling_spec_fixtures.py`, synthetic enclosure). Not done: Rittal cooling article suggestions (no sourced catalogue rows yet).
