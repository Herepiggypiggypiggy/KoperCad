# Feed naming handoff

Branch `claude/project-thread-6erqam-editor` (repo kopercad). PR and head SHA: see PROGRESS.md. Independent of the net-shorts PR.

## Where things are
- `crates/novo-editor/src/feed_name.rs`: `retarget` (called from `edit::place`, `settle`, `accept`, `draw_wire`), `mark`, `infer`, `hint`, `coil_return`.
- `crates/novo-editor/src/edit.rs`: `next_feed_name` (now unique over every potential of the project), `connect_touching` (a feed joins an occupied pin).
- Tests: `crates/novo-editor/tests/feed_names.rs`; `crates/novo-app/tests/lesson_projects.rs` (a fourth feed is PE at once).
- Lesson `assets/lessons/potentials/lesson.toml`: step `earth-feed` now teaches that the feed reads the `PE` pin; step `make-earth` is gone.

## Resume
```
export CARGO_TARGET_DIR=...
cargo test -p novo-editor
cargo test -p novo-tests -p novo-app    # slices; scripts/free-disk.sh first
```

## Decisions and traps
- `retarget` sets the feed's role to Unassigned for a moment to read the net without its own provisional definition, then writes the final role in the same transaction (one undo step).
- A potential nothing uses any more is removed when a provisional feed leaves it.
- Names are unique over the project, so the first feed of a second page is not `L1` again; use Set potential to continue a conductor.
- Interruption points are not paired on placement: the lesson `interruption_points` teaches Link cross reference and expects IP2 for a second pair. The checks PR (net-shorts) tells the user how to pair lone points instead.
