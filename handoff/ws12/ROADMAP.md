# WS-12 Cables: roadmap

Scope (roadmap WS-12): cable definitions, core-to-wire assignment (by colour or number), shields, cable connection screens and the cable overview. Done when each cable screen shows both ends and every core.
Spec: docs/DESIGN.md. Cables are proposed where connections cross a location boundary. Proposals are drawn dashed; Enter accepts, Tab moves to the next one, and declined proposals are remembered. The drawer's Cables tab is Ctrl+4. Engine and commands only; the UI belongs to the editor and drawer threads.

## Done (PR #54, merged as efac4d4)
- Cable types: 4G1.5, 5G2.5, 7x1 numbered and shielded variants, with HD 308 S2 colours. Catalog part matching against novo-parts. default_types().
- Proposals at location boundaries, grouped by end-device pair, with a stable core order (L1 L2 L3 N L+ L- signals PE). A colour type is used for up to 5 supply conductors, a numbered one otherwise. Accepting is one undo step and reuses the catalog or project part. Declined proposals are remembered, and retype() is available.
- Core edits: assign, unassign, swap, spare, shield add/connect.
- Checks: spare in use, PE misuse in both directions, core count exceeded, more cores than the part has, shield unconnected, undersized core.
- Cable screen (both ends of every core, cabinet side on the left) and overview (tag, type, length, from/to, used/cores/spare).
- Tests on the Gate 1 motor starter.

## Next, in order
1. Follow-up PR from main with the reviewers' non-blocking notes on #54 (reviews by B and C on 42a921b):
   a. BUG: in propose.rs choose_type, the colour loop `break`s when standard(2, pe) falls back to numbered, so L+PE never tries 3G (BN BU GNYE) and ends up as a numbered 3G "1,2,GNYE". Change it to `continue` (or start at needed.max(3) when pe). Add a test: one line plus PE gives 3G1.5 with L on BN, PE on GNYE and BU free.
   b. edit.rs shield_connection / check.rs: a `cable.shield.<core> = terminal:<id>` whose terminal was deleted still counts as connected, and the screen shows an empty end. Treat a missing terminal as unconnected (ShieldUnconnected) and add a test.
   c. edit.rs assign_core: the doc says a wire already on another cable is refused, but only novo-core validation (WireAlreadyOnCore) does that. Add an explicit check with a clear error, or reword the doc.
   d. screen.rs: cable_screen builds project.netlist() for every cable, so cable_screens and overview rebuild it N times, and ends::rank scans every device for each wire end. Build one netlist and one location-to-rank map per call; this needs internal helpers that take them as arguments.
   e. Make the rank tie-break agree. propose_cables breaks equal rank by DeviceId, while screen::oriented breaks it by location text, so the proposal's ends[0] and the screen's left column can disagree. Share one rule (e.g. a function in ends.rs).
   f. kind.rs is_numbered: an empty core name counts as numbered. Use `!c.is_empty() && c.chars().all(..)`.
   g. Spare marks are stored by core name, so renaming a core drops its mark. Leave this until CableCore::spare exists (item 2), but mention it in the docs.
2. When WS-02 lands typed fields, move off the properties: CableCore::spare, a typed shield connection per core and per end (PE / terminal / pin), and a stored dismissal record (shared with WS-11).
3. Proposals:
   - walk wires through junctions (today only pin-to-pin wires are proposed);
   - combine with WS-11's terminal suggestions (novo-terminals is on main; its Suggestion carries the internal and external devices), so one accept gives a field device both a terminal and a cable.
4. Cable lengths from the cabinet layout or routing; today length_m is a placeholder.
5. Generated cable pages, now that WS-13 novo-reports is on main (#55): PageKind::Generated { generator: "cable" } built from cable_screens, plus a cable list report.
6. Supply data for the UI threads. Proposals are drawn dashed, Enter accepts and Tab moves on. The Cables drawer (Ctrl+4) reads overview() and cable_screen().

## Open questions
- Cable tags currently carry only the function (=A1-W1), with no location. Revisit if DESIGN wants +CAB on cables.
- For L + PE: should the cable be 3G (with a spare BU) or a 2-core with PE? 3G is the plan in 1a.
