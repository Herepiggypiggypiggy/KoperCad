# WS-06 Schematic editor: handoff

## State
- Repo: github.com/Herepiggypiggypiggy/NovoElectric. Branch `claude/project-thread-lw2vpz`, restarted from main after #113 merged.
- Merged: #38 (editor), #62 (`coil K1`), #70 (`art` feature), #91 (proposal highlight, second main refused), #113 (dashed proposals, pointer-scoped Delete, Pin.optional, artwork text and tag anchors).
- Open: the follow-up PR from this branch (number and head in PROGRESS.md): declines as core Dismissal records, page names with letters (3a) and never a dot, the 3/3a re-letter rule with frozen pages (#119), rename refusals. Reviewers: reviewer A (correctness) and the design reviewer.
- CI (Actions) is down; local fmt, clippy -D warnings and `cargo test --workspace` are the signal.
- Nothing half-done in the working tree.

## Resume
```
git fetch origin && git checkout claude/project-thread-lw2vpz
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo run -p novo-editor --example editor -- --demo
```
If the PR merged: `git fetch origin main && git checkout -B claude/project-thread-lw2vpz origin/main` before new work.
Commit trailer: `Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_019siuxbyyRNZRgBBww1KocJ`.

## Layout (crates/novo-editor/src)
catalog.rs (search, classify, letter codes, MNEMONICS), naming.rs (tags from Page.structure, EN 50005), geometry.rs (route, target_at), edit.rs (place, settle, facing_wires/accept/proposal_holds, transform, delete, tidy_junctions, main_symbol), potential.rs (menu, suggestions), pages.rs, tool.rs (Editor state machine; proposals with highlight/follow_pointer/next_proposal/decline_proposal), art.rs (LibraryArt, `art` feature), view.rs (SchematicView + harness tests). tests/editing.rs: journey tests.

## Contacts
- Integration lead (merges): see handoff/MAIN-HANDOFF.md §0; review policy there (main reviewer + design reviewer for UI PRs).
- WS-03 canvas: junction glyphs (`JunctionGlyph::from_directions`) and single-click select under Xvfb.
- WS-08 workflow: owns the letter-code table, now in novo-symbols (`novo_symbols::letter_code`, #124); catalog::letter_code calls it.

## Rules and decisions
- One PR at a time from this branch; after READY no pushes; check the PR is open before pushing; message the reviewer with the head SHA after each push.
- Only WS-02 changes novo-core's API; edit through Tx/commands. One gesture = one undo step.
- Proposals (DESIGN.md §5 since #92): never solid. Dashed in CanvasStyle.proposal (about 55%) at 1 px; the highlighted one dashed at full strength, 2 px, with "↵ accept · Tab next". The highlight follows the pointer; Tab moves it and holds until the pointer travels > 24 px; Enter accepts; Delete with the pointer on it declines for good (core Dismissal `connect.facing:<pin>-<pin>` over the two symbols, undoable; goes away with either symbol).
- Page names (DESIGN.md §3): never a dot. Inserted pages take the next number, else a letter from `abcdefghjkmnpqrstuvwxyz` (3a, 3b; `Cover a`, then `Cover b`; 3ba between 3b and 3c, then 3bb). A run is every page named anchor + letter + anything (3a, 3ba); re-lettering shifts only the first letter (3ba → 3ca). The last fallback (`<last> new`) never panics. Shell callers of Editor::new_page/rename_page outside the canvas frame take the changes at once (app `with_editor`). Between 3 and 3a the run moves up one letter unless a page of it is frozen (`Page.frozen`); then the page goes at the end with a notice. Frozen pages cannot be renamed.
- Junctions are derived; the editor joins 2-wire junctions (tidy_junctions), so dots only stay at 3+ conductors.

## Traps
- A contact placed without a parent creates its own device by design; place the coil first or search "no K1".
- Deleting a feed must keep wires (keep_wires_through).
- Xvfb render needs libxkbcommon-x11, mesa-vulkan-drivers.
- One CARGO_TARGET_DIR per worktree; a shared one runs stale tests.
