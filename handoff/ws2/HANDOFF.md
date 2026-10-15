# WS-02 Data model: handoff

## State (2026-10-14 22:25 UTC)
- #35, #45, #82, #101, #107 merged. The repo copy of this handoff (handoff/ws2/)
  is refreshed with each PR; the shared-folder copy may be newer.
- #119 page freeze merged 2026-10-15 03:03 UTC; WS-10 told.
- #125 blocks merged 2026-10-15 03:11 UTC.
- Open: follow-ups PR (branch `claude/data-model-amzaxu`): wires_leaving,
  shared PE, detach on added device, #119 and #125 review notes.
  Interruption points are refused in blocks; per-placement scoping is a
  possible later change if users want them (DESIGN §7 call). Review
  notes 3-4 (blank revision, doc) done on ws02-followups. Notes 1-2 (a
  frozen page edited since its revision; renaming a frozen page) are for
  WS-06 to show; a `PageFreeze::is_stale` query is a possible follow-up.
- WS-11 moved terminal declines to Dismissal in #122.
- Queued locally, rebased on main 70248a3: `ws02-freeze` 3890938 (page
  freeze), then `ws02-blocks` 498e9a6 (blocks). One PR each, in that order,
  each after the previous merges. After Structure merges tell WS-11; after
  freeze merges tell WS-10.
- #117 merged 2026-10-15 02:50 UTC. WS-11 told about the second dismissal
  mechanism (terminal.suggest wire property).
- Queued third: `ws02-followups` (on blocks): `Project::wires_leaving`,
  isolated systems keep the shared PE, prefix validation.
- Workflow (WS-08) note for blocks: structural edits detach both ways.
  Adding a connection between two members of one instance, or a device
  wired only to members, should detach ("connection -K1:13 to -F2:1
  added"). A wire from a member out to the circuit keeps the link. Built
  for connections (on ws02-blocks) and for a device wired (2+ wires) only
  to members, checked at the end of each edit (on ws02-followups).
- Blocks follow-ups: definition update keeping overrides, marking overrides
  automatically from edits, instance.page after a move.
- #107 review notes (non-blocking): a `wires_leaving(items)` query so the
  editor can show every wire a move would cut; no page-extent check on the
  offset (WS-14); a text attached to a Device stays on the old page when the
  move takes the device's last symbol off it (ask WS-06/WS-14).
- CI (Actions) is down: reviewers' local checks are the signal.
- #101 review notes (non-blocking, open): an isolated system's prefix also
  renames PE, though a control-transformer secondary usually shares the
  cabinet PE (option: keep PE unprefixed); the prefix is not validated;
  name-based helpers (novo_cables::kind::is_protective, terminal-name rule)
  only know bare `PE`/`N`/`L+`; tell WS-11 and WS-12.
- Blocks: the workflow thread answered (21:51 UTC). BlockDefinition record in
  the project (self-contained .novo; libraries are copy sources:
  %APPDATA%\NovoElectric\Blocks, optional company folder; project copy keeps
  source + version). BlockInstance remembers placements. Gate 1 needs only
  place_block + parameter edits. Tags are allocated (letter code kept, next
  free number), property values and "{param}" texts substituted, `x3` = three
  instances, parts not a parameter (WS-07 proposes after). Plan the DESIGN §7
  survival rules now: per member a set of overridden fields (position, text
  placement, tag, routes) relative to the instance origin, and
  `detached` + reason on a structural edit.
- Terminal-article decision: Terminal.part per terminal; strip rule stays
  WS-11's `terminal_part.<kind>`; WS-11 retires its per-terminal `part`
  property in style.rs itself (do not touch it).
- New integration lead: cse_01FiLvRBKEcNNHbQEGhYyL1S.
- Blocks: asked the workflow thread (DESIGN owner) for Gate 1 scope before
  building (proposal: BlockDefinition template + BlockInstance members).

## Resume
```
cd /home/claude/NovoElectric && git fetch origin && git status
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings   # 600 s timeout
cargo test --workspace
NOVO_UPDATE_FIXTURES=1 cargo test -p novo-core --test format   # after format changes
```
Check PR #35 state first (open? READY?). After READY: never push to #35 again.
After #35 merges (squash): `git fetch origin main && git checkout -B
claude/data-model-amzaxu origin/main`, then new PR for new work.

## Rules (coordination/threads.md)
- After every push, send_message the main reviewer with the new head SHA.
- Check the PR is still open before pushing. Keep novo-core PRs small.
- Engines and commands only, no UI, until docs/DESIGN.md merges.
- Never ask Tadjiro to review/merge; the integration lead merges.
- Work on events; no polling loops. No admin rights anywhere (CLAUDE.md).
- Update PROGRESS.md and this file before ending any turn that changed something.

## Who
- Integration lead: session_011PfTJ1QaP1JQJPwXcFnxBE
- Main reviewer: cse_016C9nTT8cN7voW8t1NQUG4W
- WS-06 editor cse_019siuxbyyRNZRgBBww1KocJ, WS-07 parts cse_01H3m3ufThxMEzTtG41NrXaB,
  WS-11 terminals cse_01AqzMrSLZaG8zin7Yf84RZP, WS-05 symbols cse_01F6Twb8LZtCnYYzuP8saMyb.

## Model conventions and traps
- Every edit: `Tx` -> `Project::apply_op` -> `check()`. Add integrity rules in `check()`.
- New fields: `#[serde(default)]` + skip when default; `JSON_FORMAT_VERSION` stays 1;
  regenerate fixtures. Adding an `EntityKind` needs novo-storage `kind_name`.
- Rules (`ProjectInfo.rules`): explicit, off when absent; `Project::new` uses
  `ProjectRules::recommended()`; `Project::default()` is all off (tests).
  `Document::default()` uses `Project::new`, so tests needing Off use
  `Document::new(Project::default())`.
- Netlist potentials: `defined` (feeds, Wire.potential, terminal names under rule),
  `carried` (BFS through pass-through poles, nearest feed only, never into
  defining nets), `reachable`; conflicts only from `defined`. Changeovers need
  the two-group pass-through form.
- Open wire ends: `Endpoint::Free { at }` (JSON `{"type":"free","at":{x,y}}`), a
  position on the wire's page that joins no net. `Endpoint` is no longer
  `Copy`-derived `Eq/Hash/Ord` by derive (manual, on `total_cmp`/bits). Use
  `Netlist::net_of_wire(&wire)` instead of `net_of(wire.from)`; a wire open at both
  ends has no net. `Command::Connect.page` / `Tx::connect_on` are needed only
  when both ends are open. Connecting an end = `update::<Wire>` replacing it.
- PE terminals: `Terminal.pe_override` (`Auto`, `ForcedPe`, `ForcedNotPe`; file field only
  when not Auto) and `Command::SetTerminalPe { terminals, mode }` (one undo step). The
  model stores the choice only; novo-terminals applies the PE rule to `Auto` terminals and
  changes `kind` and the symbol with a forced toggle.
- Cabinet layout: `Structure.plate: Option<MountingPlate { width, height, margin }>` (enclosures
  only; file field only when set), entities `Rail` (`structure`, `x`, `y`, `length`, `kind`
  DinRail | Duct) and `CabinetPlacement` (`device`, `structure`, `x`, `y`, `rotated`, `rail`,
  `rail_offset`; at most one per device). Commands `SetPlateSize`, `AddRail`, `PlaceDevice`
  (places or moves; creates nothing when it moves), `RotatePlacement`, `RemovePlacement`;
  `place all` = one `Command::Batch` of `PlaceDevice`. Deleting a device or enclosure deletes
  its placements; deleting a rail leaves the devices on the plate, off the rail. A plate with
  rails or devices on it cannot be removed. Clearances live in `novo-layout`, not the model.
  Ducts are `Rail`s of kind `Duct`: `vertical` (runs down instead of right), `width` and
  `height` (cross-section mm, 0 while unknown; file fields only when set). `MoveRail` moves
  the devices on the rail with it; also `SetRailSize { rail, length, width, height }` and
  `DeleteRail`.
- Drawing standard: `ProjectRules.standard: String`, the symbol pack id (`iec`, `nfpa`; lowercase
  letters, digits, `_`, `-`); empty means IEC; file field only when set. Set through
  `SetProjectInfo` (undoable); `ProjectRules::is_standard_id` is the check.
  `SwitchStandard { standard, symbols: [(SymbolId, SymbolKey)] }` sets it and re-keys the listed
  symbols in one undo step; the caller computes the keys (`Pack::counterpart`), the model has no
  symbol packs and pins stay as they are.
- Macros: `Command::AdoptBlockDefinition { definition }` (`Tx::adopt_block_definition`) copies a
  definition from a macro file in under a fresh id; same checks as a captured one, now also no
  parts, feeds or potentials in members. Members keep their file-local ids.
- PRs get squash-merged: rebase follow-ups with `git rebase --onto origin/main <old-head>`.
- Commit trailers: Co-Authored-By Claude line + Claude-Session link (see git log).
