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

State at 2026-10-25 15:00 UTC (all merged): the strip length per wire (#1494), wires outside the plate as tick-off cards (#1499), "1 of 2" parts and "size not set" (#1500), the unplaced-wire cover note with pointers on the other packs (#1507), the duct fixing rule (#1509, #1512), cable cores and the label stock line (#1508).

Decisions taken by the thread while the owner was away, to review on return (all reversible, all labelled on the cards):
- Duct fixing: 25 mm from each end, at most 200 mm apart (`DUCT_FIXING_END_MM`, `DUCT_FIXING_PITCH_MM`); the holes are not in the drilling list.
- Label stock: the card names the project's label setting (Bench output > Labels tab); no new setting.
- Cable cores: a core row reads "<size> core =A1-W1" and the cover says the cable is bought and run once. The example's 12 m wires are the cores of cable =A1-W1, so the example and the estimate were right.

Known gaps: no fixture with two plated cabinets and an unplaced wire (the model refuses removing a plate that has devices), so `unplaced_notes` is unit-tested only; `WireCut` has no `core` field (the outside cards read the cable from the kind text, `CORE_MARK`); the 11-fixing duct note is proven by a unit test, not a picture.

Stamps (2026-10-26): every card header reads `Rev xxxxxx  printed YYYY-MM-DD`; the revision is a hash of what the pack prints, not a counter. The app remembers the last print per project file for the session (`wb.workpack_print`, `workpack_stale_note`); a single card printed on its own updates its own entry. Saw sizes go through `half_mm` (0.5 mm). `write_workpack` and `write_step_card` refuse the open project file (`backend::same_file`). Scenario 191.
## Cut-out cards come first (2026-10-26)
add_opening_cards and add_wiring_opening_cards stable-sort Opening cards to the front of pack.cards; code that took `cards.last()` for an opening must use `first()`. Cut-out templates label their axis numbers with mm.
