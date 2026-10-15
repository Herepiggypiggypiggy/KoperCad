# Handoff: Lessons, Drawing track

Branch `claude/project-thread-th5k45`, draft PR (see PROGRESS.md for the number and head SHA). No build requested.

State
- Six draft lessons and their projects are in `assets/lessons/`. `cargo test -p novo-app --test lesson_projects` passes.
- Regenerate the JSON after changing a builder: `KOPERCAD_UPDATE_LESSONS=1 cargo test -p novo-app --test lesson_projects`.
- The test replays every lesson's steps through `Editor`, so a changed shortcut or tag rule shows up there.

Traps
- Facts verified in code: mpcb is -Q1, mcb -F1, buttons -S, coil -K, lamp -P1; a second `no K1` contact is 23/24; feeds auto-name L1 to L4; a feed row pitch is 8 mm.
- In the live view all poles start open, including normally closed buttons, so the live-view lesson tells the person to close -S1 first.
- `tool.place_wheel` is not a command and a held Q has no id; digit favourites have no command ids. Those steps use unregistered check kinds (`wheel`), so lint will flag them until the framework adds them.
- Anchor ids (`dock.*`, `wheel`, `live.card`) are guesses at the framework's registry.

Talk to: framework thread cse_018gxMVUe63W8rvrNBwroP8s, sample/first-run thread cse_019sSSXcKhY8k5uYwY3cgSyf.
