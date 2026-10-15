# WS-17 PLC and I/O: handoff

Thread: "PLC cards, I/O addressing and the I/O list" (project thread, session id in
/mnt/project-files/coordination/threads.md). Branch `claude/project-thread-gk0xp8`.

## State
- #143 (design) and #152 (skeleton) merged. PR 3 = channel index on
  `claude/project-thread-gk0xp8`; number and head in PROGRESS.md.
  Reviewer: main reviewer (cse_016C9nTT8cN7voW8t1NQUG4W). Integration lead
  (cse_01FiLvRBKEcNNHbQEGhYyL1S) merges on READY.
- Queued locally as patches in /mnt/project-files/handoff/ws-17-plc/patches/,
  each its own PR from main after the previous one merges (`git am`):
  0003 io_rows (IoRow, spec agreed with Outputs, ping them when it lands),
  0004 commands (add_card, place_channel, to_plc, renumber_plan/apply_renumber).
- After 0004: tell WS-08/WS-06 the command API (`di K4`, `to plc`), WS-10 the
  address label (`PlcIndex::address_label`) and channel image, WS-01 the
  Devices drawer rows (`io_rows`).

## Decisions made
- A card is a `Device { kind: Plc }` coded K (letter_code); a channel is a
  non-main symbol of that device with `plc.channel = "DI 3"` (type stored, RO
  numbered on its own); the overview is the main symbol. No PLC navigator.
- Card layout string `DI16 DQ8 AI4` from the part's `plc.channels`, device
  property overrides.
- Card start addresses are allocated on add and stored (`plc.start.i`,
  `plc.start.q`); only `renumber io` moves them. Channel addresses are derived.
- Address style rule: project property `plc.address_style` (iec default,
  siemens, rockwell, plain) until WS-02 types it.
- All PLC data is in properties: no novo-core API change. Commands are single
  transacts of existing core commands, never new Command variants.
- I/O list report lives in novo-reports (Outputs thread); novo-plc supplies rows.

## Resume
```
cd NovoElectric && git fetch origin && git checkout claude/project-thread-gk0xp8
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
CI (Actions) is billing-blocked: run the checks locally and report them on the PR.
If PR 1 has merged, restart the branch from main for PR 2: `git checkout -B claude/project-thread-gk0xp8 origin/main && git am /mnt/project-files/handoff/ws-17-plc/patches/0001-*.patch` (skeleton), then PR 3 with 0002 (channel index) after PR 2 merges.

## Traps
- After READY, never push to the PR; follow-ups go in a new PR from main.
- After every push, message the reviewers with the new head SHA.
