2026-10-15 UTC: ten draft lessons written and tested (29 tests); draft PR opened.
2026-10-16 UTC: #510 merged. Nine lessons rewritten to the teaching standard (what+why, how/how_nl, demo, solve, Dutch, wording pass); every instruction run headless in crates/novo-app/src/app/lesson_verify_{terminals,cables,outputs}.rs. wire-numbers start no longer has the duplicate number (allow-list in novo-checks removed).

- 2026-10-16: plc-io press-button text now says a pushbutton is closed only while held (momentary since #606). #644 merged. Waiting on runner PR #682 (rename, cable, issue_revision, assign_part, held key) and later select-by-terminal and ui_click.
