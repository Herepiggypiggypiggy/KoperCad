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

## Dialog sweep 2 (branch claude/panels-dialogs-2)
- Place rules and DXF background use dialog_ui::hint and primary/ghost buttons. Pictures of Project rules and Project settings at 1366x768 in design/side-panels (after-1366-project-rules.png, after-1366-project-settings.png). Place rules and DXF background have no picture yet (need a cabinet / imported DXF).
