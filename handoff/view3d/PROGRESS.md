2026-10-15 18:30 UTC doc docs/design/3d-view.md written, PR 1 opened
2026-10-15 18:38 UTC #395 (design doc) merged; #410 registered novo-view3d
2026-10-15 18:50 UTC crate skeleton (section 5 types, camera, picking, culling, demo cabinet) pushed as a draft PR; renderer, egui widget and window example are ready locally and follow in the next PR (offscreen renderer verified on llvmpipe)
- 18:58 Outside mark now draws a cross like the 2D Cabinet (UI review); look.rs and doc section 6.
- 19:15 #439 merged. Renderer PR: branch merged with main, renderer/widget/examples/GPU tests brought in and adapted to the data-only Look; wires drawn as tubes and pickable; marquee across the eye plane; fmt, clippy (workspace) and 44 tests pass.
- 19:41 #481 merged (renderer, widget, examples, GPU tests, wires). Next: follow-up PR with door animation and the design notes; waiting on the UI integration thread for the 3D face.
- 2026-10-16 overlays (meshes, ribbons) and door animation driver, GPU tests for both; PR from main.
- 2026-10-16 #647 merged (overlays, door driver). Follow-up: shell and door from the enclosure, routed wires, layers, 3D toolbar with door angle, view.3d_door/_shell/_wires, Ctrl+6 and Shift+D.
- 2026-10-16 PR: 3D frame statistics panel (View > 3D), Spin and Copy, `orbit --wires`, first-frame time, numbers with wires in the design doc, what to measure on the Windows laptop.
- 2026-10-16 #673 merged. Follow-up PR: wire colours from the schematic (own colour, else cable core name; bi-colour stripe in bands), GPU tests that wires show through the closed door.
- 2026-10-16 #690 and #692 merged. PR: dimensions in the 3D view (View > 3D > Dimensions: enclosure size, one part's size and distances to neighbours and plate edge), frame governor connected to the widget (steps adapter tier down after 2 s over 20 ms), shown in the statistics panel.
- 2026-10-16 #755 review (R7): the governor reads the whole-app frame interval, so Tier::step_down now floors at Integrated; a hardware adapter never steps itself down to Software.
- 2026-10-16 PR: performance pass. Found the label pass costing 51 ms per frame at 1000 devices (View3d::project walked the scene bounds per label); now 0.45 ms. Examples: `uicost` (widget CPU headless), `orbit --tier --shadows --no-edges --heat N`. Numbers in the design doc.
- 2026-10-16 PR: label pass 1.50 to 0.51 ms at 3000 devices: chips are only laid out for devices that could show one (the overlap check was not the cost; a grid for it measured the same and was dropped).
- 2026-10-16 PR: a frame walks the scene bounds once (show passes them to paint_overlay and paint_dimensions; test now requires one walk).
- 2026-10-16 PR: wires without a colour take their potential colour (PE green-yellow, neutral and 0 V blue, positive DC red); line conductors stay grey (circuit class not in the model).
