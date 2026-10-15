# WS-02 Data model: progress log

2026-10-14 earlier UTC Merged: #11 model API v0, #14, #21 format pin + scale test, #26 xref + search.
2026-10-14 16:42 UTC Opened #35: potentials (b), terminal rule (c), junction styles (e), texts (f).
2026-10-14 16:45 UTC Reverted (e) per lead; reshaped (c) to same_name/join_link, new projects Join (792b476).
2026-10-14 16:47 UTC Added Terminal.part for WS-07 (2b921bb); PR body lists enums with new variants.
2026-10-14 16:53 UTC Review: changes needed, name-based potentials must be a rule.
2026-10-14 16:55 UTC Fixed: rules.potentials.from_terminal_names (7677b8c); reviewer pinged.
2026-10-14 17:00 UTC Handoff folder written.
2026-10-14 17:05 UTC Reviewer: #35 READY at 7677b8c (no more pushes to #35). Started SetText follow-up on local branch ws02-set-text (not pushed; open as new PR after #35 merges).
2026-10-14 17:20 UTC Local: Page.structure for WS-10 committed on ws02-set-text; roadmap updated with DESIGN.md section 10 model list.
2026-10-14 18:26 UTC (while paused) Coordinator redid ws02-set-text as #45, merged. #35 merged 17:18.
2026-10-14 21:35 UTC Resumed. Opened #82 (feeds on pins/terminals, PotentialSystem) from main, head ba861aa; reviewer pinged. 627 tests pass locally (CI down).
2026-10-14 21:50 UTC Terminal-article decision sent (Terminal.part per terminal; strip rule terminal_part.<kind> stays WS-11's; novo_parts::terminal_part resolves). Added WS-14 requests to #82 (head f6d41cf): Pin.optional, rated_current_a, Net.sources, symbols() order.
2026-10-14 22:10 UTC Local branch ws02-next (on #82 head f6d41cf): Structure+enclosure, Dismissal records (2f7d599); page freeze (775d81e). Patches saved in patches/ (git am onto #82 head or main after #82 merges).
2026-10-14 22:25 UTC Local ws02-next: MoveToPage (cross-page move with wires, labels, cable cores, dismissals). 633 tests pass. Patches refreshed.
2026-10-14 22:35 UTC #82 READY at f6d41cf (21:42). Local ws02-next: review follow-ups (prefix for isolated systems, repeated feed pin refused, format notes). Note 3 (Pin.optional from library) is for WS-05/WS-06.
2026-10-14 22:50 UTC #82 merged. Lead order: one small PR each, review fixes -> MoveToPage -> Structure+Dismissal -> freeze. Opened #101 (review fixes + isolated prefix) at 8a5569d, 642 tests pass; reviewer messaged. MoveToPage rebased onto main without Dismissal handling as local branch ws02-move (5b03fe0, patch 0005, 643 pass); push it once #101 merges. When Dismissal lands, add dismissal carry-over to move_to_page again (see patch 0003).
2026-10-14 23:00 UTC Restacked on main: ws02-move (MoveToPage) -> ws02-structure (Structure+Dismissal, dismissal carry-over in move_to_page restored) -> ws02-freeze (page freeze). Patches 0005, 0006, 0007 apply in order onto main; 646 tests pass at the top. Patches 0001-0004 are obsolete.
2026-10-14 22:05 UTC #101 merged (both reviewers READY at 8a5569d). Opened the MoveToPage PR from main with the repo handoff refreshed (terminal-article decision included). Workflow thread answered the blocks questions; plan in HANDOFF.
2026-10-14 22:15 UTC Restacked Structure (0a94030) and freeze (7368339) on top of #107 head 105cef4; patches 0001-0002 now; 651 tests pass.
2026-10-14 22:45 UTC Blocks (DESIGN §7, workflow answers) on local ws02-blocks 89282d6, stacked on freeze; patch 0003. define_block/place_block/set_block_values/set_block_override/detach_block + commands; deleting a member detaches. 655 tests pass. Not yet: definition updates (apply new version keeping overrides), auto-marking overrides from edits, instance.page after MoveToPage.
2026-10-14 22:25 UTC #107 merged (READY at 105cef4). Opening Structure+Dismissal PR from main (670 tests pass). Freeze and blocks rebased onto main.
2026-10-15 02:50 UTC #117 READY (CI no logs, Actions down; commented). ws02-followups committed locally (675 tests). Logged WS-08 detach-on-add gap for blocks.
2026-10-15 02:58 UTC #117 merged. Page freeze rebased on main 27c2e17 (680 tests pass), opening PR.
2026-10-15 03:10 UTC #119 opened (page freeze). Device-inside-block detach added on ws02-followups (676 tests).
2026-10-15 03:08 UTC #119 READY. Review notes 3-4 committed on ws02-followups.
2026-10-15 03:15 UTC #119 merged. Blocks rebased on main d7f70b8 (686 tests), opening PR. Follow-ups branch to rebase after.
2026-10-15 03:20 UTC #125 READY. Its review notes committed on ws02-followups (686 tests).
2026-10-15 03:22 UTC #125 merged. Follow-ups rebased on main 64647d0 (696 tests), opening PR.
- 2026-10-15 04:20 UTC: core PR for Outputs and WS-01: generated page order (contents first, reports after the drawing), `ProjectRules.generated_pages_off`, `DeletePage`, `Document::amend_last`, issued revisions undeletable.
- 2026-10-15 17:20 UTC: cabinet placement storage for Layout (Rail, CabinetPlacement, Structure.plate, SetPlateSize/AddRail/PlaceDevice/RotatePlacement/RemovePlacement), 1029 tests.
