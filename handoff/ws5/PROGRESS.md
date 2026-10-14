# WS-05 symbols: progress log

Times before 15:18 are approximate (reconstructed afterwards).

2026-10-14 10:00 UTC Started WS-05. Wrote format v1 and the first 126 IEC 60617 symbols as generated TOML. Opened #2.
2026-10-14 12:00 UTC #2 merged. #12 hardened the validator and added bridges() and pass_through(). Merged.
2026-10-14 13:30 UTC #15: PassThrough newtype and changeover split into common-break and common-make. Merged.
2026-10-14 15:18 UTC #18: 148 symbols, conductor poles allow 2 or more ids, private PassThrough field. Green, merged as 9b75d56.
2026-10-14 15:30 UTC Tadjiro's EPLAN pain points relayed: quick insert, consistent terminals, clamp and jumper pieces.
2026-10-14 16:32 UTC #25: aliases, favourites, end plate, end bracket, center_off. 150 symbols. Green, marked ready.
2026-10-14 16:44 UTC #25 merged as 1a8498f, before review follow-up commit 9892e50 was pushed.
2026-10-14 16:49 UTC Reapplied the follow-ups on main as #37 (head 5049bba). Messaged the reviewer and WS-08.
2026-10-14 16:58 UTC Reviewer: #37 READY at 5049bba. No further pushes to #37.
2026-10-14 17:00 UTC Wrote handoff files. Generator scripts copied to handoff/ws-05-symbols/generator/.
2026-10-14 17:12 UTC #37 merged as f0a5ea1. All WS-05 work is on main; thread retiring. Next items are in ROADMAP.md.
