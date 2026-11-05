# "?" sweep part 2: Schematic, Simulate, Live, Add-ons

2026-11-05 redo of draft PR #1847 on current main (Place macro part left out, done in #1975).

- `window_help::note(ui, text)`: text goes to the window's "?" when a window is being built, else an inline "?" mark. `dialog_ui::note` wraps it. Use it instead of `dialog_ui::hint` for explanations (`hint` stays for data lines).
- Status bar: hints and info messages show only their first clause (`status_bar::split_lead`: cut at " · ", " (" or ". "); the rest sits behind a "?" next to it. Refusals stay in full (red).
- Moved behind "?": Inspector ("Blank is unknown", decimal-comma note, long field labels become hover), Project rules and Project settings hints, Simulate motor start notes (no motor, nameplate estimate), Live card notes, Add-ons header and empty states, controller card notes.
- Stays visible: refusals, errors, stored-controllers-damaged warning, stall result (raised through the flags strip), the estimate badge.
- Not done: PLC messages, remaining Schematic toasts, floating modals without a header "?".
