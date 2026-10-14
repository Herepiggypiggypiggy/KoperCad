# Gate 1 integration: handoff

Updated 2026-10-14 20:25 UTC by the Gate 1 integration cloud worker (session_01QtW6E2sY3dMnhYDgX3zXge), which is stopping. **Owner from now: WS-01 (app shell, crates/novo-app).**

## State
- Repo: github.com/Herepiggypiggypiggy/NovoElectric.
- **PR #53** "Gate 1: host editor, command field, terminals, xref, PDF in the app". Branch `claude/gate1-integration`, **head 67637b78f5ee323eb43932e06d1890c0311b23b0**, based on main (main merged at 4951493, which includes #63).
- The PR is no longer stacked: #46, #38, #31, #49 and #50 all merged.
- Review state:
  - Main reviewer A (session_01XEdxpvQFh4HWmurh72tKwp): READY on correctness at 53f0e00. Asked to re-confirm on 67637b7.
  - Design reviewer (session_01NLERJkVHpsVcABrarTR3CF): CHANGES NEEDED at 53f0e00 with one blocker, "↵ accept · Tab next hint never shows". Fixed in 67637b7, recheck requested.
  - The coordinator (session_01DZ68tTULUv7Uhe8ecKbC47) merges once both are READY on the head.
- 67637b7 also makes a picked or scripted `view.command_field_here` open the field at once; only the Space key arms it. The coordinator had wanted that as a follow-up PR, but it was already in the fix commit. Tell the reviewers if they ask.
- Local checks on 67637b7: fmt --check, clippy --workspace --all-targets -D warnings and test --workspace (71 binaries) are green. GitHub Actions runs no jobs.
- **Once #53 is READY: no more pushes.** Follow-ups go in a new PR from main.
- More detail is in `handoff/ws1/GATE1-INTEGRATION.md` on the #53 branch; it arrives on main with #53.

## What #53 adds (crates/novo-app, one file per engine)
| File | What |
| --- | --- |
| `editor_canvas.rs`, `canvas.rs` | `EditorCanvas` hosts novo-editor's `SchematicView`. Edits are recorded through `OpenPage::edit`, `take_changes` / `take_page_request`, and `install(rs, 1)` |
| `keys.rs`, `command_field.rs`, `app/workbench.rs` | egui key → `Chord` → `Keymap::resolve` (focus scopes). Command field on `Palette::search_in`: Ctrl+K at the top, I place mode / tapped Space at the pointer. `run_command`, `run_target`, `type_command`. `with_editor` records shell-side editor edits at once. `update_marks` fills `view.canvas.marks` each frame. `APP_COMMANDS` / `app_runs`: other commands show greyed "Comes later" |
| `app/xref.rs` | xref labels (overlay painter), related highlight, G / Shift+G / Alt+Left/Right |
| `app/terminals.rs` | Ctrl+3 Terminals drawer. Terminal proposals as canvas `Mark`s (dashed; the Enter target marked twice, texted mark first); Enter/Tab bring off-screen proposals into view; accept all |
| `app/checks.rs` | Issue pill, problem list sorted by severity, F8, Warning/Error marks on problem objects |
| `app/pdf.rs` | Export PDF next to the .novo (`<stem>.pdf`). Untitled projects go to Documents\NovoElectric under a free name. "Replaced …" toast |
| `app/gate1_tests.rs` | Gate 1 e2e test, no-frame save regression test, marks/keys/export test |
| novo-editor `view.rs` | `SchematicView::set_style` (dark theme). WS-06 knows |

## Follow-ups (not done; WS-01 takes them in a new PR from main after #53 merges)
1. **PDF overwrite (reviewer A):** export next to the .novo replaces any existing `<stem>.pdf`, even a user's own PDF. Check the file's /Producer is ours before replacing, or use a distinct name. Write to a temp file and rename.
2. **Focus scope (A):** `Focus::Canvas` applies whenever a project is open, so Enter, Tab and G act while a panel button has focus. Needs the canvas widget id from novo-editor/novo-canvas.
3. lopdf → `[workspace.dependencies]`: the coordinator does this.
4. Design follow-ups:
   - xref labels into the scene (WS-03).
   - Issues as an outline mode with filter chips.
   - Drawer → drawing selection.
   - Cables (novo-cables #54) and Devices drawers.
   - Dismissing a terminal proposal (`novo_terminals::dismiss_suggestion`).
   - Inspector and tool dock.
   - Space-held pan.
5. Owner findings sent to the cloud fixer via the coordinator:
   - WS-06: `coil K1` creates -K2.
   - WS-11: terminal numbers follow accept order.
   - Done in #63: `accept all` parsing and the natural order of the device list. The app's own `proposal.accept` + "all" handling in workbench.rs can be removed after that.
6. Under Xvfb a single click doesn't select a symbol, though box select and headless clicks do. Check on Windows.

## Resume
```
git fetch origin && git checkout claude/gate1-integration
export CARGO_TARGET_DIR=<one dir> CARGO_INCREMENTAL=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
NOVO_GATE1_OUT=/tmp/g1 cargo test -p novo-app gate1_motor   # keeps Starter.novo + Starter.pdf
```
Screenshots: start `Xvfb :99`, mesa-vulkan-drivers, **openbox** (without a WM xdotool keys never arrive), then `DISPLAY=:99 target/debug/novoelectric /tmp/g1/Starter.novo`.
Merging main: `scripts/merge-main.sh`. It aborts on non-mechanical conflicts; then take main's version of files from merged PRs and re-apply this branch's diff. Never edit the crate list lines.

## Traps
- The editor reads its own keys (W, R, X, F2, Delete, Escape, PgUp/PgDn, Enter/Tab while it has wire proposals) only while the pointer is over the canvas or it has focus. The shell claims only what `claims_key` says.
- Escape for the field and popups must be taken before the canvas runs (`escape_popups`).
- Anything that edits the document outside the canvas frame must record at once: `with_editor` or `changed_outside_canvas`. Otherwise a save before the next frame loses it.
- Canvas marks: of two marks on one area the canvas notes the first, and it skips empty text.
- Commits: trailers `Co-Authored-By: Claude …` + `Claude-Session: <url>`. No model names in commits or PRs.
