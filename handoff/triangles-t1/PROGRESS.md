2026-11-04 12:40 UTC T1 started from main 9bda269; cabinet_flags.rs, pill/table/hover/inspector moved to the store, old problem_lines tests rewritten

## Follow-up PR #1954 (2026-11-05)
- Place-rule and wiring-opening flags (Source::Layout) are in the top-bar count (`counted_summary`) and are rows in the Issues list; click or Go to locates on the plate.
- Narrow devices get the triangle centred over the box top (badge spacing at 1366).
- Layout flags rebuild only when revision or rules change.
- Test `a_rule_flag_is_in_the_top_bar_count_and_the_issues_rows`; QA 9420 at both sizes (regression only: no fixture yields a rule flag through the UI).
