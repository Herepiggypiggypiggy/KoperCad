# Learning modules: roadmap

Scope: lessons that run inside the program (Learn section, steps, pointers, checks, sample project per lesson, progress). Design: `docs/design/learning-modules.md`. Requested by Tadjiro via the coordinator (2026-10-15): "sample projects that showcase different functionality, with pointers and tooltips, like a learning module". No build, small draft PRs.

## Done
- Design doc (#456), tracks (#474), engine (#479), app shell (#532), early access listing (#581). Show me how (#601), replay gate (#642). More replay ops, menu and window anchors, control-name lint (#682). In review: ui_click, list_row, type_text.

## Next, in order
1. Design merged so four content threads can write lessons against it.
2. Coordinator registers `novo-learn` (small PR).
3. `novo-learn` engine: format, parser, lint, checks, runner, progress store. Tests only.
4. App shell: event funnel, anchors, card, pointer overlay, Learn window, start-panel and Help entries, `help.learn`; one lesson end to end.
5. `build.rs` embedding, user lessons folder, `--lessons`, replay tests.
6. Anchor rollout in other threads' controls; generated anchor catalogue.
7. Convert the #428 walk-through to the `fast-keys` lesson (with the first-run thread).

## After that
Convert click-only `manual` steps to `ui_click`, the remaining anchors, `problem_free`, never-cover-target card placement. Details in HANDOFF.md.

## Open questions
See the end of the design doc (TOML vs RON, "Do it for me", Dutch).
