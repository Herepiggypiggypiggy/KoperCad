2026-11-03 14:30 UTC Task received: explanation text behind one "?" per window, about 95 % less text.
2026-11-03 15:20 UTC window_help mechanism, Heat/Vents/Hoist/Drive run changes, check_text step, text audit switch; inventory from 112 scenarios.

## 2026-11-03 PR 2 (Schematic, Simulate, Live, Add-ons, controller card)
- `window_help::note(ui, text)`: text goes to the window's "?" when a window is being built, else an inline "?" mark. `dialog_ui::note` wraps it. Use it instead of `dialog_ui::hint` for explanations (hint stays for data lines).
- Status bar: hints and info messages show only their first clause (`status_bar::split_lead`: cut at " · ", " (" or ". "); the rest sits behind a "?" next to it. Refusals stay in full (red).
- Moved: inspector "Blank is unknown", decimal-comma note, long field labels (text in parentheses is now hover), project rules and project settings hints, Place macro window (description on hover, "N devices" + "?", "Symbols only" + "?"), Simulate motor start notes, Live card notes, Add-ons header and empty states, controller card notes.
