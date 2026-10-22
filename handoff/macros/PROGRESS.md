2026-10-15 18:30 UTC Design note docs/design/macros.md written; draft PR opened.
2026-10-15 18:40 UTC Adopted the Macro library thread's file format and split (design note updated).
2026-10-15 18:50 UTC Review fixes: data folder KoperCAD (DESIGN s7 updated), revision field, BlockSource mapping, terminals step 6.
2026-10-15 19:10 UTC #396 merged. macro_place engine (place_macro, preview) with tests; entry points agreed with UI thread.
2026-10-15 19:50 UTC #473 merged. App PR: macro.save + macro.place windows, capture/free_origin helpers, tests.
2026-10-15 19:55 UTC #489: merged main, Esc closes Place window, Replace on existing name, free_origin uses pin reach.
2026-10-15 20:05 UTC #489: Replace uses the library's own path (slash-name bug), Esc clears search first, Replace label names the macro.
2026-10-15 20:35 UTC #489 merged. Parts carry-over: PartRef, captured_parts, assign on placement, Place window checkbox.
2026-10-21 06:10 UTC Starter macros grown from 4 to 15 (reversing, latch, PLC inputs, VFD, soft starter, 1-phase 24 V, brake, indication, crane hoist, pump float, heater). Source crates/novo-parts/src/macros/starter/more.rs. Users find them in Edit > Macros browser (macro.browser).
2026-10-21 13:30 UTC Starter macro wires are now routed right-angled (capture adds a down-across-down route); geometry test over all 15; QA scenario qa/scenarios/24-starter-macros.qa places each from Ctrl+K at both sizes; palette pick of a whole macro name now preselects it in the Place window.
2026-10-22 02:00 UTC RM-03, RM-11, RM-17 (real machine build): starters come out fully wired. PE goes through a PE terminal to every motor and drive, coil returns share one N rail, the star-delta power circuit has its own channels (no overlapping wires), second contacts of a device are numbered 23-24, and a new "Three-phase supply" macro was added (16 starters). Test crates/novo-tests/tests/starter_macros_clean.rs; QA scenario 24 places all 16.
