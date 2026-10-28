# Net shorts handoff

Branch `claude/project-thread-6erqam` (repo kopercad). PR and head SHA: see PROGRESS.md.

## State
Done and tested. Companion PR `feed-naming` (editor side of the same report) is separate and independent.

## Where things are
- `crates/novo-checks/src/net_shorts.rs`: `named_conductors` (pin named `PE` or `N` against `defined`, `carried`, `reachable` of the net) and `bridged_poles` (pairs of pin-disjoint pole groups of one symbol in `/protection/`, `/contactors/`, `/switches/`, not `changeover`). `permanent_conductors` is the potential a net is on through fuses, breakers and switches only.
- `crates/novo-checks/src/physical.rs`: `free_air_problem` groups the free-air wires per page.
- `crates/novo-checks/src/lib.rs`: `plain_key` (readable name of a tag-less symbol), `net_shorts::run` call, `AmbiguousInterruption` is now info.
- Tests: `crates/novo-checks/tests/net_shorts.rs`, `tests/issue_text.rs`, `tests/shipped_projects.rs` (PotentialConflict is watched), `crates/novo-tests/tests/short_checks.rs`.

## Resume
```
export CARGO_TARGET_DIR=...   # one shared dir; scripts/free-disk.sh when a write fails
cargo test -p novo-checks
cargo test -p novo-tests --test short_checks --test starter_macros_clean
cargo test -p novo-tests --test short_checks write_fixture -- --ignored   # rewrites the QA fixture
scripts/qa.sh --only cooling-tower
```

## Decisions and traps
- A conductor that reaches a side through a contactor is not counted: the star contactor of a star-delta starter closes while the delta contactor is open, and the shipped star-delta lessons would otherwise report a short. See the test `a_star_contactor_fed_through_another_contactor_is_not_a_short`.
- The new findings reuse `ProblemKind::PotentialConflict` (error), so no new kind, label, waiver key or Dutch string.
- `starter_macros_clean.rs` maps `terminal PE (no tag)` back to the library name its lists use.
