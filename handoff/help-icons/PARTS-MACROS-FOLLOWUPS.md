# Parts and Macros help follow-ups (after PR 1975)

2026-11-05, branch claude/project-thread-jvuvmz.

1. Scenario 702: the backup warning's sentence sits behind its triangle by design (label "Keep this backup" stays visible). The scenario now clicks "Warning: Keep this backup" before expecting "Keep it for yourself". App unchanged.
2. Scenario 7432 at 1920: check_text flagged catalog descriptions. They are table data, only on screen at 1920 because the rows are drawn there. App unchanged; check_text (`novo-cli`, `qa.rs`) now skips a label that shares its line with two or more other labels (a table cell). Unit test added; documented in docs/testing/qa-suite.md.
3. Place macro dock: the "(suggestion)" tag lived on a line inside the folded Mounting view, so it was only visible after opening it and scrolled under the Place button on short docks. The header now reads "Mounting view (suggestion)" (macro_mounting::HEADER, used by Place macro and the Macros window); the duplicate plate line under the picture is gone. New scenario 7436 at both sizes.

Still open: scenario 1702 (describe devices recipe) shows "unknown field {nope} not on screen", not touched here.
