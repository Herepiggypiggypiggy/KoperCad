# WS-12 Cables: handoff (rewrite whole file each time)

Updated: 2026-10-14 20:25 UTC (wrap-up of cloud session session_019U7n53LMsCesbK7taAj8hD).

## State
- PR #54 merged into main as efac4d4. It added crates/novo-cables, the CLAUDE.md crate row and the [workspace.dependencies] row. Both reviewer B and reviewer C said READY on head 42a921b.
- There are no open WS-12 PRs, unmerged code branches or WIP. claude/ws12-cables is fully merged; don't reuse it.
- The reviewers' follow-up notes are NOT done; ROADMAP "Next 1" lists every one with details. Start them as a new PR from main (suggested branch: claude/ws12-followups).
- This handoff is on branch claude/handoff-ws12 (docs-only PR "handoff: ws12 wrap-up").

## Code (crates/novo-cables/src)
- kind.rs: CableType (cores, cross_section_mm2, shield); standard() and numbered(); name() gives 4G1.5 / 7x1 / " shielded"; build_cores(); of_cable(); find_part() against novo_parts::Catalog (letter code W, same core names in order, same section, data "shield"); default_types(); type_label().
- ends.rs: owner() is the device of a pin (a terminal pin's owner is its strip); location() is the device's location, else the page structure; rank() is (has strip, device count), and higher rank is the cabinet side; designation().
- propose.rs: Conductor (Line/Neutral/DcPositive/DcNegative/Signal/Protective); conductor(); assign() (pass 1: GNYE/BN/BK/GY/BU by role; pass 2: next free non-GNYE core); choose_type(); propose_cables(); accept_proposals(tx, &[..], Option<&Catalog>); dismiss_proposal() / restore_proposal() (wire property `cable.propose=false`); summarize_proposals(); CableProposal::retype().
- edit.rs: assign_core (moves a wire within the cable), unassign_core, unassign_wire, swap_cores, spare_cores/is_spare/set_spare (cable property `cable.spare` = core names), add_shield (core SH/SH2, shield=true), shield_connection/connect_shield (cable property `cable.shield.<core>` = `pe` | `terminal:<raw id>`, or a wire on the shield core).
- check.rs: Finding::{SpareInUse, ProtectiveCoreMisused, ProtectiveOnOtherCore, CoreCountExceeded, MoreCoresThanPart, ShieldUnconnected, UndersizedCore}; check(); message().
- screen.rs: cable_screen / cable_screens / overview; CoreRow ends[0] is the cabinet side; CableScreen::to_text().
- strings.rs: every user-facing string, kept together for translation.
- tests/cables.rs: 7 integration tests (Gate 1 -W1 proposed and accepted as 4G1.5, decline/restore, Gate 1 screen, core order, numbered 7x1, retype, edits and checks). kind.rs has 4 unit tests.

## Resume
```
git fetch origin && git checkout -B claude/ws12-followups origin/main
export CARGO_TARGET_DIR=<own dir per worktree> CARGO_INCREMENTAL=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
- GitHub Actions has not been running jobs, so local checks are the CI signal.
- After each push, send the head SHA to the reviewer. New-crate and WS-12 PRs go to main reviewer B (session_01QSnXFBDRyvwwzhT1S1gFGN); the coordinator is session_01DZ68tTULUv7Uhe8ecKbC47.
- Check the roster in coordination/threads.md first; on-device threads may use other ids.
- No pushes after READY.

## Decisions and traps
- No novo-core API change. Every edit is a Tx function, so wrapping several in one Document::transact gives one undo step.
- Spare marks, shield connections and declined proposals are properties until WS-02 adds typed fields. WS-02 has three requests, which the coordinator has forwarded: CableCore::spare, a typed shield connection per core and end, and a stored dismissal record shared with WS-11's terminal.suggest.
- Proposals group by the two end devices (strip/device), not just the location pair, so two field devices get two cables.
- Colour order follows the starter catalog (4G: BN,BK,GY,GNYE; 5G: BU,BN,BK,GY,GNYE). L1 BN, L2 BK, L3 GY, N BU, PE GNYE. The Gate 1 fixture's own -W1 uses BK,BN,GY,GNYE, so the tests remove it before proposing.
- Default cross-section: 1.5 mm² when any supply or PE conductor is present, 1 mm² for pure signals. Otherwise the largest wire cross-section, rounded up to a standard size.
- Cable tag: -W<next> in the function of the cabinet-side device, with no location (=A1-W1).
- Commit trailer: "Co-Authored-By: Claude <noreply@anthropic.com>" plus the Claude-Session line. No model names in commits or PRs.
- One CARGO_TARGET_DIR per worktree; a shared one can run stale test binaries.
