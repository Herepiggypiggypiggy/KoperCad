# The user manual (source)

The offline manual that opens from Help > User manual and F1. This folder holds the **written pages** (Markdown, one file per page). The reference pages (feature index, menu by menu, all commands, keyboard shortcuts) are **generated** from the command list, key map and menus by `crates/novo-workflow/src/manual.rs`, so they cannot drift.

| What | Where |
| --- | --- |
| Written pages | `docs/manual/*.md` (listed in `SOURCES` in `manual.rs`) |
| Generator, Markdown renderer, context topics | `crates/novo-workflow/src/manual.rs` |
| Open from the program | Help > User manual, F1 (`help.manual`, `help.context`), `crates/novo-app/src/app/manual.rs` |
| Write to a folder | `kopercad-cli manual <dir> [--images <dir>]` |
| Screenshots | `docs/manual/images/<name>.png`, made by `scripts/build-manual.sh` from QA scenario `74-manual-help` at 1920x1080 |

## Adding to the manual

- A new page: add `docs/manual/<stem>.md` and a `page!` line in `SOURCES`. Links between pages are `[text](other.md#heading)`; a heading's anchor is its lower-case words joined by dashes.
- A new context topic (what F1 opens for a panel): add a row to `TOPICS` and, if a new window should use it, to `help_topic` in the app.
- A new image: write `![caption](images/name.png)` in a page and take the shot in scenario 74 with `shot name`. Until the image exists the page shows the caption.
- The tests fail when a link or a context topic points at a page or heading that does not exist, when a page loads anything from the network, and when a command or key is missing from the generated lists.

## Rules for the text

- Say what a number is: exact, planning value or model. `numbers.md` follows `crates/novo-app/src/app/model_labels.rs`; change both together.
- Name where the user finds each thing (menu path or key).
- Plain sentences, no promises about what is not in the program.
