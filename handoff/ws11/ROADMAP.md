# WS-11 Terminals: roadmap

Scope (roadmap WS-11): the terminal strip engine (order, levels, jumpers, PE, spares) and the terminal connection screen showing internal and external targets. Done when the Gate 1 project's terminal screens generate with no manual drawing.
Spec: docs/WORKFLOW.md J5/J10 (PR #24), the "Lessons from EPLAN" table in the roadmap, and the Workflow thread's ranked list (below).
Direction (Tadjiro via the lead): NovoElectric is its own program, not an EPLAN copy. Engine and commands first, with no UI until docs/DESIGN.md and #35 land.

## Done (PR #30, crate crates/novo-terminals)
- Placement of terminals into wires (place_terminals, place_named, strip_in_use, strip_by_tag), numbered next free; PE and N inferred
- Same-name rule (join or refuse) and the join link (connector or jumper); strip rows group joined terminals; build = 1-in/1-out clamps plus links
- Strip editor commands: reorder, add_spares, set_jumper/free_jumper, set_kind, set_level, join_duplicates, renumber plus renumber_preview
- One style (symbol and part) per kind per strip: set_strip_style, overrides, clear_override
- Connection screen data plus to_text; explain() gives plain-language rule sentences
- check(): unwired, single-sided fuse/disconnect, duplicates, not joined, PE/N name or potential mismatch, style override
- suggest_terminals / accept_suggestions at location boundaries (+CAB to +FLD); dismissals stored in wire property terminal.suggest=false
- DESIGN.md engine pieces: Settings::sentence (rule sentence), summarize_suggestions (accept-all summary), rename/rename_preview (find and replace)
- Tests: J5 acceptance, Gate 1 screen, symbol geometry vs novo-symbols

## Next, in order
1. When #35 (WS-02) merges: switch Settings to info.rules.terminals (SameName Off/Join/Refuse, JoinLink), drop the property keys and own enum types. Join: core netlist already joins same-name records, so remove the NotJoined check. Refuse: place_named returns ModelError::DuplicateTerminal instead of creating a second record. Off: separate rows.
1b. With that push, carry the Workflow thread's non-blocking notes: (a) the sentence() doc comment says it is also used in the Rules document (§3); (b) gather all user-facing strings (explain, check messages, sentence, summarize_suggestions, screen labels) into one strings module for later localisation (Dutch first, DESIGN §3, #43); (c) when WS-02 adds the enclosure flag on locations (§10), use it and drop the device-count fallback.
2. Act on review findings on #30 until the reviewer says READY (no pushes after READY).
3. Suggestions: ~~walk wires through junctions~~ (done: one terminal on the wire that leaves the location); use an enclosure property on locations once WS-02 has one; ~~use WS-02's dismissal record~~ (done: Dismissal records, a decline covers the whole connection).
4. After WS-13 lands: the drawn terminal strip page and connection screen on the generated-page framework.
5. DESIGN.md is on main (§2,3,5,6,7). UI is owned elsewhere: proposals drawn dashed steel blue at 55% with Enter accept and Tab next (editor), Terminals drawer Ctrl+3 linked to the selection, dock proposal count. WS-11 supplies the engine calls. Keep #30 free of UI until the lead lifts rule 4.

## Open questions
- Cable proposals with the terminal suggestion belong to WS-12 (Suggestion carries the internal and external devices).
