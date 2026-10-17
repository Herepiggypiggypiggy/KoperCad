# 3D navigation and selection: roadmap

Scope: make the 3D view easy to move and select in (hide/isolate, select by type, hints legend, section plane, saved camera views, framing, trackpad pan, cross-probing between schematic, Cabinet and 3D, hover tooltip).

## Done (merged)
- #748, #756, #794, #765 (section plane clips the depth pass too), #796, #807 (Show in 3D / Cabinet / schematic), #814 (hover tooltip), #880 (legend row), #882 (section plane in mm), #885 (Trackpad pans), #895 (stale heat note in the tooltip), #897 (unplaced device message), #898 (trackpad choice saved), #886 (legend test, in merge).

## Next
The list is empty. Candidates if wanted: clip the shadow pass with the section plane (vs_shadow is still unclipped), light-theme palette check of the 3D toolbar via `crate::theme::ui_palette()`, a desktop check of the trackpad switch on Windows.

## Open questions
None.
