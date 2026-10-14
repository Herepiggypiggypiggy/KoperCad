# WS-10 Cross references: handoff

Updated 2026-10-14 17:25 UTC.

## State
- Repo github.com/Herepiggypiggypiggy/NovoElectric, branch
  `claude/project-thread-mtm4pw`, draft PR #31.
- Head 2c9f74f93bda01a7d788d888dc3601ae7795f8a9 (no leading slash; before: cd8482a merged main incl.
  DESIGN.md; added =A2/3.4 structure prefix and go_to_tag). Previous head
  37b3247 was green on Linux + Windows; CI on cd8482a pending.
- Main reviewer (cse_016C9nTT8cN7voW8t1NQUG4W) and Workflow thread have the
  head SHA. No review verdict yet. Still a draft: mark ready when CI green.
- Asked WS-02 for Page structure field and pinned-page freeze support.
- Nothing half-done locally; working tree clean.

## Resume
```
git fetch origin claude/project-thread-mtm4pw main
git checkout claude/project-thread-mtm4pw
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run --release -p novo-xref --example scale   # ~11 ms / 10k symbols
```
Then: check CI on #31; if green, mark ready (update_pull_request draft=false)
and message the reviewer the head SHA. PR is subscribed for activity.

## Code map (crates/novo-xref)
- index.rs: XrefIndex::build (one pass, reading order = model's), label,
  labels_on_page, partners, all_references, contact_image, highlight,
  find (Project::search), issues, LiveXref (rebuild on revision change).
- link.rs: can_link, link_command -> Command::Batch of SetRole (or Replace
  when EN 50005 contact numbers must be renumbered) + Remove of a
  throwaway old device.
- nav.rs: Navigator (G cycle; only first jump enters history), JumpHistory.
- actions.rs: XrefAction ids/labels/shortcuts, for_symbol.
- function.rs: ContactKind classify via novo_symbols::Library or key name.
- tests/xref.rs: acceptance tests; examples/scale.rs: timing.

## Who to talk to (send_message)
- Integration lead / coordinator: session_011PfTJ1QaP1JQJPwXcFnxBE
- Main reviewer: cse_016C9nTT8cN7voW8t1NQUG4W
- Workflow (docs/DESIGN.md, acceptance list): cse_01JHYmmTDsVHDL7pEab8MGwy
- WS-02 data model: cse_01B1jpjzeDiXvWoBCsv1BWt9
- Canvas: cse_013KCL5cYRtSrq4yndv3Z7TD (agreed to draw labels; wants Label
  stable across rebuilds -> it is, keyed by SymbolId, PartialEq)
- App shell: cse_01AYuFPxz6eQReQdtzYKLMmW; WS-08 keys: cse_01TAFG6LPGy2WMwhJYXmxqxF
Roster: /mnt/project-files/coordination/threads.md

## Rules and decisions
- After reviewer says READY: no more pushes to #31; follow-ups in a new PR.
- After every push: message the reviewer the new head SHA. Check PR is open
  before pushing.
- Until docs/DESIGN.md and #35 land: engine and commands only, no UI.
- No novo-core API changes from this thread (ask WS-02).
- Tadjiro: NovoElectric is its own program; EPLAN items are problems to
  solve, not UI to copy. Right-click link is a fallback; primary is giving
  a contact its device (tag in place / drag onto coil).
- Contact limits read device props contacts.main/no/nc/co until WS-07.
- Reference format (DESIGN.md §3, WS-10 owns it): page.column `3.4` (no leading slash, Workflow agreed),
  `=A2/3.4` when the target page's structure differs from the reader's.
  Page structure read from page props `function`/`location` until WS-02.
- Ctrl+K exact tag + Enter: XrefIndex::go_to_tag (main location + others).
- Commits end with Co-Authored-By + Claude-Session lines; PR body keeps
  the two-line attribution block first.
