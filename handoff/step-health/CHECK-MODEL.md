# Check model button and the real Rittal results

- UI: Parts window > select a part with a STEP model > 3D model > Check model (`crates/novo-app/src/app/parts_health.rs`). Runs `part3d::health::health_of_file` in the background, shows the summary and notes, "Copy report" for a problem report.
- Real files (Tadjiro's three Rittal downloads, private in /mnt/project-files/private/rittal-step/, never in the repo), run 2026-11-05 with `kopercad-cli parts step-health`:
  - Mounting plate CAD_MPL_1552.500: reads cleanly, 175.0 x 285.0 x 2.0 mm.
  - TS 8 rail 5001.081: reads cleanly, 3 solids, 744.8 x 84.0 x 23.0 mm.
  - TS 8 enclosure 8455.600 (21 solids, 1038 faces): reads with gaps. 12 faces drawn flat (2 are DEGENERATE_TOROIDAL_SURFACE, a spindle torus), 1 face skipped. Size 1150.5 x 1859.5 x 883 mm is the whole assembly, not the cabinet.
  - Colours are one grey per solid (#b0b0b0, #808080, #585858, #bfbfbf); sub-part names are internal codes, so door and plate cannot be told apart by name. Next: pick the body from the assembly by geometry, support the spindle torus.
