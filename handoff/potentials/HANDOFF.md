# Handoff: Potentials, PE and interruption points

Merged: #239 (novo-terminals PE toggle/auto-PE), #240 (novo-xref interruption numbering). Branch `potentials-set-and-override`: manual PE toggle becomes an override (core `Terminal.pe_override`, #253) plus `potential::set_feed_potential` in novo-editor (right-click feed, Set potential). Roster: coordination/threads.md.

Decisions
- PE = `TerminalKind::ProtectiveEarth`; earthed net = PE/PEN potential defined on it or a pin of `iec/earth/earth` or `iec/earth/protective_earth` (chassis excluded).
- `mark_earthed` only touches plain feed-through terminals without `Terminal.pe_override`; the app runs it when a wire connects to ground (on demand, not every edit). `reset_pe_override` hands terminals back.
- Menu placement is specced in DESIGN PR #238 (feed right-click: Set potential, terminal right-click: Toggle PE terminal / `pe`). App shell thread wires the menus.
- Interruption names: whole numbers, lowest unused; an unnamed partner takes the other's name on link.

Next: point 22 follow-ups, see ROADMAP design note. Resume: `cargo test -p novo-terminals -p novo-editor -p novo-xref -p novo-sim`.

UI (branch `claude/project-thread-5fvoay`)
- Interruption numbers already draw (LabelKind::InterruptionName, double-click renames). New: `novo_xref::renumber_interruptions` (reading order, pairs stay together, selection or whole project), command `interruption.renumber`, context-menu rows "Number automatically" and Rename on an interruption point.
- Not done: outline/inspector entries (DESIGN lists none for these), PE chip on the inspector.
- PE toggle, reset and menus live in main (#266) and #270; this PR only adds the override diamond (Shift+O) on hand-set PE terminals.
Live potentials view (UI for #255): `crates/novo-app/src/app/live.rs`. Shift+L (`sim.live`, View) toggles it. A badge sits on every `pass_through` group of the page (open = ring, closed = green); click flips the pole, Shift+click flips every pole of the symbol. Wires are painted over the canvas from `novo_sim::solve`: potential colour (`potential_color`: kind first, L1/L2/L3 by digit), dimmed when dead, red when shorted. A card top left shows the legend, poles closed and All open. State lives in `Workbench.live`, never in the project. Not done: coil to contact coupling (clicking a coil), per-potential colours from the canvas engine instead of an app overlay, showing the live potential in wire labels, time stepping. Right-click menus, interruption points and the wheel belong to other threads.

UI (PR #270): menu rows live in `novo-editor/src/view.rs` (context_menu) fed by `potential_menu::{feed_menu, pe_menu}`; actions in `novo-app/src/app/terminals.rs`. Not done: "New…" potential from the chip grid, wire-level override row, Show where it goes.
