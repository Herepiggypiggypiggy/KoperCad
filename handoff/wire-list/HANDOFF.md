# Wire list with cut lengths: handoff

## State
- #417 merged (wire list Cut length mm / Length basis, `cutlist` report, docs/design/cut-list.md).
- Follow-up PR on `claude/project-thread-qpde09`: `kopercad-cli export cutlist`.
- Next (asked by the coordinator): labels export (wire, terminal, device, cable labels; CSV layouts, PDF sheet with user-set sizes, app preview, CLI).

## How the user opens it
- Wire list with cut lengths: generated "Wire list" page (default page), `kopercad-cli export wires`, Excel via `export reports`.
- Cut list: `kopercad-cli export cutlist` and Excel via `export reports`. App: not a default page; an "add Cut list page" command is to be agreed with "UI integration: reports and checks" (session_01TaTTPuTqhk8bHd277oJRFR), message sent, no answer yet.

## Resume
```
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p novo-reports -p novo-cli
```
Disk is small: `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`, `rm -rf target` when full.

## Decisions
- Cut list is a standard report but not a default page (`Report::proposed_by_default` false).
- Routing code is not touched; only `route_connections` is called.
