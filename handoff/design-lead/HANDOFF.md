# Design lead: handoff

Updated 2026-10-16. Branch `claude/project-thread-hsaavg`, based on main.

## State
- Audit and direction written (project mount `design/visual-audit-2026-10-16.md`, `design/direction.md`; copies in `docs/design/`). Screenshots: `docs/design/audit-2026-10-16/`.
- PR 1 (this branch): Pages flyout button moved into the top bar (was floating over the Cabinet toolbar), outline tab row uses icons so four tabs and the pin fit 248 pt, lesson texts say "top bar".
- The old design-system stack (project mount `design/design-system-stack/`) does not apply to current main (patches fail); redo from the direction doc, using the patches as reference only.

## Capture tooling
`scripts/screenshot.sh` takes `SCREENSHOT_RES=1366x768`. Needs apt: xvfb openbox xdotool imagemagick mesa-vulkan-drivers libxkbcommon-x11-0 (run `apt-get update` first). The window is 1280x800 by default: maximise it with xdotool windowsize for the target size. The driver script used for the audit: click steps by pixel; the start screen's "Open example" is at y=388 (768 high) and y=412 (1080 high), x centred.

## Traps
- Lesson tests find controls by exact label text (`click_label`, `click_control`); changing a label breaks them. Run `cargo test -p novo-app --lib lesson_verify`.
- A button outside the canvas that keeps keyboard focus routes keys to the panel; call `surrender_focus()` after a click on top-bar buttons.
- Only Windows can show font hinting and DPI scaling; say what was not verified in every PR.

## People
Roster `coordination/threads.md`. Panels thread cse_01FpsvpQaDSs3vTLkLsQYcjY (#712), theme reviewer R3 cse_01SL6nnpsSpN8Tjt3tfo8K5V, design reviewer B cse_01WP56F6BoXwpQhqa2zpkYab, audit threads cse_013jWHWnxLGGAAmqeDcLsp8a, cse_01Nm56Gmivvoc4iqTTQNR83t, cse_016Hh83aKyb6dyUC3SE895Ya, cse_01STi1vTsLJiaJmudCFUqL6F.
