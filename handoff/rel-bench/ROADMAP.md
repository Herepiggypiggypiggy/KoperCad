# REL-BENCH: roadmap

Scope: release v0.1.0 blocker B7 (`release/v0.1-plan.md`) from the motor-starter QA (`qa/motor-starter.md`): the bench output of a motor starter must be right, because the workshop cuts and drills by hand.

Checked on main (2eb1156) before starting:
- (a) wire numbers across the overload relay: fixed in `novo-numbering` (`split_at_device_bridges`); regression test added over the whole motor-starter fixture.
- (b) feeds and motor PE on the bench cut list: already included (`wires::items(.., with_feeds = true)`); regression test added.
- (c) export success messages: most exports already raise a toast with "Open folder" and error toasts expire (30 s). Still missing: STEP/AML/EPLAN, PLC tags, safety report. Documents folder missing: falls back to the home folder.
- (d) default wire size and colour: did not exist. Added "Fill in" row on Bench output > Cut list.

Next (not in this lane): ferrule tab preview and the greyed "Save to project" reason (cut list thread, bug 12).
