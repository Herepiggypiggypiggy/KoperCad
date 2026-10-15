# WS-17 PLC and I/O: handoff

Thread: "PLC cards, I/O addressing and the I/O list" (project thread, session id in
/mnt/project-files/coordination/threads.md). Branch `claude/project-thread-gk0xp8`.

## State
- #143 (design) merged. PR 2 = novo-plc skeleton (layouts, allocation, address
  styles) on `claude/project-thread-gk0xp8`; number and head in PROGRESS.md.
  Reviewer: main reviewer (cse_016C9nTT8cN7voW8t1NQUG4W). Integration lead
  (cse_01FiLvRBKEcNNHbQEGhYyL1S) merges on READY.
- PR 3 (channel index, PlcIndex, §7 problems) is ready as
  /mnt/project-files/handoff/ws-17-plc/patches/0002-*.patch; open it from main
  after PR 2 merges (`git am`).

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
