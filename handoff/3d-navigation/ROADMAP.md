# 3D navigation and selection: roadmap

Scope: make the 3D view easy to move and select in (hide/isolate, select by type, hints legend, section plane, saved camera views, framing, trackpad pan, cross-probing between schematic, Cabinet and 3D, hover tooltip).

## Done (merged)
- #748, #756, #794, #765 (section plane clips the depth pass too), #796, #807 (Show in 3D / Cabinet / schematic), #814 (hover tooltip), #880 (legend row), #882 (section plane in mm).

## Next, one PR each from fresh main
1. Done in #882. vs_shadow is still unclipped.
2. (In review, #885) Trackpad pans toggle in Layers (session choice, default off). Follow-up: save it in settings.
3. #807 nits: message when a device is unplaced for "Show in"; test asserts fit and rename.
4. #814 nit: "(out of date)" in the tooltip when the heat result is stale (`state.live.out_of_date`).
5. Section smoke-test comment wording; light-theme palette check via `crate::theme::ui_palette()`; legend test tying each chord to its command.

## Open questions
None.
