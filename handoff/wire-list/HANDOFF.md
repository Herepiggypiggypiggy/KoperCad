# Wire list with cut lengths: handoff

## State
- #417 merged (wire list Cut length mm / Length basis, `cutlist` report, docs/design/cut-list.md).
- Follow-up PR on `claude/project-thread-qpde09`: `kopercad-cli export cutlist`.
- Labels PR 1 (this PR, #482): engine, generic and Brother CSV layouts, `kopercad-cli export labels`. Next steps for a fresh session, in order: (2) PDF label sheet with user-set label sizes in novo-pdf, (3) app preview of labels in the Reports hub (#452).
- Labels cover wires, terminals, devices and cables (asked by the coordinator).

## How the user opens it
- Wire list with cut lengths: generated "Wire list" page (default page), `kopercad-cli export wires`, Excel via `export reports`.
- Cut list: `kopercad-cli export cutlist` and Excel via `export reports`. App: the Reports hub (#452) lists every standard report and gets an "Add as page" button for reports without a generated page, so no separate "add Cut list page" command is needed (agreed with the UI integration thread).

## Resume
```
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p novo-reports -p novo-cli
```
Disk is small: `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`, `rm -rf target` when full.

## Decisions
- Cut list is a standard report but not a default page (`Report::proposed_by_default` false).
- Routing code is not touched; only `route_connections` is called.
