# Scenario numbers

Branch `claude/project-thread-lhmago`. Scenario 18130 was used by the cabinet group drag (#1932) and the
docked Fast keys scenario (#1982). The newer one is now 18133 (`18133-fast-keys-pane-without-live.qa`),
behaviour unchanged. Nothing referenced the old name outside the file (older qa/pr*/ evidence folders are history).

Guard: `scenario_numbers_are_unique_apart_from_the_old_shared_ones` in `crates/novo-cli/src/qa.rs`.
The number is the text before the first `-` of the file name. Older numbers used twice are listed in
`SHARED` and must not grow; any other repeat fails. Next free in the 1813x block: 18134.

QA-tool change, no in-app location.
