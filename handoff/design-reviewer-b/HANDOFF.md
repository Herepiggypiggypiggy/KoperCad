# Design reviewer B: handoff (2026-10-15 13:25 UTC)

Review a PR: `git fetch origin pull/N/head:prN`, merge onto origin/main in a scratch branch, `CARGO_INCREMENTAL=0` fmt, clippy, targeted tests (disk is limited). Test combinations of open PRs that touch the same API: they conflict silently.

Open: none waiting. READY: #267, #269 (3fffd93), #275 (b172a74), #279 (df2c826), #282 (7eede6c), #283 (16f06d9), #288 (fb31b87), #291 (d4c586d), #293 (d26979c), #295 (285a31c), #297 (8efb31a), #306 (a22eca1), #299 (d57c2bf), #301 (679e11d), #303 (f5cf791). Waiting: #318 DESIGN text for C cable; #290 fmt fix; #307 now READY (0e32675); future Shift-keyboard-move PR needs a drag-only hint. Next: new odd-numbered PRs.
Contacts: integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S; WS-01 cse_01AYuFPxz6eQReQdtzYKLMmW; WS-03 cse_013KCL5cYRtSrq4yndv3Z7TD.
