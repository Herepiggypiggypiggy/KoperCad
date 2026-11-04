# Command-line audit (`kopercad-cli audit`)

Checks for scripts and CI. Every verb only reads. Exit 0 is clean, 1 means findings
(`--strict` counts warnings too), 2 a bad call, 3 an unreadable input. Ideas from QElectroTech's
command line (`--export-links`, `--export-nets`, `--check-elements`); the code is our own.

| Verb | What it does | Where else you find it |
| --- | --- | --- |
| `audit links <project>` | Every sheet link (interruption point): FAIL when it has no partner or no name, or when the label drawn next to it does not name a partner; WARN when all partners are on its own page or no label is drawn | Check window: "Interruption point without partner" (the lone-link part only) |
| `audit symbols <file or folder>...` | Lints symbol files: parse errors, id against path, every `novo-symbols` rule, duplicate ids, aliases repeated within a pack; WARN for missing description, keywords or artwork. OK, WARN or FAIL per file | Command line only (no symbol editor by decision) |
| `audit nets <project>` | Every net with pins, wires and potentials; nets with one pin marked open | Python `nets()`; the Check window's unconnected-point problems |
| `audit search <project> <text>` | The Ctrl+K search as a list; exit 1 when nothing is found | Ctrl+K in the app |

Each verb takes `--json`.

## Search and replace (checked 2026-11-04)

The app has find and replace for terminal names only (Terminals > Rename, `novo_terminals::rename`).
There is no project-wide replace over tags, texts, page titles or title-block fields. Bulk edit
covers parts and properties through File > Export interchange. A project-wide replace with a
preview needs a command in `novo-core` (WS-02 owns the API), so it is a separate piece of work.
