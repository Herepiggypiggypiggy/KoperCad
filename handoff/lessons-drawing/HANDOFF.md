# Handoff: Lessons, Drawing track

Branch `claude/project-thread-th5k45`, draft PR (see PROGRESS.md for the number and head SHA). No build requested.

State
- Six draft lessons and their projects are in `assets/lessons/`. `cargo test -p novo-app --test lesson_projects` passes.
- Regenerate the JSON after changing a builder: `KOPERCAD_UPDATE_LESSONS=1 cargo test -p novo-app --test lesson_projects`.
- The test replays every lesson's steps through `Editor`, so a changed shortcut or tag rule shows up there.

Traps
- Facts verified in code: mpcb is -Q1, mcb -F1, buttons -S, coil -K, lamp -P1; a second `no K1` contact is 23/24; feeds auto-name L1 to L4; a feed row pitch is 8 mm.
- In the live view all poles start open, including normally closed buttons, so the live-view lesson tells the person to close -S1 first.
- `tool.place_wheel` is not a command and a held Q has no id; digit favourites have no command ids. The wheel step uses `{ signal = "wheel_opened" }` from #479.
- Anchor ids (`dock.*`, `wheel`, `live.card`) are guesses at the framework's registry.

Talk to: framework thread cse_018gxMVUe63W8rvrNBwroP8s, sample/first-run thread cse_019sSSXcKhY8k5uYwY3cgSyf.

## 2026-10-16: teaching-standard rewrite (First hour lessons)
- placing-parts, wiring-modes, potentials, motor-starter, cross-references and live-view now follow `docs/design/lesson-plan.md` section 3: what and why in `text`, concrete keys and clicks in `how`, `solve` on action steps, `_nl` twins on every string, and a closing step that describes the result instead of "like the finished one".
- `how` and `how_nl` are not in the lesson format yet (runner work, Learning modules thread). Until it lands, `cargo test -p novo-learn` fails on those keys, so the PR stays draft.
- Rotation: "press R on a placed part" is refused when the part has wires. The lesson turns the part in hand and places a spare at (130,72), then undoes it. Tests: `r_turns_the_part_in_hand...`, `r_on_a_wired_part_is_refused...`, `duplicating_a_lone_push_button...` in `crates/novo-app/tests/lesson_projects.rs`.
- Live view: every pole starts open. The lesson closes Q1 (Shift+click, three poles), then F1 and S1, then clicks S2 to start; a click toggles, it does not hold.
- UI names inside Dutch text stay English (menus and wheel are English); only the prose is Dutch.
