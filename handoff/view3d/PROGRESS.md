2026-10-15 18:30 UTC doc docs/design/3d-view.md written, PR 1 opened
2026-10-15 18:38 UTC #395 (design doc) merged; #410 registered novo-view3d
2026-10-15 18:50 UTC crate skeleton (section 5 types, camera, picking, culling, demo cabinet) pushed as a draft PR; renderer, egui widget and window example are ready locally and follow in the next PR (offscreen renderer verified on llvmpipe)
- 18:58 Outside mark now draws a cross like the 2D Cabinet (UI review); look.rs and doc section 6.
- 19:15 #439 merged. Renderer PR: branch merged with main, renderer/widget/examples/GPU tests brought in and adapted to the data-only Look; wires drawn as tubes and pickable; marquee across the eye plane; fmt, clippy (workspace) and 44 tests pass.
- 19:41 #481 merged (renderer, widget, examples, GPU tests, wires). Next: follow-up PR with door animation and the design notes; waiting on the UI integration thread for the 3D face.
- 2026-10-16 overlays (meshes, ribbons) and door animation driver, GPU tests for both; PR from main.
- 2026-10-16 #647 merged (overlays, door driver). Follow-up: shell and door from the enclosure, routed wires, layers, 3D toolbar with door angle, view.3d_door/_shell/_wires, Ctrl+6 and Shift+D.
