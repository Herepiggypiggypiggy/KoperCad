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
1. DONE as PR #42 (Gate 1 scenario + gap list). Drive #42 to merge; then un-ignore each gap test as its PR merges (#34, #38, #37, #29, #35, #30, #36, #28).
2. Once #30 merges: set X1:PE to `TerminalKind::ProtectiveEarth` in the fixture (patch: `handoff/tester/pe-fix.patch`, apply with `git am`) and flip #30's test `gate1_checks_flag_the_pe_terminal_kind` to assert `check()` is empty.
3. Once #34 and WS-01's follow-up merge: drive the scenario through `NovoApp::execute(Command)` (returns Option<Result<Outcome>>), `NovoApp::document()`, `current_page()`, built headless with `NovoApp::with_settings(.., Settings::default())` (avoids reopening the last project). Example test: `commands_run_through_the_window_reach_storage` in handoff/ws-01-app-shell/workbench.patch.
4. After #28 merges: in e2e_motor_starter.rs assert hits[0] is -X1:2 (F2 verified fixed on 924be9b) and mark F2 fixed in TEST_REPORT.
5. DONE: Gate 0 60 fps row filled in (#42).
6. Keep re-testing main after every merge; new round in TEST_REPORT.

## Open questions
- None blocking.
