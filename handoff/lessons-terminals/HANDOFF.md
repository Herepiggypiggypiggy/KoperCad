# Handoff: terminals track lessons

Branch `claude/project-thread-n6e68e`, one draft PR. Builders: `crates/novo-tests/tests/lessons/{terminals,cables,plc,standard,documents}.rs`, helpers in `common.rs` (project file freshness and a stand-in lint).
Regenerate JSON after changing a builder: `UPDATE_LESSONS=1 cargo test -p novo-tests --test lessons`; check with `cargo test -p novo-tests --test lessons`.
Nothing was run in the real app: steps are verified by reading code and engine tests. Gaps and owners: docs/lessons/GAPS-terminals-track.md. Per-step file:line: each lesson's NOTES.md.
Decision: `finished.json` sits next to `project.json` (not in the format yet). Framework thread: cse_018gxMVUe63W8rvrNBwroP8s.
