# WS-25 symbols (packs and library growth): handoff

Updated 2026-10-15 19:00 UTC.

## State

- **Repo and branch:** `Herepiggypiggypiggy/kopercad`, branch `claude/project-thread-ds56oi` (reset to main for each PR once the previous one merged).
- **PRs:** all merged: #335 design, #351 packs, growth, audit #380 #390 #407 #418 #424 #427, #447 switch caller, #458 26 more NFPA symbols. Nothing open.
- **Library:** IEC pack 174 symbols; NFPA pack 174 (every IEC symbol has a twin; reactor, inductor and changeover switch share artwork).
- **Half-done:** the "Switch placed symbols" button (WS-01 owns it, built on `novo_workflow::switch::plan`); menu entry is `project.standard`.
- **Stray remote branch `x`:** created by mistake, cannot be deleted through the proxy; someone with branch rights should delete it.

## Switching the standard

`novo_workflow::switch::plan(project, pack)` lists re-keys and the symbols with no counterpart; `plan.command()` is one `Command::SwitchStandard` (one undo step).

## Layout

- `crates/novo-symbols/symbols/<pack>/<category>/<name>.toml`: source of truth. Packs `iec`, `nfpa`.
- `src/pack.rs`: `Pack`. `src/library.rs`: `builtin()` (IEC), `builtin_pack`, `load_dir_pack`. `build.rs` embeds one table per pack.
- `tests/packs.rs`: counterparts agree on function, behavior, bridges, connection ids/roles/order, anchor roles, letter codes.
- `src/letter.rs` (WS-08 table): new function types need a line in `table_is_pinned` and, if needed, a rule.

## How the files are made

In `handoff/ws25/generator/` (needs python 3.11+):
- `gen_growth.py <symbols dir>`: writes the new IEC symbols by deriving them from existing IEC ones.
- `gen_nfpa.py <symbols dir>`: writes the whole `symbols/nfpa` tree from the IEC tree: `REDRAW` entries supply NFPA artwork, `SAME` globs copy identical artwork, pilot lights are built in `main`. Anything without an entry is deliberately absent. It deletes NFPA files it no longer generates.
- `symlib.py`: helpers. Run in this order after any IEC change: `gen_growth.py` then `gen_nfpa.py`, then `git status`.
- The old WS-05 generator (`handoff/ws5/`) no longer reproduces the IEC files; IEC files are hand-maintained.

## Resume

```
git fetch origin main && git checkout -B claude/project-thread-ds56oi origin/main
cd handoff/ws25/generator && python3 gen_nfpa.py ../../../crates/novo-symbols/symbols
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo run -p novo-symbols --example gallery -- g.html nfpa
```

## Who to talk to

Roster: `coordination/threads.md`. Main reviewer `cse_016C9nTT8cN7voW8t1NQUG4W` (message the head SHA after every push). Integration lead `cse_01FiLvRBKEcNNHbQEGhYyL1S` merges. WS-08 keyboard (`by_alias`, favourites, letter codes) `cse_01TAFG6LPGy2WMwhJYXmxqxF`.

## Decisions

- Same path below the pack root is the same device; connection ids stay EN 50005 in both packs.
- `builtin()` stays IEC so about 50 callers are unchanged; aliases are per pack.
- No format change; version stays 1.
- The NFPA artwork is my reading of the standard and was only checked in the gallery.

## Traps

- Adding an IEC symbol without running `gen_nfpa.py` leaves it with no NFPA twin (allowed), but a changed IEC file the generator copies goes stale.
- Pushes to a merged PR's branch are lost; check the PR is open first.
