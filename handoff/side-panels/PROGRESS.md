# Side panels: progress

- 2026-10-16: Inspector, Outline, Issues, Commissioning panels done; screenshots in `/mnt/project-files/design/side-panels/`.

## Dialog sweep (branch claude/panels-dialogs)
- New `dialog_ui` helpers (title, section, hint, setting, choice, done). Project rules, Project settings, Pages, Parts bulk, What's new and Shortcuts sheet use them (MEDIUM width, primary Done, segmented choices).
- Not done yet: Cabinet rules, Background panel, safety functions (after #777), report languages (after #772).
- 2026-10-16 (later): Cabinet inspector on folding sections (Placement, Panel data, Problems, Inside and door); Rotate/Remove moved under the description. Outline, Issues and Changes checked at 150% scaling (WINIT_X11_SCALE_FACTOR=1.5 under Xvfb): fine. Changes per-page list left as is (not sectioned content); Heat has no panel yet, only the strip and legend.

## Cabinet inspector nits (branch claude/panels-cabinet-nits)
- Placement opens when a "why" is present, Panel data summary reads "not set" when empty, the Place rule value wraps inside the panel.

## Parts polish (branch claude/parts-polish)
- #798 was superseded by #841. Left from it: only one primary button in view (New part is plain, "Use for selected device" is the primary) and no 11 px buttons in the Parts guide strips.
