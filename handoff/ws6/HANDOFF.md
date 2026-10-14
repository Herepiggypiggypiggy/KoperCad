# WS-06 Schematic editor: handoff

## State
- Repo: github.com/Herepiggypiggypiggy/NovoElectric. Branch `claude/project-thread-lw2vpz`, PR #38 (open), head 904aae0.
- Review: design reviewer cse_0114emQ5fSFrzcALbR4S18TU review (6 items) and workflow #43 delta addressed at 904aae0; recheck requested from both. No READY yet. Follow-ups listed in PR body 'Not in this PR'.
- Main merged in at 20b5d73 (#25, #3, #27, #26). Locally fmt, clippy -D warnings and workspace tests pass.
- Nothing half-done in the working tree.

## Resume
```
git fetch origin && git checkout claude/project-thread-lw2vpz
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo run -p novo-editor --example editor -- --demo   # Gate 1 page drawn through the editor
```
Commit trailer: `Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_019siuxbyyRNZRgBBww1KocJ`.

## Layout (crates/novo-editor/src)
catalog.rs (search, classify, MNEMONICS), naming.rs (tags, EN 50005), geometry.rs (route, target_at), edit.rs (place, auto_connect, draw_wire, transform, delete, rename), potential.rs (feeds, suggestions), pages.rs (next_page_name, add_page, neighbour), tool.rs (Editor state machine), art.rs (LibraryArt), view.rs (SchematicView + 6 egui harness tests). tests/editing.rs: 19 journey tests. examples/editor.rs: eframe host.

## Contacts (coordination/threads.md)
- Integration lead (merges): session_011PfTJ1QaP1JQJPwXcFnxBE
- Design reviewer for #38: cse_0114emQ5fSFrzcALbR4S18TU (message after every push with head SHA)
- WS-01 app shell cse_01AYuFPxz6eQReQdtzYKLMmW: will swap PlaceholderCanvas for SchematicView after #38; forwards show()'s ChangeSet to backend.record; follows take_page_request().
- WS-02 cse_01B1jpjzeDiXvWoBCsv1BWt9: #35 has Wire.potential, AssignPotentials, AddText/MoveText.
- WS-03 cse_013KCL5cYRtSrq4yndv3Z7TD: #28 has SymbolArt::labels and tag_anchor; will do frame column numbers + title block in a later PR.
- WS-08 cse_01TAFG6LPGy2WMwhJYXmxqxF: asked to expose search and next_tag for novo-editor to consume.
- Workflow (DESIGN.md) cse_01JHYmmTDsVHDL7pEab8MGwy.

## Rules and decisions
- After READY: no pushes to #38; follow-ups in new PRs from main. Check the PR is open before pushing.
- Until DESIGN.md merges: engines/commands, thin UI, no new panels or dialogs.
- Junctions are derived, never placed (no J/Tab pieces). Wiring is pin to pin.
- Spec is docs/DESIGN.md (PR #24): Enter = accept proposal (never repeat); repeat-last-tool only on empty-canvas menu; potentials declared at source (no per-wire assign, no override until Wire.potential); facing pins at a distance are PROPOSALS (Editor.proposals, Enter accept, Tab next, Esc/click drops), touching pins + pins on wires connect automatically (edit::settle); drag from a pin connects (6 px pin radius), W = chains; PE/N/PEN by name = Editor::supply_names rule.
- Only WS-02 changes novo-core API; edit the model only through Tx/commands. One gesture = one undo step.
- eframe MSAA: install(render_state, 1).

## Traps
- A contact placed without a parent creates its own device by design; place the coil first or search "K1 no".
- Deleting a feed must keep wires (keep_wires_through), else re-assigning a potential cuts the wire.
- Xvfb render needs libxkbcommon-x11, mesa-vulkan-drivers.
