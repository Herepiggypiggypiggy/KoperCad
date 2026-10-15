# Wire list with cut lengths: handoff

## State
- Branch `claude/project-thread-qpde09` (local name `wire-list-lengths`), stacked on the routing branch `claude/project-thread-euksez` (#381, `route_connections`). Draft PR; rebase onto main once #381 merges.
- Changes: `novo-reports/src/lengths.rs`, `wires.rs`, `cutlist.rs`, `report.rs` (`proposed_by_default`), `defaults.rs`, tests in `tests/reports.rs`, `tests/live.rs`, `novo-cli/tests/cli.rs`.

## Resume
```
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p novo-reports -p novo-cli -p novo-pdf -p novo-tests
```
Disk is small: `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`, `rm -rf target` when full.

## Decisions
- Cut list is a standard report but not a default page (`proposed_by_default` false).
- Estimate and end allowances are constants documented in docs/design/cut-list.md.
- Routing code is not touched; only `route_connections` is called.
