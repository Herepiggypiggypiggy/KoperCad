# Potentials, PE and interruption points (thread "Potentials, PE and interruption points")

Owns Tadjiro's list points 10, 11, 22, 23 (model/engine side). UI and menus go through Workflow and Layout; feed auto-numbering L1/L2/L3 (point 9) is the Fast keyboard thread; the page model (point 12) is the Pages thread.

Already in main (found 2026-10-15): potentials, kinds, `PotentialFeed` role, wire/pin/terminal feeds, netlist `defined`/`carried`/`reachable` (wires inherit the net's potential), `TerminalKind::ProtectiveEarth`, `novo_terminals::set_kind`, interruption role with name partners and xref labels.

Done / in review
- PR A (novo-terminals): `toggle_pe`, `mark_earthed`, `earthed_terminals` (point 11 engine).
- PR B (novo-xref): `next_interruption_name`, `auto_name_interruption(s)` (point 10 numbering).

Next
1. Point 23 engine: one command to set a potential on whatever is right-clicked (feed symbol, terminal feed, pin feed, wire) by `PotentialSpec`; design with Workflow/Layout.
2. Point 22: state-dependent potentials. Netlist stays static (`reachable`); add a `PotentialState` evaluator in novo-sim that resolves `carried` by contact state, so simulation can change a wire's potential.
3. Hook-up for app: place earth symbol => role `PotentialFeed{PE}`; call `mark_earthed` after wiring; call `auto_name_interruption` on placement (UI threads).
