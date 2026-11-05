# Evidence for the Calculations follow-up (source at git f3dc74c156)

HEAD.txt is the full sha of the code the pictures were taken from. Pictures: /mnt/project-files/qa/pr1934-f3dc74c1/shots/ (not in the repo).

- Scenarios 18740 and 186700 to 186703: 8 of 8 pass at 1366x768 and 1920x1080 (report.md, summary.md on the mount).
- Lesson anchors for the Mechanical and Busbar heat tabs (`calc.tab.mechanical`, `calc.tab.busbar_heat`): `cargo test -p novo-app anchor` from this head, 13 passed, including `anchors_doc_is_current` and `every_anchor_the_shell_registers_is_in_the_list` (the tab registers each anchor every frame it is on screen). Listed in docs/design/lesson-anchors.md.
- Model, not measurement: busbar values are planning values.
