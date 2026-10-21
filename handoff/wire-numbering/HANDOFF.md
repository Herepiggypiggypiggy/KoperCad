# Wire numbering workflow: handoff
Engine: crates/novo-numbering (design docs/design/wire-numbering.md). UI: crates/novo-app/src/app/numbering.rs.
Where the user finds it: Ctrl+K "wire numbering" (opens on the Numbers tab), "renumber wires" (Preview), "check wire numbers" (Conflicts).
Added: `Group.page_lock`, `Reason::PageLocked`, `GroupPlan.{page,fixed,page_lock}`; `changed_groups()` skips PageLocked so Autopilot proposes nothing on locked pages.
Tests: crates/novo-numbering/tests/numbering.rs (locked page), numbering_tests.rs (Numbers tab). QA: qa/scenarios/24-wire-numbering-table.qa, shots in /mnt/project-files/qa/wire-numbering/.
Not done: see ROADMAP items 3-5.
