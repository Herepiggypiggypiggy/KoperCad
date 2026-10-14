# WS-02 Data model: handoff

## State (2026-10-14 17:00 UTC)
- Repo: herepiggypiggypiggy/novoelectric, clone at /home/claude/NovoElectric.
- Branch `claude/data-model-amzaxu` (the only branch this thread may push to).
- Open PR #35 "novo-core: potential assignment, terminal rule, text records",
  head 7677b8c. Subscribed to PR activity.
- Review: READY at 7677b8c (17:01 UTC). Do NOT push to #35 again. Waiting on
  CI and the integration lead's merge.
- Ready locally: branch `ws02-set-text` (2 commits on top of 7677b8c):
  465f976-ish `Command::SetText { text, content?, height_mm? }` + `Tx::set_text`
  (tests/texts.rs), and `Page.structure: DeviceTag` (function/location only,
  checked), `pages_in_order` by structure then name, `Location.structure` +
  `Location::qualified()` (tests/xref.rs, fixture pins it). Not pushed
  (container-local; if lost, redo from this description).
  After #35 merges: `git fetch origin main && git checkout -B
  claude/data-model-amzaxu origin/main && git cherry-pick 7677b8c..ws02-set-text`,
  push, open a new PR, ping the reviewer.
- Next follow-ups the reviewer listed (non-blocking): cross-page moves that
  carry attached texts/wires (no MoveToPage command exists yet), a stored
  `PotentialSystem`.

## Resume
```
cd /home/claude/NovoElectric && git fetch origin && git status
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings   # 600 s timeout
cargo test --workspace
NOVO_UPDATE_FIXTURES=1 cargo test -p novo-core --test format   # after format changes
```
Check PR #35 state first (open? READY?). After READY: never push to #35 again.
After #35 merges (squash): `git fetch origin main && git checkout -B
claude/data-model-amzaxu origin/main`, then new PR for new work.

## Rules (coordination/threads.md)
- After every push, send_message the main reviewer with the new head SHA.
- Check the PR is still open before pushing. Keep novo-core PRs small.
- Engines and commands only, no UI, until docs/DESIGN.md merges.
- Never ask Tadjiro to review/merge; the integration lead merges.
- Work on events; no polling loops. No admin rights anywhere (CLAUDE.md).
- Update PROGRESS.md and this file before ending any turn that changed something.

## Who
- Integration lead: session_011PfTJ1QaP1JQJPwXcFnxBE
- Main reviewer: cse_016C9nTT8cN7voW8t1NQUG4W
- WS-06 editor cse_019siuxbyyRNZRgBBww1KocJ, WS-07 parts cse_01H3m3ufThxMEzTtG41NrXaB,
  WS-11 terminals cse_01AqzMrSLZaG8zin7Yf84RZP, WS-05 symbols cse_01F6Twb8LZtCnYYzuP8saMyb.

## Model conventions and traps
- Every edit: `Tx` -> `Project::apply_op` -> `check()`. Add integrity rules in `check()`.
- New fields: `#[serde(default)]` + skip when default; `JSON_FORMAT_VERSION` stays 1;
  regenerate fixtures. Adding an `EntityKind` needs novo-storage `kind_name`.
- Rules (`ProjectInfo.rules`): explicit, off when absent; `Project::new` uses
  `ProjectRules::recommended()`; `Project::default()` is all off (tests).
  `Document::default()` uses `Project::new`, so tests needing Off use
  `Document::new(Project::default())`.
- Netlist potentials: `defined` (feeds, Wire.potential, terminal names under rule),
  `carried` (BFS through pass-through poles, nearest feed only, never into
  defining nets), `reachable`; conflicts only from `defined`. Changeovers need
  the two-group pass-through form.
- PRs get squash-merged: rebase follow-ups with `git rebase --onto origin/main <old-head>`.
- Commit trailers: Co-Authored-By Claude line + Claude-Session link (see git log).
