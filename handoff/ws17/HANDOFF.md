# WS-17 PLC and I/O: handoff

Thread: "PLC cards, I/O addressing and the I/O list" (project thread, session id in
/mnt/project-files/coordination/threads.md). Branch `claude/project-thread-gk0xp8`.

## State
- PR 1 (design only): `docs/design/plc.md` plus this handoff. See PROGRESS.md
  for the PR number and head SHA. Reviewers: design and workflow reviewer
  (cse_0114emQ5fSFrzcALbR4S18TU) and main reviewer (cse_016C9nTT8cN7voW8t1NQUG4W).
  Integration lead (cse_01FiLvRBKEcNNHbQEGhYyL1S) merges on READY.
- `crates/novo-plc` does not exist yet; it is registered in
  `[workspace.dependencies]`. PR 2 creates it (never edit the crate list).

## Decisions made
- A card is a `Device { kind: Plc }`; a channel is a non-main symbol of that
  device with `plc.channel`; the overview is the main symbol. No PLC navigator.
- Card layout string `DI16 DQ8 AI4` from the part's `plc.channels`, device
  property overrides.
- Card start addresses are allocated on add and stored (`plc.start.i`,
  `plc.start.q`); only `renumber io` moves them. Channel addresses are derived.
- Address style rule: project property `plc.address_style` (iec default,
  siemens, rockwell, plain) until WS-02 types it.
- All PLC data is in properties: no novo-core API change.
- I/O list report lives in novo-reports (Outputs thread); novo-plc supplies rows.

## Resume
```
cd NovoElectric && git fetch origin && git checkout claude/project-thread-gk0xp8
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
CI (Actions) is billing-blocked: run the checks locally and report them on the PR.
If PR 1 has merged, restart the branch from main for PR 2.

## Traps
- After READY, never push to the PR; follow-ups go in a new PR from main.
- After every push, message the reviewers with the new head SHA.
