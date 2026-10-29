# Visual redesign lane V2 (top bar, app menu, menu trimming): roadmap

Scope: docs/design/visual-redesign.md section 14, lanes V2a (top bar), V2b (app menu), V2c (menu trimming and the budget test).

Done (PR 1, branch `claude/visual-v2-menus`): V2b, the app menu. The mark opens one two-pane menu (areas on the left, the commands of the area on the right, search across all areas, About and Preferences in the footer), F10 and Alt+letter, from the same registry layout as the menu bar, icons and short labels from the presentation table. New setting "Menu names in the top bar" (Preferences, Appearance).
Next: V2a top bar (40 pt bar, icon workspace segment, breadcrumb pill with page switcher, search pill, issues pill, breakpoints), then V2c (label policy 28, relocation per section 5.2, the budget test) once the destinations exist (V3 rails, V4 centres).
Open: the design default for the menu words is hidden; this PR keeps them shown (`MENU_NAMES_DEFAULT` in `settings.rs`) because lessons and QA scenarios click the menu words. Lane V7 flips the constant together with those updates.
