# WS-02 Data model: roadmap

## Scope
`crates/novo-core` only: the single source of truth. Every other crate reads it
through queries and changes it only through `Command`/`Tx` (undo, scripts and AI
share that path). Only WS-02 changes novo-core's public API; others request changes.

## Done (merged to main)
- #11 Model API v0: ids, entities, Tx/Op/Document undo, Commands, netlist, JSON.
- #14, #21 Integrity rules, pinned JSON format (fixtures), 10k-symbol scale test.
- #26 Cross references (`cross_references`, interruption partners, contact links,
  `SetRole`) and `Project::search`.

## In review: PR #35 (branch `claude/data-model-amzaxu`)
- (b) Potential assignment: `Wire.potential`, `AssignPotentials`, `SetWirePotential`,
  `PotentialSet`, `PotentialKind::Pen`; potentials from terminal names behind
  `rules.potentials.from_terminal_names`.
- (c) Terminal rule: `rules.terminals { same_name: Off|Join|Refuse, join_link: Connector|Jumper }`.
- (f) Text records: `Text`, `TextContent`, `TextPlacement`, `AddText`, `MoveText`.
- `Terminal.part` (WS-07 request).
- (e) junction orientation: DROPPED by the lead (glyphs derive from topology).

## Next, in order (each a small new PR from main after #35 merges)
1. PR "texts and page structure" (ready locally on branch `ws02-set-text`):
   `Command::SetText`; `Page.structure` (WS-10, `=A2/3.4` references).
2. DESIGN.md section 10 (merged on main) model additions, Gate 1 first:
   a. Feed on a terminal or pin (a source is not a fake symbol).
   b. `PotentialSystem` record: L1, L2, L3, N, PE with line voltage.
   c. Stored dismissal for declined proposals (WS-11 uses a wire property now).
   d. Enclosure flag on locations (terminal proposals pick the cabinet side).
   e. `BlockDefinition` / `BlockInstance` with member back-references.
3. WS-10: per-page freeze marker ("frozen at revision") + stored locations,
   so references on frozen pages don't move (DESIGN.md section 8 pinned flow).
4. Reviewer non-blocking: cross-page move carrying attached texts and wires.

## Open questions
- None blocking. Reviewer's cross-PR notes (#28 draw Text records, #29 copy
  texts on duplicate, #30 PEN mapping, drop own name-based PE/N) routed to the lead.
