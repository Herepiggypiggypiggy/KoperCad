# User manual and in-app help: roadmap

Scope: an offline manual bundled with the program, organised by task and by menu, with generated reference pages, context help (F1) and Ctrl+K entries.

Done (first PR): generator and Markdown renderer (`novo-workflow::manual`), nine written pages, generated menu/command/shortcut/feature pages, Help > User manual, F1, `kopercad-cli manual`, QA scenario 74, `scripts/build-manual.sh`.

Next, in order:
1. Screenshots: run `scripts/build-manual.sh` on a machine with the QA packages, commit `docs/manual/images/`, bundle them (fill `IMAGES` in `app/manual.rs` with `include_bytes!`).
2. Lessons link to manual pages (ask the Learning threads for the anchor per lesson; a `manual` field in the lesson file).
3. PDF edition of the manual through `novo-pdf` (the HTML prints with a print style sheet today).
4. More workflow pages as features land: wire numbering schemes in depth, PLC and I/O, protection and sizing, part import, macros, variants.
5. Panel-level `?` buttons that call `help.context` with a topic.

Open questions: whether the manual should ship in the installer folder as well as being written to the profile at first use (today it is written on each open, so it always matches the build).
