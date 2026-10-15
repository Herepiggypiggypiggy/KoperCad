# 3D look and feel: roadmap

Scope: look, materials, lighting, edges, anti-aliasing, themes, camera feel, view cube, saved views, hover and selection, labels. Not the renderer architecture (thread "3D view: renderer and architecture").

1. docs/design/3d-look.md with sketch (this PR).
2. Palette and light rig data in novo-theme (`view3d`), tests.
3. Camera maths, transitions, saved views, view cube hit testing (in the view crate once it exists).
4. Renderer passes once docs/design/3d-view.md and the spike land.
5. Quality tiers, frame governor, integrated GPU measurement.

Open: exact axes and crate name come from the architecture contract.
