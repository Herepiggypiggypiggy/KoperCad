2026-10-15 13:12 UTC Started as design reviewer B.
2026-10-15 13:25 UTC #254 e7cca47 READY (canvas 118 tests, fmt/clippy clean). #252 f4233a1 CHANGES NEEDED: keys on LabelKind::DeviceTag which #254 changes; its test fails on #252+#254.
2026-10-15 13:40 UTC #252 388e9f8 READY (rebased with #254, fmt/clippy/tests pass). #254 merged.
2026-10-15 13:50 UTC #251 72dccf0 READY (look matches Layout mock-up; 4 mm end overhang accepted).
2026-10-15 14:05 UTC #259 b60400a READY (plain drag leaves wires as open ends, Shift carries). WS-01 follow-ups: Shift read at drop + dock hint, status line, Preferences default, tests.
2026-10-15 14:15 UTC #261 6bc814c READY (Preferences window; checked from code and tests, not in a window). Nits: Connections heading, (default), mark menu + Ctrl+,.
2026-10-15 14:25 UTC #262 81cdca1 READY (grid join; tests-file conflict with main, keep both).
2026-10-15 14:30 UTC #263 bb94e15 READY (corner/T preview; follow-ups: stored handle, hide piece on Lines pin drag).
2026-10-15 14:40 UTC #262 re-READY on 9530630 (merge of main only).
2026-10-15 14:45 UTC #266 baff400 READY (PE toggle, part search, grid double-click wire; even PR, taken at WS-01's request).
2026-10-15 15:10 UTC #265 52b64f7 READY (label-only drag). #267 b41e837 CHANGES: test fails on main (#266 grid double-click vs wires), Delete on a run must delete wire, add label_drag guard. #260/#264 left to the original reviewer.
2026-10-15 15:30 UTC #271 3fdf358 READY (live potentials overlay; follow-ups: repaint, tokens, paint order). #269 waits for #267 new head. Evens (#260,#268,#270,#272) are the original reviewer's.
2026-10-15 15:40 UTC #269 0ddb866 CHANGES (the #267 fixes missing, conflicts with main). #263 re-READY on aa8e481 (merge of main only).
2026-10-15 15:55 UTC #267 31e97f0 READY (all three fixes). #269 64397c9 CHANGES: corner drag makes diagonal stretches (DESIGN §4/§6); needs orthogonal constraint + test.
2026-10-15 16:10 UTC #269 78d6367 READY (orthogonal drag fixed). #267 re-READY on b255817 (merge of main only). #271 3fdf358 READY earlier (merged).
2026-10-15 16:30 UTC #269 3fffd93 READY (retargeted on main after #267). #275 b172a74 READY (New… potential, wire override menu; follow-ups on the PR).
2026-10-15 16:45 UTC #279 df2c826 READY (Show where it goes; follow-ups: highlight not selection, scope note).
2026-10-15 17:00 UTC #282 7eede6c READY (hover note on Show where it goes). #283 16f06d9 READY (no corner piece on pin drags; follow-up to #263).
17:30 UTC #288 fb31b87 READY (Corners mode squares slanted wire on corner drag; follow-up to #269)
17:45 UTC #291 d4c586d READY (Preferences Connections/Moving follow-ups to #259/#261)
17:55 UTC #293 d26979c READY (feed names L1,L2,L3 per page; editing.rs test conflict keep both)
18:10 UTC #295 285a31c READY; #297 8efb31a READY; #290 a88ec39 CHANGES (fmt gate1_tests.rs:1859; design OK)
18:25 UTC #299 d57c2bf READY; #301 679e11d READY; #303 f5cf791 READY
18:35 UTC #307 7f12126 CHANGES (Shift hint stale on keyboard moves)
18:45 UTC #307 0e32675 READY (chips only; Shift-keyboard-move commit split out, hint must be drag-only when it lands)
19:00 UTC #293 269da3b READY (re-check); #306 a22eca1 READY (Tab/Q while wiring; assigned by lead)
19:45 UTC #297 a11e13d READY (re-check); #318 43b406f CHANGES (docs only: C cable line missing from DESIGN)
19:55 UTC #318 e731257 READY (C cable line; DESIGN text added)
20:10 UTC #323 3632c97 READY (Outline Changes list); #325 220a9c5 READY (earthing on reconnected wire)
20:40 UTC #327 8d0ef34 READY; #329 50b0d17 READY; #331 899689d READY; #333 1d3d86b READY
21:00 UTC #335 0fc8ae7 READY (symbol packs DESIGN); #337 21f1da4 READY (inspector slide/pin/reset)
21:20 UTC #330 2b32e50 READY; #338 29c02e0 READY; #339 4124ef0 CHANGES (clippy needless borrow in novo-pdf/tests/open_end.rs:27)
21:35 UTC #341 42853ed READY (user keymap file)
21:45 UTC #339 ffbfd27 READY (clippy fix confirmed)
22:00 UTC #348 9c45d1e READY (rename group/panel in outline); #350 d622e2a READY (palette ranking)
22:15 UTC #363 372828c READY (page reorder in outline)
22:25 UTC #365 69bd421 READY (docs: wheel sentence + hands-on checklist)
22:40 UTC #377 82a866d READY (contact blade animation, timer ring, reduce motion)
18:15 UTC #383 2477ae5 CHANGES (design): DESIGN.md sentence for Project settings modal vs Rules document
18:16 UTC #383 93d4dcb READY (project drawing standard window)
18:25 UTC #393 e4a2ae4 READY (blade line width)
18:30 UTC #395 8972e56 CHANGES (design): Home=fit, right-click vs orbit, Alt+click, Esc, Ctrl+T
