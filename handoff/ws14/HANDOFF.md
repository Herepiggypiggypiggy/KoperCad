# WS-14 Design checks: handoff

Updated 2026-10-14 by the WS-14 session started by the cloud lead
(session_01DZ68tTULUv7Uhe8ecKbC47).

## State
- Branch `claude/ws14-design-checks` (local name `ws14-checks`), from main
  f6ee2f1. Code commit f0c92b5; this handoff is the commit after it.
- No PR yet: the lead opens or asks for it. Reviewer: main reviewer.
- fmt, clippy -D warnings and `cargo test --workspace` pass on Linux.
- Nothing half-done.

## Resume
```
git fetch origin claude/ws14-design-checks main
git checkout -B ws14-checks origin/claude/ws14-design-checks
cargo test -p novo-checks
cargo run --release -p novo-checks --example checks_scale
```

## Code map (crates/novo-checks)
- lib.rs: Severity, ProblemKind (+ALL, severity(), label()), Problem,
  Summary (pill_text), check / check_with, Context (page rank, symbols per
  device main-first, symbols per terminal), reading-order sort.
- checks.rs: one function per check.
- contacts.rs: ContactKind from the symbol key, contact_limit_property.
- tests/checks.rs: one test per check; tests/motor_starter.rs: broken and
  clean Gate 1 starter, undo clears problems, stress page in debug.
- examples/checks_scale.rs: release timing on a 10k-symbol page.

## Timing (release, this container, median of 10, 10k symbols, 10k wires)
- netlist build ~7 ms; check_with (netlist kept) ~7.5 ms with parts
  assigned, ~13.6 ms when all 10k devices lack a part (10k infos);
  check (builds the netlist) ~15 / ~23 ms.
- Run it once per project revision, reusing the app's netlist, not per frame.

## Decisions
- Touching pins connect through a zero-length wire (WS-06 editor), so
  zero-length pin-to-pin wires between different symbols are fine; a pin on
  another point with no wire is reported as "touches ... but no wire".
- One UnconnectedPin problem per symbol, naming all its free points.
- "Cable core used twice" is not a check: core refuses it
  (ModelError::WireAlreadyOnCore) on every edit and load.
- Missing parts are info and not on the pill; terminals are not checked
  for parts.
- `at` lists drawn objects (symbols, wires); an undrawn device, terminal or
  cable is named itself, and the problem has no page.
- Pill wording: "No issues", "1 error", "5 warnings", "2 errors · 5
  warnings". novo-theme's `issue_pill(errors, warnings)` takes the counts.

## Traps
- `Document::default()` uses `Project::new` (recommended rules: terminal
  Join, potentials from names); `Project::default()` has rules off.
- Use a target dir of your own per worktree (stale binaries otherwise).

## Who to talk to
Roster: /mnt/project-files/coordination/threads.md (or
handoff/MAIN-HANDOFF.md). WS-02 for core requests, WS-10 for contact
classification, WS-01/WS-03 for the UI that consumes this.
