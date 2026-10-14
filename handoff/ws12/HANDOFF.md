# WS-12 Cables: handoff (rewrite whole file each time)

Updated: see PROGRESS.md last line.

## State
- Repo: github.com/Herepiggypiggypiggy/NovoElectric. Branch `claude/ws12-cables` (from main 9fa18ed).
- PR: see PROGRESS.md (number and head SHA are logged after each push).
- Review: head SHA sent to the main reviewer session_01XEdxpvQFh4HWmurh72tKwp and the coordinator session_01DZ68tTULUv7Uhe8ecKbC47.
- GitHub Actions is not running jobs: local fmt/clippy/test are the CI signal.

## Code
crates/novo-cables/src: kind.rs (types, colours, catalog match), ends.rs (owner device and location of a wire end), propose.rs, edit.rs, check.rs, screen.rs, strings.rs. Tests: tests/cables.rs (dev-dep novo-tests). Also edited: CLAUDE.md crate row, [workspace.dependencies] novo-cables row, Cargo.lock.

## Resume
```
git fetch origin && git checkout claude/ws12-cables
export CARGO_TARGET_DIR=/home/user/target CARGO_INCREMENTAL=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
After each push, send_message the reviewer and coordinator the new head SHA. Check the PR is still open before pushing. No pushes after READY.

## Decisions and traps
- No novo-core API change. Every edit is a Tx function (one undo step when run in one Document::transact).
- Spare marks (`cable.spare`), shield connections (`cable.shield.<core>`) and declined proposals (`cable.propose=false` on wires) are properties until WS-02 adds typed fields (requests in ROADMAP Next 2).
- Proposals group by the two end devices (strip/device), not just the location pair: two field devices get two cables.
- A terminal end's owner is its strip; its location is the strip's location (WS-11's suggest ignores terminal ends; we need them for -X1 to -M1).
- Colour order follows the starter catalog (4G: BN,BK,GY,GNYE; 5G: BU,BN,BK,GY,GNYE). The Gate 1 fixture's own -W1 uses BK,BN,GY,GNYE; tests remove it before proposing.
- Cross-section default: 1.5 mm² with any supply/PE conductor, 1 mm² for pure signals.
- Merge conflicts on CLAUDE.md / Cargo.toml crate rows: keep both rows.
