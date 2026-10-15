# Handoff: Potentials, PE and interruption points

Merged: #239 (novo-terminals PE toggle/auto-PE), #240 (novo-xref interruption numbering). Branch `potentials-set-and-override`: manual PE toggle becomes an override (core `Terminal.pe_override`, #253) plus `potential::set_feed_potential` in novo-editor (right-click feed, Set potential). Roster: coordination/threads.md.

Decisions
- PE = `TerminalKind::ProtectiveEarth`; earthed net = PE/PEN potential defined on it or a pin of `iec/earth/earth` or `iec/earth/protective_earth` (chassis excluded).
- `mark_earthed` only touches plain feed-through terminals without `Terminal.pe_override`; the app runs it when a wire connects to ground (on demand, not every edit). `reset_pe_override` hands terminals back.
- Menu placement is specced in DESIGN PR #238 (feed right-click: Set potential, terminal right-click: Toggle PE terminal / `pe`). App shell thread wires the menus.
- Interruption names: whole numbers, lowest unused; an unnamed partner takes the other's name on link.

Next: point 22 follow-ups, see ROADMAP design note. Resume: `cargo test -p novo-terminals -p novo-editor -p novo-xref -p novo-sim`.

UI (branch `claude/project-thread-5fvoay`, PR "after build", not for the next build)
- Interruption numbers already draw (LabelKind::InterruptionName, double-click renames). New: `novo_xref::renumber_interruptions` (reading order, pairs stay together, selection or whole project), command `interruption.renumber`, context-menu rows "Number automatically" and Rename on an interruption point.
- PE: toggle and auto-PE after wiring came from main; this PR adds "PE: back to automatic" (`terminal.pe_auto`, shown when a hand choice exists) and the override diamond (Shift+O) on hand-set PE terminals.
- Not done: outline/inspector entries (DESIGN lists none for these), Dutch aliases beyond the three in language.rs, PE chip on the inspector.
