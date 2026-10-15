# Tester / test report writer: roadmap

## Scope
Test every PR and main after merges (build, fmt, clippy, tests, roadmap gate
scenarios), look for gaps, keep `docs/TEST_REPORT.md` current, and own the
tests-only crate `crates/novo-tests` (Gate 1 motor starter fixture and
cross-crate tests). Check every PR for admin rights or machine-wide paths
(Tadjiro's hard rule: everything works without admin rights). Report to the
integration lead (session_011PfTJ1QaP1JQJPwXcFnxBE).

## Done
- novo-tests crate: Gate 1 fixture (`motor_starter`, `motor_starter_with_changes`), stress page.
- Tests: gate1_motor_starter, gate0_stress, fixture_matches_library, robustness (F1 NaN/inf), gate0_storage (save/reopen, crash recovery).
- Findings: F1 (fixed #11), Q1 (resolved #12), F2 (terminal tag steals clicks, WS-03, open).
- TEST_REPORT rounds 1-5 and the Gate 0 summary (merged in #32). Gate 0: all met except 60 fps on a real GPU (Tadjiro's PC run pending).
- Standard-user installer verified in #3's CI.

## Next, in order
1. Drive the Round 11-12 PR (Gate 2 assessment) to merge.
2. Wiring modes: Corners and Lines journeys once engine PRs 1 and 2 merge; unignore the random-move guard; drop the same-net filter.
3. Keep re-testing main after every merge; new round in TEST_REPORT.
4. Windows: check main CI, then the first real build and test with the release thread.

## Open questions
- None blocking.
