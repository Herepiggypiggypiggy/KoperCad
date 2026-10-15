# WS-17 PLC and I/O: progress

2026-10-15 03:30 UTC Thread started (coordinator brief: design-first PR, then engine in small PRs).
2026-10-15 03:50 UTC Wrote docs/design/plc.md: cards and channels as devices, addresses allocated like tags then kept, derived channel addresses, address style rule, I/O list, problems, who builds what.
2026-10-15 03:58 UTC Opened PR #143 (design only, head fb4807a); sent to design reviewer and main reviewer; told the integration lead. Added WS-17 to coordination/threads.md.
2026-10-15 04:10 UTC #143 head ad10cf4 (channels numbered per type). Engine PR 2 (novo-plc skeleton: layouts, allocation, address styles, 18 tests) committed on local branch ws17-engine (437a29f); pushes after #143 merges.
2026-10-15 04:25 UTC Channel index (PlcIndex: labels, free/next channel, §7 problems) committed locally on ws17-index (0f0953e), on top of the skeleton. Both as patches in handoff/ws-17-plc/patches/. Workspace tests with the skeleton: 731 passed, 0 failed, 2 ignored; clippy -D warnings clean (Linux).
2026-10-15 03:45 UTC Design reviewer and main reviewer: CHANGES on fb4807a/ad10cf4 (cards are K per DESIGN §6 / letter_code; plc.channel must carry the type; commands are composed transacts).
2026-10-15 03:55 UTC #143 head b8e3961 fixes all three plus the optional notes; sent to both reviewers and the lead. Local engine rebased on main 96289b8: plc.channel = "DI 3", TypeMismatch issue, Rockwell reports no byte overlaps; patches refreshed.
2026-10-15 03:47 UTC #143 merged (design READY from the design reviewer and the main reviewer on b8e3961).
2026-10-15 03:55 UTC PR 2 (novo-plc skeleton) opened from main 6fa1492. Workspace on 96289b8 + skeleton + index: clippy -D warnings clean, 758 passed, 0 failed, 1 ignored (Linux).
2026-10-15 03:58 UTC PR #152 opened (skeleton), head 8527c4a; sent to main reviewer and lead.
2026-10-15 04:05 UTC Outputs agreed the io_rows shape (ROADMAP). PR 3 patch now also has Issue::message/at and READS.
2026-10-15 04:15 UTC PR 4 (io_rows, IoRow) committed locally on ws17-io; patch 0003. Next: commands (add card, place channel, to plc, renumber io).
2026-10-15 04:30 UTC PR 5 (commands: add_card, place_channel, to_plc, renumber_plan/apply_renumber) committed locally on ws17-commands; patch 0004.
2026-10-15 04:00 UTC #152 CHANGES (CHANNEL doc said integer; Rockwell DQ/RO collide). Fixed in c7a9347: doc reads "DI 3", Rockwell digital uses the card's point (bit offset). Sent to main reviewer.
2026-10-15 04:05 UTC Local stack (0002-0004) rebased onto #152 head c7a9347; workspace clippy clean, 766 passed / 0 failed / 1 ignored (before the rebase, base 96289b8). Patches now apply on top of #152.
2026-10-15 04:10 UTC #152 READY (main reviewer) on c7a9347; waiting for the lead to merge. Frozen: no more pushes.
2026-10-15 04:15 UTC #152 merged. PR 3 (channel index) opened from main f5e4980.
2026-10-15 04:20 UTC PR #160 opened (channel index), head 95430f7, sent to main reviewer. Patches 0003-0004 rebased onto it (branch ws17-next).
2026-10-15 04:25 UTC #160 READY (main reviewer) on 95430f7. Non-blocking notes (Rockwell SharedSlot error, skip NoSuchChannel on bad layout, READS Page) go into the io_rows PR.
2026-10-15 04:30 UTC #160 merged. PR 4 (io_rows + #160 review notes) opened from main.
