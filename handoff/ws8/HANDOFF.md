# WS-08 Keyboard workflow: handoff

## State (2026-10-14 17:00 UTC)
- Repo: github.com/Herepiggypiggypiggy/NovoElectric. Branch `claude/project-thread-9lkslj`, PR #29, head 946d8ed.
- #29 is ON HOLD for the main reviewer. Do not push to it again. After READY, follow-up work goes in a new PR from main (coordination rule 1).
- 946d8ed fixes the fmt failure on 01d8023 (formatting only). CI was running on 946d8ed at handoff time.
- Nothing is half-done in the working tree.
- WS-06 (#38) will drop its own symbol search and tag allocation and depend on novo-workflow after #29 merges (API names sent 17:01).

## Crate layout (crates/novo-workflow/src)
keymap.rs (keys and scopes), commands.rs (COMMANDS, GROUPS, verbs, arguments, availability),
palette.rs (command field engine: Palette::search_in(Mode, query, Sources) -> Vec<Row>; Row has target, group, completion, shortcut, disabled),
insert.rs (symbol search over Library aliases, favourites slots, child_queries, letter_code, next_tag),
locate.rs (over novo-core cross_references), renumber.rs, duplicate.rs, history.rs, orientation.rs (R/X only), fuzzy.rs.
Tests: unit tests per module, plus tests/journeys.rs against the novo-tests Gate 1 motor starter.

## Resume
```
git fetch origin && git checkout claude/project-thread-9lkslj
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p novo-workflow
```
Check that #29 is still open before pushing. After every push, message the reviewer with the SHA.

## People (session ids; send_message uses session_<id without cse_>)
- Integration lead / coordinator: session_011PfTJ1QaP1JQJPwXcFnxBE (merges) and session_018btnHEHYdedX4jueCivWjT
- Main reviewer: session_016C9nTT8cN7voW8t1NQUG4W
- Workflow thread (owns DESIGN.md and WS-08 acceptance): session_01JHYmmTDsVHDL7pEab8MGwy
- WS-01 app shell: session_01AYuFPxz6eQReQdtzYKLMmW. WS-05 symbols: session_01F6Twb8LZtCnYYzuP8saMyb. WS-06 editor: session_019siuxbyyRNZRgBBww1KocJ
- Full roster: /mnt/project-files/coordination/threads.md

## Decisions already made (do not re-open)
- No manual corners or T-pieces, and no J key. Tab = next proposal, Enter = accept, R/X orient.
- One command field: Ctrl+K at the top, I (place mode) and a tapped Space at the pointer. No menu bar. The empty field is the grouped menu.
- Favourites 1-9 act only while the place field is empty.
- Search builds on novo-core Project::search; aliases come from novo-symbols by_alias. Don't duplicate either.
- The canvas owns Escape (escape_with) and Space tap-vs-hold; the editor owns preview before Enter.
- Theme commands are not in the list (the app is dark only).

## Traps
- Command ids must match the app shell Action ids (file.new, view.fit, view.command_palette ...).
- Grammar verbs must match whole words (test: "pdfs" is not "pdf s").
- `m3 K1` is ambiguous; child_queries returns both readings and palette keeps the one whose device exists.

## Lead fixes waiting (read first)
See lead-fixes/README.md: review items and #35 fallout fixed locally at c3f4d7f by the integration lead; ALREADY PUSHED at 18:06 UTC; just `git pull` the PR branch.
