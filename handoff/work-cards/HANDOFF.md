# Work cards handoff

Branch claude/project-thread-uevsfv (kopercad). PR 1 holds everything listed in ROADMAP.md as done.
Code: crates/novo-layout/src/workpack.rs, crates/novo-pdf/src/workpack.rs, crates/novo-app/src/app/workpack.rs
(plus small edits in workshop.rs, bench.rs, view3d_build_order.rs, view3d.rs, menus and commands).
Tests: crates/novo-layout/tests/workpack.rs, crates/novo-pdf/tests/workpack.rs (+ unit tests in the module),
crates/novo-app/src/app/workpack_tests.rs. QA: qa/scenarios/183..187.

Resume: `export CARGO_TARGET_DIR=/tmp/target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`
(the cloud disk fills with debug info). `WORKPACK_DUMP=/path/x.pdf cargo test -p novo-pdf --test workpack a_dump`
writes a sample PDF; `pdftoppm -r 60 -png` shows it. QA needs `apt-get install openbox mesa-vulkan-drivers
libvulkan1 libxkbcommon-x11-0`; `scripts/qa.sh run --only workpack --out /mnt/project-files/qa/workpack/<name>`.

Traps: the packet cover cuts long CoverLine text with "..." and a test forbids that; QA scenarios that open a
fixture write next to the copy in /tmp/kopercad-qa-open-N; the Wires cards group by Circuit (power, control,
PE) to match the print pack's build order numbers. Scenario numbers 183-187 are this thread's block.

Wires not on a card: the wire cards carry the wires of the plated cabinet's checklist wire section (an end in
that cabinet). Cut-list wires with no end there (supply and PE leads of another cabinet, wires between other
cabinets) are on no card of this cabinet; the count check compares with the checklist, not the project. A
checklist wire that is missing from the cut list is printed with ~ and named on the cover (test
`every_card_length_is_a_cut_list_length_and_nothing_falls_back`).
