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

## Design note: live potentials (point 22), 2026-10-15
- Two views. Declared: what the drawing says (feeds, `Net::potentials`, `carried`, `reachable`), stored and unchanged by simulation. Live: what a conductor carries under a contact state, computed by `novo_sim::solve(project, netlist, &PoleState)`, never stored.
- `PoleState` = set of closed poles `(symbol, pass_through group)`; default all open. A potential flows from nets that define it through closed poles into undefined nets, never into a net with its own definition; two potentials meeting = `Live::Shorted`.
- Next steps (not in the first PR): coil -> contact coupling (a coil with live potential on both pins closes the poles of its device's contacts, via xref device links), iterate to a fixpoint for latching circuits; timers; canvas shows live state as a tint, declared label unchanged (DESIGN §Potential feeds).

Live view v1 done (app overlay, Shift+L). Next: coil coupling in the UI once novo-sim has it; tint terminals and pins; show live potential in the wire hover tip.
