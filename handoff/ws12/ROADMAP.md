# WS-12 Cables: roadmap

Scope (roadmap WS-12): cable definitions, core-to-wire assignment (colour or number), shields, cable connection screens, cable overview. Done when each cable screen shows both ends and every core.
Spec: docs/DESIGN.md (cables proposed where connections cross a location boundary, drawn dashed, Enter accepts, Tab next, declined proposals remembered; Cables drawer tab Ctrl+4). Engine and commands first; no UI here.

## Done (first PR, crate crates/novo-cables)
- kind: CableType (4G1.5, 5G2.5, 7x1 numbered, shielded variants), HD 308 S2 colours (L1 BN, L2 BK, L3 GY, N BU, PE GNYE) in the starter catalog's core order, default_types(), find_part() against the novo-parts catalog (letter code W, same cores and cross-section)
- propose: wires crossing a location boundary pin to pin, grouped by the two devices they connect (strip -X1 in +CAB to motor -M1 in +FLD); stable core order L1 L2 L3 N L+ L- signals PE; colour type for up to 5 supply conductors, numbered otherwise; cross-section from the wires; tag -W<next> in the cabinet device's function; accept_proposals in one Tx (part from catalog, reused if the project has it); dismiss/restore via wire property cable.propose=false; retype() for a shielded or bigger type
- edit: assign (moves within cable), unassign, unassign_wire, swap, set_spare (cable property cable.spare), add_shield (SH core), connect_shield (PE, terminal, or a wire; cable property cable.shield.<core>)
- check: SpareInUse, ProtectiveCoreMisused, ProtectiveOnOtherCore, CoreCountExceeded, MoreCoresThanPart, ShieldUnconnected, UndersizedCore
- screen: cable_screen (both ends per core, cabinet side left), cable_screens, overview (tag, type, length, from/to, used/cores/spare), to_text
- strings module with every user-facing string
- Tests: Gate 1 -W1 proposed and accepted as 4G1.5 L1/L2/L3/PE (one undo step), Gate 1 screen, core order, numbered with spares, edits and checks

## Next, in order
1. Act on review findings until READY (no pushes after READY).
2. Requests to WS-02 (described in the PR): `CableCore::spare: bool`, a typed shield connection on CableCore (PE / terminal / pin, per end), and a stored dismissal record for proposals (shared with WS-11's terminal.suggest). Move off the properties once they land.
3. Proposals: walk wires through junctions; cables between a strip and a field junction box with its own strip; combine with WS-11's terminal suggestions (Suggestion carries internal and external devices) so a field device gets terminal + cable in one accept.
4. Lengths from the cabinet layout / routing (placeholder `length_m` today).
5. After WS-13: generated cable pages (PageKind::Generated { generator: "cable" }) from cable_screens.
6. UI is owned by the editor/drawer threads: proposals dashed, Enter accept, Tab next; Cables drawer Ctrl+4 reads overview() and cable_screen().

## Open questions
- Cable tag structure: today the cabinet device's function only (`=A1-W1`), no location, matching the Gate 1 fixture. Revisit if DESIGN wants `+CAB` on cables.
