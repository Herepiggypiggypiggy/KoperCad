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
