# Lessons: Drawing track (draft content)

Scope: lesson content for the Drawing track (design PR #456, `docs/design/learning-modules.md`): `assets/lessons/<id>/lesson.toml` plus `project.json` and `finished.json`, all `status = "draft"`.

Done
- placing-parts, wiring-modes, potentials, motor-starter, cross-references, live-view (live-view reuses `../cross-references/finished.json`).
- Sample projects are built with the real editor calls by `crates/novo-app/tests/lesson_projects.rs`, which also replays each lesson through `Editor` and checks the live view.

Next
1. Framework (novo-learn) lands: lint the six files, fix unknown keys and check kinds, flip to `ready` once anchors exist.
2. Add `glossary.drawing.toml` terms (potential, feed, pin, self-hold, contact image) in the framework's glossary format.
3. More Drawing lessons: proposals-and-tags, duplicate-and-numbering, pages-and-functions, blocks-and-macros.

Open questions (asked of the framework thread)
- `finished` key, a `parent` field and `tee` / `feed_row` / `via` ops in `solve`, check kinds `wheel`, `potential_kind`, `live_pole`, a `symbol`-only `device` check.
